//! Performance mode — WIREFRAME (`1o`), cards F1, F2 and F3.
//!
//! A phone on a music stand, a set being played off it. `1o` draws the whole
//! screen as four things: a thin top bar carrying `2 / 5`, the song's title with
//! `G · capo 2 · 96 bpm` beneath it, and a close; the chart, full-bleed, filling
//! everything else at larger type than the card on song detail; a bottom bar
//! saying what is up next with a keep-awake toggle and a way into the running
//! order; and a progress strip along the very bottom.
//!
//! ## The keep-awake toggle now keeps the screen awake (F3, then F4)
//!
//! It reads and flips [`PlaybackStore::keep_awake`](crate::store::PlaybackStore)
//! and that is still the whole of what *this file* does with it. What changed is
//! at the other end: card F4 landed the thing that reads that signal, so the
//! screen does stay on.
//!
//! F3 shipped this control with nothing behind it and this header said so in as
//! many words — a toggle that promises the screen will stay on and does not is a
//! promise broken in the middle of a gig, which is the worst moment this app
//! has. It shipped anyway because the alternative left the bar's middle cell
//! empty and gave F4 a screen to redesign rather than a signal to read. That bet
//! came off exactly as written: F4 is an `Effect` in [`crate::app`] and not one
//! line here moved.
//!
//! **The effect is up there and not in this file, on purpose.** See
//! [`crate::keep_awake`] for the whole argument; the short version is that this
//! screen has four ways out of it and the release must not be on any of them.
//! The rule is `wanted(route, toggle)` — performance mode *and* the toggle — so
//! leaving by any means at all is a recomputation that comes out `false`, and
//! there is no path to forget because there is no path. A phone still holding
//! its display on an hour after the set is the same bug in the other direction.
//!
//! ## The running order is a bottom sheet, and the wireframe does not draw it
//!
//! `1o` draws the `≡ set` button and stops. F3 decided it opens a bottom sheet,
//! matching the three this app already has (`add_to_setlist`, `sort_sheet`,
//! `setlist_picker`) — same surface, same scrim, same dismiss. See
//! [`super::running_order`], which is where it lives and why it is mounted in
//! `crate::app` rather than nested in this file.
//!
//! ## The chevrons are the control, not a hint (F2)
//!
//! `1o` captions itself *"swipe for next song"* and draws a thin chevron at
//! each edge as the thing that tells you the swipe is there. **The swipe half
//! of that is not buildable on the platform this app ships to**, and not for
//! want of trying: `TouchGesture::process` turns a moving finger into
//! `MouseWheel` deltas and nothing else, the horizontal half of a scroll
//! dispatches no handler at all, and a finger that has moved emits nothing
//! whatever when it lifts — so there is no event a swipe could be *committed*
//! on even if its progress could be watched. `src/gesture_reachability.rs`
//! holds the table, `src/bin/gesture_probe.rs` is the harness that measured it,
//! and card C6 walked into exactly this and shipped explicit controls instead.
//! rinch#266 and #267 do not change the answer; stage 3 — real pointer events
//! with capture — is what would, and it is not written.
//!
//! So the chevrons carry the whole job rather than advertising a gesture that
//! is not there, and that inverts what they have to *be*. A hint can be a 26 px
//! whisker at the edge of the glass; a control that is the only way through the
//! set cannot. See [`CHEVRON_W`] for what that costs and what it buys.
//!
//! It is also why nothing on this screen reaches for `onmousedown`,
//! `onmousemove` or any of the `ondrag*` family. Every one of them is answered
//! by the desktop backend and by no phone, which is precisely the trap
//! `gesture_reachability` exists to catch: a swipe built against them would
//! look finished on this machine and be dead on the stand.
//!
//! ## Theme: this screen is *not* the viewer
//!
//! The obvious thing to do, having just built `1k`, is to give `1o` the same
//! dark chrome regardless of theme. The handoff says otherwise in words:
//! *"performance mode defaults to following the app theme, with a Settings
//! option to force it dark"*. So by default nothing here re-declares a palette —
//! the screen inherits `crate::app`'s tokens like every other screen.
//!
//! That distinction is why `Route::full_screen` and `Route::dark_chrome` are
//! two questions rather than one; see the note on the latter.
//!
//! **The switch exists now (H1), and this screen reads it.** The root below
//! re-declares [`theme::DARK_NEUTRALS`] when — and only when — Settings →
//! Performance mode theme says Always dark, exactly the way the attachment
//! viewer re-declares them unconditionally: custom properties inherit and the
//! nearest declaration wins, so every `var(--sla-*)` under this screen resolves
//! dark without a second palette existing or a hex being written twice.
//!
//! The condition is [`crate::derive::dark_chrome`] and not a comparison written
//! out here, because the same answer is needed in two other places — the strip
//! behind the status bar, and whether Android draws its clock in dark glyphs,
//! both in `crate::app` — and a screen that painted itself dark while the strip
//! above it stayed cream is precisely the seam D5 found and fixed the first
//! time. One rule, three readers, unit-tested against every route the app has.
//!
//! ## The chart is drawn by the same component the viewer uses
//!
//! [`super::chart_surface::ChartSurface`], mounted at [`STAGE_CHART_PX`]
//! instead of the viewer's own base size. Its header argues the seam; the short
//! version is that "draw a chart full-screen" is three attachment kinds, a
//! framework workaround and four empty-state sentences, and having two copies
//! of that would mean fixing the next fault in one of them.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{
    Segment, performance_meta, playing_at, steps, strip_gap, strip_segments, up_next,
};
use crate::model::{AttachmentId, SetlistId, SongId};
use crate::store::{
    AttachmentsStore, NavStore, PlaybackStore, Route, SetlistsStore, SettingsStore, SongsStore,
};
use crate::theme::{DARK_NEUTRALS, T_CHIP, T_META, T_META_SMALL, T_ROW_TITLE};
use crate::ui::icon;

