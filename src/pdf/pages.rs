//! A PDF page, as a picture: rasterising with hayro and caching the result.
//!
//! Card D4, and the other half of the decision `docs/PDF.md` took on
//! 2026-08-26. D3 took the parser and left the pixels; this is the pixels.
//!
//! ## Why page one is drawn at import and every other page is not
//!
//! The card says "rasterise on import". Taken literally — every page, at the
//! moment the file arrives — that is a loop this app cannot afford to run on
//! the thread it draws with.
//!
//! `docs/PDF.md` §5 quoted 6.0 ms a page, which was PDFium's *render* on a
//! 9955HX. This whole pipeline — parse, render, quantise, deflate, write —
//! measured **14 ms a page** on the same machine on 2026-08-28, and **37.3 and
//! 38.5 ms** for the same page on the moto g stylus 5G the card exists for,
//! read out of logcat on two separate imports. That is 2.7× the desktop, at the
//! optimistic end of the "3 to 8× slower" band that document could only guess
//! at, and it retires the first of its device-only questions.
//!
//! It is still enough. At 38 ms a page, a 30-page chart book is **1.1 seconds**
//! and a 200-page fake book is **7.6 seconds**, both on the thread that draws —
//! and Android calls five seconds of unresponsive main thread an ANR and kills
//! the process. `docs/PDF.md` reaches the same conclusion at the end of §5
//! without the device numbers: "Both renderers are fast enough to rasterise
//! lazily instead — page one at import for the card, the rest on first open —
//! and that is probably the better design."
//!
//! So that is what this does, and it is a deliberate departure from the card's
//! wording rather than an oversight:
//!
//! - **Page one is drawn synchronously during [`crate::pdf::import`]**, because
//!   it is what the card on song detail shows and the import is already the
//!   moment the user is waiting on. One page is one page: 14 ms here, 38 ms on
//!   the phone, well inside a tap's worth of latency either way.
//! - **Every other page is drawn the first time something asks for it**, through
//!   [`ensure_page`]. Nothing asks yet — the full-screen viewer is D5 — so D4
//!   draws exactly one page per chart and leaves the seam behind it.
//!
//! What this buys, beyond not hanging: a user who imports a 200-page fake book
//! to play four songs out of it pays for four pages, not two hundred, and the
//! cache costs 170 KiB instead of 8.4 MiB.
//!
//! ## Half a cache is the normal state, not a disaster to recover from
//!
//! Because rendering is on demand, "some pages are cached and some are not" is
//! not a corruption — it is what every chart looks like. There is nothing to
//! repair on the next open, and nothing that needs a journal or a marker file,
//! because the absence of `page-7.png` and the never-having-asked-for-page-7
//! are the same fact and want the same response: draw it now.
//!
//! That only holds if a page file is never *partly* there. A process killed
//! between `create` and the last `write` — Android kills apps for breathing —
//! would otherwise leave a truncated PNG, which is worse than no PNG at all:
//! `cached_page` would find it, hand it to Rinch's image loader, and the loader
//! would fail to decode a file that will never get better on its own. So each
//! page is written to `page-N.png.part` and then **renamed** into place.
//! `rename(2)` within a directory is atomic on every filesystem this app can be
//! on, so the visible name either does not exist or is a whole PNG. A `.part`
//! left behind by a kill is invisible to `cached_page`, costs a few tens of
//! kilobytes, and is overwritten the next time that page is asked for.
//!
//! ## The cache is 16 greys, and that is not a compromise
//!
//! `docs/PDF.md` measured the same 30-page chart book at 8.35 MiB as RGB8 PNG,
//! 3.05 MiB as Luma8, and **1.30 MiB as a 16-colour palette** — 44 KiB a page.
//! A chord chart is black ink on white paper; the only pixels that are not one
//! or the other are the antialiased edges of glyphs and staff lines, and
//! sixteen levels put a step every 17/255, which is under the 6/255 or so that
//! a human eye can resolve as banding on a soft edge at this size.
//!
//! That was re-checked on 2026-08-28 rather than taken on trust, because a
//! spike's judgement about how something *looks* is the part of a spike most
//! worth repeating. Method: render a page with poppler's `pdftoppm -gray` at
//! the same 1080 px, quantise poppler's own output to these sixteen levels, and
//! diff it against itself — which isolates what the palette costs from what the
//! renderer costs. Over a generated chord chart, a staff-notation lead sheet
//! with 0.4 pt staff lines (the hardest case there is: a hairline is *all*
//! antialiasing) and the 22-page fig2dev manual:
//!
//! ```text
//! mean |error|  0.07 - 0.29 of 255
//! worst pixel   8 of 255, which is half a step and cannot be otherwise
//! pixels off by more than one level: 0.000 %
//! ```
//!
//! Side by side at 1:1 the two are indistinguishable — staff lines, note heads,
//! stems, chord symbols and 8 pt lyrics all survive. The palette is not a
//! compromise on this material; it is free.
//!
//! **What it does cost is colour, and on the wrong document it costs size too.**
//! A chart with a highlighted section comes back as a photocopy of itself — a
//! 50-page album booklet checked at the same time turned a red logo into a mid
//! grey, still legible and no longer red. That page also cost **347 KB** rather
//! than 42, because film grain is not something a palette or a deflate stream
//! can do anything with. Neither is a reason to change the format: this app is
//! for charts, and both facts are written down here rather than discovered
//! later. `PALETTE_LEVELS` and the ramp under it are the two lines a later card
//! would edit if a chart turns out to deserve its hues.
//!
//! ## Why 1080 pixels wide and only one cache
//!
//! 1080 px is a full-bleed page on a 393 pt-wide phone at 2.75×, which is what
//! D5's viewer needs; the card on song detail is about 320 px and gets there by
//! downscaling. Two caches at two sizes would be two renders, two sets of files
//! to keep consistent, and a card that goes stale differently from the viewer.
//! One cache at the larger size costs 44 KiB a page instead of about 20 and is
//! the only one that can serve both.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use hayro::hayro_interpret::InterpreterSettings;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{RenderCache, RenderSettings, render};
use hayro_syntax::Pdf;

