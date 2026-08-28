//! Turning a saved `page.html` back into something the app can draw — card E5.
//!
//! ## What "render" turned out to mean, and the four measurements behind it
//!
//! The card is one line: *render a captured page in the card and the viewer*.
//! `sanitise`'s own header had already assumed the highest-fidelity reading —
//! "the captured page is opened later by E5 in the app's own renderer" — and
//! Rinch is a browser-grade stack, Stylo for the cascade and Parley for the
//! text, so the obvious thing to try was handing it the saved file whole.
//!
//! It can be handed markup. `NodeHandle::set_inner_html`
//! (`rinch-core/src/dom/mod.rs`) parses a string and builds real DOM nodes
//! under any element the app already owns, and those nodes get real Stylo
//! styling and real Taffy layout. So the mechanism exists, it works on a
//! subtree, and it is what this file feeds. **What it cannot be handed is the
//! saved file**, and the reasons were measured rather than assumed:
//!
//! 1. **Rinch's HTML parser is a hand-rolled scanner**, not html5ever
//!    (`rinch-dom/src/html_parser.rs`). Run over the three sites `docs/
//!    CAPTURE.md` says the engine actually captures — hymnal.net, CifraClub,
//!    guitaretab — the first text node it produces on every one of them is the
//!    string `!doctype html>`, because a `<!…>` declaration is not a tag it
//!    knows and the leftovers fall through to the text branch. It decodes six
//!    named entities and no numeric ones, and it has no implicit end tags, so
//!    the unclosed `<p>` and `<li>` that html5ever exists to recover from nest
//!    instead of closing.
//! 2. **A `<style>` block in injected markup is loaded into the *document's*
//!    stylist.** `append_child` calls `maybe_load_style_css`
//!    (`rinch-dom/src/style_resolution/mod.rs`), which hands the CSS to
//!    `load_stylo_css` and re-resolves every node in the tree. There is one
//!    stylist per document and one document per window, so a stranger's
//!    `p { margin: 0 }` would restyle SetListArray, not just the attachment.
//! 3. **Most of the page's CSS is gone anyway, on purpose.** E1 drops every
//!    `<link rel=stylesheet>` because a stylesheet is a request that fails on a
//!    train. Measured on the capture this file was built against: hymnal.net's
//!    saved page contains **zero** `<style>` blocks. Its chord display is
//!    `<div class="chord-text">` boxes that the site's external sheet makes
//!    `inline-block` with the chord `block` above the syllable — and the class
//!    that hides the whole scaffold is `hidden`. Without the sheet there is no
//!    fidelity left to preserve: the "high-fidelity" render of that page is a
//!    column of one-syllable lines that was never meant to be visible.
//! 4. **Rinch's UA stylesheet is deliberately small** (`rinch-dom/src/dom_impl/
//!    mod.rs`): it sets `display` for the usual tags, bold for `<strong>`,
//!    italic for `<em>`, list indentation — and nothing else. No `<pre>`
//!    monospace, no `white-space: pre`, no heading sizes. A chord chart handed
//!    over raw comes back proportional and word-wrapped, which is the one thing
//!    a chart cannot survive.
//!
//! So the decision is: **the app rebuilds the page and Rinch lays it out.**
//! `page.html` is parsed with html5ever — the parser this crate already carries
//! and the one that is spec-correct about the markup chord sites really serve —
//! walked, and re-emitted as a small, well-formed, self-contained fragment with
//! the app's own typography inlined on it. That fragment goes to
//! `set_inner_html`, and from there it is Stylo and Parley and Taffy doing the
//! work: real inline flow, real `<pre>`, real tables, real images, real line
//! breaking. The toy parser is then only ever fed markup this file generated,
//! which is the one input it is reliable on.
//!
//! What that buys over the extracted text the viewer showed before E5:
//! headings that look like headings, the chart's images, `<pre>` blocks with
//! their columns intact, emphasis, tables, and links that are visibly links.
//! What it does not buy is the site's own visual identity, and nothing could,
//! because E1 did not save it.
//!
//! ### Reader mode is not what this is
//!
//! Narrowing a page to its chart is **E3**, it is implemented in `reader.rs`,
//! and it happens at capture time. This file renders whatever was saved. A
//! full-page capture therefore still opens on the site's navigation — the
//! "found and not fixed" note E2 left — and that is E3's to fix by capturing
//! less, not this file's to fix by drawing less.
//!
//! ## The rules, in one table
//!
//! | Input | What comes out |
//! | --- | --- |
//! | `head`, `style`, `script`, `title`, `meta`, `link`, `template`, `iframe`, `object`, `embed`, `canvas`, `svg`, form controls | dropped, subtree and all |
//! | a tag in [`STYLED`] | itself, with the app's inline style |
//! | any other element | unwrapped — its children are emitted in its place |
//! | any element with nothing visible under it | dropped, wrapper and all |
//! | `<img src="assets/000.png">` | an absolute path and both axes in pixels, or its `alt` if the file is gone |
//! | a text node | its characters, with `&`, `<` and `>` escaped |
//! | the site's own `style=` attribute | dropped, except `display: none` |
//!
//! The first row is belt and braces: `sanitise` already removed all of it, and
//! it is checked again here for the reason `sanitise`'s own header gives for
//! being strict — a file written to disk outlives the assumption that this
//! month's sanitiser ran over it. A page captured by an older build, or one
//! edited by hand in the library directory, arrives here too.
//!
//! The "unwrap the rest" row is what keeps the fragment small and the styling
//! predictable. A chord page is a few hundred `<div>`s of layout scaffolding
//! that meant something only in the sheet that is gone; keeping them would
//! produce a few hundred empty blocks. Keeping their *children* loses nothing.
//!
//! The row after it is the same argument applied to the tags that *are* on the
//! list, and it is measured rather than tidy — see [`emit`]. On the CifraClub
//! capture, emitting every styled element produced **809 elements, most of them
//! empty `<div>`s**; pruning the ones with nothing visible under them takes it
//! to 495 and is what makes [`CARD_ELEMENTS`] mean "how much of the page"
//! rather than "how deep into its wrappers".
//!
//! ## Why the site's inline `style` goes but `display: none` stays
//!
//! An inline style is per-node, so unlike a `<style>` block it cannot escape
//! into the app. It is dropped anyway, because on the app's paper a stranger's
//! `color: #fff` is an invisible chart and a stranger's `font-family` is a
//! chart in a face that is not on the device. The app supplies the typography.
//!
//! `display: none` is the exception and it is not a style question: it is the
//! site saying *this content is not on the page*. A site that ships two copies
//! of a chart and hides one — which is exactly what a "show chords / hide
//! chords" toggle is — would otherwise render both.
//!
//! ## Sizes are in `em`, so one fragment serves both places
//!
//! Every length this file emits is in `em` except an image's two axes. That is
//! what lets the same markup be the preview in a card at 12.5 px and the whole
//! page in the viewer at 14.5 px, and it is what makes the viewer's `A+` / `A−`
//! work at all: zoom is a `font-size` on the host and the document follows it.
//! `song_detail` and `attachment_viewer` therefore draw the same object at two
//! sizes rather than two different objects, which is the thing the card asked
//! for.
//!
//! Images are the exception because of **K28**: `width: 100%` on an `<img>`
//! lays the node out at the bitmap's *unscaled* height, because Rinch's Taffy
//! measure closure only derives the missing axis when the other arrives as a
//! `known_dimension` and a percentage does not survive the content-sizing pass
//! as one. `song_detail::page_image` states both axes in pixels for a PDF page
//! for exactly this reason and this does the same, from [`image_pixels`]. The
//! consequence is that a zoom step has to re-render the fragment rather than
//! just restate a font size — see [`Sizing`].