use super::chart_surface::{ChartSurface, STAGE_CHART_PX, page_span};

/// The width of the two cells either side of the title.
///
/// Equal, and stated rather than left to `flex`, because the title in the
/// middle is supposed to be centred **in the bar** and not in whatever the two
/// ends left over. A `flex: 1` centre between two natural-width neighbours
/// centres itself between them instead, which puts the title a few pixels left
/// on `2 / 5` and a few more on `10 / 12` — a title that moves when the song
/// changes, which is exactly the thing a fixed cell prevents.
///
/// 54 px holds `12 / 15` at this type size with room to spare, and it is wide
/// enough for the 40 px touch target the ✕ sits in.
const BAR_CELL: &str = "width: 54px; flex-shrink: 0;";

/// How wide a chevron's tap target is.
///
/// `1o` draws a 26 px strip down each edge, and that is sketch geometry for a
/// *hint*: in the wireframe the swipe was the control and the chevron only had
/// to be seen. It is the control here — see the header — so it has to be *hit*,
/// on a phone, at arm's length, by somebody whose hands are already holding an
/// instrument, and 26 px is under every touch-target floor there is. 48 px
/// clears the usual 44 px minimum with a little to spare and matches the width
/// the rest of this app's bare glyphs already round their boxes to.
const CHEVRON_W: u32 = 48;

/// How tall it is.
///
/// This one goes the *other* way from the wireframe, which runs its strip from
/// 120 px below the top bar to 120 px above the bottom one — very nearly the
/// full height of the chart. Two 480 px-tall targets down the edges of the page
/// would be a wall: on the desktop every click down either side of the chart
/// changes song, and on the phone it is a 48 px-wide band of the chart, twice,
/// that can never be touched for any other reason again — which matters the
/// moment F3 or a later card wants a tap on the chart itself to mean something.
///
/// 120 px is tall enough to forgive a vertical miss by a thumb that is not
/// looking — the whole point of a control on a phone on a stand — and short
/// enough to leave the rest of the edge alone.
const CHEVRON_H: u32 = 120;

/// The glyph inside that box.
///
/// Small for the box it sits in, deliberately. The chart is the thing being
/// read; a chevron that competes with it for the eye has already lost the
/// argument the wireframe's own colour choice makes — it draws the *live* one
/// no heavier than a piece of body text. 22 px is a shade over the 19 px the ✕
/// in the top bar is drawn at, which is the right order between them: the
/// chevrons are glanced at from a metre away and the ✕ is reached for on
/// purpose.
const CHEVRON_PX: u32 = 22;

/// How far a chevron fades when there is nothing that way.
///
/// `1o` says this in colour, drawing the dead chevron at `#c9c5bd` against a
/// live `#1a1a1a` on white. There is no such literal to copy here — this app
/// has tokens, and whatever it does has to be right in both themes. So the live
/// chevron is `--sla-muted`, the register the meta line under the title is
/// already in, which is lighter than the wireframe's ink and is the "do not
/// compete with the chart" half of the instruction honoured in a token; and the
/// dead one is that same colour at this opacity, which lands within a shade of
/// the wireframe's grey on light paper and stays visible-but-inert on dark.
///
/// Faded *and* inert, and both halves matter. A dim chevron keeps its box and
/// swallows its own tap rather than wrapping round to the other end of the set:
/// a › on the last song that jumped back to the first would be indistinguishable
/// from a › that had not registered the tap, which is the argument
/// `attachment_viewer::page_after` already makes about the last page of a PDF.
const CHEVRON_DIM: &str = "0.3";

/// How tall the two chips in the bottom bar are.
///
/// The same 40 px box the ✕ above the chart sits in, and for the same reason
/// its note gives: these are bare-ish controls reached for on purpose, on a
/// phone, by somebody holding an instrument, and 40 px is the touch target the
/// handoff puts a floor under. The wireframe draws them as 12 px text in 3 px of
/// padding — about 22 px tall — which is sketch geometry for something being
/// looked at rather than hit.
const CHIP_H: u32 = 40;

/// How tall the progress strip is.
///
/// Straight from `1o`, which is unusually specific about it, and it survives the
/// move to the hi-fi language unchanged: 6 px is thick enough to hold a colour
/// at a metre and thin enough that nobody reads it as a control to be dragged —
/// which matters, because it is not one. See the strip itself for why.
const STRIP_H: u32 = 6;

/// The room between the strip and the gesture bar's inset below it.
///
/// `1o` draws 10 px there and nothing under it, because a wireframe has no
/// gesture bar. This is that gap; the inset is added to it, not instead of it,
/// so the strip is clear of the pill *and* not welded to it.
const STRIP_GAP_BELOW: u32 = 8;

/// The padding either side of the bar and the strip.
///
/// 14 px rather than the 22 px `SCREEN_PAD` every other screen uses. This bar is
/// the one place in the app where the horizontal room is genuinely contested —
/// two chips and a song title on a 393 px phone — and `1o` itself draws its
/// bottom bar at 14 px while drawing everything else wider. The strip takes the
/// same number so its ends line up with the words above it.
const BAR_PAD: u32 = 14;

/// The sentence for a song in the set that has no chart to put on the stand.
///
/// A real and ordinary state, not a fault: plenty of songs in a working book
/// are three chords somebody has never written down, and they are still in the
/// set and still get played. So this says what is true and nothing more — no
/// "add one" call to action, because the middle of a gig is the one moment a
/// person is definitely not going to type a chart in.
pub const NO_CHART: &str = "No chart for this song.";

