//! The full-screen attachment viewer — WIREFRAME (`1k`), card D5.
//!
//! `1k` is drawn once and is the whole specification: a dark top bar with back,
//! the file's name with the song's name beneath it, expand and overflow; the
//! chart on a neutral backdrop; a dark bottom bar with `‹ 1 / 2 ›`, zoom out,
//! zoom in, rotate. The handoff's own caption is the behaviour — *"chrome fades
//! away, tap to bring it back"* — and its README adds the one instruction the
//! drawing cannot carry: the chrome is dark **regardless of theme**, so use the
//! dark mode's `paper` and `ink` rather than the wireframe's `#1a1a1a`.
//!
//! That is done by re-declaring [`theme::DARK_NEUTRALS`] on this screen's own
//! root. Custom properties inherit and the nearest declaration wins, so every
//! `var(--sla-*)` under here — including the ones inside `rinch-components`'
//! dropdown, which this app styles through `menu::MENU_SURFACE` — resolves dark
//! without a hex being written twice or a second set of tokens existing.
//!
//! ## The chrome's state machine, and why a tap only ever brings it back
//!
//! [`Chrome`] is a two-state machine with a generation counter, and it is a
//! plain value with plain methods so that `cargo test` can drive every
//! transition without a window or a timer thread.
//!
//! The counter is the part that is not obvious. Hiding is done by a
//! [`set_timeout`] armed when the chrome appears, and the user can re-arm it —
//! by tapping, by turning a page, by zooming — long before the first one fires.
//! Without a generation, the *original* timer comes back four seconds into a
//! session the user has been interacting with the whole time and takes the
//! chrome away mid-tap. So every rousing bumps the generation, and a timeout
//! only hides the chrome if it is still the live one. `clear_timeout` exists
//! and would also work, but a cancelled handle is a fact about the framework
//! and a stale generation is a fact this file can assert.
//!
//! **A tap on the page restores the chrome and never hides it.** That is the
//! handoff read literally — `1k`'s caption gives the tap exactly one job — and
//! it is also the right call for what this screen is *for*: a chart on a music
//! stand, read at arm's length, tapped at with the hand that is not holding an
//! instrument. Making the tap a toggle, as a photo gallery would, means every
//! mis-aimed reach for `›` blanks the controls the user was reaching for.
//!
//! The other direction is not missing, it is on a button: **⤢ hides the chrome
//! now**. That is the job the wireframe's expand glyph has on a screen that is
//! already full-screen — there is nothing left to expand *into* except the
//! space the bars are sitting in — and it means "get out of my way" is a
//! deliberate act rather than a side effect of touching the picture.
//!
//! ## Rasterising, and the question `docs/PDF.md` §9 left for this card
//!
//! That section closed D4 with the off-thread question unanswered: "one page at
//! 38 ms does not need a thread; a viewer that wants twenty in a row (D5) will
//! have to ask this again." It is asked and answered in
//! [`pages::prefetch`](crate::pdf::pages::prefetch): the page the reader asked
//! for is drawn **synchronously, in the tap handler**, and its neighbours are
//! drawn on a worker that never touches the UI. See that function for why the
//! worker is safe to be this casual about.
//!
//! What matters here is *where* the synchronous render is called from, and it
//! is the same rule D4 wrote into `pages::cached_page`: **never from a render
//! closure.** A closure re-runs on every redraw; a tap handler runs once per
//! tap. So [`ensure_page`](crate::pdf::pages::ensure_page) is called from the
//! component body (for the page the viewer opens on) and from the ‹ / ›
//! handlers (for the page being turned to), and the reactive closure that
//! actually draws the `<img>` only ever calls `cached_page` and `page_pixels`,
//! neither of which can rasterise anything.
//!
//! ## Zoom is a bigger bitmap, not a better one
//!
//! D4 rasterises at a fixed 1080 px and its own header says why that number:
//! "a full-bleed page on a 393 pt-wide phone at 2.75×, which is what D5's
//! viewer needs". So at 100 % this viewer is showing the cache **1:1 with the
//! phone's physical pixels** — nothing is being thrown away and nothing can be
//! gained by re-rendering. Above 100 % the bitmap is upscaled and goes soft.
//!
//! Re-rendering at the zoomed scale would be sharp, and it was rejected on two
//! measurements rather than on taste. It costs 38 ms per zoom step on the
//! device (D4's figure), *and* it multiplies a cache the whole PNG format
//! choice exists to keep small — `docs/PDF.md` priced 300 songs on 44 KiB a
//! page, and a second copy of every viewed page at 200 % is four times the
//! pixels of the first. Against that, what softness actually buys the reader is
//! size: somebody zooming a chord chart is trying to read 8 pt lyrics from a
//! metre away, and a 2× upscale of a 127 dpi render is bigger *and* still
//! legible, which is the trade they are asking for. A sharp-when-zoomed cache
//! is a real improvement and it is a card of its own, not a line in this one.
//!
//! ## Rotate, and the framework gap it shares with zoom
//!
//! Rotation is `transform: rotate()` on the `<img>` with the surrounding box
//! sized to the turned page — [`page_box`] does that arithmetic. Nothing is
//! re-rendered, which is right for the reason rotate exists at all: a chart
//! that arrives on its side is a *scanning* accident, and turning the bitmap
//! back upright loses nothing because the pixels were always upright.
//!
//! Both of those need the page's size in pixels, and so does simply putting it
//! on the screen, because of the Rinch gap D4 hit and card **K28** records:
//! `width: 100%` on an `<img>` lays the node out at the bitmap's *unscaled*
//! height — 1398 px of page hanging out of a 393 px window. Rinch's Taffy
//! measure closure derives the missing axis from the aspect ratio only when the
//! other axis arrives as a `known_dimension`, and a percentage does not survive
//! the content-sizing pass as one.
//!
//! This screen uses `song_detail`'s workaround rather than a third idea: state
//! **both** axes in pixels, computed from the PNG's IHDR via
//! `pages::page_pixels`, so Taffy never calls the measure function at all.
//!
//! It is worth saying plainly why fixing K28 upstream was *not* the smaller job
//! here, since a full-screen viewer is exactly where you would expect it to be.
//! Fixing `width: 100%` would give this screen one axis. It still would not fit
//! a page to a screen: that is a two-axis constraint (CSS's `contain`), it needs
//! the viewport's height, and it needs to know which of the two axes the page is
//! bound by before it can decide what to do. And zoom and rotate both need the
//! same two numbers for their own arithmetic regardless. The fix would remove
//! no line of this file, so it stays where it belongs — a framework bug, filed,
//! fixed on its own card.
//!
//! ## Typed charts are in here too, and the wireframe says so in one glyph
//!
//! `1k` draws `tab.pdf`, but the interaction table routes *the primary
//! attachment card* here and D2 made typed text a chart that can be primary. So
//! a `Text` attachment opens in this viewer, and the wireframe's own bottom bar
//! is the evidence that this was intended: the zoom controls are drawn `A−` and
//! `A+`, which is a type-size control, not a magnifier. On a typed chart that is
//! literally what they do.
//!
//! What differs is only what cannot apply. A typed chart is one flow of text
//! rather than pages, so it reads `1 / 1` with both chevrons dim; and rotating a
//! reflowable column of text is meaningless, so ⟲ is dim as well. Dimmed rather
//! than removed, because the handoff draws one bottom bar and `1o` establishes
//! dimming as this app's way of saying "not from here" — and because a bar whose
//! buttons move depending on what you opened is a bar you have to look at.
//!
//! A captured page (E1/E2) has an extracted body and no pages either, so it
//! lands in the same branch and is drawn as its text. Rendering the captured
//! *markup* is card E5's, and this screen will want revisiting when it exists.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::menu::{AttachmentMenuItems, MENU_SURFACE};
use crate::model::{Attachment, AttachmentId, AttachmentKind, SongId};
use crate::store::{AttachmentsStore, NavStore, Route, SongsStore};
use crate::theme::{DARK_NEUTRALS, T_META, T_META_SMALL};
use crate::ui::icon;