use std::path::{Path, PathBuf};

use markup5ever_rcdom::Handle;

use super::dom;
use super::PAGE_FILE;

/// How many elements the card's preview is allowed to build.
///
/// The card is a box a few lines tall on a screen that also lists every other
/// chart on the song, and `song_detail`'s performance rule is about exactly
/// this: a screen must not do a page's worth of work to draw a thumbnail. A
/// hundred and forty elements is comfortably more than fills the preview box on
/// every page measured, and it means a 176 KB CifraClub capture costs the card
/// the first fraction of itself rather than all 858 of its elements.
pub const CARD_ELEMENTS: usize = 140;

/// How many elements the full-screen viewer is allowed to build.
///
/// Six thousand, against 858 for the largest page `docs/CAPTURE.md` measured.
/// It is not a performance budget — it is a stop on a page that turns out to be
/// pathological, so that a bad capture is a truncated chart rather than a phone
/// that stops answering. `Page::truncated` says when it fired, and the screens
/// say so out loud rather than silently ending mid-song.
pub const VIEWER_ELEMENTS: usize = 6000;

/// How deep the emitted fragment is allowed to nest.
///
/// The three real captures measured 13, 17 and 19 levels. Thirty-two leaves
/// room and closes a hole that is not this app's: Rinch's parser recurses once
/// per level with no depth guard of its own, so a page nested a few thousand
/// deep would take the process on a stack overflow rather than fail to draw.
/// Past the limit an element is unwrapped rather than dropped, so the deepest
/// content is flattened and not lost.
const MAX_DEPTH: usize = 32;

/// Widest an image is drawn, as a fraction of the column it is drawn in.
///
/// A captured page's images are a chord box, a scan, a site logo. None of them
/// is worth more than the column, and several are much wider than it — the
/// musicnotes captures in `docs/CAPTURE.md` are sheet-music scans over a
/// thousand pixels across. Bigger than the column would overflow the card, and
/// there is nothing to gain from it in either place.
const IMAGE_COLUMN_FRACTION: f32 = 1.0;

/// The host's type size and the width it has to fit in.
///
/// Both are needed and neither can be a percentage. `base_px` is what the `em`
/// lengths in the fragment resolve against, and it is also what an image's
/// pixel size is capped against, so a zoom step changes it and the fragment is
/// built again. That re-render is why `attachment_viewer` already puts its
/// scrolling box inside a `for` keyed on the zoom: the node is rebuilt on every
/// step regardless, for a scroll-offset fault that has nothing to do with this,
/// and rebuilding it re-runs this render for free.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizing {
    /// The font size on the host element, in CSS pixels.
    pub base_px: f32,
    /// How wide the host is, in CSS pixels.
    pub column_px: u32,
}

/// A rendered page, ready for `NodeHandle::set_inner_html`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    /// The fragment. Well-formed, self-contained, and free of anything that
    /// could reach the network or the stylist.
    pub markup: String,
    /// Elements emitted. Zero means there was nothing on the page to draw.
    pub elements: usize,
    /// Images whose file was found on disk and drawn.
    pub images: usize,
    /// Images the page referenced whose file was not there — a capture whose
    /// `assets/` was deleted underneath it, or an image E1 recorded as
    /// `Missed`. Each becomes its `alt` text.
    pub broken_images: usize,
    /// The element budget ran out before the page did.
    pub truncated: bool,
}

impl Page {
    /// Nothing came out. The screens show their own sentence rather than an
    /// empty box, because an empty box looks like a chart that is still
    /// loading and this one never will be.
    pub fn is_empty(&self) -> bool {
        self.elements == 0 && self.markup.trim().is_empty()
    }
}