/// The sentence for a set with nothing in it.
///
/// Reachable: "Play set" is drawn on the setlist screen whether or not the set
/// has songs, and a set created a minute ago and not filled in yet is the
/// commonest empty one there is.
pub const EMPTY_SET: &str = "This set has no songs in it yet.";

/// The sentence for a set that is not in the library any more.
///
/// The same shape `setlist_detail` uses for the same fact. Deleting the set you
/// are playing is not something anybody does on purpose, but the overflow menus
/// that can do it are two taps from here and the state has to have words and a
/// way out rather than a blank screen.
pub const SET_GONE: &str = "This setlist is gone.";

#[component]
pub fn Performance(setlist: Option<SetlistId>) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let setlists = use_store::<SetlistsStore>();
    let songs = use_store::<SongsStore>();
    let attachments = use_store::<AttachmentsStore>();
    let playback = use_store::<PlaybackStore>();
    // Read for one thing only: whether this screen forces itself dark. See the
    // "Theme" section of the header.
    let settings = use_store::<SettingsStore>();

    let id = setlist.unwrap_or_default();

    // Leaving. Both halves matter and in this order: `stop` clears the playing
    // set so that nothing downstream thinks a gig is still on, and the route
    // lands on the set's own detail screen rather than on `nav.back()`'s tab
    // root — you got here from that screen (or from the card for it on the
    // Setlists tab), and it is where the rest of the set is.
    let close = move || {
        playback.stop();
        nav.go(Route::SetlistDetail(id));
    };

    // How wide the chart column is. The window the app is really in, less the
    // safe-area insets, because `crate::app` pads the root by them and so they
    // are width this screen genuinely does not have.
    //
    // This used to read `crate::WIDTH` — the 393 of the design handoff — with a
    // note saying that the day Rinch handed out a real viewport width, this and
    // the viewer both wanted replacing with it. That is card K31, and this is
    // the replacement. Still copied rather than shared with the viewer: the two
    // screens subtract different things from the same width, and a helper that
    // took a list of what to subtract would be longer than either.
    let safe = crate::platform::safe_area();
    let column = (crate::platform::viewport_width() - safe.left - safe.right).max(1.0) as u32;

    // The page a PDF chart opens on, drawn before the frame that shows it
    // rather than after. Same self-healing job the viewer and `song_detail`
    // both do at mount: a chart imported before D4 landed, or one whose
    // import-time render was interrupted, has a `chart.pdf` and no pages, and
    // this is where it gets them.
    //
    // **From the component body and from the tap handlers, never from a render
    // closure.** That is `pages::cached_page`'s rule and `attachment_viewer`'s
    // header explains it: a closure re-runs on every redraw and a tap handler
    // runs once per tap. F1 called this only at mount and left a note saying the
    // song F2 moves *to* would need it too; `step` below is where that note is
    // paid off, for exactly the reason the viewer's own ‹ / › handlers carry it.
    //
    // It is a free function ([`realise_chart`]) with a closure over it rather
    // than a closure alone, because F3's running-order sheet moves the index
    // too, from a component in another file, and a song jumped to from the sheet
    // needs its first page drawn exactly as much as one stepped to with a
    // chevron. Two copies of that rule would be one copy to forget.
    let realise = move || realise_chart(setlists, songs, attachments, playback, id);
    realise();

    // How long the set actually is, asked fresh every time rather than closed
    // over. A number read once at mount would be the length the set had when
    // the screen opened, and `current` is already careful to count only the
    // songs that still resolve — the two have to be counting the same thing or
    // the last › in the set stops agreeing with the counter above it.
    let playable = move || ordered(setlists, songs, id).len();

    // Which way the set can go from here. Read inside the chevrons' style
    // closures, so the fade follows `playback.index` without this component
    // being rebuilt, and read again inside their tap handlers — see `chevron`.
    let step_state = move || steps(playback.index.get(), playable());

    // Moving. `PlaybackStore::next`/`prev` already clamp at both ends, so this
    // is not the place index arithmetic gets written a second time; `steps`
    // above is the *question* those two cannot be asked, and this is the
    // answer being acted on.
    let step = move |forward: bool| {
        if forward {
            playback.next(playable());
        } else {
            playback.prev();
        }
        realise();
    };

    // The set itself is checked once, at mount, the way `setlist_detail` checks
    // it: nothing on this screen can delete the set it is playing. Everything
    // *inside* the set is read reactively below, because F2 will move through
    // it without leaving the route.
    //
    // A net, not the only answer, since card J7: `crate::app` carries an
    // effect that leaves `Route::Performance` the moment `setlists.get(id)`
    // starts coming back `None`, calling `PlaybackStore::stop` on the way out
    // exactly as `close` below does — so this sentence is what a frame in
    // between looks like, not where the set-deleted-mid-gig story ends.
    if setlists.get(id).is_none() {
        return stranded(__scope, SET_GONE, close);
    }

    rsx! {
        div {
            style: {move || format!(
                "flex: 1; display: flex; flex-direction: column; min-height: 0; {}",
                // Reactive, not decided at mount: dark mode and this setting can
                // both be flipped from Settings while a set is open behind them,
                // and a palette chosen once when the gig started would not
                // follow. Empty in the default case so the screen inherits the
                // app's tokens untouched — see the header.
                if crate::derive::dark_chrome(
                    Route::Performance(id),
                    settings.performance_theme.get(),
                ) {
                    format!("{DARK_NEUTRALS} background: var(--sla-paper); color: var(--sla-ink);")
                } else {
                    String::new()
                },
            )},

            // ── Top bar: 2 / 5 · title / key · ✕ ──────────────────────────
            //
            // Thin, because `1o` asks for thin and because every pixel of it is
            // a pixel of chart. A hairline under it rather than the wireframe's
            // 1.5px ink rule: the wireframe draws every border that way and the
            // hi-fi language does not — `--sla-hairline` is what separates a bar
            // from content everywhere else in this app, and the README is
            // explicit that the hi-fi language wins where the two disagree.
            div {
                style: "display: flex; align-items: center; gap: 8px; \
                        padding: 7px 12px 8px; flex-shrink: 0; \
                        border-bottom: 1px solid var(--sla-hairline);",

                div {
                    style: {format!("{BAR_CELL} {T_META_SMALL}")},
                    {move || current(setlists, songs, playback, id)
                        .map(|now| now.position)
                        .unwrap_or_default()}
                }

                div {
                    style: "flex: 1; min-width: 0; text-align: center;",
                    // The title is centred by a flex row rather than by the
                    // `text-align: center` its parent already sets, and that is
                    // not belt-and-braces — it is a fault seen on the phone on
                    // 2026-09-01 and then fixed. A single `div` carrying both
                    // the centring and `overflow: hidden` came out **left
                    // aligned**, with the meta line directly beneath it
                    // correctly centred by the very same inherited property;
                    // the only difference between the two is that the title
                    // clips and the meta line does not. So the clipping box is
                    // now its own element and the centring is done by the row
                    // around it, which is a mechanism that does not depend on
                    // how a clipped box reports its width.
                    div {
                        style: "display: flex; justify-content: center; min-width: 0;",
                        div {
                            // One line, clipped. A long title that wrapped
                            // would make the bar taller, and a bar that changes
                            // height between two songs moves the chart under
                            // the reader's eye at the exact moment they are
                            // looking down at it.
                            style: {format!(
                                "{T_ROW_TITLE} white-space: nowrap; overflow: hidden; \
                                 text-overflow: ellipsis; min-width: 0;"
                            )},
                            {move || current(setlists, songs, playback, id)
                                .map(|now| now.title)
                                .unwrap_or_default()}
                        }
                    }
                    // Nought-or-one, so a song with no key, no capo and no
                    // tempo gets no second row at all rather than an empty one
                    // — `derive::performance_meta` returns "" for that and this
                    // is the half of the arrangement that honours it.
                    for line in meta_line(setlists, songs, playback, id) {
                        div {
                            key: {line.clone()},
                            style: {format!("{T_META_SMALL} margin-top: 2px;")},
                            {line.clone()}
                        }
                    }
                }

                div {
                    style: {format!("{BAR_CELL} display: flex; justify-content: flex-end;")},
                    {close_button(__scope, close)}
                }
            }

            // ── The stage: the chart, with the chevrons over it ───────────
            //
            // A box of its own rather than the chart sitting straight in the
            // column, because the chevrons are positioned against it. `1o` puts
            // them *on* the chart and not in gutters beside it, and the layer
            // below can only be told `top: 0; bottom: 0` if the thing it is
            // nought from is the chart's box and not the whole screen — with
            // the top bar included they would centre themselves a bar's height
            // too low, and the bar's height is a text metric nobody should be
            // subtracting by hand.
            div {
                style: "flex: 1; min-height: 0; display: flex; flex-direction: column; \
                        position: relative;",

                // ── The chart ─────────────────────────────────────────────
                //
                // A one-element keyed `for` over the song being played, which is
                // the shape the whole rest of this screen already uses and which is
                // what makes F2 safe for free: move `playback.index` and the key
                // changes, so the surface is torn down and rebuilt for the new
                // song. That matters because `ChartSurface` owns the scrolling box
                // — landing on song 3 half way down song 2's chart would be the
                // fault — and it is the same property the viewer wants badly enough
                // to have a comment of its own about. Performance mode passes the
                // page, zoom and rotation as fixed constants rather than signals,
                // so the scroll offset is the whole of the per-chart state there is
                // to lose, and a remount is what loses it.
                for now in current(setlists, songs, playback, id).into_iter().collect::<Vec<_>>() {
                    for chart in now.chart.into_iter().collect::<Vec<_>>() {
                        ChartSurface {
                            key: {format!("{}:{}", now.song, chart)},
                            attachment: {chart},
                            // Read here rather than inside the surface, which is
                            // that component's own note: `AttachmentsStore::body`
                            // is an object read off the database, and the surface
                            // is remounted more often than the song changes.
                            body: {attachments.body(chart).unwrap_or_default()},
                            column: {column},
                            // The one number `1o` asks for in words — "larger type
                            // than the detail preview" — and the whole of what this
                            // screen varies about how a chart is drawn.
                            base_px: {STAGE_CHART_PX},
                            // F1 shows page 1 and only page 1; the span is passed
                            // anyway so that a multi-page PDF that fails to draw
                            // says "Page 1 of this PDF" rather than "This PDF",
                            // which is the sentence that is true of the file.
                            span: {attachments.get(chart).map(|a| page_span(&a)).unwrap_or(1)},
                        }
                    }
                    // A song nobody has written down — three chords somebody has
                    // always just known — said in the same chrome rather than as a
                    // blank screen. The top bar stays above it, so the counter
                    // still says where in the set you are and the ✕ is still where
                    // it was.
                    for sentence in absent(&now) {
                        div {
                            key: {sentence},
                            style: {format!("flex: 1; min-height: 0; {T_META} padding: 48px 22px; text-align: center;")},
                            {sentence}
                        }
                    }
                }

                // An empty set never yields a `now` at all, so its sentence hangs
                // off the outer `for` being empty rather than off a branch inside
                // it. Same nought-or-one shape, one level out.
                for sentence in empty_note(setlists, songs, playback, id) {
                    div {
                        key: {sentence},
                        style: {format!("flex: 1; min-height: 0; {T_META} padding: 48px 22px; text-align: center;")},
                        {sentence}
                    }
                }

                // ── Edge chevrons ─────────────────────────────────────────
                //
                // A layer over the chart rather than two columns beside it: `1o`
                // draws them on the page, and a column would take 96 px of width
                // off every chart in the book to hold two glyphs — on a screen
                // whose one typographic instruction was "bigger", and where a
                // monospaced chart cannot wrap.
                //
                // Three things about this box are load-bearing rather than tidy:
                //
                // * **`z-index`.** `ChartSurface`'s root is `overflow: auto`, which
                //   Rinch treats as a stacking context and hoists into the
                //   z-index-0 phase — the phase that paints last and is hit-tested
                //   first. An unnumbered sibling *after* it in the tree therefore
                //   paints underneath it and never sees a tap. `library`'s FAB
                //   carries the same 10 for the same reason and its note is the
                //   long version; 10 also keeps this below the 40 the bottom
                //   sheets live at.
                // * **`pointer-events: none` here, `auto` on the two boxes.** The
                //   layer spans the whole chart, so without this it would be the
                //   thing every tap on the chart landed on — and the chart is most
                //   of the screen.
                // * **`align-items: center`.** This is what actually centres the
                //   chevrons vertically, and it is flexbox rather than a percentage
                //   `top` because the height to take a percentage of is `flex: 1`
                //   of whatever the top bar left over, which is not a number this
                //   file knows.
                //
                // Scrolling a long chart still works with a finger on a chevron.
                // Rinch resolves a wheel to a scroll container by walking up from
                // the hit node and, when that fails, geometrically at the point
                // (`app/event_dispatch.rs`, and its own comment says the fallback
                // is there for absolutely-positioned overlays exactly like this
                // one); the geometric pass finds the chart's box under the glyph.
                div {
                    style: "position: absolute; left: 0; top: 0; right: 0; bottom: 0; \
                            z-index: 10; pointer-events: none; display: flex; \
                            align-items: center; justify-content: space-between;",
                    {chevron(
                        __scope,
                        TablerIcon::ChevronLeft,
                        move || step_state().back,
                        move || step(false),
                    )}
                    {chevron(
                        __scope,
                        TablerIcon::ChevronRight,
                        move || step_state().forward,
                        move || step(true),
                    )}
                }
            }

            // ── The foot: bottom bar, then progress strip (F3) ────────────
            //
            // One box holding both, and the room for the phone's gesture bar
            // under them as a third — see the spacer at the bottom of it for why
            // that is a box of its own and not padding on this one.
            //
            // **`display: none` when the set has nothing playable in it**, rather
            // than an `if` around the whole box. Two reasons, and the first is
            // the same one `crate::app`'s bottom nav gives for the same trick:
            // the two chips in here are controls, and tearing a control down and
            // rebuilding it is how a control loses a tap — the chevrons carry
            // that note too. The second is what an empty set would make this bar
            // *say*: "up next" with nothing to name, a strip with no segments,
            // and a `set` button opening a sheet listing nothing. The screen
            // already says the true thing in words in the middle of the chart
            // area, and a bar full of blanks under it would be three more ways
            // of saying nothing.
            div {
                style: {move || format!(
                    "display: {}; flex-direction: column; flex-shrink: 0; \
                     border-top: 1px solid var(--sla-hairline);",
                    if playable() == 0 { "none" } else { "flex" },
                )},

                // ── The bar: up next · keep awake · set ───────────────────
                div {
                    style: {format!(
                        "display: flex; align-items: center; gap: 8px; \
                         padding: 6px {BAR_PAD}px;"
                    )},

                    // The left cell takes what is left after the two chips have
                    // had their natural width, and clips rather than wraps: a
                    // long title that pushed the chips out of the bar would move
                    // the two controls somebody reaches for without looking.
                    div {
                        style: "flex: 1; min-width: 0; display: flex; \
                                align-items: baseline; gap: 6px; overflow: hidden;",
                        for up in up_next(playback.index.get(), &ordered(setlists, songs, id)) {
                            // Nought-or-one, and the whole line is one item, so
                            // that "up next Blackbird" and "last song" cannot be
                            // half-drawn between them — see `derive::up_next`
                            // for why the two states are an enum.
                            div {
                                key: {up.label()},
                                style: "display: flex; align-items: baseline; gap: 6px; \
                                        min-width: 0; overflow: hidden;",
                                span {
                                    style: {format!("{T_META_SMALL} flex-shrink: 0;")},
                                    {up.label()}
                                }
                                // The title in ink against the muted label, which
                                // is the whole of `1o`'s two-tone treatment of
                                // this line and is what makes it readable as a
                                // *name* at a glance rather than as a sentence.
                                for title in up.title() {
                                    span {
                                        key: {title.clone()},
                                        style: "font-size: 13px; color: var(--sla-ink); \
                                                white-space: nowrap; overflow: hidden; \
                                                text-overflow: ellipsis; min-width: 0;",
                                        {title.clone()}
                                    }
                                }
                            }
                        }
                    }

                    // The control F4 will make true. It reads and flips the
                    // signal and nothing else — see the module header.
                    {bar_chip(
                        __scope,
                        TablerIcon::Sun,
                        "keep awake",
                        move || playback.keep_awake.get(),
                        move || playback.keep_awake.set(!playback.keep_awake.get()),
                    )}

                    // The way into the running order. It hands the sheet the set
                    // rather than letting it work out which gig is on: the sheet
                    // is mounted out in `crate::app` and this screen is the only
                    // thing that knows the answer for certain.
                    {bar_chip(
                        __scope,
                        TablerIcon::List,
                        "set",
                        || false,
                        move || nav.running_order_for.set(Some(id)),
                    )}
                }

                // ── The progress strip ────────────────────────────────────
                //
                // One segment per song — see `derive::strip_segments` for why
                // not the wireframe's five, and `derive::strip_gap` for what
                // happens to a twenty- or a sixty-song set.
                //
                // It is an indicator and not a control: nothing here takes an
                // `onclick`. A 6 px-tall segment is under every touch-target
                // floor there is, on a screen where a mis-tap changes the chart
                // somebody is playing off, and the thing it would do — jump to a
                // song — already has a control with room for a title and a
                // number in the sheet the `set` chip opens.
                //
                // Every box in here carries its own height, and the room under
                // the strip is a box of its own rather than padding on the bar
                // around it. Both are the fix for a fault seen on the phone on
                // 2026-09-01 and not on the desktop: with the height on the
                // strip's container and the gesture-bar inset as its parent's
                // `padding-bottom`, the segments came out 18 px tall instead of
                // 6 and ran off the bottom of the glass, under where the gesture
                // pill lives. Whatever the engine was doing with the container's
                // height, a `height` on the thing being painted cannot be
                // stretched by it, and a spacer with a height is a box the
                // layout has to reserve rather than padding it can eat.
                div {
                    style: {move || format!(
                        "display: flex; flex-shrink: 0; padding: 0 {BAR_PAD}px; gap: {}px;",
                        strip_gap(playable()),
                    )},
                    for (position, segment) in
                        strip_segments(playback.index.get(), playable()).into_iter().enumerate()
                    {
                        div {
                            key: {position},
                            style: {format!(
                                "flex: 1; min-width: 0; height: {STRIP_H}px; \
                                 border-radius: 3px; background: {};",
                                segment_ink(segment),
                            )},
                        }
                    }
                }

                // The gesture bar's room, and the strip's own breathing space
                // above it. See the note on the strip for why this is a box and
                // not padding: `Route::full_screen` takes away the bottom nav,
                // which is what pads `crate::app` clear of the phone's gesture
                // pill (`safe.bottom.max(NAV_MIN_GAP)` there, and the same `max`
                // here so the desktop preview keeps the design's own gap), so
                // this screen has to reserve that room itself or the strip is
                // drawn under a system control.
                div {
                    style: {format!(
                        "height: {}px; flex-shrink: 0;",
                        STRIP_GAP_BELOW as f32 + safe.bottom.max(crate::NAV_MIN_GAP),
                    )},
                }
            }
        }
    }
}

