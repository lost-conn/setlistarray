//! PDFs: what the app can learn about one, and how one becomes an attachment.
//!
//! Cards D3 and D4. The renderer question is already settled — `docs/PDF.md`
//! chose **hayro**, the pure-Rust rasteriser, on 2026-08-26, over
//! `pdfium-render`'s 6.4 MB C++ blob and over deferring to the system viewer.
//! This file is D3's half of that decision (`hayro-syntax`, the parser: is this
//! a PDF, and how many pages has it) and [`pages`] is D4's (the rasteriser: what
//! does a page look like). They are split because they are asked at different
//! moments and one of them is thirty times more expensive than the other —
//! every import parses, and only a page somebody is about to look at is drawn.
//!
//! ## Why the page count is parsed here rather than left to D4
//!
//! Because the row the handoff draws says it. `song_detail::primary_subtitle`
//! renders `primary · 2 pages` and has been able to since D1; until now nothing
//! could fill the number in, and a PDF row that said `primary · pdf` where the
//! design says `2 pages` is the card not being done. The count also costs
//! almost nothing on top of what has to happen anyway: `Pdf::new` reads the
//! trailer and the page tree, which is the same parse D4's rasteriser starts
//! from, and 33 lines further down `pages().len()` is already sitting there.
//!
//! ## A PDF hayro cannot read is still imported
//!
//! This is the decision worth arguing about, so: **the magic number decides
//! whether this is a PDF; hayro decides only how many pages it has.** If
//! `Pdf::new` fails, [`page_count`] returns `None`, the import goes ahead, and
//! the row reads `primary · pdf` — the fallback `primary_subtitle` has had
//! since D1.
//!
//! The alternative — refusing anything hayro will not parse — hands a 0.x
//! library a veto over what the user is allowed to keep. `docs/PDF.md` is
//! explicit that hayro "is 0.x software with a list of known gaps, and it will
//! eventually draw something wrong", and the same is true of parsing. Somebody
//! whose chart came out of a bad exporter would be told their chart is not a
//! PDF, which is both untrue and unfixable from inside this app. Keeping the
//! file costs a row with no page count; refusing it costs the chart.
//!
//! ## The file on disk is not called what the user called it
//!
//! It is [`PDF_FILE`], always. The name the user's file had lives in
//! `Attachment.title`, where it is text on a row and nothing else. That is not
//! tidiness: on Android the name is scavenged out of a `content://` URI (see
//! [`crate::picker::name_from_content_uri`]) and on the desktop it is whatever
//! the file system had, so it is outside data either way, and outside data does
//! not get to be a path component. `capture` writes a fixed `page.html` beside
//! its `assets/` for the same reason, and D4's rasterised pages landed in this
//! directory next to `chart.pdf` — `page-1.png` and so on, named by
//! [`pages::page_file`].
//!
//! ## The order of operations, and what happens when the disk says no
//!
//! Judge in memory, then attach, then write, then undo the attach if the write
//! failed. That is the sequence `capture::write_into`'s header sets out for E2
//! and the reason is the same: `SongsStore::attach` mints a row *and* a
//! directory *and* — if this is the song's first chart — makes it primary, so
//! attaching before knowing whether the bytes are any good would leave an empty
//! ghost holding the card on song detail. Nothing here creates a directory
//! itself; `Repo::create_attachment` does, which is what keeps every attachment
//! directory owned by a row that can delete it again.

pub mod pages;
#[cfg(test)]
pub mod test_support;

use std::sync::Arc;

use hayro_syntax::Pdf;

use crate::model::{Attachment, AttachmentId, AttachmentKind, SongId};
use crate::picker::{PickRequest, PickedFile};
use crate::store::SongsStore;

/// The imported file, inside the attachment's own directory. See the module
/// header for why it is not the user's name.
pub const PDF_FILE: &str = "chart.pdf";

