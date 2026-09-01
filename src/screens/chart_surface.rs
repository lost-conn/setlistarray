//! The one place a chart is drawn full-screen — extracted by card F1.
//!
//! Not a screen and not a route, for the same reason `captured_page` is not:
//! it is a component because two screens have to agree about something, and a
//! shared component is the only way to make an agreement that cannot drift.
//! `captured_page`'s header states the version of the argument that was about
//! the card and the viewer; this is the version that is about the viewer
//! (`1k`, D5) and performance mode (`1o`, F1).
//!
//! Those two screens want the same picture and different furniture. The viewer
//! wraps it in dark chrome that fades away, a page counter, a zoom ladder and a
//! rotate button; performance mode wraps it in a thin top bar with the set
//! position and the song's key. What is *inside* is identical in both, and it
//! is not a small thing to have written twice: three attachment kinds, the K28
//! two-axis sizing workaround, the centre-until-it-overflows rule, the padding
//! that differs between a page and a column of typed text, and four different
//! sentences for four different ways of having nothing to show. A second copy
//! of that would not stay a copy — the first thing either screen fixed would
//! only be fixed on one of them, and the fault would be invisible until
//! somebody opened the same chart from the other door.
//!
//! ## The seam, and why it falls exactly here
//!
//! This component owns **the scrolling box and everything in it**; the caller
//! owns the bars around it. The alternative seam — the caller keeps its own
//! scroll box and this draws only the content — was rejected because three of
//! the things that make a chart readable live on the box itself and are decided
//! by what is *in* it: `overflow-x` (a zoomed page pans, and D5's own note
//! records a chart that lost the first three characters of every line without
//! it), `align-items` (see [`PageBox::wider_than`] — a page too wide to be
//! centred must not be), and the padding, which is nothing at all for a PDF and
//! a real inset for typed text. Handing those three to every caller to get
//! right is handing them three chances to get one of them wrong.
//!
//! What the caller passes is only ever *how big*: which page, at what zoom, at
//! what rotation, how wide the column is and what type size a typed chart is
//! set in. The viewer varies all five; performance mode fixes all five and
//! passes a bigger `base_px` than the viewer's, which is the whole of what
//! `1o`'s "larger type than the detail preview" means.
//!
//! ## Nothing in here is reactive, and that is the caller's job
//!
//! There is not a `Signal` in this file. The viewer already rebuilds its whole
//! chart subtree whenever the page, the zoom or the rotation changes — a keyed
//! one-element `for`, for a scroll-offset fault written up in
//! `attachment_viewer`'s own comment — so a component mounted inside that `for`
//! is remounted with the new numbers for free, and a component that took
//! signals would be reading them a second time to reach exactly the same
//! answer. Performance mode has nothing to vary at all in F1.
//!
//! That is also what makes the two expensive things in here safe. Reading a
//! PNG's size is 24 bytes off the front of a file and rendering a captured page
//! is a full html5ever parse; both happen once per mount, which is the rule
//! `pdf::pages` and `captured_page` already wrote for themselves, and neither
//! is ever reached from a render closure.

use rinch::prelude::*;

use crate::model::{Attachment, AttachmentId, AttachmentKind};
use crate::store::AttachmentsStore;
use crate::theme::T_META;

use super::captured_page::CapturedPageView;

/// The type size a typed chart is drawn at in the viewer, at 100 %, in CSS
/// pixels.
///
/// `theme::T_CHART` renders a chart at 12.5 px inside the card on song detail
/// and `chart_editor` types into it at 13.5 px. The viewer is one of the two
/// places a chart is the *only* thing on the display, so it starts a shade
/// larger again and the zoom ladder goes up from there.
pub const CHART_BASE_PX: f32 = 14.5;

/// The type size performance mode draws a typed chart at.
///
/// `1o`'s one typographic instruction is *"larger type than the detail
/// preview"*, and it is larger than the viewer's too — deliberately, because
/// the two screens are read from different distances. The viewer is a chart in
/// your hands; `1o` is a phone on a stand, a metre away, being read by somebody
/// whose hands are full. There is no zoom ladder on it to correct a guess that
/// came out too small (that is not F1's, and on a stand it would be a control
/// you have to walk over to press), so the guess is made once and made
/// generous: half again the card's 12.5 px.
///
/// It is not larger still because a monospaced chart cannot wrap — `white-space:
/// pre` is what keeps a chord over its syllable — so every extra pixel of type
/// is width a long lyric line spends running off the right of the screen. 18.5
/// is the last rung that keeps a typical 40-column chart inside a 393 px phone.
pub const STAGE_CHART_PX: f32 = 18.5;