/// The song on the stand, everything about it the screen draws.
///
/// One value read in one pass, because the four things below it — the counter,
/// the title, the meta line and the chart — all have to be about the *same*
/// song. Four independent reads of `playback.index` would be four chances for
/// them to disagree, and a counter that says `3 / 5` over the chart for song 2
/// is worse than either of them being wrong alone.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Now {
    song: SongId,
    /// `2 / 5`.
    position: String,
    title: String,
    /// `G · capo 2 · 96 bpm`, or empty when the song carries none of the three.
    meta: String,
    /// The song's primary chart, when it has one.
    chart: Option<AttachmentId>,
}

/// The songs of the set that still resolve, in the order the set holds them.
///
/// A song id in the set that no longer resolves is skipped, exactly as
/// `setlist_detail` skips it. Which makes this the one definition of "how long
/// is the set" the screen has, and it has to be: [`current`] picks the song at
/// an index into this list and the chevrons ask only for its length, and if
/// those two ever counted different things the › over the last song in the set
/// would be live while the counter above it read `5 / 5`.
///
/// A set that is not in the library at all is an empty one here. The caller
/// tells the two apart before it gets this far — see the `SET_GONE` check at
/// mount — because they need different sentences and only one of them is a
/// fault.
///
/// `pub(super)` since F3, so the running-order sheet lists exactly this and not
/// a second resolution of the same set. The sheet draws a numbered list, the
/// strip draws a segment per entry and the counter says `2 / 4`; all three are
/// the same list or none of them mean anything.
pub(super) fn ordered(
    setlists: SetlistsStore,
    songs: SongsStore,
    id: SetlistId,
) -> Vec<crate::model::Song> {
    let Some(setlist) = setlists.get(id) else {
        return Vec::new();
    };
    setlist
        .song_ids
        .iter()
        .filter_map(|sid| songs.get(*sid))
        .collect()
}