/// How wide a cached page is, in pixels. See the module header.
pub const PAGE_WIDTH: u32 = 1080;

/// How tall a cached page is allowed to get.
///
/// Not a taste decision — a bound the format forces. `vello_cpu::Pixmap` sizes
/// itself in `u16`, so 65,535 is the hard ceiling, and a page whose height
/// scaled past it would not be clipped, it would wrap or panic. Real pages are
/// nowhere near: US Letter at 1080 px wide is 1398 px tall. But a PDF can
/// declare any `MediaBox` it likes, including a 2 pt × 4000 pt strip, and a
/// hostile or merely broken one must come back as a smaller page rather than as
/// a crash. 8192 px leaves room for a genuinely long page — a six-panel foldout
/// chart is about 5600 — and caps the render at 1080 × 8192 × 4 = 34 MB of
/// intermediate bitmap, which is survivable on a phone.
const MAX_PAGE_HEIGHT: u32 = 8192;

/// How many entries the palette has. Sixteen, so a pixel is a nibble and two
/// pixels are a byte; see the module header for what that costs and buys.
const PALETTE_LEVELS: u32 = 16;

/// Everything that can stop a page becoming a PNG.
///
/// Separate from [`crate::pdf::ImportError`] on purpose: none of these stops an
/// import. A chart whose pages cannot be drawn is still a chart the user chose
/// to keep, which is the same argument the module header of `crate::pdf` makes
/// about a page count, and for the same reason — a 0.x rasteriser does not get
/// a veto over what is in somebody's library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// hayro could not parse the file far enough to find a page tree. The same
    /// condition that makes `crate::pdf::page_count` return `None`.
    Unreadable,
    /// There is no page with that number. One-based, so page 0 lands here too.
    NoSuchPage,
    /// hayro parsed the page and could not draw it — either it said so, or it
    /// panicked and [`draw`] caught it. See [`draw`] for why a panic is a
    /// result here rather than the end of the process.
    NotDrawn,
    /// The PNG could not be written. The directory is gone, the disk is full,
    /// the file is a directory. Carries the OS's own words.
    Write(String),
}

/// What the PNG for one page is called inside the attachment's directory.
///
/// Beside `chart.pdf` rather than in a `pages/` subdirectory of its own,
/// because `crate::pdf`'s module header said that is where they would land and
/// because the directory has exactly one owner — the attachment row that
/// created it and deletes it — so there is nothing here for a subdirectory to
/// separate this from.
///
/// One-based and unpadded. One-based because it is the number the card and the
/// viewer say out loud ("2 pages"), and a cache whose file names are off by one
/// from the interface is a bug waiting for somebody to have a bad afternoon.
/// Unpadded because padding only buys a tidy `ls` and only up to whatever width
/// was guessed: `page-010.png` sorts before `page-9.png` just as badly as
/// `page-10.png` does the moment a chart has a thousand pages.
pub fn page_file(page: u32) -> String {
    format!("page-{page}.png")
}

/// The cached PNG for one page, if it is already there. **Never renders.**
///
/// This is what the UI calls, and it is one `stat` — song detail asks it on
/// every redraw of the attachment card, from inside a reactive closure, and a
/// function that could take 30 ms on a phone has no business being reachable
/// from there. Everything that is allowed to rasterise is [`ensure_page`] and
/// [`cache_page`], and neither is called from a render closure.
pub fn cached_page(directory: &Path, page: u32) -> Option<PathBuf> {
    let path = directory.join(page_file(page));
    path.is_file().then_some(path)
}