/// How long the chrome stays up with nothing happening, in milliseconds.
///
/// Four seconds. Not authored anywhere in the handoff — `1k` says "fades away"
/// and stops — so it is chosen against the two things it has to sit between: a
/// glance at `2 / 6` to check where you are, which is under a second, and the
/// gap between two deliberate taps by somebody holding a guitar, which is
/// comfortably more than four. Anything under about two seconds takes the
/// controls away while a reader is still deciding which one to press; anything
/// much over five stops being an auto-hide and starts being a bar that happens
/// to disappear eventually.
const CHROME_LINGER_MS: u32 = 4000;

/// The zoom ladder, in percent, coarsest step first.
///
/// Discrete rather than continuous because the input is a button, not a pinch:
/// `A−` / `A+` have to move by an amount somebody can see in one press, and
/// four rungs cover the whole useful range — the page as drawn, and three
/// magnifications of it. 300 % is the top because at that point one page column
/// is three phone-widths across and the reader is panning more than reading;
/// there is nothing below 100 % because the bottom rung already fits the page
/// to the screen and a smaller chart is not a thing anybody wants.
pub const ZOOM_STEPS: [u32; 4] = [100, 150, 200, 300];

/// The type size a typed chart is drawn at, at 100 %, in CSS pixels.
///
/// `theme::T_CHART` renders a chart at 12.5 px inside the card on song detail
/// and `chart_editor` types into it at 13.5 px. This screen is the one place a
/// chart is the *only* thing on the display, so it starts a shade larger again
/// and the zoom ladder goes up from there.
const CHART_BASE_PX: f32 = 14.5;