/// Read the stores for the song being played, or `None` when there is not one.
///
/// `None` covers both a set that is gone and a set with nothing in it; the two
/// are told apart by the caller, which can still ask the store whether the set
/// exists. Everything else is handled rather than reported: a song id that no
/// longer resolves is already gone from [`ordered`], and an `index` past the
/// end of what survives is clamped by [`playing_at`] rather than blanking the
/// screen.
fn current(
    setlists: SetlistsStore,
    songs: SongsStore,
    playback: PlaybackStore,
    id: SetlistId,
) -> Option<Now> {
    let ordered = ordered(setlists, songs, id);
    let (index, position) = playing_at(playback.index.get(), ordered.len())?;
    let song = &ordered[index];
    Some(Now {
        song: song.id,
        position,
        title: song.title.clone(),
        meta: performance_meta(song),
        chart: song.primary(),
    })
}

/// The meta line, as a nought-or-one vector — `rsx!`'s `for` is the only
/// conditional the macro has, and an absent line has to be an absent element
/// rather than an empty one.
fn meta_line(
    setlists: SetlistsStore,
    songs: SongsStore,
    playback: PlaybackStore,
    id: SetlistId,
) -> Vec<String> {
    current(setlists, songs, playback, id)
        .map(|now| now.meta)
        .filter(|meta| !meta.is_empty())
        .into_iter()
        .collect()
}