/// How big a cached page is, in pixels, read from the file rather than decoded.
///
/// The card has to state a width *and* a height on the `<img>` — see
/// `song_detail::page_image` for the framework reason — so it needs the page's
/// shape, and it needs it on every redraw. Decoding a 1080 x 1398 PNG to find
/// out would be 1.5 MB of work per frame.
///
/// So this reads the IHDR instead: PNG's first chunk is fixed at bytes 16..24
/// of every conforming file, big-endian width then height, and a decoder is not
/// required to learn them. The eight-byte signature is checked first so that a
/// `.part` renamed by hand, or a JPEG somebody dropped in the directory, comes
/// back as `None` rather than as two numbers read out of the middle of it.
pub fn page_pixels(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;

    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut head = [0u8; 24];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    if head[..8] != SIGNATURE || &head[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes([head[16], head[17], head[18], head[19]]);
    let height = u32::from_be_bytes([head[20], head[21], head[22], head[23]]);
    (width > 0 && height > 0).then_some((width, height))
}

/// The cached PNG for one page, drawing it first if it is not there yet.
///
/// The self-healing path, and the answer to "what happens when the cache is
/// half built". Three quite different situations arrive here and all three want
/// the same thing done:
///
/// - a chart imported before this card existed, which has a `chart.pdf` and no
///   pages at all;
/// - a chart whose import-time render of page one failed or was interrupted;
/// - any page past the first, which D4 never draws eagerly.
///
/// Reads the whole PDF back off disk, so it is not free — up to
/// [`crate::pdf::MAX_BYTES`] of it. That is the price of not keeping a 32 MB
/// buffer alive for the lifetime of the app, and it is paid once per page that
/// was missing rather than once per look.
pub fn ensure_page(directory: &Path, page: u32) -> Option<PathBuf> {
    if let Some(path) = cached_page(directory, page) {
        return Some(path);
    }
    let bytes = std::fs::read(directory.join(super::PDF_FILE)).ok()?;
    cache_page(directory, Arc::new(bytes), page).ok()
}

/// Which pages are worth drawing before anybody asks for them, given that
/// `page` of `count` is the one on screen.
///
/// Next first, then previous, and nothing else. A viewer's next move is a page
/// turn in one of two directions and there is no third; going wider — the whole
/// document, or a window of five — would spend a phone's battery rasterising a
/// fake book somebody opened to check one chord.
///
/// Previous is included even though it is usually already drawn, because the
/// one case where it is not is the one that matters: opening a chart at page 1,
/// paging forward to 6 and then back is the *only* way to reach a page whose
/// predecessor was never rendered, and it happens the moment a chart is opened
/// twice.
///
/// Separate from [`prefetch`] and pure, so the policy can be tested without a
/// thread, a disk or a PDF.
pub fn neighbours(page: u32, count: u32) -> Vec<u32> {
    [page.saturating_add(1), page.saturating_sub(1)]
        .into_iter()
        .filter(|n| *n >= 1 && *n <= count && *n != page)
        .collect()
}

/// How many prefetch threads may be alive at once.
///
/// One. The work is a single 38 ms render and the queue behind it is at most
/// two pages deep, so a second worker would not finish sooner — it would only
/// contend for the same core as the thread that is drawing the screen. The
/// counter's real job is the pathological case: holding `›` down, or a user
/// tapping through a fake book faster than a page renders, must not leave one
/// detached thread per tap.
const MAX_PREFETCH_THREADS: usize = 1;

static PREFETCH_IN_FLIGHT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Draw `pages` into the cache **on a thread of its own**, and tell nobody.
///
/// ## Why this is the answer to `docs/PDF.md` §9's question 3
///
/// That section closed the D4 write-up with "rasterising off the UI thread was
/// **not** answered, because D4 does not do it. One page at 38 ms does not need
/// a thread; a viewer that wants twenty in a row (D5) will have to ask this
/// again." This is D5 asking it, and the answer is a thread — but a much
/// smaller one than the question implies, because of what it is *not* allowed
/// to do.
///
/// **It never touches the UI.** It renders a PNG and stops. There is no signal
/// write, no `run_on_main_thread` hop, no callback, and therefore no way for it
/// to be observed half-done or to write into a screen that has been left. The
/// viewer's page turn calls [`ensure_page`] synchronously as it always would;
/// if this thread got there first that call is one `stat`, and if it did not,
/// the page is drawn on the main thread exactly as it would have been with no
/// prefetch at all. **Correctness never depends on the thread having run** —
/// which is why it can be this casual about being cancelled, outliving the
/// screen, or losing a race.
///
/// **The race it can lose is already safe.** Two renders of the same page write
/// `page-N.png.part` and rename it over `page-N.png`, and `rename(2)` within a
/// directory is atomic — so a reader either sees the old file or the new one,
/// never a half-written one, and since both renders produce the same bytes it
/// does not matter which wins. That property was built into [`cache_page`] by
/// D4 for a different reason (an interrupted import), and it is what makes this
/// card's threading a nine-line function instead of a lock.
///
/// The measurement that says a thread is worth having at all: D4 timed the
/// whole pipeline at **37.3 / 38.5 ms a page on the moto g stylus 5G**. A page
/// turn that has to rasterise is therefore two and a half dropped frames, which
/// is not an ANR and not even close — but it is a hitch on *every* turn through
/// a document nobody has read yet, which is exactly the state a chart is in the
/// first time it is opened on stage. Prefetching the neighbour moves that cost
/// into the seconds the reader spends looking at the page they asked for.
pub fn prefetch(directory: &Path, pages: &[u32]) {
    use std::sync::atomic::Ordering;

    let wanted: Vec<u32> = pages
        .iter()
        .copied()
        .filter(|page| cached_page(directory, *page).is_none())
        .collect();
    if wanted.is_empty() {
        return;
    }

    // Claim a slot, or give up entirely. Giving up costs nothing — the page is
    // still drawn on demand by `ensure_page`, a frame later than it would have
    // been. There is deliberately no queue behind this: the pages a *stale*
    // worker was asked for are the neighbours of a page the reader has already
    // moved on from.
    if PREFETCH_IN_FLIGHT
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
            (n < MAX_PREFETCH_THREADS).then_some(n + 1)
        })
        .is_err()
    {
        return;
    }

    let directory = directory.to_path_buf();
    let spawned = std::thread::Builder::new()
        .name("sla-pdf-prefetch".into())
        // Not the default 2 MB: `draw` allocates a 1080 x 8192 x 4 pixmap on
        // the *heap*, so the stack only carries hayro's interpreter recursion
        // over a content stream. 1 MB is what the platform with the smaller
        // default (Android, 1 MB for a Java-created thread) already lives with.
        .stack_size(1024 * 1024)
        .spawn(move || {
            for page in wanted {
                ensure_page(&directory, page);
            }
            PREFETCH_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        });

    // A machine that cannot spawn a thread is a machine under real pressure,
    // and the honest response is to want nothing from it. The slot has to come
    // back or every later prefetch is refused for the life of the process.
    if spawned.is_err() {
        PREFETCH_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Draw one page of `data` and put the PNG in `directory`.
///
/// Takes the bytes rather than reading them, because the one caller that
/// matters — [`crate::pdf::import`] — is already holding the whole file in an
/// `Arc` it parsed the page count out of, and reading it back off the disk it
/// has just been written to would be a second copy of a 32 MB chart for
/// nothing.
///
/// Overwrites an existing page. That is not the common path — [`ensure_page`]
/// checks first — but it is the right behaviour for the one caller that does
/// not check: an import writing page one over whatever a previous life of the
/// same attachment id left behind.
pub fn cache_page(
    directory: &Path,
    data: Arc<Vec<u8>>,
    page: u32,
) -> Result<PathBuf, RenderError> {
    let png = draw(data, page)?;

    // The rename dance the module header argues for. `.part` sits beside the
    // real name so the rename never crosses a filesystem — a temporary
    // directory would, and `rename(2)` across mount points fails with EXDEV.
    let final_path = directory.join(page_file(page));
    let part_path = directory.join(format!("{}.part", page_file(page)));
    std::fs::write(&part_path, &png).map_err(|e| RenderError::Write(e.to_string()))?;
    if let Err(e) = std::fs::rename(&part_path, &final_path) {
        // Leaving a `.part` behind is survivable and being wrong about whether
        // the page landed is not, so the failed attempt is swept up here rather
        // than left for the next open to puzzle over.
        let _ = std::fs::remove_file(&part_path);
        return Err(RenderError::Write(e.to_string()));
    }
    Ok(final_path)
}

/// Rasterise one page to a 16-grey palette PNG, in memory.
///
/// ## Why the render is wrapped in `catch_unwind`
///
/// `docs/PDF.md` is explicit that hayro "is 0.x software with a list of known
/// gaps, and it will eventually draw something wrong". Drawing something wrong
/// is a picture nobody likes; *panicking* is the whole app gone, and the file
/// that did it is a file the user picked and will pick again. There is no
/// upside to letting a malformed content stream take the process with it when
/// the alternative is a chart with no preview.
///
/// This is safe to do here in a way it is not everywhere: hayro forbids unsafe
/// code at the crate level, so an unwind cannot leave it in a torn state; and
/// everything the render touches — the `Pdf`, the `RenderCache`, the `Pixmap` —
/// is created inside the closure and dropped by the unwind, so there is no
/// shared invariant left broken. The panic message still reaches stderr (and
/// logcat) through the default hook, which is where a bug report would want it.
fn draw(data: Arc<Vec<u8>>, page: u32) -> Result<Vec<u8>, RenderError> {
    if page == 0 {
        return Err(RenderError::NoSuchPage);
    }
    let index = page as usize - 1;

    let drawn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let pdf = Pdf::new(data).map_err(|_| RenderError::Unreadable)?;
        let pages = pdf.pages();
        let page = pages.get(index).ok_or(RenderError::NoSuchPage)?;

        let (width_pt, height_pt) = page.render_dimensions();
        let (pixels, width, height) = scaled(width_pt, height_pt)?;

        let pixmap = render(
            page,
            &RenderCache::new(),
            // The default resolver, which with hayro's `embed-fonts` on is what
            // substitutes real Helvetica and Courier for a chart that named the
            // base-14 fonts without embedding them. `docs/PDF.md` §5 checked
            // that case by eye and found it correct; the crate's own example
            // overrides this only to reach for CJK faces, which a chord chart
            // is not.
            &InterpreterSettings::default(),
            &RenderSettings {
                x_scale: pixels,
                y_scale: pixels,
                width: Some(width as u16),
                height: Some(height as u16),
                // Paper, not transparency. A PDF page has no background of its
                // own — it is defined as whatever the medium is — and the
                // default here is `TRANSPARENT`, which would quantise to
                // whatever the unpainted RGBA happened to be and produce a
                // black page in the palette encoder below.
                bg_color: WHITE,
            },
        );

        Ok(palette_png(pixmap, width, height))
    }));

    match drawn {
        Ok(Ok(Ok(png))) => Ok(png),
        // The render came back but the encoder did not like it. `png`'s
        // encoding errors here can only be a parameter this module chose
        // wrongly, so there is nothing to tell the user apart from "no picture".
        Ok(Ok(Err(_))) => Err(RenderError::NotDrawn),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(RenderError::NotDrawn),
    }
}