/// Space kept clear either side of a rasterised page at 100 %.
///
/// Zero, and deliberately. D4 picked its 1080 px cache width as "a full-bleed
/// page on a 393 pt-wide phone at 2.75×, which is what D5's viewer needs" — so
/// any inset here would be downscaling the cache by exactly the amount inset
/// and throwing away the pixel-for-pixel match that width was chosen to get.
/// `1k` draws a 14 px gutter, but it draws it around a white page on a *light*
/// grey backdrop, where the page needs an edge to be a page. On the viewer's
/// dark backdrop it already has one, and `1o` asks for full-bleed in words.
pub const PAGE_GUTTER: u32 = 0;

/// How many pages this attachment has, as far as a reader is concerned.
///
/// A PDF's count comes from D3's import and can be absent — `crate::pdf`'s
/// header is explicit that a file hayro cannot parse is still a file the user
/// chose to keep, and it is stored with `page_count: None`. Such a chart is one
/// page here, and that one page will fail to draw and say so, which is a better
/// answer than a viewer that refuses to open.
///
/// Everything else is one page. A typed chart and a captured page are a single
/// flow of text; paging through them is not a concept they have.
pub fn page_span(attachment: &Attachment) -> u32 {
    match attachment.kind {
        AttachmentKind::Pdf => attachment.page_count.unwrap_or(1).max(1),
        AttachmentKind::Text | AttachmentKind::CapturedPage => 1,
    }
}

/// Where a page goes: the two axes of the `<img>`, the box it occupies once it
/// has been turned, and how far to turn it.
///
/// Both of the first two are needed rather than one. The `<img>`'s own size has
/// to be stated on both axes because of K28 (see `attachment_viewer`'s header,
/// which records the framework gap in full). The frame's size is what the
/// *layout* has to reserve, and it is not the same thing the moment the page is
/// on its side — a `transform` moves paint and hit-testing, and leaves the box
/// where it was, so a page turned 90° inside a box shaped like the untuned one
/// would overhang it by exactly the difference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageBox {
    /// The `<img>`'s width and height, in CSS pixels.
    pub image: (u32, u32),
    /// The width and height the turned page occupies, in CSS pixels.
    pub frame: (u32, u32),
    /// Clockwise degrees for `transform: rotate()`.
    pub degrees: u32,
}

impl PageBox {
    /// Whether the page is wider than the screen it is being read on, which is
    /// only ever true once it has been zoomed.
    ///
    /// This decides one thing and it is not cosmetic. The page sits in a flex
    /// column that centres its items, and a centred item wider than its
    /// container overflows *both* sides — while the scrollable range only ever
    /// runs from zero to the overflow on the far side. So the left half of a
    /// zoomed page ends up outside the scroll range and is unreachable: a
    /// 150 %-zoomed chart lost its first three characters on every line, with
    /// a horizontal drag that did nothing at all, measured on `:99` on
    /// 2026-08-28.
    ///
    /// The fix is to stop centring exactly when centring is what breaks it:
    /// a page that fits is centred, a page that does not starts at the left
    /// edge, and then all of its overflow is on the scrollable side.
    pub fn wider_than(self, column: u32) -> bool {
        self.frame.0 > column
    }
}