/// The sentence to draw instead of a chart, when the song has none.
fn absent(now: &Now) -> Vec<&'static str> {
    match now.chart {
        Some(_) => Vec::new(),
        None => vec![NO_CHART],
    }
}

/// The sentence for a set that exists and holds nothing.
///
/// Checked through [`current`] rather than by counting `song_ids`, so that a
/// set holding nothing but ids of deleted songs — which is a set with nothing
/// playable in it — gets the same honest answer as one that was never filled
/// in.
fn empty_note(
    setlists: SetlistsStore,
    songs: SongsStore,
    playback: PlaybackStore,
    id: SetlistId,
) -> Vec<&'static str> {
    match current(setlists, songs, playback, id) {
        Some(_) => Vec::new(),
        None => vec![EMPTY_SET],
    }
}

/// One edge chevron: a bare glyph in a tap target big enough to find.
///
/// `live` is a closure rather than a `bool`, and it is read twice for two
/// different reasons. The style closure reads it on every redraw, so the fade
/// follows the song without this component being rebuilt — the chevrons sit
/// outside the keyed `for` that rebuilds the chart, deliberately, because
/// tearing down a control every time it is used is how a control loses a tap.
/// The tap handler reads it again at the moment of the tap, so a chevron that
/// has *just* gone dim cannot be caught by a finger already on its way down.
///
/// Checking inside the handler rather than by not binding one is the shape
/// `attachment_viewer::ChromeButton` settled on for the same job: `rsx!` builds
/// the props either way, and one branch is easier to be sure of than two
/// arrangements of the tree.
///
/// `onclick` and nothing else. See the module header — the whole family of
/// handlers a swipe would want is unreachable on the platform this ships to,
/// and `src/gesture_reachability.rs` fails the build for anything from it.
fn chevron(
    scope: &mut RenderScope,
    glyph: TablerIcon,
    live: impl Fn() -> bool + Copy + 'static,
    step: impl Fn() + 'static,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            onclick: move || {
                if live() {
                    step()
                }
            },
            style: {move || format!(
                "width: {CHEVRON_W}px; height: {CHEVRON_H}px; flex-shrink: 0; \
                 display: flex; align-items: center; justify-content: center; \
                 color: var(--sla-muted); pointer-events: auto; opacity: {};",
                if live() { "1" } else { CHEVRON_DIM },
            )},
            {icon(__scope, glyph, CHEVRON_PX)}
        }
    }
}

