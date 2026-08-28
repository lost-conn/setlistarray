//! The one place a captured page is drawn — card E5.
//!
//! Not a screen and not a route, which is why it is the only file in here that
//! `screens::mod` exports without a `Route` pointing at it. It is a component
//! because of the sentence the card asked for: *the card and the viewer should
//! agree; a captured page should look like the same object in both places, the
//! way a PDF does.* A PDF gets that for free — `pdf::pages` rasterises one PNG
//! and both screens draw the same file. A captured page has no such artefact,
//! so the agreement has to be a shared component instead: `song_detail` mounts
//! this in its card and `attachment_viewer` mounts it full-screen, and the only
//! thing they pass differently is how big it is.
//!
//! What it draws, in order:
//!
//! | State | Drawn |
//! | --- | --- |
//! | `page.html` renders | the page, through [`crate::capture::render`] |
//! | no `page.html`, but the row has extracted text | that text, through the same renderer |
//! | neither | one sentence saying which of the two it is |
//!
//! The middle row is not a consolation prize, it is the honest answer to a real
//! state. `Attachment::body` lives in the database and `page.html` lives in a
//! directory on a phone, so the two go missing independently: a library folder
//! deleted by a file manager, a restore that brought the database back and not
//! the files, an `--seed` run where there was never a directory at all. In
//! every one of those the user still has the words, and showing them beats
//! showing an error about a file they did not know existed. When the files were
//! *expected* — there is a library on this device and the page is not in it —
//! the muted line above the text says so, because that is a thing the reader can
//! act on and E6's re-check is the thing that would fix it.
//!
//! ## Why the markup goes in through `set_inner_html`
//!
//! The long version is `crate::capture::render`'s header. The short version is
//! that `rsx!` needs its tags at compile time and a stranger's page does not
//! have any, so the choice is between flattening the page to a list of blocks —
//! which loses inline flow, and inline flow is where the `<b>` around each chord
//! symbol lives — and handing Rinch markup. `NodeHandle::set_inner_html` is the
//! documented way to do the second, the nodes it builds are ordinary DOM nodes
//! that Stylo and Taffy treat like any other, and the fragment it is given is
//! one this app generated rather than one a website served.
//!
//! It is called **once, from a component body**, which is the same rule
//! `pdf::pages` wrote for rasterising and for the same reason: the work is a
//! parse and a walk, and a render closure re-runs on every redraw. A component
//! body runs once per mount. That is also what makes the viewer's zoom work —
//! `attachment_viewer` already rebuilds its scrolling box on every zoom step,
//! for a scroll-offset fault that has nothing to do with this, so a new
//! `base_px` arrives as a remount and the page is simply drawn again at the new
//! size. Measured at **1.8 ms** for the largest page `docs/CAPTURE.md` captured
//! (CifraClub, 177 KB), against the 38 ms `pdf::pages` spends turning one PDF
//! page on the same phone.

use rinch::prelude::*;

use crate::capture::render::{self, Page, Sizing};
use crate::model::AttachmentId;
use crate::store::AttachmentsStore;
use crate::theme::T_META_SMALL;

/// The sentence for a capture whose files are not on this device.
///
/// Present tense and no blame: the row is still in the library, the text may
/// still be readable underneath, and the thing that fixes it is E6's re-check
/// rather than anything the reader can do right now.
pub const FILES_GONE: &str = "The saved page is no longer on this device.";

/// The sentence for a capture with a `page.html` that came to nothing.
///
/// Distinct from [`FILES_GONE`] because the two are different faults with
/// different fixes — a deleted directory against a file that is there and holds
/// no page — and `song_detail::note` makes the same argument about giving every
/// empty state its own words rather than a shared "couldn't load".
pub const UNREADABLE: &str = "This saved page could not be read.";

/// Said when the element budget ran out before the page did.
///
/// Only ever seen in the viewer. The card is a preview by construction and
/// stops early on every page long enough to be worth capturing, so saying it
/// there would be saying it always.
pub const TRUNCATED: &str = "This page was too long to draw in full.";

/// What one mounting of this component decided to draw.
///
/// A plain value with a plain constructor, so `cargo test` can drive every
/// branch — including the two that need a directory on disk — without a window.
#[derive(Clone, Debug, PartialEq)]
pub enum Shown {
    /// The saved page itself.
    Page(Page),
    /// The extracted text. `gone` is true when there was a library to hold the
    /// files and they were not in it, which is what puts [`FILES_GONE`] above
    /// the text.
    Text { page: Page, gone: bool },
    /// Neither, and which of the two sentences says so.
    Nothing(&'static str),
}

impl Shown {
    /// The fragment to inject, if there is one.
    pub fn markup(&self) -> Option<&str> {
        match self {
            Shown::Page(page) | Shown::Text { page, .. } => Some(&page.markup),
            Shown::Nothing(_) => None,
        }
    }