/// Space kept clear either side of the page at 100 %.
///
/// Zero, and deliberately. D4 picked its 1080 px cache width as "a full-bleed
/// page on a 393 pt-wide phone at 2.75×, which is what D5's viewer needs" — so
/// any inset here would be downscaling the cache by exactly the amount inset
/// and throwing away the pixel-for-pixel match that width was chosen to get.
/// `1k` draws a 14 px gutter, but it draws it around a white page on a *light*
/// grey backdrop, where the page needs an edge to be a page. On this screen's
/// dark backdrop it already has one.
const PAGE_GUTTER: u32 = 0;

/// What the chrome is doing, and which hide-timer is allowed to change it.
///
/// See the module header for why the generation exists. In one line: a timeout
/// armed when the chrome appeared must not hide chrome the user has roused
/// since, and comparing generations is a rule this file can test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chrome {
    /// Whether the two bars are on screen.
    pub shown: bool,
    /// The only hide-timeout that may still act. Every state change bumps it,
    /// which is what makes every timeout armed before that change a no-op.
    pub armed: u32,
}

impl Chrome {
    /// A viewer opens with its chrome up — you have to be able to see where
    /// the back button is before it goes away — and with the first hide timer
    /// armed for generation 1.
    pub fn opening() -> Self {
        Self {
            shown: true,
            armed: 1,
        }
    }

    /// The user did something: tapped the page, turned it, zoomed, rotated.
    ///
    /// The chrome comes back if it was gone and stays if it was there, and
    /// either way the clock restarts. Returns the generation to arm the new
    /// timeout with.
    pub fn roused(&mut self) -> u32 {
        self.shown = true;
        self.armed += 1;
        self.armed
    }

    /// ⤢ — put the chrome away now rather than in four seconds.
    ///
    /// Arms nothing: there is no timer wanted for chrome that is already gone.
    /// The bump is still necessary, because the timeout armed when it appeared
    /// is still out there and must find itself stale rather than hide chrome
    /// the user may have roused in between.
    pub fn dismissed(&mut self) {
        self.shown = false;
        self.armed += 1;
    }

    /// A hide-timeout went off. Ignored unless it is the live one.
    pub fn expired(&mut self, generation: u32) {
        if generation == self.armed {
            self.shown = false;
        }
    }
}

/// How many pages this attachment has, as far as the viewer is concerned.
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

/// The page a move lands on, which is never outside the document.
///
/// Clamping rather than wrapping. A chart is a physical object in the reader's
/// head — page 6 of 6 is the back of it — and a `›` that silently returned to
/// page 1 would look exactly like a `›` that had not registered the tap.
pub fn page_after(page: u32, delta: i32, span: u32) -> u32 {
    let span = span.max(1);
    let moved = page as i64 + delta as i64;
    moved.clamp(1, span as i64) as u32
}

/// The next rung up the zoom ladder, or the top one again.
pub fn zoom_in(percent: u32) -> u32 {
    ZOOM_STEPS
        .iter()
        .copied()
        .find(|step| *step > percent)
        .unwrap_or(ZOOM_STEPS[ZOOM_STEPS.len() - 1])
}

/// The next rung down, or the bottom one again.
pub fn zoom_out(percent: u32) -> u32 {
    ZOOM_STEPS
        .iter()
        .copied()
        .rev()
        .find(|step| *step < percent)
        .unwrap_or(ZOOM_STEPS[0])
}

/// A quarter turn anticlockwise, which is the direction `1k`'s ⟲ points.
pub fn turned(quarter: u8) -> u8 {
    (quarter + 3) % 4
}

/// Where a page goes: the two axes of the `<img>`, the box it occupies once it
/// has been turned, and how far to turn it.
///
/// Both of the first two are needed rather than one. The `<img>`'s own size has
/// to be stated on both axes because of K28 (module header). The frame's size
/// is what the *layout* has to reserve, and it is not the same thing the moment
/// the page is on its side — a `transform` moves paint and hit-testing, and
/// leaves the box where it was, so a page turned 90° inside a box shaped like
/// the untuned one would overhang it by exactly the difference.
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
/// same argument about the card; this is the viewer's version, and it can be
/// more specific because it knows which page was asked for.
pub fn nothing_to_show(kind: AttachmentKind, page: u32, span: u32) -> String {
    match kind {
        // The self-healing render in the component body and in the page
        // handlers has already run and come back with nothing, so unlike the
        // card's version of this sentence there is no "not yet" about it:
        // hayro was handed this page and could not draw it. `docs/PDF.md` is
        // explicit that a 0.x rasteriser will meet a file it cannot draw, and
        // `pages::draw` catches the panic rather than taking the process, so
        // this is the state that catch lands in.
        AttachmentKind::Pdf if span > 1 => {
            format!("Page {page} of this PDF could not be drawn.")
        }
        AttachmentKind::Pdf => "This PDF could not be drawn.".to_string(),
        AttachmentKind::Text => "Nothing typed yet.".to_string(),
        AttachmentKind::CapturedPage => "Saved on this device. No preview yet.".to_string(),
    }
}