/// The saved page inside an attachment directory, if it is still there.
///
/// `None` covers both of the ways a capture stops existing: the whole directory
/// gone, and `page.html` gone from inside it. The screens do not tell those
/// apart because the reader cannot act on the difference — what they do with it
/// is fall back to the extracted text, which is in the database and survives
/// either.
pub fn read(directory: &Path) -> Option<String> {
    let bytes = std::fs::read(directory.join(PAGE_FILE)).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Rebuild a saved page as a fragment Rinch can lay out.
///
/// Pure, synchronous and file-system-reading: it stats every image the page
/// references, which is what makes a deleted `assets/` show as captions rather
/// than as blank boxes. Cost is a full html5ever parse of the saved file plus
/// one walk, so it belongs in a component body and **never in a render
/// closure** — the rule `pdf::pages` wrote for rasterising and for the same
/// reason: a closure re-runs on every redraw.
pub fn render(saved: &str, directory: &Path, sizing: Sizing, budget: usize) -> Page {
    let dom = dom::parse(saved.as_bytes());
    let root = dom::root(&dom);
    // `<body>` if the parse produced one, which it does for anything that came
    // off a web server; the whole tree if it did not, so that a fragment saved
    // by hand still draws. html5ever synthesises `<html><head><body>` around a
    // bare fragment, so in practice this always finds one — the fallback is
    // insurance rather than a path anything has been seen to take.
    let start = dom::find_element(&root, &html5ever::local_name!("body")).unwrap_or(root);

    let mut state = State {
        page: Page::default(),
        budget,
        sizing,
        directory: directory.to_path_buf(),
        visible: 0,
    };
    let mut out = String::new();
    for child in dom::children(&start) {
        emit(&child, 0, false, &mut out, &mut state);
    }
    state.page.markup = out;
    state.page
}

/// The extracted text, as a fragment — the fallback when the saved file is not
/// there to render.
///
/// It exists so that a captured page has exactly **one** drawing path in the
/// app rather than two. Before E5 the card and the viewer each drew their own
/// column of `<div>`s from `Attachment::body`; going through the same fragment
/// and the same host instead means the two cannot drift apart, the text scales
/// with the same `em` ladder the page does, and there is one place that decides
/// what a captured page looks like.
///
/// `<pre>` rather than a paragraph because the text is `dom::text_of`'s output,
/// which keeps the chart's line breaks and the runs of spaces that put a chord
/// over its syllable. That is the same argument `theme::T_CHART` makes for the
/// preview it replaces.
pub fn from_text(text: &str) -> Page {
    if text.trim().is_empty() {
        return Page::default();
    }
    let style = STYLED
        .iter()
        .find(|(t, _)| *t == "pre")
        .map(|(_, s)| *s)
        .unwrap_or_default();
    let mut markup = String::with_capacity(text.len() + 128);
    open_tag("pre", style, &mut markup);
    for ch in text.chars() {
        push_escaped(ch, &mut markup);
    }
    markup.push_str("</pre>");
    Page {
        markup,
        elements: 1,
        ..Page::default()
    }
}

struct State {
    page: Page,
    budget: usize,
    sizing: Sizing,
    directory: PathBuf,
    /// How much has been emitted that a reader could actually see: a word, a
    /// picture, a rule, a line break. See [`emit`] for what it is counted for.
    visible: usize,
}

impl State {
    fn spent(&self) -> bool {
        self.page.elements >= self.budget
    }
}

/// Emit one node and everything under it.
///
/// `preformatted` is true anywhere under a `<pre>`, and it changes two things.
/// See [`push_text`] for the whitespace, and the unwrapping rule below for the
/// reason a `<div>` inside a chart is not a `<div>` when it gets here.
fn emit(node: &Handle, depth: usize, preformatted: bool, out: &mut String, state: &mut State) {
    if state.spent() {
        state.page.truncated = true;
        return;
    }

    if let Some(text) = dom::text_content(node) {
        // A run of spaces between two blocks is not something anybody sees, and
        // saying so here is what lets the pruning below throw the block away.
        if text.chars().any(|c| !c.is_whitespace()) {
            state.visible += 1;
        }
        push_text(&text, preformatted, out);
        return;
    }

    let Some(tag) = dom::tag(node) else {
        // A comment, the doctype, the document itself. Comments carry E1's
        // "Captured by SetListArray" note and the markers `replace_with_note`
        // leaves where a script was; none of them is content. Descend anyway,
        // because the document node is how the walk reaches `<html>` when the
        // fragment had no `<body>`.
        for child in dom::children(node) {
            emit(&child, depth, preformatted, out, state);
        }
        return;
    };
    let name = tag.as_ref().to_ascii_lowercase();

    if DROPPED.contains(&name.as_str()) {
        return;
    }
    if hidden(node) {
        return;
    }
    if name == "img" {
        emit_image(node, out, state);
        return;
    }

    // Past the depth limit, and for every tag the app does not style, the
    // element goes and its children stay. See the module header: a chord page
    // is mostly scaffolding for a stylesheet that is not here.
    //
    // **Inside a `<pre>` that also goes for every block element**, and the
    // reason is a chart that was measured coming out double-spaced. CifraClub
    // writes its chart as one `<div>` per line inside the `<pre>`, each `<div>`
    // ending in a newline — which is a block box *and* a line break, so every
    // line of Wonderwall arrived with a blank line under it. That markup is not
    // wrong; the site's stylesheet makes those divs inline, and E1 threw the
    // stylesheet away. Inside preformatted text the line breaks are in the
    // characters, so anything that would add another one is unwrapped and only
    // the inline elements — the `<b>` around each chord symbol — are kept.
    let styleable =
        !preformatted || name == "pre" || INLINE_IN_PRE.contains(&name.as_str());
    let Some(style) = STYLED
        .iter()
        .find(|(t, _)| *t == name)
        .map(|(_, s)| *s)
        .filter(|_| depth < MAX_DEPTH && styleable)
    else {
        for child in dom::children(node) {
            emit(&child, depth, preformatted, out, state);
        }
        return;
    };
    let preformatted = preformatted || name == "pre";

    if VOID.contains(&name.as_str()) {
        // `<br>` and `<hr>` have no children and no end tag. Rinch's parser has
        // the same void list, so emitting one is what keeps the two agreeing
        // about where the element ends. Both are visible in their own right —
        // a rule is a line and a break is a line — so neither is pruned.
        state.page.elements += 1;
        state.visible += 1;
        open_tag(&name, style, out);
        return;
    }

    // Everything under here goes into its own buffer, and it is kept only if
    // something a reader can see came out of it.
    //
    // This is not tidiness. Measured on the CifraClub capture, emitting every
    // styled element produced **809 elements of which most were empty
    // `<div>`s** — the scaffolding a modern site hangs its layout on, whose
    // stylesheet E1 threw away. Each one is a Taffy node, a Stylo resolution
    // and a block box, all of it laying out nothing; and on the card, where
    // [`CARD_ELEMENTS`] is what stands between the preview and the whole page,
    // the entire budget was being spent on empty boxes before a word of the
    // chart was reached. Pruning them is what makes the budget mean "how much
    // of the page" rather than "how deep into its wrappers".
    //
    // A container that holds only whitespace goes with them. Whitespace between
    // two blocks was never visible; whitespace *inside* a kept element still is
    // — a `<pre>`'s indentation is the chart — and that case never reaches here,
    // because the `<pre>` itself has visible text under it.
    let before = state.visible;
    let mut inner = String::new();
    for child in dom::children(node) {
        emit(&child, depth + 1, preformatted, &mut inner, state);
    }
    if state.visible == before {
        return;
    }

    state.page.elements += 1;
    open_tag(&name, style, out);
    out.push_str(&inner);
    out.push_str("</");
    out.push_str(&name);
    out.push('>');
}

fn open_tag(name: &str, style: &str, out: &mut String) {
    out.push('<');
    out.push_str(name);
    if !style.is_empty() {
        out.push_str(" style=\"");
        push_attr(style, out);
        out.push('"');
    }
    out.push('>');
}

/// An image, as an absolute path and two axes in pixels — or as its caption.
///
/// Three things have to be true before a picture is drawn, and each failure is
/// a real state on somebody's phone:
///
/// * **The `src` has to be a local asset.** `assets::rewrite` writes
///   `assets/NNN.ext` and nothing else, and an image it could not fetch loses
///   its `src` entirely. Anything else in there — an absolute URL from a page
///   captured by an older build, a `data:` URI, a `..` — is not drawn, because
///   the promise this feature makes is that the page works with the network
///   off.
/// * **The file has to still be there.** A library directory is a directory on
///   a phone; things delete it.
/// * **Its size has to be readable**, because of K28. An image whose header
///   this app cannot read has no size to state, and Rinch would lay it out at
///   its own unscaled height.
///
/// Each of those falls back to the `alt` text, which is precisely what `alt` is
/// for and what E1 kept it for: *"an image that does not arrive keeps its `alt`
/// and loses its `src`, so the saved page shows a captioned box rather than
/// sitting there trying to reach a network that is not there."*
fn emit_image(node: &Handle, out: &mut String, state: &mut State) {
    let alt = dom::attr(node, "alt").unwrap_or_default();
    let caption = |out: &mut String, state: &mut State| {
        state.page.broken_images += 1;
        let alt = alt.trim();
        if alt.is_empty() {
            return;
        }
        state.page.elements += 1;
        state.visible += 1;
        out.push_str("<span style=\"");
        push_attr(MISSING_IMAGE_STYLE, out);
        out.push_str("\">");
        push_text(alt, false, out);
        out.push_str("</span>");
    };

    let Some(src) = dom::attr(node, "src") else {
        caption(out, state);
        return;
    };
    let Some(path) = local_asset(&state.directory, &src) else {
        caption(out, state);
        return;
    };
    let Some((width, height)) = image_pixels(&path) else {
        caption(out, state);
        return;
    };

    let (width, height) = fitted(width, height, &state.sizing);
    state.page.elements += 1;
    state.page.images += 1;
    state.visible += 1;
    out.push_str("<img src=\"");
    // A bare absolute path rather than a `file://` URL, which is the rule
    // `song_detail::page_image` wrote down for a rasterised PDF page: Rinch's
    // `FileImageLoader` strips that scheme if it is there and reads the rest as
    // a path either way, and a path that has been through URL encoding is a
    // path a library name with a space in it can break.
    push_attr(&path.to_string_lossy(), out);
    out.push_str("\" style=\"");
    push_attr(
        &format!("width: {width}px; height: {height}px; max-width: 100%; border-radius: 4px;"),
        out,
    );
    out.push_str("\">");
}

/// `assets/NNN.ext` resolved against the attachment directory, and nothing
/// else.
///
/// The check is not decoration. This path is handed to Rinch's image loader,
/// which does `std::fs::read` on whatever string it is given, so an `src` that
/// escaped the attachment directory would read a file from somewhere else on
/// the device. `write_into` makes the same check on the way in for the same
/// reason; this is the other end of it, and it has to exist separately because
/// the file on disk is what is trusted here, not the `Asset` that was written.
fn local_asset(directory: &Path, src: &str) -> Option<PathBuf> {
    let rest = src.strip_prefix("assets/")?;
    if rest.is_empty() || rest.contains('/') || rest.contains('\\') || rest.starts_with('.') {
        return None;
    }
    let path = directory.join("assets").join(rest);
    path.is_file().then_some(path)
}

/// An image's box, in CSS pixels: never wider than the column, and never
/// stretched.
///
/// Shrunk on both axes together, so a chord diagram keeps its shape. Rounded up
/// on the height for the reason `song_detail::page_image` rounds up: a picture
/// a pixel shorter than its own shape leaves a hairline of background showing
/// under its bottom edge.
fn fitted(width: u32, height: u32, sizing: &Sizing) -> (u32, u32) {
    let limit = (sizing.column_px as f32 * IMAGE_COLUMN_FRACTION).max(1.0) as u32;
    if width <= limit || width == 0 {
        return (width.max(1), height.max(1));
    }
    let scaled = (limit as u64 * height as u64).div_ceil(width as u64) as u32;
    (limit, scaled.max(1))
}

/// An image's pixels, read out of its header without decoding it.
///
/// The same argument `pdf::pages::page_pixels` makes for a PNG, applied to the
/// four formats `assets::extension` can actually name: decoding a 1 MB scan to
/// learn two numbers is work with no output, and it would happen once per image
/// per render. Every one of these formats puts its dimensions in a fixed place
/// near the front of the file.
///
/// `svg`, `bmp` and `avif` are the three extensions the downloader can write
/// that are not here. SVG has no pixel size at all — its dimensions are CSS,
/// and Rinch has no SVG decoder to draw it with either; the other two are rare
/// enough on a chord site that a caption is a fair answer until one turns up.
/// All three fall through to `alt`, which is the same place a missing file
/// goes, and nothing crashes on any of them.
pub fn image_pixels(path: &Path) -> Option<(u32, u32)> {
    let bytes = read_head(path, 64 * 1024)?;
    png_pixels(&bytes)
        .or_else(|| gif_pixels(&bytes))
        .or_else(|| webp_pixels(&bytes))
        .or_else(|| jpeg_pixels(&bytes))
}

/// The front of a file, or as much of it as there is.
fn read_head(path: &Path, limit: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut head = Vec::new();
    file.take(limit as u64).read_to_end(&mut head).ok()?;
    Some(head)
}

fn png_pixels(b: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if b.len() < 24 || b[..8] != SIGNATURE || &b[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
    let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
    (w > 0 && h > 0).then_some((w, h))
}

fn gif_pixels(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 10 || (&b[..6] != b"GIF87a" && &b[..6] != b"GIF89a") {
        return None;
    }
    let w = u16::from_le_bytes([b[6], b[7]]) as u32;
    let h = u16::from_le_bytes([b[8], b[9]]) as u32;
    (w > 0 && h > 0).then_some((w, h))
}

/// The three WebP flavours: lossy (`VP8 `), lossless (`VP8L`) and extended
/// (`VP8X`). They keep their size in three different places and none of them is
/// where the others are, which is why this is three arms rather than one.
fn webp_pixels(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 30 || &b[..4] != b"RIFF" || &b[8..12] != b"WEBP" {
        return None;
    }
    match &b[12..16] {
        b"VP8 " => {
            // The keyframe header: a three-byte start code, then 14 bits of
            // width and 14 bits of height.
            if b[23..26] != [0x9d, 0x01, 0x2a] {
                return None;
            }
            let w = (u16::from_le_bytes([b[26], b[27]]) & 0x3fff) as u32;
            let h = (u16::from_le_bytes([b[28], b[29]]) & 0x3fff) as u32;
            (w > 0 && h > 0).then_some((w, h))
        }
        b"VP8L" => {
            if b[20] != 0x2f {
                return None;
            }
            let bits = u32::from_le_bytes([b[21], b[22], b[23], b[24]]);
            let w = (bits & 0x3fff) + 1;
            let h = ((bits >> 14) & 0x3fff) + 1;
            Some((w, h))
        }
        b"VP8X" => {
            let w = u32::from_le_bytes([b[24], b[25], b[26], 0]) + 1;
            let h = u32::from_le_bytes([b[27], b[28], b[29], 0]) + 1;
            Some((w, h))
        }
        _ => None,
    }
}

/// JPEG is the one that has to be walked: the size lives in whichever start-of-
/// frame marker the encoder chose, after however many other segments it wrote
/// first. Every segment states its own length, so this is a hop rather than a
/// scan, and it stops at the first frame header.
fn jpeg_pixels(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 4 || b[0] != 0xff || b[1] != 0xd8 {
        return None;
    }
    let mut i = 2usize;
    while i + 3 < b.len() {
        if b[i] != 0xff {
            // Not on a marker boundary. A truncated or padded file rather than
            // something to resynchronise on.
            return None;
        }
        let marker = b[i + 1];
        // Padding fill bytes, and the standalone markers that carry no length.
        if marker == 0xff {
            i += 1;
            continue;
        }
        if matches!(marker, 0xd8 | 0x01) || (0xd0..=0xd7).contains(&marker) {
            i += 2;
            continue;
        }
        let length = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        // Every start-of-frame marker but the four that are not frames.
        let is_frame = (0xc0..=0xcf).contains(&marker)
            && !matches!(marker, 0xc4 | 0xc8 | 0xcc);
        if is_frame {
            if i + 9 >= b.len() {
                return None;
            }
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
            return (w > 0 && h > 0).then_some((w, h));
        }
        if length < 2 {
            return None;
        }
        i += 2 + length;
    }
    None
}

/// Whether the site's own inline style says this is not on the page.
///
/// Substring rather than a CSS parse, and deliberately: the only declaration
/// being looked for is one whose whole point is to be unmissable, and a parser
/// for a stranger's inline CSS is a great deal of surface for one bit of
/// information. `display:none` and `display: none` are both spelled here
/// because both are what sites write.
fn hidden(node: &Handle) -> bool {
    let Some(style) = dom::attr(node, "style") else {
        return false;
    };
    let style = style.to_ascii_lowercase();
    style.contains("display:none") || style.contains("display: none")
}

/// Escape a text node into the fragment.
///
/// Only `&`, `<` and `>`, because those are the three characters that would
/// otherwise be read back as markup — and because Rinch's parser decodes
/// exactly six named entities, so anything cleverer would arrive on screen as
/// its own source. Everything else, including a non-breaking space, goes
/// through as UTF-8, which the parser passes along untouched.
///
/// Outside a `<pre>` a run of whitespace is collapsed to one space, which is
/// what the renderer would do to it anyway and what `dom::text_of` does for the
/// extracted text. It is worth doing here rather than leaving to the layout
/// because a saved page is mostly its own indentation: on the hymnal.net
/// capture, collapsing took the fragment from 34.6 KB to 21.5 KB, and every one
/// of those bytes was a character Rinch would otherwise parse, allocate a
/// tendril for and then measure to zero width.
///
/// **Inside a `<pre>` nothing is touched.** The runs of spaces are what put the
/// chord over the syllable; they are the chart.
fn push_text(text: &str, preformatted: bool, out: &mut String) {
    if preformatted {
        for ch in text.chars() {
            push_escaped(ch, out);
        }
        return;
    }
    let mut space = out.ends_with(' ');
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !space {
                out.push(' ');
                space = true;
            }
        } else {
            push_escaped(ch, out);
            space = false;
        }
    }
}