    /// Whether the note belongs above the content rather than under it.
    ///
    /// Two different jobs, and putting both in the same place got one of them
    /// wrong. [`FILES_GONE`] explains *what you are looking at* — the words
    /// without the page — and on `:99` on 2026-08-28 it sat at the bottom of a
    /// two-thousand-pixel column of extracted text, which is a footnote nobody
    /// reaches to a question they had at the top. [`TRUNCATED`] is the opposite
    /// kind of sentence: it is about what came *after* what you can see, so it
    /// belongs where the page stops. [`UNREADABLE`] is the only thing on the
    /// screen and it does not matter which side it is on.
    pub fn note_before(&self) -> bool {
        !matches!(self, Shown::Page(_))
    }

    /// The muted line, above or under the content — see [`note_before`].
    ///
    /// [`note_before`]: Shown::note_before
    pub fn note(&self) -> Option<&'static str> {
        match self {
            Shown::Page(page) if page.truncated => Some(TRUNCATED),
            Shown::Page(_) => None,
            Shown::Text { gone: true, .. } => Some(FILES_GONE),
            Shown::Text { gone: false, .. } => None,
            Shown::Nothing(sentence) => Some(sentence),
        }
    }
}

/// Decide what to draw, reading the disk once.
///
/// Split out from the component so that the decision is testable and so that
/// both screens provably make it the same way. The `directory` is `None` in the
/// in-memory library the `--seed` screenshots and the tests run in — there was
/// never a file, so its absence is not a fault and no sentence is shown for it.
pub fn decide(
    directory: Option<&std::path::Path>,
    body: Option<&str>,
    sizing: Sizing,
    budget: usize,
) -> Shown {
    let saved = directory.and_then(render::read);
    if let Some(saved) = saved {
        let page = render::render(&saved, directory.unwrap_or(std::path::Path::new("")), sizing, budget);
        if !page.is_empty() {
            return Shown::Page(page);
        }
        // The file is there and nothing came out of it. Fall through to the
        // text, because a page with no markup this app will draw can still have
        // had its words extracted at capture time — and say `UNREADABLE` rather
        // than `FILES_GONE` if there is no text either, because the files are
        // exactly where they should be.
        let text = render::from_text(body.unwrap_or_default());
        if text.is_empty() {
            return Shown::Nothing(UNREADABLE);
        }
        return Shown::Text {
            page: text,
            gone: false,
        };
    }

    let text = render::from_text(body.unwrap_or_default());
    // `directory.is_some()` is the whole of "were the files expected": a
    // persistent library hands out a directory for every attachment whether or
    // not anything has been written into it, and an in-memory one hands out
    // none at all.
    let gone = directory.is_some();
    if text.is_empty() {
        return Shown::Nothing(if gone { FILES_GONE } else { UNREADABLE });
    }
    Shown::Text { page: text, gone }
}

/// A captured page, drawn at the size the caller asks for.
///
/// `base_px` is the type size the fragment's `em` lengths resolve against — see
/// `render`'s header for why every length in it is relative — and `column_px`
/// is how wide the host is, which is what an image is fitted to. `budget` is
/// `render::CARD_ELEMENTS` from the card and `render::VIEWER_ELEMENTS` from the
/// viewer.
#[component]
pub fn CapturedPageView(
    attachment: Option<AttachmentId>,
    base_px: Option<f32>,
    column_px: Option<u32>,
    budget: Option<usize>,
    style: Option<String>,
) -> NodeHandle {
    let attachments = use_store::<AttachmentsStore>();
    let id = attachment.unwrap_or_default();
    let sizing = Sizing {
        base_px: base_px.unwrap_or(13.0),
        column_px: column_px.unwrap_or(crate::WIDTH),
    };

    // Read and render once, here in the body, never in a render closure. Both
    // halves are expensive by this screen's standards — `body` is an object
    // read off the database and the render is a full html5ever parse — and
    // neither can change while this component is mounted: a captured page is a
    // file that was written once and a row that nothing edits.
    let shown = decide(
        attachments.directory(id).as_deref(),
        attachments.body(id).as_deref(),
        sizing,
        budget.unwrap_or(render::VIEWER_ELEMENTS),
    );

    let outer = format!(
        "display: flex; flex-direction: column; min-width: 0; {}",
        style.unwrap_or_default()
    );
    // The type size lives on the host and everything under it is relative to
    // this one number. `min-width: 0` is what lets a `<pre>` wider than the
    // column scroll inside itself instead of widening the whole screen.
    let host = format!(
        "font-size: {:.2}px; line-height: 1.5; min-width: 0;",
        sizing.base_px
    );
    let markup = shown.markup().unwrap_or_default().to_string();
    let note = shown.note();
    let before = shown.note_before();

    rsx! {
        div {
            style: {outer.clone()},
            if let Some(sentence) = note.filter(|_| before) {
                div {
                    style: {format!("{T_META_SMALL} margin-bottom: 6px;")},
                    {sentence}
                }
            }
            {injected(__scope, markup.clone(), host.clone())}
            if let Some(sentence) = note.filter(|_| !before) {
                div {
                    style: {format!("{T_META_SMALL} margin-top: 6px;")},
                    {sentence}
                }
            }
        }
    }
}