/// The most of a file this app will take in.
///
/// Sized for a phone rather than for this desktop, because
/// `rinch_android::file_picker::read_content_uri` holds the file three times
/// over on the way in — a Java `byte[]`, a `Vec<i8>`, and the `Vec<u8>` it maps
/// into — so 32 MB of chart is very nearly 100 MB of peak heap in a process
/// Android is free to kill for using it. Against that: `docs/PDF.md` measured a
/// 30-page scanned Real Book page at 6.3 MB, which is the largest thing a chord
/// chart realistically is, so this leaves five times the headroom that document
/// says is needed.
///
/// A round *decimal* 32 MB rather than 32 MiB, so that the sentence a refusal
/// shows reads `over 32 MB` — `crate::model::fmt_bytes` is decimal, and a
/// ceiling that rendered as `33.6 MB` would look like a number somebody typed
/// wrong.
pub const MAX_BYTES: u64 = 32_000_000;

/// What to ask the platform picker for. `extensions` is honoured by the desktop
/// dialog and ignored by Android — which is exactly why [`import`] checks the
/// bytes rather than the name.
pub const PICK_REQUEST: PickRequest = PickRequest {
    title: "Pick a PDF",
    kind: "PDF",
    extensions: &["pdf"],
    max_bytes: MAX_BYTES,
};

/// How far into a file the header is allowed to be.
///
/// `%PDF-` is supposed to be the first five bytes and frequently is not — a
/// byte-order mark, an HTTP preamble a download tool left behind, a mail
/// gateway's junk. Every real PDF reader scans a leading window instead of
/// insisting on offset zero, and PDF 32000-1's own conformance note allows the
/// header to be preceded by arbitrary bytes as long as the cross-reference
/// offsets are measured from it. 1 KiB is the window Acrobat is documented to
/// use and it is what hayro's own repair path will cope with.
const HEADER_WINDOW: usize = 1024;

/// The longest a title taken from a file name is allowed to be.
///
/// The same 40 as `chart_editor::TITLE_LIMIT`, and deliberately: both end up in
/// the same collapsed row under the same card, and two producers with two
/// different cut-off points would show as a ragged column.
const TITLE_LIMIT: usize = 40;

/// What an imported PDF is called when nothing usable came back with it —
/// which on Android is most `content://` URIs. The register matches
/// `chart_editor`'s `lyrics`: a lower-case common noun, so the row reads
/// `chart · pdf` and looks like a description rather than a failed lookup.
const FALLBACK_TITLE: &str = "chart";

/// Everything that can stop a file becoming an attachment.
///
/// A variant each, rather than one string, because the screen has to be able to
/// tell them apart later — `NotAPdf` is worth explaining to the user, `Write`
/// is worth reporting as a fault, and `Cancelled` is not an error at all and so
/// is not in here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// Zero bytes. A file that exists and is empty, which the picker will
    /// happily hand over.
    Empty,
    /// No `%PDF-` header in the first [`HEADER_WINDOW`] bytes. On Android this
    /// is the *expected* failure rather than an exotic one: the SAF picker
    /// cannot be told to offer only PDFs, so the user can pick a photo.
    NotAPdf,
    /// Over [`MAX_BYTES`].
    TooLarge(u64),
    /// There is no library on disk to put it in — the in-memory mode the
    /// `--seed` screenshots and the tests run in. Nothing was attached.
    NoLibrary,
    /// The row could not be written. `Storage` has already recorded the fault
    /// with the detail; this only says the import did not happen.
    NotAttached,
    /// The row went in and the bytes did not. The row has been taken out again.
    Write(String),
}

impl ImportError {
    /// The sentence the screen shows. Each one says what happened and, where
    /// there is one, what to do about it — the register `song_form`'s "A song
    /// needs a title. Everything else can wait." set.
    pub fn message(&self) -> String {
        match self {
            Self::Empty => "That file is empty.".to_string(),
            Self::NotAPdf => "That is not a PDF. Pick a file ending in .pdf.".to_string(),
            Self::TooLarge(bytes) => format!(
                "That file is {}. This app will not open one over {}.",
                crate::model::fmt_bytes(*bytes),
                crate::model::fmt_bytes(MAX_BYTES)
            ),
            Self::NoLibrary => {
                "There is no library on this device to save it in.".to_string()
            }
            Self::NotAttached => "That chart could not be saved.".to_string(),
            Self::Write(_) => "That chart could not be written to this device.".to_string(),
        }
    }
}