/// Fit a page of `pixels` into a `column`-wide screen, at `zoom` percent, after
/// `quarter` quarter-turns.
///
/// **Fit to width, always, and let the height run off the bottom.** The screen's
/// height never appears in this function and that is a decision rather than an
/// omission: Rinch surfaces no viewport height to app code, and fitting the
/// *whole* page into the display would make a chord chart's lyrics too small to
/// read at the exact moment somebody is trying to read them at arm's length.
/// Width-fitting is also self-correcting for the only case that would overflow
/// — anything taller than about 16:9 scrolls, which is the right answer for a
/// foldout chart and the wrong answer for nothing.
///
/// Turning the page swaps which of the bitmap's axes the column constrains.
/// Upright, the column is the page's width; on its side, the column is the
/// page's *height*, because that is the edge now running left to right.
pub fn page_box(pixels: (u32, u32), column: u32, zoom: u32, quarter: u8) -> PageBox {
    let (page_w, page_h) = pixels;
    let quarter = quarter % 4;
    let degrees = quarter as u32 * 90;

    // A page with a zero axis cannot come out of `page_pixels`, which rejects
    // one — but this function is public arithmetic and a caller that has not
    // been through that check must not get a division by zero out of it.
    if page_w == 0 || page_h == 0 {
        return PageBox {
            image: (0, 0),
            frame: (0, 0),
            degrees,
        };
    }

    let across = ((column as u64 * zoom as u64) / 100).max(1) as u32;

    // Rounded up on both branches, so a page is never a pixel shorter than its
    // own shape and never leaves a hairline of backdrop inside its own edge —
    // the same reasoning `song_detail::page_image` records.
    if quarter % 2 == 0 {
        let down = (across as u64 * page_h as u64).div_ceil(page_w as u64) as u32;
        PageBox {
            image: (across, down),
            frame: (across, down),
            degrees,
        }
    } else {
        // On its side the image's own *height* is what spans the column, so
        // that is the axis `across` is assigned to, and its width follows from
        // the page's shape.
        let image_w = (across as u64 * page_w as u64).div_ceil(page_h as u64) as u32;
        PageBox {
            image: (image_w, across),
            frame: (across, image_w),
            degrees,
        }
    }
}

/// A typed or captured chart, one line per entry, ready for `rsx!`'s `for`.
///
/// A blank line becomes a single space for the same reason `song_detail` does
/// it: an empty `div` collapses to no height, and the blank lines between a
/// verse and a chorus are part of how a chart is read.
pub fn chart_lines(body: &str) -> Vec<(usize, String)> {
    body.lines()
        .enumerate()
        .map(|(index, line)| {
            let text = if line.trim().is_empty() {
                " ".to_string()
            } else {
                line.to_string()
            };
            (index, text)
        })
        .collect()
}

/// The sentence shown in place of a chart, when there is no chart to show.
///
/// Every branch here is a real state on somebody's phone, and each one gets its
/// own words rather than a shared "couldn't load". `song_detail::note` makes the
/// same argument about the card; this is the full-screen version, and it can be
/// more specific because it knows which page was asked for.
pub fn nothing_to_show(kind: AttachmentKind, page: u32, span: u32) -> String {
    match kind {
        // The self-healing render in the viewer's component body and in its
        // page handlers has already run and come back with nothing, so unlike
        // the card's version of this sentence there is no "not yet" about it:
        // hayro was handed this page and could not draw it. `docs/PDF.md` is
        // explicit that a 0.x rasteriser will meet a file it cannot draw, and
        // `pages::draw` catches the panic rather than taking the process, so
        // this is the state that catch lands in.
        AttachmentKind::Pdf if span > 1 => {
            format!("Page {page} of this PDF could not be drawn.")
        }
        AttachmentKind::Pdf => "This PDF could not be drawn.".to_string(),
        AttachmentKind::Text => "Nothing typed yet.".to_string(),
        // Unreachable since E5, and left as a panic-free empty string rather
        // than a sentence so that it stays that way: a captured page is drawn
        // by `captured_page::CapturedPageView`, which owns the page, the text
        // it falls back to when the files are gone, and both of the sentences
        // for when there is neither. `empty_state` returns before it can get
        // here. A fourth wording of the same state, chosen in a function that
        // can no longer tell which of the two faults it is looking at, would be
        // worse than none.
        AttachmentKind::CapturedPage => String::new(),
    }
}

/// The sentence for a row that is not in the library any more.
///
/// The viewer has its own screen for this — a bar with a working ← in it, since
/// "Remove attachment" is an item in the viewer's own overflow menu and the
/// state it lands you in has to have a way out. This is the same fact said
/// inside the picture rather than instead of the screen, for a caller whose
/// chrome is still perfectly usable: performance mode's ✕ and its position
/// counter are about the *set*, and a set does not stop existing because one
/// song's chart did.
pub const CHART_GONE: &str = "This chart is gone.";

