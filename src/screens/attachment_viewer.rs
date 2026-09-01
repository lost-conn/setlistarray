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
//! ## The picture is not drawn here any more
//!
//! It was, until card F1. Performance mode (`1o`) needs the same three
//! attachment kinds drawn the same way at a different size, and the one thing
//! this app has never allowed is two implementations of the same picture that
//! can drift apart — so the scrolling box and everything in it moved out to
//! [`super::chart_surface::ChartSurface`], which both screens mount. Its header
//! is where the seam is argued; what stayed here is the chrome, which is the
//! part `1k` and `1o` genuinely do not share. Several of the notes below are
//! still about the drawing, and they stay because they are about *why the
//! viewer asks for what it asks for* — the zoom ladder, the rotation, and the
//! rule about where a rasterise may be called from, all of which are this
//! screen's controls and this screen's responsibility.
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
//! sized to the turned page — [`page_box`](super::chart_surface::page_box) does
//! that arithmetic. Nothing is
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
//! The workaround is `song_detail`'s rather than a third idea: state **both**
//! axes in pixels, computed from the PNG's IHDR via `pages::page_pixels`, so
//! Taffy never calls the measure function at all. It lives in `chart_surface`
//! now, with the drawing it is part of, and is recorded here because this is
//! the card that hit it.
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
//! ## A captured page is drawn by the component the card draws it with
//!
//! Until E5 a captured page landed in the typed-text branch above and was drawn
//! as its extracted body, and the note here said this screen would want
//! revisiting. It does not any more: `captured_page::CapturedPageView` renders
//! the saved `page.html` itself — headings, images, `<pre>` charts with their
//! columns intact — and `song_detail`'s card mounts the *same component* at a
//! smaller type size. That is what makes a captured page the same object in
//! both places, the way one rasterised PNG makes a PDF the same object in both
//! places.
//!
//! Two of this screen's controls therefore mean something slightly different on
//! one. `A−` / `A+` set the type size the page's whole `em` ladder resolves
//! against, so the page scales rather than only its text — the component is
//! remounted by the zoom-keyed `for` below and simply drawn again at the new
//! size. `⟲` stays dim, for the reason it is dim on a typed chart: a reflowable
//! document has no orientation to correct.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::menu::{AttachmentMenuItems, MENU_SURFACE};
use crate::model::{AttachmentId, AttachmentKind, SongId};
use crate::store::{AttachmentsStore, NavStore, Route, SongsStore};
use crate::theme::{DARK_NEUTRALS, T_META, T_META_SMALL};
use crate::ui::icon;

use super::chart_surface::{CHART_BASE_PX, ChartSurface, PAGE_GUTTER, page_span};

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
    // E5 narrowed this from "not a PDF" to "typed": a captured page's text is
    // read by `CapturedPageView`, which needs it only as the fallback for a
    // `page.html` that is not on the device, and reading it here as well would
    // be a second megabyte-sized object read per mount for a string this
    // screen no longer draws.
    let body = Signal::new(if kind == AttachmentKind::Text {
        attachments.body(id).unwrap_or_default()
    } else {
        String::new()
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
            // Drawn by `chart_surface::ChartSurface`, which performance mode
            // (`1o`) mounts as well — see its header for why the seam is where
            // it is. Everything this screen still decides is a number passed
            // into it.
            //
            // The surface is inside a one-element `for` whose key is the
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
                ChartSurface {
                    key: {view.clone()},
                    attachment: {id},
                    // Cloned per rebuild rather than read per rebuild. See
                    // `ChartSurface`'s note on this prop: the read is off the
                    // database and this subtree is rebuilt on every press of
                    // `A+`.
                    body: {body.get()},
                    page: {page.get()},
                    zoom: {zoom.get()},
                    quarter: {quarter.get()},
                    column: {column},
                    // The zoom ladder is a *type* control on a typed or
                    // captured chart — `1k` draws its two zoom buttons as `A−`
                    // and `A+`, which is the wireframe saying so — and a scale
                    // control on a page. One number does both because both are
                    // "the same thing, this much bigger".
                    base_px: {CHART_BASE_PX * zoom.get() as f32 / 100.0},
                    span: {span},
                    // The tap target for "bring the chrome back" is the whole
                    // chart, which is most of the screen — the one gesture the
                    // handoff names should not need aiming.
                    onclick: rouse,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