/// Whether these bytes are a PDF at all.
///
/// The one check that decides whether a file is imported. It looks at the
/// bytes and never at the name, because on Android there is often no name and
/// the picker cannot be restricted to PDFs — see [`crate::picker`].
pub fn looks_like_pdf(bytes: &[u8]) -> bool {
    let window = &bytes[..bytes.len().min(HEADER_WINDOW)];
    window.windows(5).any(|w| w == b"%PDF-")
}

/// How many pages, or `None` when hayro could not get far enough to say.
///
/// Takes an [`Arc`] rather than a slice because `Pdf` keeps the buffer it
/// parses — `hayro_syntax` is a zero-copy parser and every object it hands back
/// borrows the original bytes. Passing `&[u8]` would mean copying a 32 MB chart
/// to count its pages, and then the caller would still be holding the original
/// to write to disk.
///
/// Zero pages comes back as `None` too. A PDF whose page tree is empty has
/// nothing D4 could draw, and `song_detail` rendering `primary · 0 pages` would
/// be a worse answer than the one it already has for a chart it cannot describe.
pub fn page_count(data: Arc<Vec<u8>>) -> Option<u32> {
    let pdf = Pdf::new(data).ok()?;
    let pages = pdf.pages().len();
    (pages > 0).then(|| pages as u32)
}

/// What to call the chart, from whatever name came back with the file.
///
/// The extension goes, because the kind is already on the row — `descriptor()`
/// puts `· pdf` after the title, and `landslide.pdf · pdf` reads like a
/// stutter. Everything else is the same treatment `chart_editor::chart_title`
/// gives a typed chart: whitespace collapsed, cut at [`TITLE_LIMIT`] with an
/// ellipsis, and a plain word when there is nothing left.
pub fn title_for(name: Option<&str>) -> String {
    let name = name.unwrap_or_default().trim();
    let stem = match name.rsplit_once('.') {
        // Only a real extension is dropped. A song called `Mr. Jones` keeps its
        // full stop, because ` Jones` is not an extension by any reading — and
        // a name that is *nothing but* an extension loses everything, which is
        // what sends it to the fallback below rather than titling a chart
        // `.pdf`.
        Some((stem, extension))
            if (1..=8).contains(&extension.chars().count())
                && extension.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            stem
        }
        _ => name,
    };
    let collapsed = stem.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return FALLBACK_TITLE.to_string();
    }
    if collapsed.chars().count() <= TITLE_LIMIT {
        return collapsed;
    }
    let cut: String = collapsed.chars().take(TITLE_LIMIT).collect();
    format!("{}…", cut.trim_end())
}