/// A chart, full-screen, at the size the caller asks for.
///
/// `page`, `zoom` and `quarter` are the viewer's three controls; performance
/// mode leaves all three at their defaults. `column` is how wide the host is —
/// a page is fitted to it and a captured page's images are sized against it —
/// and `base_px` is the type size a typed chart is set in and the size a
/// captured page's whole `em` ladder resolves against.
///
/// `body` is passed in rather than read here, and it is the one prop that could
/// have gone either way. `AttachmentsStore::body` is an object read off the
/// database, the viewer reads it once when the *screen* mounts, and this
/// component is remounted on every zoom step — so reading it here would turn
/// one read per chart into one per press of `A+` on a string that can be a
/// megabyte. The caller reading it once and handing over a clone is the cheaper
/// half of that trade by a wide margin.
#[component]
pub fn ChartSurface(
    attachment: Option<AttachmentId>,
    body: String,
    page: Option<u32>,
    zoom: Option<u32>,
    quarter: Option<u8>,
    column: Option<u32>,
    base_px: Option<f32>,
    span: Option<u32>,
    onclick: Option<Callback>,
) -> NodeHandle {
    let attachments = use_store::<AttachmentsStore>();

    let id = attachment.unwrap_or_default();
    let page = page.unwrap_or(1);
    let zoom = zoom.unwrap_or(100);
    let quarter = quarter.unwrap_or(0);
    let column = column.unwrap_or(crate::WIDTH).max(1);
    let base_px = base_px.unwrap_or(CHART_BASE_PX);
    let span = span.unwrap_or(1);

    // Read once, at mount, and this is the only place the kind is decided —
    // every branch below is a `match` on the answer. Both callers have already
    // checked that the row exists before mounting this, so the `else` is the
    // race rather than the path: a chart removed from the viewer's own overflow
    // menu, or a song whose only chart was deleted from another screen while
    // the set was playing.
    let Some(chart) = attachments.get(id) else {
        return rsx! {
            div {
                style: {format!("flex: 1; min-height: 0; {T_META} padding: 40px 22px; text-align: center;")},
                {CHART_GONE}
            }
        };
    };
    let kind = chart.kind;
    let pdf = kind == AttachmentKind::Pdf;

    rsx! {
        div {
            onclick: move || { if let Some(cb) = &onclick { cb.invoke() } },
            // Both overflow axes scroll. Vertical is the page that is taller
            // than the display, which is most pages; horizontal only appears
            // once the page is zoomed past the column, and Rinch's Android
            // recogniser does produce horizontal deltas (`touch_gesture.rs`),
            // so a zoomed chart can be panned with a finger rather than only
            // read down the middle.
            style: {format!(
                "flex: 1; min-height: 0; overflow-y: auto; overflow-x: auto; \
                 display: flex; flex-direction: column; align-items: center; \
                 padding: {};",
                // A page brings its own margins — it is a picture of a sheet of
                // paper, and `PAGE_GUTTER` explains why nothing is added around
                // it. Typed text does not: without this the first line of a
                // chart sits against the underside of the top bar and the last
                // against the top of whatever is below. The bottom is the
                // deeper of the two so that the final line clears the viewer's
                // chrome when it comes back.
                if pdf { format!("{PAGE_GUTTER}px") } else { "18px 16px 28px".to_string() },
            )},

            for spread in page_spread(attachments, id, pdf, page, zoom, quarter, column) {
                div {
                    key: {spread.0.clone()},
                    // The frame the turned page occupies. `flex-shrink: 0`
                    // because this is a flex column and the default would
                    // squeeze a tall page rather than let it scroll.
                    style: {format!(
                        "width: {}px; height: {}px; flex-shrink: 0; align-self: {}; \
                         display: flex; align-items: center; justify-content: center;",
                        spread.1.frame.0,
                        spread.1.frame.1,
                        // See `PageBox::wider_than`: centring a page that
                        // overflows puts half of it outside the scroll range,
                        // where no gesture can reach it.
                        if spread.1.wider_than(column) { "flex-start" } else { "center" },
                    )},
                    img {
                        src: {spread.0.clone()},
                        // Both axes in pixels — K28, `attachment_viewer`'s
                        // header. The rotation is paint-only, about the image's
                        // own centre (Rinch's `transform-origin` default), and
                        // the centre is where the flex box above has just put
                        // it, so a quarter turn lands exactly inside the frame
                        // with no translate to get wrong.
                        style: {format!(
                            "width: {}px; height: {}px; flex-shrink: 0; transform: rotate({}deg);",
                            spread.1.image.0, spread.1.image.1, spread.1.degrees
                        )},
                    }
                }
            }

            // E5: the saved page itself, at the size asked for. Remounted with
            // this whole component, so a zoom step renders the page again
            // against the new `base_px` — which is what makes `A+` scale the
            // images and the headings and not only the prose.
            for captured in captured_of(kind, id) {
                CapturedPageView {
                    attachment: {captured},
                    base_px: {base_px},
                    // The scrolling box pads itself by 16 px each side for
                    // typed text (see the `padding` above), and that is width
                    // an image inside the page genuinely does not have.
                    column_px: {column.saturating_sub(32).max(1)},
                    budget: {crate::capture::render::VIEWER_ELEMENTS},
                    style: "align-self: stretch;",
                }
            }

            for (index, line) in text_of(&body, pdf) {
                div {
                    key: {index},
                    // `white-space: pre` and a monospaced face are what keep a
                    // chord over its syllable — `theme::T_CHART` makes the same
                    // argument. What is deliberately *not* carried over from it
                    // is `overflow: hidden`: the card on song detail clips a
                    // long line rather than widen itself, and these two screens
                    // are where the whole line is supposed to be reachable.
                    style: {format!(
                        "font-family: var(--sla-font-mono); white-space: pre; \
                         line-height: 1.5; align-self: flex-start; \
                         font-size: {base_px:.1}px;"
                    )},
                    {line.clone()}
                }
            }

            for sentence in empty_state(attachments, id, kind, pdf, &body, page, span) {
                div {
                    key: {sentence.clone()},
                    style: {format!("{T_META} padding: 40px 22px; text-align: center;")},
                    {sentence.clone()}
                }
            }
        }
    }
}