/// One chip in the bottom bar: a glyph, a word, and one state the whole box
/// carries.
///
/// `on` is a closure for the same reason [`chevron`]'s `live` is: the keep-awake
/// chip has to follow its signal without this component being rebuilt, and a
/// `bool` read once at mount would be the state the screen opened in for as long
/// as the gig lasted.
///
/// The state is said in the fill and not in the glyph, which is a choice worth
/// stating because a two-glyph toggle (a sun and a crossed-out sun) was the
/// obvious alternative. Ink-on-paper for on and fill-with-muted-ink for off is
/// the language `ui::Chip` already teaches everywhere else in this app — the
/// filter chips on the library screen are exactly this — and a control on a
/// stage is not the place to teach a second one. It also keeps the glyph a
/// constant, so the chip is never torn down and rebuilt to change a picture.
///
/// **The ink is declared on the glyph and the word, not on the box they sit
/// in**, and that is a fault seen on the phone on 2026-09-01 and then fixed
/// rather than a stylistic preference. With `background` and `color` both on the
/// chip's own reactive style, tapping the toggle repainted the background and
/// left the label the colour it had been: `--sla-paper` text on `--sla-fill`,
/// which is very nearly invisible and looked for all the world like the tap had
/// half-registered. The rule it implies is the one `crate::app`'s nav item
/// already obeys by accident — a child whose *own* style closure re-runs
/// re-resolves its colour, a bare text node under a restyled parent does not —
/// so every element carrying ink here has a closure of its own.
///
/// `onclick` and nothing else; see the module header and
/// `src/gesture_reachability.rs`.
fn bar_chip(
    scope: &mut RenderScope,
    glyph: TablerIcon,
    label: &'static str,
    on: impl Fn() -> bool + Copy + 'static,
    tap: impl Fn() + 'static,
) -> NodeHandle {
    let ink = move || {
        if on() {
            "var(--sla-paper)"
        } else {
            "var(--sla-ink-2)"
        }
    };
    let __scope = scope;
    rsx! {
        div {
            onclick: move || tap(),
            style: {move || format!(
                "{T_CHIP} height: {CHIP_H}px; padding: 0 13px; border-radius: 999px; \
                 display: flex; align-items: center; gap: 6px; flex-shrink: 0; \
                 white-space: nowrap; background: {};",
                if on() { "var(--sla-ink)" } else { "var(--sla-fill)" },
            )},
            span {
                style: {move || format!("display: flex; align-items: center; color: {};", ink())},
                {icon(__scope, glyph, 15)}
            }
            span {
                style: {move || format!("color: {};", ink())},
                {label}
            }
        }
    }
}

/// What one segment of the progress strip is painted in.
///
/// The handoff says this in words — *"played segments solid ink, current segment
/// accent, upcoming muted"* — and every one of the three is a token rather than
/// a hex, so the strip is right in both themes without a second palette. The
/// wireframe's own `#ddd9d1` for an upcoming segment is `--sla-hairline` in the
/// hi-fi set: the lightest thing this app draws that is still a thing.
pub(super) fn segment_ink(segment: Segment) -> &'static str {
    match segment {
        Segment::Played => "var(--sla-ink)",
        Segment::Current => "var(--sla-accent)",
        Segment::Upcoming => "var(--sla-hairline)",
    }
}