fn push_escaped(ch: char, out: &mut String) {
    match ch {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        _ => out.push(ch),
    }
}

/// The same, plus the quote that would end the attribute.
fn push_attr(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

/// Dropped whole, subtree included.
///
/// `sanitise` removed every one of these at capture time. They are listed again
/// because this file reads what is on disk rather than what the sanitiser
/// returned, and a saved page outlives the build that saved it.
const DROPPED: &[&str] = &[
    "head", "title", "meta", "link", "base", "style", "script", "noframes", "template", "iframe",
    "frame", "frameset", "object", "embed", "applet", "canvas", "svg", "math", "form", "input",
    "button", "select", "option", "optgroup", "textarea", "label", "audio", "video", "source",
    "track", "map", "area", "dialog",
];

/// Emitted with no children and no end tag, matching Rinch's own void list.
const VOID: &[&str] = &["br", "hr"];

/// The only elements kept inside a `<pre>` — see the unwrapping rule in
/// [`emit`]. Everything here is inline in Rinch's UA sheet, so none of it adds
/// a line the characters did not already ask for.
const INLINE_IN_PRE: &[&str] = &[
    "a", "b", "strong", "i", "em", "u", "s", "span", "code", "kbd", "samp", "sup", "sub", "small",
    "mark", "br",
];

/// What a missing image leaves behind.
const MISSING_IMAGE_STYLE: &str =
    "font-style: italic; color: var(--sla-muted); font-size: 0.85em;";

/// Every tag the app keeps, and the typography it keeps it with.
///
/// This is the app's stylesheet for somebody else's document, and it is inline
/// on each element rather than in a `<style>` block for the reason in the module
/// header: a `<style>` block would be loaded into the whole window's stylist.
///
/// Sizes are in `em` so the fragment scales with the host — see the header. The
/// colours are `var(--sla-*)` and resolve against whatever the host inherits,
/// which is how the same markup comes out dark inside `attachment_viewer`'s
/// re-declared neutrals and light inside `song_detail`'s card without either
/// screen saying so.
///
/// The margins are the browser's proportions rather than the browser's numbers.
/// A default `<h1>` is 2em with 0.67em of margin, which is a magazine headline;
/// on a 393 px phone showing somebody else's chord page it is four words per
/// line. Everything here is compressed towards the app's own type scale, and
/// `<pre>` — the one element that actually holds the chart on most of the sites
/// this engine captures — gets the mono face and `white-space: pre` that
/// Rinch's UA sheet does not supply.
const STYLED: &[(&str, &str)] = &[
    ("h1", "font-size: 1.45em; font-weight: 700; line-height: 1.25; margin: 0.5em 0 0.3em;"),
    ("h2", "font-size: 1.25em; font-weight: 700; line-height: 1.3; margin: 0.5em 0 0.25em;"),
    ("h3", "font-size: 1.1em; font-weight: 600; margin: 0.45em 0 0.2em;"),
    ("h4", "font-size: 1em; font-weight: 600; margin: 0.4em 0 0.2em;"),
    ("h5", "font-size: 0.95em; font-weight: 600; margin: 0.4em 0 0.2em;"),
    ("h6", "font-size: 0.9em; font-weight: 600; margin: 0.4em 0 0.2em;"),
    ("p", "margin: 0.45em 0; line-height: 1.5;"),
    // No `overflow` of its own, and that was a correction rather than an
    // omission: with `overflow-x: auto` on it, Rinch drew a scrollbar thumb
    // down the right-hand edge of the chart — visible in the card on `:99` on
    // 2026-08-28, a grey pill sitting on the last letter of the first line.
    // A chart wider than the screen is reached by scrolling the screen, which
    // is exactly what `attachment_viewer` already does for a typed chart and
    // says so: its scrolling box carries both overflow axes precisely so the
    // whole line is reachable, and its lines deliberately do not carry
    // `T_CHART`'s own `overflow: hidden`. In the card the wrapper clips, which
    // is what a preview is for.
    (
        "pre",
        "font-family: var(--sla-font-mono); white-space: pre; font-size: 0.92em; \
         line-height: 1.5; margin: 0.6em 0;",
    ),
    ("code", "font-family: var(--sla-font-mono); font-size: 0.92em;"),
    ("kbd", "font-family: var(--sla-font-mono); font-size: 0.92em;"),
    ("samp", "font-family: var(--sla-font-mono); font-size: 0.92em;"),
    (
        "blockquote",
        "margin: 0.5em 0 0.5em 0.9em; padding-left: 0.7em; color: var(--sla-muted); \
         border-left: 2px solid var(--sla-hairline);",
    ),
    ("ul", "margin: 0.4em 0; padding-left: 1.4em;"),
    ("ol", "margin: 0.4em 0; padding-left: 1.5em;"),
    ("li", "margin: 0.12em 0; line-height: 1.5;"),
    ("dl", "margin: 0.4em 0;"),
    ("dt", "font-weight: 600; margin-top: 0.3em;"),
    ("dd", "margin: 0 0 0.2em 0.9em;"),
    // A link cannot be followed with the network off and this app has no
    // browser to hand it to, so it is drawn as the record it is — `sanitise`
    // rewrote it to absolute precisely so that record survives — and not as
    // something to press. Underlined and accent-coloured, because a chart that
    // says "capo 2, see the intro tab" wants the reader to see that the words
    // were a link.
    ("a", "color: var(--sla-accent); text-decoration: underline;"),
    ("strong", ""),
    ("b", ""),
    ("em", ""),
    ("i", ""),
    ("u", ""),
    ("s", ""),
    ("sup", "font-size: 0.75em; vertical-align: super;"),
    ("sub", "font-size: 0.75em; vertical-align: sub;"),
    ("small", "font-size: 0.85em; color: var(--sla-muted);"),
    ("mark", "background: var(--sla-fill);"),
    ("span", ""),
    ("div", ""),
    ("section", ""),
    ("article", ""),
    ("main", ""),
    ("header", ""),
    ("footer", ""),
    ("nav", ""),
    ("aside", ""),
    ("figure", "margin: 0.5em 0;"),
    ("figcaption", "font-size: 0.85em; color: var(--sla-muted); margin-top: 0.2em;"),
    ("br", ""),
    ("hr", "height: 1px; background: var(--sla-hairline); margin: 0.7em 0;"),
    ("table", "margin: 0.5em 0;"),
    ("thead", ""),
    ("tbody", ""),
    ("tfoot", ""),
    ("tr", ""),
    ("th", "padding: 0.1em 0.5em 0.1em 0; text-align: left; font-weight: 600;"),
    ("td", "padding: 0.1em 0.5em 0.1em 0; text-align: left;"),
    ("caption", "font-size: 0.85em; color: var(--sla-muted);"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn sizing() -> Sizing {
        Sizing {
            base_px: 14.5,
            column_px: 393,
        }
    }

    fn render_str(html: &str) -> Page {
        render(html, Path::new("/nonexistent"), sizing(), VIEWER_ELEMENTS)
    }

    #[test]
    fn a_chart_in_a_pre_keeps_its_columns_and_its_face() {
        let page = render_str("<html><body><pre>G       C\nHello    there</pre></body></html>");
        assert!(page.markup.contains("white-space: pre"), "{}", page.markup);
        assert!(page.markup.contains("var(--sla-font-mono)"));
        assert!(page.markup.contains("G       C\nHello    there"));
    }

    /// The four measurements in the module header, as one assertion each: a
    /// doctype is not text, a `<style>` block never reaches the stylist, the
    /// `<title>` is not a heading, and a script is gone even though `sanitise`
    /// should already have taken it.
    #[test]
    fn nothing_that_would_leak_into_the_app_survives() {
        let page = render_str(
            "<!doctype html><html><head><title>Site name</title>\
             <style>p{margin:0}</style></head>\
             <body><p>Verse one</p><script>alert(1)</script></body></html>",
        );
        assert!(!page.markup.contains("doctype"), "{}", page.markup);
        assert!(!page.markup.contains("<style"), "{}", page.markup);
        assert!(!page.markup.contains("margin:0"), "{}", page.markup);
        assert!(!page.markup.contains("Site name"), "{}", page.markup);
        assert!(!page.markup.contains("alert"), "{}", page.markup);
        assert!(page.markup.contains("Verse one"));
    }

    /// Measured on the CifraClub capture: one `<div>` per line inside the
    /// `<pre>`, each ending in a newline, is a block box *and* a line break,
    /// and Wonderwall came out double-spaced.
    #[test]
    fn a_block_inside_a_chart_does_not_add_a_line_the_characters_already_have() {
        let page = render_str(
            "<body><pre><div>[Intro] <b>Em7</b>\n</div><div>Today is gonna\n</div></pre></body>",
        );
        assert!(!page.markup.contains("<div"), "{}", page.markup);
        assert!(page.markup.contains("<b>Em7</b>"), "{}", page.markup);
        assert!(
            page.markup.contains("[Intro] <b>Em7</b>\nToday is gonna\n"),
            "{}",
            page.markup
        );
    }

    /// Outside a chart the indentation of somebody's HTML is not content, and
    /// a saved page is mostly indentation.
    #[test]
    fn whitespace_collapses_outside_a_chart_and_never_inside_one() {
        let prose = render_str("<body><p>one\n\n        two</p></body>");
        assert!(prose.markup.contains("one two"), "{}", prose.markup);
        let chart = render_str("<body><pre>G       C\n</pre></body>");
        assert!(chart.markup.contains("G       C\n"), "{}", chart.markup);
    }

    #[test]
    fn an_element_the_app_does_not_style_keeps_its_children() {
        // `<center>` is not on the list; its text still is.
        let page = render_str("<body><center><p>Chorus</p></center></body>");
        assert!(!page.markup.contains("<center"));
        assert!(page.markup.contains("Chorus"));
    }

    #[test]
    fn the_site_saying_a_thing_is_not_on_the_page_is_believed() {
        let page = render_str(
            "<body><div style=\"display: none\">second copy</div><p>first copy</p></body>",
        );
        assert!(!page.markup.contains("second copy"), "{}", page.markup);
        assert!(page.markup.contains("first copy"));
    }

    #[test]
    fn a_stranger_s_colours_do_not_come_with_it() {
        let page = render_str("<body><p style=\"color:#fff;font-family:Comic\">Verse</p></body>");
        assert!(!page.markup.contains("#fff"), "{}", page.markup);
        assert!(!page.markup.contains("Comic"));
        assert!(page.markup.contains("Verse"));
    }

    #[test]
    fn markup_in_the_text_is_escaped_rather_than_re_read() {
        let page = render_str("<body><pre>a &lt; b &amp; c</pre></body>");
        assert!(page.markup.contains("a &lt; b &amp; c"), "{}", page.markup);
    }

    #[test]
    fn the_budget_stops_a_page_and_says_that_it_did() {
        let body: String = (0..50).map(|n| format!("<p>line {n}</p>")).collect();
        let page = render(
            &format!("<body>{body}</body>"),
            Path::new("/nonexistent"),
            sizing(),
            10,
        );
        assert!(page.truncated);
        // The budget is checked on the way *into* an element and an element is
        // counted on the way out of it, so a full stack of open containers can
        // still finish — never more than one per level of nesting.
        assert!(page.elements <= 10 + MAX_DEPTH, "{}", page.elements);
        assert!(!page.markup.contains("line 49"));
    }

    #[test]
    fn a_page_with_nothing_on_it_is_empty_rather_than_a_blank_box() {
        let page = render_str("<body></body>");
        assert!(page.is_empty());
    }

    /// Void elements have to be emitted the way Rinch's parser expects to read
    /// them, or everything after a `<br>` ends up inside it.
    #[test]
    fn a_line_break_closes_itself() {
        let page = render_str("<body><p>one<br>two</p></body>");
        assert!(page.markup.contains("<br>"), "{}", page.markup);
        assert!(!page.markup.contains("</br>"));
        assert!(page.markup.ends_with("</p>"), "{}", page.markup);
    }

    #[test]
    fn a_missing_image_becomes_its_caption_and_is_counted() {
        let page = render_str("<body><img src=\"assets/000.png\" alt=\"Chord box\"></body>");
        assert_eq!(page.images, 0);
        assert_eq!(page.broken_images, 1);
        assert!(page.markup.contains("Chord box"), "{}", page.markup);
        assert!(!page.markup.contains("<img"));
    }

    /// The half of the asset rule that matters: a `src` that is not a plain
    /// name inside `assets/` is never turned into a path, because that path
    /// goes straight to `std::fs::read` inside the framework.
    #[test]
    fn an_image_src_cannot_point_outside_the_attachment_directory() {
        for src in [
            "assets/../../../etc/passwd",
            "../secret.png",
            "/etc/passwd",
            "https://example.com/a.png",
            "assets/nested/a.png",
            "assets/.hidden",
        ] {
            assert!(
                local_asset(Path::new("/library/7"), src).is_none(),
                "{src} was accepted"
            );
        }
    }

    #[test]
    fn an_image_that_is_there_is_drawn_with_both_axes_stated() {
        let dir = tempdir("render-image");
        std::fs::create_dir_all(dir.join("assets")).expect("assets");
        std::fs::write(dir.join("assets/000.png"), png(800, 400)).expect("png");
        let page = render(
            "<body><img src=\"assets/000.png\" alt=\"Chord box\"></body>",
            &dir,
            sizing(),
            VIEWER_ELEMENTS,
        );
        assert_eq!(page.images, 1);
        assert_eq!(page.broken_images, 0);
        // Wider than the 393 px column, so it comes back fitted to it and half
        // as tall — K28: both axes, in pixels, always.
        assert!(page.markup.contains("width: 393px; height: 197px"), "{}", page.markup);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_image_smaller_than_the_column_is_left_alone() {
        assert_eq!(fitted(120, 90, &sizing()), (120, 90));
        assert_eq!(fitted(786, 393, &sizing()), (393, 197));
    }

    #[test]
    fn every_format_the_downloader_can_write_gives_up_its_size() {
        let dir = tempdir("render-formats");
        std::fs::create_dir_all(&dir).expect("dir");
        let cases: [(&str, Vec<u8>, (u32, u32)); 4] = [
            ("a.png", png(640, 480), (640, 480)),
            ("a.gif", gif(320, 200), (320, 200)),
            ("a.webp", webp_lossy(300, 150), (300, 150)),
            ("a.jpg", jpeg(1024, 768), (1024, 768)),
        ];
        for (name, bytes, expected) in cases {
            let path = dir.join(name);
            std::fs::write(&path, bytes).expect("write");
            assert_eq!(image_pixels(&path), Some(expected), "{name}");
        }
        // And something that is none of them.
        let path = dir.join("a.svg");
        std::fs::write(&path, b"<svg width=\"10\"></svg>").expect("write");
        assert_eq!(image_pixels(&path), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_truncated_image_header_is_none_rather_than_a_panic() {
        let dir = tempdir("render-truncated");
        std::fs::create_dir_all(&dir).expect("dir");
        for (name, bytes) in [
            ("short.png", png(4, 4)[..12].to_vec()),
            ("short.jpg", vec![0xff, 0xd8, 0xff]),
            ("short.webp", b"RIFF\0\0\0\0WEBPVP8 ".to_vec()),
            ("short.gif", b"GIF89a".to_vec()),
            ("empty", Vec::new()),
        ] {
            let path = dir.join(name);
            std::fs::write(&path, bytes).expect("write");
            assert_eq!(image_pixels(&path), None, "{name}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A page nested past the limit is flattened, not dropped and not a stack
    /// overflow in a parser that has no depth guard of its own.
    #[test]
    fn a_pathologically_nested_page_keeps_its_words() {
        let deep = format!(
            "<body>{}<p>bottom</p>{}</body>",
            "<div>".repeat(200),
            "</div>".repeat(200)
        );
        let page = render_str(&deep);
        assert!(page.markup.contains("bottom"), "{}", page.markup);
        assert!(
            page.markup.matches("<div").count() <= MAX_DEPTH,
            "{} divs survived",
            page.markup.matches("<div").count()
        );
    }

    /// Everything this file emits has to be readable by the parser on the other
    /// side of `set_inner_html`, and that parser is not html5ever. The one
    /// property that matters is balance: every non-void tag opened is closed,
    /// because Rinch's parser has no implicit end tags and would swallow the
    /// rest of the page into the first one left open.
    #[test]
    fn the_fragment_is_balanced() {
        let page = render_str(
            "<body><div><p>one<br>two<img src=\"x\"></p><ul><li>a<li>b</ul>\
             <table><tr><td>c</table><hr></div></body>",
        );
        let mut stack: Vec<String> = Vec::new();
        let mut rest = page.markup.as_str();
        while let Some(open) = rest.find('<') {
            rest = &rest[open + 1..];
            let end = rest.find('>').expect("every tag closes");
            let inner = &rest[..end];
            rest = &rest[end + 1..];
            if let Some(name) = inner.strip_prefix('/') {
                assert_eq!(stack.pop().as_deref(), Some(name), "{}", page.markup);
            } else {
                let name = inner
                    .split(|c: char| c.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !VOID.contains(&name.as_str()) && name != "img" {
                    stack.push(name);
                }
            }
        }
        assert!(stack.is_empty(), "left open: {stack:?}");
    }

    // -- fixtures ----------------------------------------------------------

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sla-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// A PNG header with the right dimensions and no image behind it. Nothing
    /// here decodes, so nothing here needs one.
    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        b.extend_from_slice(&13u32.to_be_bytes());
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&[8, 6, 0, 0, 0]);
        b
    }

    fn gif(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"GIF89a".to_vec();
        b.extend_from_slice(&(w as u16).to_le_bytes());
        b.extend_from_slice(&(h as u16).to_le_bytes());
        b.extend_from_slice(&[0, 0, 0]);
        b
    }

    fn webp_lossy(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"RIFF".to_vec();
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(b"WEBPVP8 ");
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&[0, 0, 0]); // frame tag
        b.extend_from_slice(&[0x9d, 0x01, 0x2a]);
        b.extend_from_slice(&(w as u16).to_le_bytes());
        b.extend_from_slice(&(h as u16).to_le_bytes());
        b
    }

    /// Two segments before the frame header, so the walk has to hop.
    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let mut b = vec![0xff, 0xd8];
        for _ in 0..2 {
            b.extend_from_slice(&[0xff, 0xe0]);
            b.extend_from_slice(&6u16.to_be_bytes());
            b.extend_from_slice(&[0, 0, 0, 0]);
        }
        b.extend_from_slice(&[0xff, 0xc0]);
        b.extend_from_slice(&17u16.to_be_bytes());
        b.push(8);
        b.extend_from_slice(&(h as u16).to_be_bytes());
        b.extend_from_slice(&(w as u16).to_be_bytes());
        b.extend_from_slice(&[3; 10]);
        b
    }
}