/// One bare glyph in the top or bottom bar.
///
/// Not `ui::IconButton`, which wears a `--sla-fill` pill — `1k` draws the chrome
/// as glyphs on flat dark, and a row of raised pills would turn a bar the design
/// wants to disappear into the busiest thing on the screen. The 40 px box is
/// kept, because that is the touch target the handoff sets a floor under and a
/// bare glyph needs it more than a visible button does.
///
/// `gone` is not decoration either, and only the ⋮ passes it. Once
/// `DropdownMenu` has been opened, it hoists its target to the body — the boxes
/// are at the window origin, confirmed through the debug DOM — and an
/// ancestor's `display: none` never reaches it. Its own does, because after the
/// hoist this `div` is the hoisted node's own child rather than the hidden
/// bar's descendant.
#[component]
fn ChromeButton(
    glyph: Option<TablerIcon>,
    size: Option<u32>,
    dim: bool,
    gone: bool,
    onclick: Option<Callback>,
) -> NodeHandle {
    let glyph = glyph.unwrap_or(TablerIcon::Point);
    let size = size.unwrap_or(20);

    rsx! {
        div {
            onclick: move || {
                // A dim control is inert rather than absent, so the bar keeps
                // its shape at both ends of a document. Checked here rather
                // than by not binding the handler, because `rsx!` builds the
                // props either way and one branch is easier to be sure of.
                if !dim && !gone && let Some(cb) = &onclick {
                    cb.invoke()
                }
            },
            style: {format!(
                "width: 40px; height: 40px; display: {}; align-items: center; \
                 justify-content: center; flex-shrink: 0; color: var(--sla-ink); \
                 opacity: {};",
                if gone { "none" } else { "flex" },
                if dim { "0.35" } else { "1" },
            )},
            {icon(__scope, glyph, size)}
        }
    }
}