/// Draw the first page of the current song's chart, if it is a PDF that has
/// never been rendered.
///
/// See the long note at the call site in [`Performance`]: this is
/// `pages::cached_page`'s self-healing pass and it must be called from a
/// component body or a tap handler, **never from a render closure**. It is out
/// here rather than inside the component because the running-order sheet moves
/// the index too, and a song jumped to from the sheet needs its page drawn
/// exactly as much as one stepped to with a chevron.
pub(super) fn realise_chart(
    setlists: SetlistsStore,
    songs: SongsStore,
    attachments: AttachmentsStore,
    playback: PlaybackStore,
    id: SetlistId,
) {
    if let Some(chart) = current(setlists, songs, playback, id).and_then(|now| now.chart)
        && let Some(directory) = attachments.directory(chart)
    {
        crate::pdf::pages::ensure_page(&directory, 1);
    }
}

/// The bare ✕ in the corner of the bar.
///
/// Bare rather than `ui::IconButton`'s filled pill, which is what `1o` draws
/// and is also the right call for the screen: this bar is trying to be as close
/// to not-there as a bar can be, and a raised chip in the corner of it would be
/// the most conspicuous thing above the chart. The 40 px box is kept, because
/// that is the touch target the handoff sets a floor under and a bare glyph
/// needs it more than a visible button does.
fn close_button(scope: &mut RenderScope, close: impl Fn() + 'static) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            onclick: move || close(),
            style: "width: 40px; height: 40px; display: flex; align-items: center; \
                    justify-content: center; flex-shrink: 0; color: var(--sla-ink-2);",
            {icon(__scope, TablerIcon::X, 19)}
        }
    }
}

/// What is left when the set has gone out from under the screen.
///
/// A sentence and a way out, and the way out is the same ✕ the real screen
/// carries — in the same corner, so a person who was already reaching for it
/// finds it where it was. `close` still calls `PlaybackStore::stop` on the way,
/// which matters more here than on the normal path: a playing set that no
/// longer exists is exactly the state nothing else in the app should be left
/// believing in.
fn stranded(scope: &mut RenderScope, sentence: &'static str, close: impl Fn() + 'static) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",
            div {
                style: "display: flex; align-items: center; justify-content: flex-end; \
                        padding: 7px 12px 8px; flex-shrink: 0; \
                        border-bottom: 1px solid var(--sla-hairline);",
                {close_button(__scope, close)}
            }
            div {
                style: {format!("flex: 1; {T_META} padding: 48px 22px; text-align: center;")},
                {sentence}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::derive::{Segment, UpNext, strip_segments, up_next};
    use crate::model::Song;

    /// A set of three songs, one of which has since been deleted out of the
    /// library. This is not a contrived state: the song overflow menu's Delete
    /// is two taps from a setlist and nothing stops it being used while a gig
    /// is on — card J5 is the schema's promise that the *set* survives it.
    fn a_set_missing_its_middle_song() -> (SetlistsStore, SongsStore, SetlistId) {
        let songs = SongsStore::new(vec![
            Song::new(1, "Carolina", "M. Ward"),
            Song::new(3, "Blackbird", "The Beatles"),
        ]);
        let setlists = SetlistsStore::new(Vec::new());
        let id = setlists.add("Friday night");
        setlists.add_song(id, 1);
        // Song 2 was deleted from the library; the set still holds its id.
        setlists.add_song(id, 2);
        setlists.add_song(id, 3);
        (setlists, songs, id)
    }

    /// The one definition of "how long is this set" the screen has, and the
    /// whole reason F3's bar and strip take a list rather than a setlist: a
    /// song that is not in the library any more cannot be played, cannot be
    /// counted, and must not be named as the one coming up.
    #[test]
    fn a_deleted_song_is_not_in_the_running_order() {
        let (setlists, songs, id) = a_set_missing_its_middle_song();
        let set = ordered(setlists, songs, id);
        let titles: Vec<&str> = set.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Carolina", "Blackbird"]);
    }

    /// So the bar names the next song that survives, skipping the hole in the
    /// set rather than saying "up next" about a chart nobody can put up.
    #[test]
    fn up_next_skips_the_hole_a_deleted_song_leaves() {
        let (setlists, songs, id) = a_set_missing_its_middle_song();
        let set = ordered(setlists, songs, id);
        assert_eq!(up_next(0, &set), Some(UpNext::Song("Blackbird".into())));
        // And the second of the two is the last one, even though the set holds
        // three ids — the label has to agree with what can actually be played.
        assert_eq!(up_next(1, &set), Some(UpNext::Last));
    }

    /// And the strip draws a segment per *playable* song, so the two segments
    /// under a two-song stand agree with the `1 / 2` above it. Three segments
    /// here would leave one that could never become the current one.
    #[test]
    fn the_strip_counts_what_survives_and_not_what_the_set_holds() {
        let (setlists, songs, id) = a_set_missing_its_middle_song();
        let len = ordered(setlists, songs, id).len();
        assert_eq!(len, 2);
        assert_eq!(
            strip_segments(0, len),
            [Segment::Current, Segment::Upcoming]
        );
        assert_eq!(strip_segments(1, len), [Segment::Played, Segment::Current]);
    }

    /// A set whose songs have *all* been deleted is an empty set as far as
    /// every one of these is concerned — the screen hides the bar for it and
    /// says so in words over the chart area instead.
    #[test]
    fn a_set_of_nothing_but_deleted_songs_is_an_empty_set() {
        let songs = SongsStore::new(Vec::new());
        let setlists = SetlistsStore::new(Vec::new());
        let id = setlists.add("Friday night");
        setlists.add_song(id, 1);
        setlists.add_song(id, 2);

        let set = ordered(setlists, songs, id);
        assert!(set.is_empty());
        assert_eq!(up_next(0, &set), None);
        assert!(strip_segments(0, set.len()).is_empty());
    }
}