/// The scale factor and pixel size for a page of `width_pt` × `height_pt`.
///
/// Uniform, so the page keeps its shape, and clamped at both ends. The upper
/// clamp is [`MAX_PAGE_HEIGHT`] and the reason is in that constant. The lower
/// one is that a page must be at least one pixel in each direction: a
/// `MediaBox` of zero — which a broken exporter does produce — would otherwise
/// ask `Pixmap` for a zero-sized buffer and get an image nothing can decode.
fn scaled(width_pt: f32, height_pt: f32) -> Result<(f32, u32, u32), RenderError> {
    if !width_pt.is_finite() || !height_pt.is_finite() || width_pt <= 0.0 || height_pt <= 0.0 {
        return Err(RenderError::NotDrawn);
    }
    let by_width = PAGE_WIDTH as f32 / width_pt;
    let by_height = MAX_PAGE_HEIGHT as f32 / height_pt;
    let scale = by_width.min(by_height);

    let width = ((width_pt * scale).round() as u32).clamp(1, PAGE_WIDTH);
    let height = ((height_pt * scale).round() as u32).clamp(1, MAX_PAGE_HEIGHT);
    Ok((scale, width, height))
}

/// A rendered page as a 4-bit indexed PNG over sixteen evenly spaced greys.
///
/// Two pixels to a byte, each row padded out to a whole one — that padding is
/// the PNG spec's, not a choice, and forgetting it produces an image that
/// shears one pixel further left on every row, which is a very memorable way to
/// spend an afternoon.
///
/// The luma weights are Rec. 601's 0.299 / 0.587 / 0.114 in integer form
/// (77 / 150 / 29 over 256), the same ones `image`'s `into_luma8` uses, so the
/// output is comparable with the Luma8 column `docs/PDF.md` measured.
fn palette_png(
    pixmap: hayro::vello_cpu::Pixmap,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, png::EncodingError> {
    let pixels = pixmap.take_unpremultiplied();

    let row_bytes = width.div_ceil(2) as usize;
    let mut packed = vec![0u8; row_bytes * height as usize];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let px = pixels[y * width as usize + x];
            let luma = (px.r as u32 * 77 + px.g as u32 * 150 + px.b as u32 * 29) >> 8;
            // Nearest of the sixteen levels rather than a truncating divide:
            // `luma / 17` would map 255 to 15 and 254 to 14, so a page of
            // not-quite-white paper would come back a shade darker than the
            // page beside it for no reason a reader could see a cause for.
            let index = ((luma * (PALETTE_LEVELS - 1) + 127) / 255) as u8;
            let byte = &mut packed[y * row_bytes + x / 2];
            // High nibble first: PNG packs sub-byte samples big-endian within
            // the byte, so pixel 0 of a pair is the top four bits.
            if x % 2 == 0 {
                *byte |= index << 4;
            } else {
                *byte |= index;
            }
        }
    }

    // 0, 17, 34 … 255. Evenly spaced so that black and white are both exact:
    // ink that was #000 stays #000 and paper that was #FFF stays #FFF, and
    // every step in between is 17/255 wide.
    let mut palette = Vec::with_capacity(PALETTE_LEVELS as usize * 3);
    for level in 0..PALETTE_LEVELS {
        let value = (level * 255 / (PALETTE_LEVELS - 1)) as u8;
        palette.extend_from_slice(&[value, value, value]);
    }

    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Four);
        encoder.set_palette(palette);
        // Measured on 2026-08-28 rather than guessed, over a generated lead
        // sheet and the 22-page fig2dev manual, whole pipeline, ms per page and
        // bytes per page:
        //
        //     Fast       9.7 / 10.2 ms      49.9 / 69.2 KB
        //     Balanced  14.2 / 15.1 ms      41.7 / 59.4 KB
        //     High      21.4 / 24.9 ms      41.0 / 58.3 KB
        //
        // `High` is the one to reject outright: half as much time again to save
        // 1.6 % of a file. Between the other two it is 4.5 ms of a phone's main
        // thread against 18 % of the cache, and the cache is the thing this
        // format was chosen to make small — `docs/PDF.md` priced 300 songs on
        // it — so `Balanced` it is, and it is what lands on that document's
        // 44 KiB-a-page figure.
        encoder.set_compression(png::Compression::Balanced);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&packed)?;
        writer.finish()?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::scratch;
    use crate::pdf::test_support::inked_pdf;

    /// An empty directory of this run's own, named the way `db::scratch` names
    /// its libraries so that a stray one is traceable to the test that left it.
    /// Made rather than merely named, because an attachment's directory is
    /// always made for it by `Repo::create_attachment` before anything writes
    /// into it, and a test that skipped that step would be testing a state the
    /// app cannot be in.
    fn empty(name: &str) -> PathBuf {
        let dir = scratch(name).path().to_path_buf();
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    /// A directory with a `chart.pdf` in it, the way an attachment's directory
    /// looks the moment `crate::pdf::import` has finished with it.
    fn charted(name: &str) -> PathBuf {
        let dir = empty(name);
        std::fs::write(dir.join(super::super::PDF_FILE), inked_pdf(1)).expect("the chart");
        dir
    }

    /// Decode a PNG back to `(width, height, bit depth, colour type, pixels as
    /// 8-bit grey)`. The `png` crate expands 4-bit indexed to RGB for us when
    /// asked, which is what a reader — Rinch's image loader included — will do.
    fn decoded(path: &Path) -> (u32, u32, png::BitDepth, png::ColorType, Vec<u8>) {
        let file = std::fs::File::open(path).expect("a png");
        let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().expect("a png header");
        let depth = reader.info().bit_depth;
        let colour = reader.info().color_type;
        let mut buffer = vec![0; reader.output_buffer_size().expect("a bounded png")];
        let frame = reader.next_frame(&mut buffer).expect("a png body");
        let samples = frame.color_type.samples();
        let grey = buffer[..frame.buffer_size()]
            .chunks(samples)
            .map(|px| px[0])
            .collect();
        (frame.width, frame.height, depth, colour, grey)
    }

    // ── what comes out ──────────────────────────────────────────────────────

    #[test]
    fn a_page_is_drawn_at_the_cache_width_and_keeps_its_shape() {
        let dir = charted("pdf_pages_shape");
        let path = ensure_page(&dir, 1).expect("page one");

        let (width, height, depth, colour, _) = decoded(&path);
        assert_eq!(width, PAGE_WIDTH);
        // 612 x 792 at 1080 wide is 1397.6, and the round is deliberate.
        assert_eq!(height, 1398);
        assert_eq!(depth, png::BitDepth::Four, "a nibble a pixel, or it is not a palette");
        assert_eq!(colour, png::ColorType::Indexed);
    }

    /// The check that separates "hayro drew the page" from "hayro handed back a
    /// blank the same size as the page", which is the failure mode
    /// `docs/PDF.md` caught PDFium in.
    #[test]
    fn the_ink_in_the_pdf_is_the_ink_in_the_png() {
        let dir = charted("pdf_pages_ink");
        let path = ensure_page(&dir, 1).expect("page one");
        let (width, height, _, _, grey) = decoded(&path);

        let at = |x: u32, y: u32| grey[(y * width + x) as usize];
        assert_eq!(at(width / 4, height / 2), 0, "the left half is filled black");
        assert_eq!(at(width * 3 / 4, height / 2), 255, "the right half is bare paper");
    }

    /// Sixteen levels means every pixel is a multiple of 17 and there is no
    /// seventeenth value hiding in the file.
    #[test]
    fn every_pixel_is_one_of_sixteen_greys() {
        let dir = charted("pdf_pages_levels");
        let path = ensure_page(&dir, 1).expect("page one");
        let (_, _, _, _, grey) = decoded(&path);

        let mut levels: Vec<u8> = grey.clone();
        levels.sort_unstable();
        levels.dedup();
        assert!(levels.len() <= PALETTE_LEVELS as usize, "found {levels:?}");
        assert!(
            levels.iter().all(|v| v % 17 == 0),
            "a value that is not a multiple of 17 is not on the palette: {levels:?}"
        );
    }

    /// The card sizes its `<img>` from these two numbers on every redraw, so
    /// they had better be the file's and not a decoder's guess.
    #[test]
    fn a_pages_size_is_read_off_the_front_of_the_file() {
        let dir = charted("pdf_pages_size");
        let path = ensure_page(&dir, 1).expect("page one");
        assert_eq!(page_pixels(&path), Some((PAGE_WIDTH, 1398)));

        let (width, height, _, _, _) = decoded(&path);
        assert_eq!(
            page_pixels(&path),
            Some((width, height)),
            "the header and the decoder have to agree, or the card is drawing a lie"
        );
    }

    #[test]
    fn something_that_is_not_a_png_has_no_size() {
        let dir = empty("pdf_pages_size_junk");
        let short = dir.join("short.png");
        std::fs::write(&short, b"\x89PNG\r\n\x1a\n").expect("a truncated file");
        assert_eq!(page_pixels(&short), None, "eight bytes is a signature, not a page");

        let jpeg = dir.join("actually.png");
        std::fs::write(&jpeg, [0xff, 0xd8, 0xff, 0xe0, 0, 16, b'J', b'F', b'I', b'F', 0,
                               1, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0])
            .expect("a jpeg wearing the wrong name");
        assert_eq!(
            page_pixels(&jpeg),
            None,
            "without the signature check this would read two numbers out of the middle of a JPEG"
        );

        assert_eq!(page_pixels(&dir.join("not-there.png")), None);
    }

    // ── the cache is a cache ────────────────────────────────────────────────

    #[test]
    fn nothing_is_cached_until_something_asks() {
        let dir = charted("pdf_pages_cold");
        assert_eq!(cached_page(&dir, 1), None, "importing is not this module's job");
        ensure_page(&dir, 1).expect("page one");
        assert!(cached_page(&dir, 1).is_some(), "and now it is there");
    }

    /// The reuse this card is measured on. A sentinel in place of the real PNG
    /// proves it by surviving: if `ensure_page` re-rendered, the sentinel would
    /// be gone.
    #[test]
    fn a_cached_page_is_handed_back_rather_than_drawn_again() {
        let dir = charted("pdf_pages_reuse");
        let path = ensure_page(&dir, 1).expect("page one");
        std::fs::write(&path, b"not a png, and not to be touched").expect("the sentinel");

        let again = ensure_page(&dir, 1).expect("page one again");
        assert_eq!(again, path);
        assert_eq!(
            std::fs::read(&path).expect("the sentinel"),
            b"not a png, and not to be touched",
            "a second ask re-rendered a page that was already on disk"
        );
    }

    #[test]
    fn a_page_that_is_not_there_is_not_drawn() {
        let dir = charted("pdf_pages_past_the_end");
        assert_eq!(ensure_page(&dir, 2), None, "the chart has one page");
        assert_eq!(ensure_page(&dir, 0), None, "and pages are one-based");
        assert!(!dir.join(page_file(2)).exists(), "and nothing was written for it");
    }

    #[test]
    fn a_chart_with_no_file_behind_it_draws_nothing() {
        let dir = empty("pdf_pages_no_file");
        assert_eq!(ensure_page(&dir, 1), None);
    }

    #[test]
    fn a_pdf_hayro_cannot_read_is_a_chart_with_no_pages_rather_than_a_crash() {
        let dir = empty("pdf_pages_unreadable");
        std::fs::write(dir.join(super::super::PDF_FILE), b"%PDF-1.4\nnothing here\n")
            .expect("the chart");

        assert_eq!(
            cache_page(&dir, Arc::new(b"%PDF-1.4\nnothing here\n".to_vec()), 1),
            Err(RenderError::Unreadable)
        );
        assert_eq!(ensure_page(&dir, 1), None);
        assert!(!dir.join(page_file(1)).exists(), "and no half a file either");
    }

    // ── half a cache ────────────────────────────────────────────────────────

    /// The atomic-rename claim, from both ends: a finished page leaves no
    /// `.part` behind, and a `.part` left behind by something that died is not
    /// mistaken for a page.
    #[test]
    fn a_page_arrives_whole_or_not_at_all() {
        let dir = charted("pdf_pages_atomic");
        let orphan = dir.join(format!("{}.part", page_file(1)));
        std::fs::write(&orphan, b"half a png from a process that was killed").expect("the orphan");

        assert_eq!(
            cached_page(&dir, 1),
            None,
            "a `.part` is not a page and must never be handed to an image decoder"
        );

        let path = ensure_page(&dir, 1).expect("page one");
        assert!(!orphan.exists(), "the interrupted attempt was overwritten and renamed away");
        let (width, _, _, _, _) = decoded(&path);
        assert_eq!(width, PAGE_WIDTH, "and what landed is a whole PNG");
    }

    /// Half a cache is the normal state, not a repair job: page three existing
    /// while pages one and two do not is a chart somebody scrolled to, and
    /// asking for page one after that must simply draw page one.
    #[test]
    fn pages_are_independent_of_one_another() {
        let dir = charted("pdf_pages_independent");
        std::fs::write(dir.join(page_file(7)), b"a page from another life").expect("page seven");

        assert!(cached_page(&dir, 1).is_none());
        assert!(ensure_page(&dir, 1).is_some(), "a stranger in the directory blocks nothing");
        assert_eq!(
            std::fs::read(dir.join(page_file(7))).expect("page seven"),
            b"a page from another life",
            "and nothing else in the directory was touched"
        );
    }

    // ── the arithmetic that keeps a hostile page from being a crash ─────────

    #[test]
    fn a_very_tall_page_is_scaled_down_rather_than_overflowing_the_pixmap() {
        // 2 pt wide, 4000 pt tall. At 1080 px wide this would be 2,160,000 px
        // tall, which is 33 times what a `u16` can hold.
        let (_, width, height) = scaled(2.0, 4000.0).expect("a strip is still a page");
        assert!(height <= MAX_PAGE_HEIGHT, "{height} would not fit in a Pixmap");
        assert!(width >= 1, "and it is still an image");
        assert!(
            (width as f32 / height as f32 - 2.0 / 4000.0).abs() < 0.001,
            "and it kept its shape: {width}x{height}"
        );
    }

    #[test]
    fn a_page_with_no_size_is_refused_rather_than_rounded_up_to_one_pixel() {
        assert_eq!(scaled(0.0, 792.0), Err(RenderError::NotDrawn));
        assert_eq!(scaled(612.0, 0.0), Err(RenderError::NotDrawn));
        assert_eq!(scaled(f32::NAN, 792.0), Err(RenderError::NotDrawn));
        assert_eq!(scaled(f32::INFINITY, 792.0), Err(RenderError::NotDrawn));
    }

    #[test]
    fn an_ordinary_page_is_scaled_by_width_and_not_by_height() {
        let (scale, width, height) = scaled(612.0, 792.0).expect("US Letter");
        assert_eq!(width, PAGE_WIDTH);
        assert_eq!(height, 1398);
        assert!((scale - 1080.0 / 612.0).abs() < 0.0001);
    }

    // -- What is drawn ahead of the reader (D5) -----------------------------

    /// Run one prefetch test at a time, with no worker left over from the last.
    ///
    /// `PREFETCH_IN_FLIGHT` is a *process-wide* counter and `cargo test` runs a
    /// module's tests on several threads at once, so without this the second
    /// prefetch test to start is refused a worker slot by the first — and
    /// refusal is not a failure in the app (the page is drawn on demand
    /// instead), so it would show up here as a page that mysteriously never
    /// appears. Draining the counter as well as taking the lock matters because
    /// the slot is released *inside* the worker, a moment after the file lands.
    ///
    /// The single slot is not a limitation worth designing around in the app:
    /// there is one viewer, on one screen, at a time.
    fn alone() -> std::sync::MutexGuard<'static, ()> {
        static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let mut waited = 0;
        while PREFETCH_IN_FLIGHT.load(std::sync::atomic::Ordering::SeqCst) > 0 && waited < 200 {
            std::thread::sleep(std::time::Duration::from_millis(25));
            waited += 1;
        }
        guard
    }

    /// Next first. A reader's next move is forward far more often than back,
    /// and with one worker the order is the priority.
    #[test]
    fn the_page_after_this_one_is_drawn_before_the_page_before_it() {
        assert_eq!(neighbours(3, 6), vec![4, 2]);
    }

    /// Neither end reaches outside the document. Page 0 does not exist —
    /// `page_file` is one-based and `draw` rejects a zero — and a page past the
    /// last is a render that would fail and cost 38 ms doing it.
    #[test]
    fn neither_cover_is_prefetched_past() {
        assert_eq!(neighbours(1, 6), vec![2], "nothing before the front");
        assert_eq!(neighbours(6, 6), vec![5], "nothing after the back");
        assert!(neighbours(1, 1).is_empty(), "a one-page chart has no neighbours");
    }

    /// A count of zero is what a PDF hayro could not parse looks like by the
    /// time the viewer has clamped it. Nothing to draw ahead, and no panic
    /// working that out.
    #[test]
    fn a_document_of_no_pages_asks_for_nothing() {
        assert!(neighbours(1, 0).is_empty());
    }

    /// The end-to-end promise of the prefetch, made without a timer: after it
    /// has run, the page it was asked for is on disk and `ensure_page` finds it
    /// with a `stat` rather than a render.
    ///
    /// Joined by polling rather than by a handle, because `prefetch` gives none
    /// back on purpose — the viewer never waits on it, and a version that
    /// returned something to wait on would be a version somebody could wait on.
    #[test]
    fn a_prefetched_page_is_on_disk_for_the_next_reader_to_find() {
        let _one = alone();
        let dir = empty("prefetched-page");
        std::fs::write(dir.join(super::super::PDF_FILE), inked_pdf(3)).expect("the chart");

        assert!(cached_page(&dir, 2).is_none(), "nothing is drawn yet");
        prefetch(&dir, &neighbours(1, 3));

        let mut waited = 0;
        while cached_page(&dir, 2).is_none() && waited < 100 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            waited += 1;
        }
        assert!(
            cached_page(&dir, 2).is_some(),
            "page 2 was never drawn after {} ms",
            waited * 50
        );
        // And the page it produced is a real one, not an empty file left by a
        // worker that fell over: same check the synchronous path gets.
        let (width, height, _, _, _) = decoded(&cached_page(&dir, 2).expect("page 2"));
        assert_eq!((width, height), (PAGE_WIDTH, 1398));
    }

    /// Asking for a page that is already cached spawns nothing and, more to the
    /// point, does not redraw it. `prefetch` filters before it claims a worker
    /// slot, so a reader paging back and forth over two drawn pages costs
    /// nothing at all.
    #[test]
    fn a_page_already_drawn_is_not_drawn_again() {
        let _one = alone();
        let dir = charted("prefetch-already-drawn");
        let path = ensure_page(&dir, 1).expect("page one");
        let first = std::fs::metadata(&path).expect("page one").modified().ok();

        prefetch(&dir, &[1]);
        std::thread::sleep(std::time::Duration::from_millis(150));

        let again = std::fs::metadata(&path).expect("page one").modified().ok();
        assert_eq!(first, again, "the cached page was rewritten");
    }

    /// The worker cannot take the process with it. A directory with no
    /// `chart.pdf` is exactly what an attachment looks like between
    /// `SongsStore::attach` minting the row and the import writing the bytes,
    /// and a prefetch that landed in that window has to come back with nothing
    /// rather than panic on a thread nobody is joining.
    #[test]
    fn a_prefetch_of_a_chart_that_is_not_there_yet_is_survivable() {
        let _one = alone();
        let dir = empty("prefetch-no-chart");
        prefetch(&dir, &[1, 2]);
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(cached_page(&dir, 1).is_none());

        // And the worker slot came back, so the *next* prefetch is not refused
        // for the rest of the process — the failure mode this would have if the
        // counter were only decremented on the happy path.
        let charted = charted("prefetch-after-a-miss");
        prefetch(&charted, &[1]);
        let mut waited = 0;
        while cached_page(&charted, 1).is_none() && waited < 100 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            waited += 1;
        }
        assert!(cached_page(&charted, 1).is_some(), "the slot was never released");
    }
}