/// The host element, with the fragment already inside it.
///
/// The one imperative call in the app, and it is the framework's own escape
/// hatch rather than a way round `rsx!`: `rsx!` builds the element, this hands
/// its children over as markup, and everything Rinch does with them afterwards
/// — the cascade, the layout, the inline flow — is what it does with any other
/// node. An empty fragment is still injected, which is a no-op that clears
/// nothing, so there is no branch here for the case with no page.
///
/// Visible to the crate because card E3's capture screen previews a page that
/// is not an attachment yet — there is no `AttachmentId` for it and no
/// directory behind it — so it cannot mount [`CapturedPageView`] and reaches
/// for the host instead. That keeps the "one place decides what a captured page
/// looks like" rule the module header states: the preview and the library draw
/// the same fragment through the same element.
pub(crate) fn injected(scope: &mut RenderScope, markup: String, style: String) -> NodeHandle {
    let __scope = scope;
    let host = rsx! { div { style: {style.clone()} } };
    host.set_inner_html(&markup);
    host
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn sizing() -> Sizing {
        Sizing {
            base_px: 13.0,
            column_px: 313,
        }
    }

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sla-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn decided(directory: Option<&Path>, body: Option<&str>) -> Shown {
        decide(directory, body, sizing(), render::VIEWER_ELEMENTS)
    }

    #[test]
    fn a_saved_page_is_drawn_in_preference_to_the_text_beside_it() {
        let dir = tempdir("captured-page");
        std::fs::write(dir.join("page.html"), "<body><pre>G  C\nline</pre></body>").expect("write");
        let shown = decided(Some(&dir), Some("the extracted text"));
        let Shown::Page(page) = &shown else {
            panic!("expected the page, got {shown:?}");
        };
        assert!(page.markup.contains("G  C\nline"));
        assert!(!page.markup.contains("the extracted text"));
        assert_eq!(shown.note(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The deletion case the card asks about: the row is still in the library
    /// and the files under it are not.
    #[test]
    fn files_deleted_underneath_fall_back_to_the_text_and_say_so() {
        let dir = tempdir("captured-gone");
        let shown = decided(Some(&dir), Some("Verse one\nVerse two"));
        let Shown::Text { page, gone } = &shown else {
            panic!("expected the text, got {shown:?}");
        };
        assert!(*gone);
        assert!(page.markup.contains("Verse one"));
        assert_eq!(shown.note(), Some(FILES_GONE));
        assert!(
            shown.note_before(),
            "it explains what the reader is looking at, so it goes first"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// In-memory mode — `--seed`, and every test in this crate that builds a
    /// library without a repository. There was never a file, so nothing is
    /// missing and nothing is said.
    #[test]
    fn a_library_with_no_directories_shows_the_text_and_no_complaint() {
        let shown = decided(None, Some("Verse one"));
        let Shown::Text { gone, .. } = &shown else {
            panic!("expected the text, got {shown:?}");
        };
        assert!(!*gone);
        assert_eq!(shown.note(), None);
    }

    #[test]
    fn a_page_that_will_not_parse_into_anything_says_which_fault_it_is() {
        let dir = tempdir("captured-unreadable");
        // Well-formed, and every element in it is one the app drops.
        std::fs::write(dir.join("page.html"), "<html><head><title>x</title></head></html>")
            .expect("write");
        assert_eq!(decided(Some(&dir), None), Shown::Nothing(UNREADABLE));
        assert_eq!(decided(Some(&dir), None).note(), Some(UNREADABLE));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nothing_at_all_names_the_files_rather_than_the_parse() {
        let dir = tempdir("captured-empty");
        assert_eq!(decided(Some(&dir), None), Shown::Nothing(FILES_GONE));
        assert_eq!(decided(None, None), Shown::Nothing(UNREADABLE));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A capture written before this card existed has a `page.html` and no
    /// extracted text; one written by an older build that only kept the text
    /// has the text and no file. Both draw something.
    #[test]
    fn either_half_of_a_capture_is_enough_to_draw_it() {
        let dir = tempdir("captured-half");
        std::fs::write(dir.join("page.html"), "<body><p>from the file</p></body>").expect("write");
        assert!(matches!(decided(Some(&dir), None), Shown::Page(_)));
        std::fs::remove_file(dir.join("page.html")).expect("remove");
        assert!(matches!(
            decided(Some(&dir), Some("from the row")),
            Shown::Text { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_page_too_long_for_the_budget_says_so_and_still_draws() {
        let dir = tempdir("captured-long");
        let body: String = (0..80).map(|n| format!("<p>line {n}</p>")).collect();
        std::fs::write(dir.join("page.html"), format!("<body>{body}</body>")).expect("write");
        let shown = decide(Some(&dir), None, sizing(), 12);
        assert!(matches!(shown, Shown::Page(_)));
        assert_eq!(shown.note(), Some(TRUNCATED));
        assert!(
            !shown.note_before(),
            "it is about what came after, so it goes where the page stops"
        );
        assert!(shown.markup().unwrap().contains("line 0"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