/// Take a picked file into the library as a chart on `song`.
///
/// The whole of D3's import, and the only thing the screen calls. See the
/// module header for why the order is judge / attach / write / undo, and why a
/// PDF hayro cannot parse is imported anyway.
pub fn import(
    songs: SongsStore,
    song: SongId,
    file: PickedFile,
) -> Result<AttachmentId, ImportError> {
    let PickedFile { name, bytes } = file;

    // Judged before anything is written, so a refusal leaves the library
    // exactly as it was found. `NotAPdf` comes before the size check on
    // purpose: told that a 40 MB video is both too large and not a PDF, "not a
    // PDF" is the half that explains what to do next.
    if bytes.is_empty() {
        return Err(ImportError::Empty);
    }
    if !looks_like_pdf(&bytes) {
        return Err(ImportError::NotAPdf);
    }
    if bytes.len() as u64 > MAX_BYTES {
        return Err(ImportError::TooLarge(bytes.len() as u64));
    }

    let data = Arc::new(bytes);
    let pages = page_count(Arc::clone(&data));

    // `bytes_on_disk` is known now rather than corrected afterwards: a copied
    // file is exactly as big as the bytes that were copied, so unlike a capture
    // — which cannot know its own size until every asset has landed — there is
    // no second write to make here.
    let attachment = Attachment {
        id: 0,
        kind: AttachmentKind::Pdf,
        title: title_for(name.as_deref()),
        bytes_on_disk: data.len() as u64,
        page_count: pages,
        source_url: None,
        captured_at: None,
        // A PDF's text is not extracted. G2's search inside attachments will
        // want it and D4's rasteriser is where it would come from; until then
        // this is honestly empty rather than a copy of the file as mojibake.
        body: None,
        // A PDF has no source to check against. E6's re-check reads
        // `source_url` before it ever reads this, so a PDF is already
        // excluded on the field above — this stays `None` rather than a day
        // that would never mean anything.
        rechecked_at: None,
    };

    let attachments = songs.attachments();
    let id = songs
        .attach(song, attachment)
        .ok_or(ImportError::NotAttached)?;

    let Some(directory) = attachments.directory(id) else {
        // In-memory mode: the row exists and there is nowhere for the file. An
        // attachment with no bytes behind it would render as a chart the viewer
        // could never open, so the row comes back out.
        songs.detach(song, id);
        return Err(ImportError::NoLibrary);
    };

    if let Err(e) = std::fs::write(directory.join(PDF_FILE), data.as_slice()) {
        songs.detach(song, id);
        return Err(ImportError::Write(e.to_string()));
    }

    // D4: draw page one, now, while the bytes are still in hand and the user is
    // still watching the thing they asked for happen. One page, so this is a
    // frame's worth of work rather than the whole-book loop the card's wording
    // would suggest — `pages`' module header is the argument for that, and for
    // why every other page waits until something asks.
    //
    // The result is deliberately dropped. A chart hayro will not draw is still a
    // chart the user chose to keep, exactly as a chart hayro will not *count* is
    // — see the module header — and the card on song detail already has an
    // honest sentence for a PDF with no picture behind it. Undoing an import
    // over a failed preview would be the 0.x library getting the veto this
    // module spent four paragraphs refusing to give it.
    let _ = pages::cache_page(&directory, Arc::clone(&data), 1);

    // And now say so. The row went into `AttachmentsStore` — and onto the
    // screen — several statements ago, when the directory behind it was still
    // empty, so the card has already drawn itself once against a chart with no
    // file and no page and has already said "No page preview yet." Nothing
    // since has touched a signal. See `AttachmentsStore::files_changed` for why
    // this is a redraw and not a save.
    attachments.files_changed(id);

    Ok(id)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::db::scratch;
    use crate::pdf::test_support::inked_pdf;
    use rinch::prelude::Memo;
    use crate::model::Song;
    use crate::picker::test_support::Canned;
    use crate::picker::{FilePicker, Picked};
    use crate::store::{AttachmentsStore, SetlistsStore, Storage};

    /// A library with a real directory behind it, because the whole point of
    /// [`import`] is that bytes land somewhere.
    fn on_disk(name: &str) -> (SongsStore, AttachmentsStore, Storage) {
        let dir = scratch(name);
        let storage = Storage::open(&dir);
        let attachments = AttachmentsStore::restored(storage, Vec::new());
        let setlists = SetlistsStore::restored(storage, Vec::new());
        let songs = SongsStore::restored(storage, attachments, setlists, Vec::new());
        songs.create(Song::new(0, "Carolina", "M. Ward")).expect("a song");
        (songs, attachments, storage)
    }

    fn picked(name: &str, bytes: Vec<u8>) -> PickedFile {
        PickedFile {
            name: Some(name.to_string()),
            bytes,
        }
    }

    // ── is this a PDF ───────────────────────────────────────────────────────

    #[test]
    fn the_header_is_looked_for_rather_than_insisted_on_at_offset_zero() {
        assert!(looks_like_pdf(b"%PDF-1.7\nrest"));
        // A byte-order mark, or whatever a download tool left in front.
        assert!(looks_like_pdf(b"\xef\xbb\xbf%PDF-1.4"));
        let mut late = vec![b' '; 900];
        late.extend_from_slice(b"%PDF-1.4");
        assert!(looks_like_pdf(&late));
    }

    #[test]
    fn something_that_is_not_a_pdf_is_not_a_pdf() {
        assert!(!looks_like_pdf(b""));
        assert!(!looks_like_pdf(b"\x89PNG\r\n\x1a\n"));
        assert!(!looks_like_pdf(b"%PDF"), "the dash is part of the header");
        // Past the window is past the window: a file with the header a
        // megabyte in is not one this app is going to go looking through.
        let mut buried = vec![b' '; HEADER_WINDOW + 10];
        buried.extend_from_slice(b"%PDF-1.4");
        assert!(!looks_like_pdf(&buried));
    }

    // ── counting pages ──────────────────────────────────────────────────────

    #[test]
    fn a_pdf_is_counted_by_its_pages_and_not_its_objects() {
        assert_eq!(page_count(Arc::new(inked_pdf(1))), Some(1));
        assert_eq!(page_count(Arc::new(inked_pdf(3))), Some(3));
    }

    #[test]
    fn a_pdf_hayro_cannot_read_counts_as_no_count_rather_than_no_pdf() {
        // A header and nothing behind it. This is the case the module header
        // argues about: it is still imported, it simply has no page count.
        let broken = b"%PDF-1.4\nnothing here at all\n".to_vec();
        assert_eq!(page_count(Arc::new(broken.clone())), None);
        assert!(looks_like_pdf(&broken), "and it is still a PDF as far as import cares");
    }

    // ── the name it wears ───────────────────────────────────────────────────

    #[test]
    fn the_extension_comes_off_because_the_row_already_says_pdf() {
        assert_eq!(title_for(Some("landslide.pdf")), "landslide");
        assert_eq!(title_for(Some("Set 1 — full.PDF")), "Set 1 — full");
    }

    #[test]
    fn a_full_stop_that_is_not_an_extension_is_left_alone() {
        assert_eq!(title_for(Some("Mr. Jones")), "Mr. Jones");
        assert_eq!(title_for(Some("track.02.chart.pdf")), "track.02.chart");
    }

    #[test]
    fn a_file_with_no_name_gets_a_word_rather_than_a_blank_row() {
        assert_eq!(title_for(None), FALLBACK_TITLE);
        assert_eq!(title_for(Some("   ")), FALLBACK_TITLE);
        assert_eq!(title_for(Some(".pdf")), FALLBACK_TITLE);
    }

    #[test]
    fn a_long_name_is_cut_the_same_way_a_typed_chart_is() {
        let title = title_for(Some(&format!("{}.pdf", "la ".repeat(40))));
        assert!(title.chars().count() <= TITLE_LIMIT + 1, "{title}");
        assert!(title.ends_with('…'));
        assert!(!title.contains(" …"));
    }

    // ── the import path ─────────────────────────────────────────────────────

    #[test]
    fn a_picked_pdf_lands_as_a_chart_with_its_size_and_its_pages() {
        let (songs, attachments, storage) = on_disk("pdf-import");
        let song = songs.songs.get()[0].id;
        let bytes = inked_pdf(2);
        let size = bytes.len() as u64;

        let id = import(songs, song, picked("landslide.pdf", bytes.clone())).expect("imported");

        let row = attachments.get(id).expect("the row");
        assert_eq!(row.kind, AttachmentKind::Pdf);
        assert_eq!(row.title, "landslide");
        assert_eq!(row.page_count, Some(2));
        assert_eq!(row.bytes_on_disk, size);
        assert_eq!(row.body, None, "a PDF has no extracted text yet");

        // The first chart a song gets is its primary one — D1's rule, and this
        // import goes through it rather than around it.
        assert_eq!(songs.get(song).unwrap().primary_attachment, Some(id));

        // And the bytes are actually there, under the fixed name.
        let file = attachments.directory(id).unwrap().join(PDF_FILE);
        assert_eq!(std::fs::read(&file).unwrap(), bytes);

        storage.close();
    }

    #[test]
    fn a_pdf_hayro_cannot_parse_is_still_kept_with_no_page_count() {
        let (songs, attachments, storage) = on_disk("pdf-unparseable");
        let song = songs.songs.get()[0].id;

        let id = import(songs, song, picked("odd.pdf", b"%PDF-1.4\nbroken".to_vec()))
            .expect("kept anyway");

        assert_eq!(attachments.get(id).unwrap().page_count, None);
        assert!(attachments.directory(id).unwrap().join(PDF_FILE).exists());
        storage.close();
    }

    #[test]
    fn a_file_that_is_not_a_pdf_is_refused_and_leaves_nothing_behind() {
        let (songs, attachments, storage) = on_disk("pdf-not-a-pdf");
        let song = songs.songs.get()[0].id;

        let refused = import(songs, song, picked("holiday.jpg", b"\xff\xd8\xff\xe0 JFIF".to_vec()));

        assert_eq!(refused, Err(ImportError::NotAPdf));
        assert!(attachments.items.get().is_empty(), "no row");
        assert!(songs.get(song).unwrap().attachments.is_empty(), "and the song is untouched");
        assert_eq!(songs.get(song).unwrap().primary_attachment, None);
        storage.close();
    }

    #[test]
    fn an_empty_file_is_refused_before_it_is_looked_at() {
        let (songs, _, storage) = on_disk("pdf-empty");
        let song = songs.songs.get()[0].id;
        assert_eq!(
            import(songs, song, picked("nothing.pdf", Vec::new())),
            Err(ImportError::Empty)
        );
        storage.close();
    }

    #[test]
    fn a_file_over_the_ceiling_is_refused_by_size_not_by_heap() {
        let (songs, _, storage) = on_disk("pdf-too-large");
        let song = songs.songs.get()[0].id;

        let mut huge = inked_pdf(1);
        huge.resize(MAX_BYTES as usize + 1, b' ');
        let refused = import(songs, song, picked("book.pdf", huge));

        assert_eq!(refused, Err(ImportError::TooLarge(MAX_BYTES + 1)));
        assert!(songs.get(song).unwrap().attachments.is_empty());
        storage.close();
    }

    /// The `--seed` screenshots and every in-memory test run without a
    /// database. Importing there would mint a row pointing at a directory that
    /// does not exist, so it does not happen at all.
    #[test]
    fn with_no_library_on_disk_nothing_is_attached() {
        let songs = SongsStore::new(vec![Song::new(1, "Carolina", "M. Ward")]);
        let refused = import(songs, 1, picked("landslide.pdf", inked_pdf(1)));

        assert_eq!(refused, Err(ImportError::NoLibrary));
        assert!(songs.get(1).unwrap().attachments.is_empty(), "the row was taken back out");
        assert!(songs.attachments().items.get().is_empty());
    }

    /// The failure that matters most, because it is the one that could leave
    /// the library lying: a row saying there is a chart, and no chart.
    /// The bug this found, in the two halves that can be checked.
    ///
    /// `SongsStore::attach` puts the row into `AttachmentsStore::items` — and
    /// therefore on screen — before this function has written a byte into the
    /// directory it just minted, so the card draws itself once against an empty
    /// directory and says "No page preview yet." Everything after that is
    /// `std::fs`, which no signal is watching, so without a deliberate word to
    /// the screen that sentence stays put over a picture that exists. It was
    /// found exactly that way, on the private display, with the PNG on disk and
    /// the card insisting there was none.
    ///
    /// Half one, below: by the time `import` returns, the page is there — so
    /// anything that redraws afterwards is guaranteed to find it. Half two is
    /// [`attachments_notice_new_files`]: `files_changed` does redraw. A test
    /// that counted the notifications in between is not available, because a
    /// `Memo` is pulled rather than pushed and collapses two writes into one
    /// recompute — which is precisely why the bug was invisible until a real
    /// screen drew it.
    #[test]
    fn an_import_has_drawn_page_one_by_the_time_it_returns() {
        let (songs, attachments, _) = on_disk("pdf-import-page-first");
        let id = import(songs, 1, picked("landslide.pdf", inked_pdf(2))).expect("imported");
        let directory = attachments.directory(id).expect("a directory");

        assert!(directory.join(PDF_FILE).is_file(), "the chart");
        assert!(pages::cached_page(&directory, 1).is_some(), "and page one, drawn");
        assert!(
            pages::cached_page(&directory, 2).is_none(),
            "and only page one — the rest wait until something asks"
        );
    }

    /// The other half. A `Memo` stands in for the card: it recomputes when, and
    /// only when, something writes to the signal it read.
    #[test]
    fn attachments_notice_new_files() {
        let (songs, attachments, _) = on_disk("pdf-files-changed");
        let id = import(songs, 1, picked("landslide.pdf", inked_pdf(1))).expect("imported");

        let runs = Rc::new(RefCell::new(0usize));
        let counter = Rc::clone(&runs);
        let card = Memo::new(move || {
            attachments.items.get().len();
            *counter.borrow_mut() += 1;
        });
        card.get();
        let before = *runs.borrow();

        attachments.files_changed(id);
        card.get();
        assert_eq!(*runs.borrow(), before + 1, "a redraw, from a row that did not change");

        // And an id nothing knows about is not an excuse to redraw every card
        // on the screen.
        attachments.files_changed(9_999);
        card.get();
        assert_eq!(*runs.borrow(), before + 1);
    }

    #[test]
    fn a_write_that_cannot_land_takes_the_row_back_out_with_it() {
        let (songs, attachments, storage) = on_disk("pdf-write-fails");
        let song = songs.songs.get()[0].id;

        // Import once so the attachments root exists, then make the *next*
        // attachment's directory unwritable by replacing it — the id is
        // predictable because ids are minted in order.
        let first = import(songs, song, picked("one.pdf", inked_pdf(1))).expect("first");
        let doomed_dir = attachments
            .directory(first)
            .unwrap()
            .with_file_name((first as u64 + 1).to_string());
        std::fs::create_dir_all(&doomed_dir).unwrap();
        let mut perms = std::fs::metadata(&doomed_dir).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o500);
        std::fs::set_permissions(&doomed_dir, perms).unwrap();

        let refused = import(songs, song, picked("two.pdf", inked_pdf(1)));

        assert!(
            matches!(refused, Err(ImportError::Write(_))),
            "{refused:?}"
        );
        assert_eq!(
            songs.get(song).unwrap().attachments,
            vec![first],
            "the half-made chart is gone and the good one is not"
        );
        assert_eq!(songs.get(song).unwrap().primary_attachment, Some(first));
        // And the undo took the directory with it, because `detach` deletes
        // the row and `Repo::delete_attachment` deletes what the row owned —
        // there is nothing left of the failed import to find.
        assert!(!doomed_dir.exists());
        storage.close();
    }

    // ── picker and import, joined the way the screen joins them ─────────────

    /// What `song_detail`'s callback does, in a function a test can call.
    ///
    /// The same three arms in the same order, returning the sentence the screen
    /// would put in its `trouble` signal — `None` meaning the screen says
    /// nothing, which is both what a success looks like and what a cancel looks
    /// like. Keeping the shape identical is the point: if the screen ever grew
    /// a fourth case these would stop matching, and the tests below are the
    /// only place that would be noticed without a phone in hand.
    fn on_answer(songs: SongsStore, song: SongId, answer: Picked) -> Option<String> {
        match answer {
            Picked::Cancelled => None,
            Picked::Failed(why) => Some(why),
            Picked::Chose(file) => import(songs, song, file).err().map(|e| e.message()),
        }
    }

    /// The card's whole shape in one test: a picker is asked, it answers
    /// without a dialog, and a chart is on the song afterwards. `Canned` is the
    /// reason [`crate::picker::FilePicker`] is a trait rather than a second
    /// pair of `#[cfg]` functions, and this is what it was for.
    #[test]
    fn a_pick_that_chooses_a_pdf_ends_with_a_chart_on_the_song() {
        let (songs, attachments, storage) = on_disk("pdf-through-the-picker");
        let song = songs.songs.get()[0].id;
        let bytes = inked_pdf(2);
        let picker = Canned::new(Picked::Chose(picked("landslide.pdf", bytes.clone())));

        // The callback is the only thing that learns the answer, so what it did
        // has to be recorded from inside it — exactly as the screen records a
        // failure into a signal.
        let trouble = Rc::new(RefCell::new(Some("nothing has happened yet".to_string())));
        let recorder = Rc::clone(&trouble);
        picker.pick(
            PICK_REQUEST,
            Box::new(move |answer| *recorder.borrow_mut() = on_answer(songs, song, answer)),
        );

        assert_eq!(*trouble.borrow(), None, "nothing went wrong");
        assert_eq!(picker.asked.borrow()[0], PICK_REQUEST, "and it was asked for a PDF");

        let charts = songs.get(song).unwrap().attachments;
        assert_eq!(charts.len(), 1);
        let row = attachments.get(charts[0]).unwrap();
        assert_eq!(row.title, "landslide");
        assert_eq!(row.kind, AttachmentKind::Pdf);
        assert_eq!(row.page_count, Some(2));
        assert_eq!(row.bytes_on_disk, bytes.len() as u64);
        assert_eq!(
            std::fs::read(attachments.directory(charts[0]).unwrap().join(PDF_FILE)).unwrap(),
            bytes
        );
        storage.close();
    }

    /// Closing the dialog is not an error and must not look like one. The
    /// screen shows nothing at all for this, so the thing worth asserting is
    /// that nothing at all happened.
    #[test]
    fn a_pick_that_is_cancelled_says_nothing_and_changes_nothing() {
        let (songs, attachments, storage) = on_disk("pdf-cancelled");
        let song = songs.songs.get()[0].id;
        let picker = Canned::new(Picked::Cancelled);

        let trouble = Rc::new(RefCell::new(Some("nothing has happened yet".to_string())));
        let recorder = Rc::clone(&trouble);
        picker.pick(
            PICK_REQUEST,
            Box::new(move |answer| *recorder.borrow_mut() = on_answer(songs, song, answer)),
        );

        assert_eq!(*trouble.borrow(), None, "a cancel is not something to complain about");
        assert!(songs.get(song).unwrap().attachments.is_empty());
        assert!(attachments.items.get().is_empty());
        storage.close();
    }

    /// The picker itself failed — the file vanished, or the `ContentResolver`
    /// refused. `import` is never reached, so the library is untouched and the
    /// screen shows what the platform said.
    #[test]
    fn a_pick_that_failed_never_reaches_the_import() {
        let (songs, attachments, storage) = on_disk("pdf-pick-failed");
        let song = songs.songs.get()[0].id;
        let picker = Canned::new(Picked::Failed("No such file or directory".into()));

        let trouble = Rc::new(RefCell::new(None));
        let recorder = Rc::clone(&trouble);
        picker.pick(
            PICK_REQUEST,
            Box::new(move |answer| *recorder.borrow_mut() = on_answer(songs, song, answer)),
        );

        assert!(trouble.borrow().is_some());
        assert!(attachments.items.get().is_empty(), "nothing was written");
        storage.close();
    }

    #[test]
    fn every_refusal_says_something_a_person_could_act_on() {
        for error in [
            ImportError::Empty,
            ImportError::NotAPdf,
            ImportError::TooLarge(99_000_000),
            ImportError::NoLibrary,
            ImportError::NotAttached,
            ImportError::Write("EACCES".into()),
        ] {
            let message = error.message();
            assert!(!message.is_empty(), "{error:?}");
            assert!(message.ends_with('.'), "{message}");
        }
        assert!(ImportError::TooLarge(99_000_000).message().contains("99 MB"));
    }
}