/// The page on screen, as `(src, geometry)` — or nothing, for every kind that
/// is not a PDF and every PDF page that is not on disk.
///
/// A nought-or-one vector because `rsx!`'s `for` is the only reactive
/// conditional the macro has; `song_detail` uses the same shape for the same
/// reason.
///
/// **Nothing in here can rasterise.** `cached_page` is one `stat` and
/// `page_pixels` is 24 bytes off the front of a file. Drawing a page that is
/// not there yet is the caller's job and is done in a tap handler — see
/// `attachment_viewer`'s header for the rule and `pdf::pages` for why it
/// exists.
fn page_spread(
    attachments: AttachmentsStore,
    id: AttachmentId,
    pdf: bool,
    page: u32,
    zoom: u32,
    quarter: u8,
    column: u32,
) -> Vec<(String, PageBox)> {
    if !pdf {
        return Vec::new();
    }
    let Some(directory) = attachments.directory(id) else {
        return Vec::new();
    };
    let Some(path) = crate::pdf::pages::cached_page(&directory, page) else {
        return Vec::new();
    };
    let Some(pixels) = crate::pdf::pages::page_pixels(&path) else {
        return Vec::new();
    };
    vec![(
        path.to_string_lossy().into_owned(),
        page_box(pixels, column, zoom, quarter),
    )]
}

/// The attachment to hand `CapturedPageView`, or nothing — the nought-or-one
/// shape everything conditional in here uses, because `rsx!`'s `for` is the
/// only conditional the macro has.
fn captured_of(kind: AttachmentKind, id: AttachmentId) -> Vec<AttachmentId> {
    match kind {
        AttachmentKind::CapturedPage => vec![id],
        _ => Vec::new(),
    }
}

/// The lines of a typed chart, and nothing at all for anything else.
///
/// A captured page used to come through here as its extracted text; since E5 it
/// is drawn as the page by `captured_page::CapturedPageView`, which falls back
/// to that same text itself when the saved file is gone. One of the two has to
/// own it or the screen draws it twice.
fn text_of(body: &str, pdf: bool) -> Vec<(usize, String)> {
    if pdf {
        return Vec::new();
    }
    chart_lines(body)
}