#[component]
pub fn AttachmentViewer(song: Option<SongId>, attachment: Option<AttachmentId>) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let attachments = use_store::<AttachmentsStore>();

    let song_id = song.unwrap_or_default();
    let id = attachment.unwrap_or_default();

    // Read once, at mount. The row cannot change under this screen except by
    // being deleted, which is a real thing to do from the ⋮ in this screen's own
    // top bar — and [`gone`] is where that lands.
    let (Some(song), Some(chart)) = (songs.get(song_id), attachments.get(id)) else {
        return gone(__scope, nav, song_id);
    };

    let span = page_span(&chart);
    let kind = chart.kind;
    let pdf = kind == AttachmentKind::Pdf;
    let title = chart.title.clone();
    let song_name = song.title.clone();

    // The typed or captured text, fetched once. `AttachmentsStore::body` is an
    // object read off the database and the list signal never carries one, so
    // doing this inside a reactive closure would repeat a megabyte-sized read
    // on every redraw — including every one caused by tapping the picture to
    // bring the chrome back.
    let body = Signal::new(if pdf {
        String::new()
    } else {
        attachments.body(id).unwrap_or_default()
    });

    let page = Signal::new(1u32);
    let zoom = Signal::new(ZOOM_STEPS[0]);
    let quarter = Signal::new(0u8);
    let chrome = Signal::new(Chrome::opening());
    let menu_open = Signal::new(false);

    // How wide the page column is. `crate::WIDTH` is the window the designs
    // assume and what `song_detail` already sizes its card page against; the
    // safe-area insets come off it because `crate::app` pads the root by them,
    // so they are width this screen genuinely does not have. On Android the
    // window is whatever the device gives, and 393 is what the moto g stylus
    // 5G reports — the day a device disagrees, the fix is a viewport width from
    // the framework rather than a second guess here.
    let safe = crate::platform::safe_area();
    let column = (crate::WIDTH as f32 - safe.left - safe.right).max(1.0) as u32
        - (2 * PAGE_GUTTER).min(crate::WIDTH - 1);

    // Put the chrome away. The *only* path to `shown == false`, because of the
    // hoisted-target fault written up on the top bar below: an open menu
    // outlives the bar it hangs off, so closing it is part of what hiding the
    // chrome means, not a courtesy alongside it.
    let shut = move |what: &dyn Fn(&mut Chrome)| {
        menu_open.set(false);
        chrome.update(|c| what(c));
    };

    // Arm a hide-timeout for the generation the chrome is currently on.
    //
    // Called both from the component body (for the timer the screen opens with)
    // and from tap handlers. The two are not the same in one respect worth
    // knowing: `set_timeout` cancels a timeout armed *during a render* if the
    // component unmounts first, but one armed inside a click handler has no
    // owning scope and will fire even after the viewer is gone. That is safe
    // rather than merely tolerable — the only thing it does is write to a
    // signal whose scope has been disposed, which this framework treats as a
    // warn-once no-op (the same property `crate::picker` relies on for a file
    // dialog that answers after its screen has been left).
    let arm = move |generation: u32| {
        set_timeout(CHROME_LINGER_MS, move || {
            // `expired` is generation-checked, so a stale timeout closes no
            // menu either — it takes this branch and does nothing at all.
            if chrome.get().armed == generation {
                shut(&|c: &mut Chrome| c.expired(generation));
            }
        });
    };

    // Draw a page if it is not drawn, and line up its neighbours behind it.
    //
    // Synchronous, on the thread that draws, and never from a render closure —
    // see the module header. The prefetch is fire-and-forget and nothing here
    // waits on it.
    let realise = move |target: u32| {
        if !pdf {
            return;
        }
        let Some(directory) = attachments.directory(id) else {
            return;
        };
        crate::pdf::pages::ensure_page(&directory, target);
        crate::pdf::pages::prefetch(&directory, &crate::pdf::pages::neighbours(target, span));
    };

    // The page the viewer opens on, drawn before the first frame rather than
    // after it. Same shape as `SongDetail`'s own mount-time `ensure_page`, and
    // the same self-healing job: a chart imported before D4 landed, or one
    // whose import-time render was interrupted, has a `chart.pdf` and no pages,
    // and this is where it gets them.
    realise(1);
    arm(chrome.get().armed);

    // Everything the user can do that should keep the chrome up.
    let rouse = move || {
        let mut state = chrome.get();
        let generation = state.roused();
        chrome.set(state);
        arm(generation);
    };

    let turn = move |delta: i32| {
        let target = page_after(page.get(), delta, span);
        if target != page.get() {
            realise(target);
            page.set(target);
        }
        rouse();
    };

    rsx! {
        div {
            // The dark chrome, declared once and inherited by everything under
            // it — including the dropdown, which is why the overflow menu comes
            // out dark on a light-themed phone without knowing where it is.
            style: {format!(
                "{DARK_NEUTRALS} flex: 1; display: flex; flex-direction: column; \
                 min-height: 0; background: var(--sla-paper); color: var(--sla-ink);"
            )},

            // ── Top bar: ← · filename / song · ⤢ · ⋮ ───────────────────────
            //
            // Hidden with `display: none`, and left mounted. Both halves of
            // that sentence were arrived at by getting them wrong, on `:99` on
            // 2026-08-28, and both are about `DropdownMenu`:
            //
            // * **Hidden is not enough on its own.** With the menu *open*, the
            //   component hoists its target out of the ancestor chain — the
            //   same hoisting that put its dismiss backdrop above its panel in
            //   K22 — so `display: none` on this bar never reached the ⋮, which
            //   kept a 40x40 box and stayed painted in the corner of the chart
            //   after the chrome faded. The fix is [`shut`] below: the chrome
            //   never goes away without closing the menu first, and a closed
            //   menu hoists nothing.
            // * **Unmounting is worse.** Replacing this with `if
            //   chrome.get().shown { … }` fixed the stray glyph and broke the
            //   menu instead: remounted, the dropdown resolved `bottom-end`
            //   against a target box it no longer had and drew its panel half
            //   off the right edge of the window, tall enough to push the
            //   bottom bar out of the `overflow: hidden` root. A component that
            //   measures its anchor at mount does not want to be mounted four
            //   times a minute.
            div {
                style: {move || format!(
                    "{} align-items: center; gap: 4px; padding: 2px 10px; flex-shrink: 0;",
                    if chrome.get().shown { "display: flex;" } else { "display: none;" }
                )},
                ChromeButton {
                    glyph: TablerIcon::ArrowLeft,
                    size: 20,
                    onclick: move || nav.go(Route::SongDetail(song_id)),
                }
                div { style: "flex: 1; min-width: 0; padding: 0 4px;",
                    div { style: "font-weight: 600; font-size: 15px;", {title.clone()} }
                    // `T_META_SMALL` is muted, which on this screen's dark
                    // neutrals is `#9B9188` — the handoff's own dark-mode muted,
                    // and the wireframe's `opacity: .7` said the same thing in
                    // the only way a wireframe can.
                    div { style: {format!("{T_META_SMALL} margin-top: 1px;")}, {song_name.clone()} }
                }
                ChromeButton {
                    glyph: TablerIcon::Maximize,
                    size: 17,
                    onclick: move || shut(&Chrome::dismissed),
                }
                DropdownMenu {
                    opened_fn: move || menu_open.get(),
                    on_close: move || menu_open.set(false),
                    position: "bottom-end",
                    DropdownMenuTarget {
                        ChromeButton {
                            glyph: TablerIcon::DotsVertical,
                            size: 19,
                            // The one control that has to hide itself; see
                            // `ChromeButton` and the note on the bar above.
                            gone: {move || !chrome.get().shown},
                            onclick: move || { menu_open.update(|v| *v = !*v); rouse(); },
                        }
                    }
                    DropdownMenuDropdown {
                        style: {MENU_SURFACE},
                        AttachmentMenuItems { song: {song_id}, attachment: {id} }
                    }
                }
            }

            // ── The chart ─────────────────────────────────────────────────
            //
            // The tap target for "bring the chrome back" is this whole area,
            // which is most of the screen — the one gesture the handoff names
            // should not need aiming.
            //
            // Both overflow axes scroll. Vertical is the page that is taller
            // than the display, which is most pages; horizontal only appears
            // once the page is zoomed past the column, and Rinch's Android
            // recogniser does produce horizontal deltas (`touch_gesture.rs`),
            // so a zoomed chart can be panned with a finger rather than only
            // read down the middle.
            // The scrolling box is inside a one-element `for` whose key is the
            // page, the zoom and the rotation, which forces the framework to
            // build a **new node** whenever any of those change. That is a
            // strange-looking thing to want, and it is here for a fault found
            // on the phone on 2026-08-28:
            //
            //   zoom to 200 %, drag the page sideways to read the end of a
            //   staff, then zoom back to 100 % — and the page sits half off the
            //   left edge of the screen with no way to bring it back, because
            //   a drag on content that no longer overflows does nothing.
            //
            // The scroll offset is the framework's, on the node, and it is
            // clamped when a *scroll event* arrives rather than when the
            // content it is measured against shrinks. Nothing in this app can
            // reach `set_scroll_left` — the handle belongs to the node, not to
            // the component — so the offset can only be cleared by there being
            // a different node to have one.
            //
            // Rebuilding costs one `<img>`, and it buys a second thing worth
            // having on its own: turning to page 2 now starts at the top of
            // page 2 rather than wherever page 1 was left scrolled to.
            //
            // A framework that clamped every scroll offset after layout would
            // make this unnecessary, and would fix the same latent fault
            // everywhere else in the app. That is a Rinch change, not a viewer
            // change, and it is written up in the card rather than made here.
            for view in [format!("{}:{}:{}", page.get(), zoom.get(), quarter.get())] {
            div {
                key: {view.clone()},
                onclick: rouse,
                style: {format!(
                    "flex: 1; min-height: 0; overflow-y: auto; overflow-x: auto; \
                     display: flex; flex-direction: column; align-items: center; \
                     padding: {};",
                    // A page brings its own margins — it is a picture of a
                    // sheet of paper, and `PAGE_GUTTER` explains why nothing is
                    // added around it. Typed text does not: without this the
                    // first line of a chart sits against the underside of the
                    // top bar and the last against the top of the bottom one.
                    // The bottom is the deeper of the two so the final line
                    // clears the chrome when it comes back.
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
                            // overflows puts half of it outside the scroll
                            // range, where no gesture can reach it.
                            if spread.1.wider_than(column) { "flex-start" } else { "center" },
                        )},
                        img {
                            src: {spread.0.clone()},
                            // Both axes in pixels — K28, module header. The
                            // rotation is paint-only, about the image's own
                            // centre (Rinch's `transform-origin` default), and
                            // the centre is where the flex box above has just
                            // put it, so a quarter turn lands exactly inside
                            // the frame with no translate to get wrong.
                            style: {format!(
                                "width: {}px; height: {}px; flex-shrink: 0; transform: rotate({}deg);",
                                spread.1.image.0, spread.1.image.1, spread.1.degrees
                            )},
                        }
                    }
                }

                for (index, line) in text_of(body, pdf) {
                    div {
                        key: {index},
                        // `white-space: pre` and a monospaced face are what
                        // keep a chord over its syllable — `theme::T_CHART`
                        // makes the same argument. What is deliberately *not*
                        // carried over from it is `overflow: hidden`: the card
                        // on song detail clips a long line rather than widen
                        // itself, and this screen is the one place the whole
                        // line is supposed to be reachable.
                        style: {move || format!(
                            "font-family: var(--sla-font-mono); white-space: pre; \
                             line-height: 1.5; align-self: flex-start; \
                             font-size: {:.1}px;",
                            CHART_BASE_PX * zoom.get() as f32 / 100.0
                        )},
                        {line.clone()}
                    }
                }

                for sentence in empty_state(attachments, id, kind, pdf, body, page, span) {
                    div {
                        key: {sentence.clone()},
                        style: {format!("{T_META} padding: 40px 22px; text-align: center;")},
                        {sentence.clone()}
                    }
                }
            }
            }

            // ── Bottom bar: ‹ · 1 / 2 · › · | · A− · A+ · ⟲ ───────────────
            div {
                style: {move || format!(
                    "{} align-items: center; padding: 2px 8px; flex-shrink: 0; \
                     border-top: 1px solid var(--sla-hairline);",
                    if chrome.get().shown { "display: flex;" } else { "display: none;" }
                )},
                ChromeButton {
                    glyph: TablerIcon::ChevronLeft,
                    size: 20,
                    dim: {move || page.get() <= 1},
                    onclick: move || turn(-1),
                }
                div {
                    style: "flex: 1; text-align: center; font-size: 13px; font-weight: 500;",
                    {move || format!("{} / {span}", page.get())}
                }
                ChromeButton {
                    glyph: TablerIcon::ChevronRight,
                    size: 20,
                    dim: {move || page.get() >= span},
                    onclick: move || turn(1),
                }
                // The wireframe's `|` — a hairline, not a pipe character, so it
                // sits at the height of the glyphs either side of it rather
                // than on the text baseline.
                div {
                    style: "width: 1px; height: 20px; background: var(--sla-hairline); \
                            margin: 0 6px; flex-shrink: 0;",
                }
                ChromeButton {
                    glyph: TablerIcon::ZoomOut,
                    size: 19,
                    dim: {move || zoom.get() <= ZOOM_STEPS[0]},
                    onclick: move || { zoom.update(|z| *z = zoom_out(*z)); rouse(); },
                }
                ChromeButton {
                    glyph: TablerIcon::ZoomIn,
                    size: 19,
                    dim: {move || zoom.get() >= ZOOM_STEPS[ZOOM_STEPS.len() - 1]},
                    onclick: move || { zoom.update(|z| *z = zoom_in(*z)); rouse(); },
                }
                ChromeButton {
                    glyph: TablerIcon::Rotate,
                    size: 19,
                    dim: {!pdf},
                    onclick: move || { quarter.update(|q| *q = turned(*q)); rouse(); },
                }
            }
        }
    }
}