/// The one honest sentence, when neither a page nor a line came back.
///
/// Nought-or-one again. Checked against the same two functions the content
/// branches use, so it is impossible for this to appear beside a chart or to be
/// missing when there is nothing else on the screen.
fn empty_state(
    attachments: AttachmentsStore,
    id: AttachmentId,
    kind: AttachmentKind,
    pdf: bool,
    body: &str,
    page: u32,
    span: u32,
) -> Vec<String> {
    // A captured page answers this question for itself. `CapturedPageView`
    // knows whether it found a `page.html`, whether it fell back to the text
    // and whether the files were expected to be there, and it says the
    // corresponding one of its own sentences — none of which this function can
    // tell apart from out here.
    if kind == AttachmentKind::CapturedPage {
        return Vec::new();
    }
    let shown = if pdf {
        attachments
            .directory(id)
            .and_then(|directory| crate::pdf::pages::cached_page(&directory, page))
            .is_some()
    } else {
        !body.trim().is_empty()
    };
    if shown {
        return Vec::new();
    }
    vec![nothing_to_show(kind, page, span)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(kind: AttachmentKind, pages: Option<u32>) -> Attachment {
        Attachment {
            id: 1,
            kind,
            title: "tab.pdf".into(),
            bytes_on_disk: 4096,
            page_count: pages,
            source_url: None,
            captured_at: None,
            body: None,
        }
    }

    // -- Page counts --------------------------------------------------------

    #[test]
    fn a_pdf_has_as_many_pages_as_the_import_counted() {
        assert_eq!(page_span(&chart(AttachmentKind::Pdf, Some(6))), 6);
    }

    /// A PDF hayro could not parse is stored with no page count — `crate::pdf`
    /// keeps the file anyway — and the viewer must still open it. One page,
    /// which then fails to draw and says so.
    #[test]
    fn a_pdf_with_no_page_count_is_one_page_rather_than_none() {
        assert_eq!(page_span(&chart(AttachmentKind::Pdf, None)), 1);
        assert_eq!(page_span(&chart(AttachmentKind::Pdf, Some(0))), 1);
    }

    /// Typed text and a captured page are one flow, whatever is in the column.
    #[test]
    fn text_and_captures_are_a_single_page_even_if_a_count_got_written() {
        assert_eq!(page_span(&chart(AttachmentKind::Text, Some(9))), 1);
        assert_eq!(page_span(&chart(AttachmentKind::CapturedPage, Some(9))), 1);
    }


    // -- Geometry -----------------------------------------------------------

    /// The K28 workaround, as arithmetic: a US Letter page out of D4's cache,
    /// in the 393 px window the designs assume, comes out 393 x 509 — and both
    /// numbers are stated, because `width: 100%` would lay this node out at the
    /// bitmap's own 1398 px.
    #[test]
    fn an_upright_page_fits_the_column_and_keeps_its_shape() {
        let box_ = page_box((1080, 1398), 393, 100, 0);
        assert_eq!(box_.image, (393, 509));
        assert_eq!(box_.frame, box_.image, "upright, the frame is the page");
        assert_eq!(box_.degrees, 0);
    }

    /// Zoom is the same page, wider. Nothing is re-rendered — see
    /// `attachment_viewer`'s header for why the softness that buys is the trade
    /// the reader asked for.
    #[test]
    fn zooming_multiplies_both_axes_and_leaves_the_shape_alone() {
        let hundred = page_box((1080, 1398), 393, 100, 0);
        let two_hundred = page_box((1080, 1398), 393, 200, 0);
        assert_eq!(two_hundred.image.0, 786);
        assert_eq!(two_hundred.image.0, hundred.image.0 * 2);
        // Within a pixel of double, allowing for each having rounded up once.
        assert!(two_hundred.image.1.abs_diff(hundred.image.1 * 2) <= 1);
    }

    /// On its side, the column constrains the page's *height*, because that is
    /// the edge now running across the screen. A portrait page turned a quarter
    /// turn is a landscape frame, and if the frame did not swap with it the
    /// `transform` — which moves paint, not layout — would hang the page over
    /// its own box by the difference.
    #[test]
    fn a_turned_page_swaps_which_axis_the_column_binds() {
        let turned_ = page_box((1080, 1398), 393, 100, 1);
        assert_eq!(turned_.image.1, 393, "the image's height spans the column");
        assert_eq!(turned_.image.0, 304);
        assert_eq!(turned_.frame, (393, 304), "and the frame is landscape");
        assert_eq!(turned_.degrees, 90);
    }

    /// Half a turn is upright again, so the geometry is the untuned geometry
    /// with a different `rotate()`. This is the case a naive "swap on any
    /// rotation" would get wrong.
    #[test]
    fn a_half_turn_is_the_same_box_upside_down() {
        let upright = page_box((1080, 1398), 393, 100, 0);
        let inverted = page_box((1080, 1398), 393, 100, 2);
        assert_eq!(inverted.image, upright.image);
        assert_eq!(inverted.frame, upright.frame);
        assert_eq!(inverted.degrees, 180);
    }

    /// A page only stops being centred once it is too wide to be — which is
    /// the state that made the left of a zoomed chart unreachable on `:99`
    /// before `wider_than` existed.
    #[test]
    fn a_page_is_centred_until_it_is_wider_than_the_screen() {
        assert!(!page_box((1080, 1398), 393, 100, 0).wider_than(393));
        assert!(page_box((1080, 1398), 393, 150, 0).wider_than(393));
        // Turned on its side it is the *frame* that has to be measured, not
        // the image: at 300 % the image is 1179 px tall and 911 px wide, and
        // only one of those is across the screen.
        let turned_ = page_box((1080, 1398), 393, 300, 1);
        assert_eq!(turned_.frame.0, 1179);
        assert!(turned_.wider_than(393));
    }

    /// `page_pixels` rejects a zero axis, so this cannot arrive from the cache
    /// — but the arithmetic is public and must not divide by zero for anyone.
    #[test]
    fn a_page_with_no_pixels_is_a_box_with_no_size_rather_than_a_panic() {
        assert_eq!(page_box((0, 0), 393, 100, 0).image, (0, 0));
        assert_eq!(page_box((1080, 0), 393, 100, 1).frame, (0, 0));
    }

    // -- What is drawn when there is nothing to draw ------------------------

    /// The page that could not be drawn says which page it was, because in a
    /// six-page chart "this could not be drawn" beside a `4 / 6` is a sentence
    /// about the wrong thing.
    #[test]
    fn a_page_that_will_not_render_says_which_page_it_was() {
        assert_eq!(
            nothing_to_show(AttachmentKind::Pdf, 4, 6),
            "Page 4 of this PDF could not be drawn."
        );
        assert_eq!(
            nothing_to_show(AttachmentKind::Pdf, 1, 1),
            "This PDF could not be drawn."
        );
    }

    /// A typed chart has its own reason for being empty and gets its own words,
    /// the same argument `song_detail::note` makes about the card.
    #[test]
    fn an_empty_typed_chart_says_why_it_is_empty() {
        assert_eq!(nothing_to_show(AttachmentKind::Text, 1, 1), "Nothing typed yet.");
    }

    /// The third kind no longer answers here at all. E5 gave a captured page
    /// its own component, and that component is the only thing that can tell a
    /// deleted `assets/` directory apart from a `page.html` with nothing in it
    /// — so it owns both sentences and this file asks it nothing.
    #[test]
    fn a_captured_page_is_not_this_file_s_empty_state_to_write() {
        let attachments = AttachmentsStore::new(Vec::new());
        assert!(
            empty_state(
                attachments,
                1,
                AttachmentKind::CapturedPage,
                false,
                "",
                1,
                1,
            )
            .is_empty()
        );
        assert_eq!(captured_of(AttachmentKind::CapturedPage, 7), vec![7]);
        assert!(captured_of(AttachmentKind::Text, 7).is_empty());
    }

    /// A blank line is a line. The gap between a verse and a chorus is part of
    /// how a chart is read, and an empty `div` has no height.
    #[test]
    fn blank_lines_in_a_chart_keep_their_height() {
        let lines = chart_lines("G      C\n\nlyrics here");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].1, " ");
        assert_eq!(lines[0].1, "G      C", "and the chord spacing is untouched");
    }

    // -- The two type sizes -------------------------------------------------

    /// `1o`'s only typographic instruction, as an assertion. The card's preview
    /// is `theme::T_CHART` at 12.5 px and the viewer starts at 14.5; the stand
    /// is read from further away than either.
    #[test]
    fn the_stage_reads_a_chart_larger_than_either_screen_that_came_before_it() {
        assert!(STAGE_CHART_PX > CHART_BASE_PX);
        assert!(CHART_BASE_PX > 12.5, "which is what the card draws");
    }
}