/// What is left when the chart or its song has been deleted out from under the
/// viewer.
///
/// Reachable in one real way: the overflow menu in this screen's own top bar
/// carries `menu::AttachmentMenuItems`, and one of those items is "Remove
/// attachment". Taking a chart away while looking at it is a legitimate thing
/// to do, and the state it lands in has to have a way out of it — which is why
/// this is a bar with a working ← in it rather than the bare sentence
/// `chart_editor` leaves behind for the same case.
fn gone(scope: &mut RenderScope, nav: NavStore, song: SongId) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            style: {format!(
                "{DARK_NEUTRALS} flex: 1; display: flex; flex-direction: column; \
                 background: var(--sla-paper); color: var(--sla-ink);"
            )},
            div {
                style: "display: flex; align-items: center; padding: 2px 10px;",
                ChromeButton {
                    glyph: TablerIcon::ArrowLeft,
                    size: 20,
                    onclick: move || nav.go(Route::SongDetail(song)),
                }
            }
            div { style: {format!("{T_META} padding: 40px 22px; text-align: center;")},
                "This chart is gone."
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
/// `page_pixels` is 24 bytes off the front of a file. That is what makes this
/// safe to call from a render closure, which it is: reading `page`, `zoom` and
/// `quarter` here is exactly what re-runs it when any of the three changes.
fn page_spread(
    attachments: AttachmentsStore,
    id: AttachmentId,
    pdf: bool,
    page: Signal<u32>,
    zoom: Signal<u32>,
    quarter: Signal<u8>,
    column: u32,
) -> Vec<(String, PageBox)> {
    if !pdf {
        return Vec::new();
    }
    let Some(directory) = attachments.directory(id) else {
        return Vec::new();
    };
    let Some(path) = crate::pdf::pages::cached_page(&directory, page.get()) else {
        return Vec::new();
    };
    let Some(pixels) = crate::pdf::pages::page_pixels(&path) else {
        return Vec::new();
    };
    vec![(
        path.to_string_lossy().into_owned(),
        page_box(pixels, column, zoom.get(), quarter.get()),
    )]
}

/// The lines of a typed or captured chart, and nothing at all for a PDF.
fn text_of(body: Signal<String>, pdf: bool) -> Vec<(usize, String)> {
    if pdf {
        return Vec::new();
    }
    chart_lines(&body.get())
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
    body: Signal<String>,
    page: Signal<u32>,
    span: u32,
) -> Vec<String> {
    let shown = if pdf {
        attachments
            .directory(id)
            .and_then(|directory| crate::pdf::pages::cached_page(&directory, page.get()))
            .is_some()
    } else {
        !body.get().trim().is_empty()
    };
    if shown {
        return Vec::new();
    }
    vec![nothing_to_show(kind, page.get(), span)]
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

    // -- The chrome ---------------------------------------------------------

    /// The whole of `1k`'s caption, as a sequence: the chrome starts up, goes
    /// away by itself, and a tap brings it back.
    #[test]
    fn the_chrome_fades_away_and_a_tap_brings_it_back() {
        let mut chrome = Chrome::opening();
        assert!(chrome.shown, "a viewer opens showing where its back button is");

        chrome.expired(chrome.armed);
        assert!(!chrome.shown, "the timer armed at open is the one that hides it");

        let generation = chrome.roused();
        assert!(chrome.shown, "a tap on the page brings it back");
        assert_eq!(generation, chrome.armed, "and re-arms the clock");

        chrome.expired(generation);
        assert!(!chrome.shown, "which then hides it again");
    }

    /// The reason the generation counter exists at all.
    ///
    /// Turn a page two seconds in: the timeout armed when the viewer opened is
    /// still out there and fires at four seconds. Without a generation it takes
    /// the chrome away two seconds after the user last touched it, and the bar
    /// they are paging with disappears under their finger.
    #[test]
    fn a_timeout_armed_before_the_last_tap_does_not_hide_the_chrome() {
        let mut chrome = Chrome::opening();
        let first = chrome.armed;

        let second = chrome.roused();
        assert_ne!(first, second, "every rousing is a new generation");

        chrome.expired(first);
        assert!(chrome.shown, "the stale timeout is ignored");

        chrome.expired(second);
        assert!(!chrome.shown, "the live one is not");
    }

    /// ⤢ is the only thing that hides the chrome on demand, and it has to
    /// invalidate the pending timeout too — otherwise the timer armed when the
    /// chrome appeared is still live, and a tap that brings the chrome back
    /// milliseconds later gets it taken away again by a clock the user never
    /// started.
    #[test]
    fn expand_hides_the_chrome_now_and_leaves_no_live_timer_behind() {
        let mut chrome = Chrome::opening();
        let opening = chrome.armed;

        chrome.dismissed();
        assert!(!chrome.shown);

        let after = chrome.roused();
        chrome.expired(opening);
        assert!(chrome.shown, "the timer from before ⤢ cannot act");

        chrome.expired(after);
        assert!(!chrome.shown);
    }

    /// A tap while the chrome is up does not put it away. See the module
    /// header: `1k` gives the tap one job, and a viewer read at arm's length
    /// must not blank its own controls when somebody reaches past them.
    #[test]
    fn tapping_visible_chrome_keeps_it_and_only_restarts_the_clock() {
        let mut chrome = Chrome::opening();
        let before = chrome.armed;
        let after = chrome.roused();

        assert!(chrome.shown, "still up");
        assert_eq!(after, before + 1, "on a fresh four seconds");
    }

    // -- Page bounds --------------------------------------------------------

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

    /// Both ends clamp rather than wrap. A `›` on the last page that jumped to
    /// the first would be indistinguishable from a `›` that missed the tap.
    #[test]
    fn paging_stops_at_both_covers() {
        assert_eq!(page_after(1, -1, 6), 1);
        assert_eq!(page_after(6, 1, 6), 6);
        assert_eq!(page_after(3, 1, 6), 4);
        assert_eq!(page_after(3, -1, 6), 2);
        assert_eq!(page_after(1, 1, 1), 1, "a one-page chart has nowhere to go");
    }

    /// The bounds hold against a span of zero, which `page_span` cannot
    /// produce but arithmetic taking a `u32` has to survive being handed.
    #[test]
    fn paging_a_document_of_no_pages_stays_on_page_one() {
        assert_eq!(page_after(1, 1, 0), 1);
        assert_eq!(page_after(1, -1, 0), 1);
    }

    // -- Zoom and rotate ----------------------------------------------------

    #[test]
    fn zoom_walks_the_ladder_and_stops_at_both_ends() {
        assert_eq!(zoom_in(100), 150);
        assert_eq!(zoom_in(150), 200);
        assert_eq!(zoom_in(200), 300);
        assert_eq!(zoom_in(300), 300);
        assert_eq!(zoom_out(300), 200);
        assert_eq!(zoom_out(100), 100);
    }

    #[test]
    fn four_presses_of_rotate_come_back_to_where_they_started() {
        let mut quarter = 0u8;
        for _ in 0..4 {
            quarter = turned(quarter);
        }
        assert_eq!(quarter, 0);
        assert_eq!(turned(0), 3, "⟲ points anticlockwise");
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

    /// Zoom is the same page, wider. Nothing is re-rendered — see the module
    /// header for why the softness that buys is the trade the reader asked for.
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

    /// The other two kinds have their own reasons for being empty and get their
    /// own words, the same argument `song_detail::note` makes about the card.
    #[test]
    fn an_empty_chart_of_each_kind_says_why_it_is_empty() {
        assert_eq!(nothing_to_show(AttachmentKind::Text, 1, 1), "Nothing typed yet.");
        assert_eq!(
            nothing_to_show(AttachmentKind::CapturedPage, 1, 1),
            "Saved on this device. No preview yet."
        );
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
}
