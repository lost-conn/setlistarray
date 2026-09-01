//! Performance mode — WIREFRAME (`1o`), card F1.
//!
//! A phone on a music stand, a set being played off it. `1o` draws the whole
//! screen as two things: a thin top bar carrying `2 / 5`, the song's title with
//! `G · capo 2 · 96 bpm` beneath it, and a close; and the chart, full-bleed,
//! filling everything else at larger type than the card on song detail.
//!
//! ## What F1 is, and what the wireframe draws that is not in it
//!
//! `1o` also draws edge chevrons, a bottom bar (`up next …`, keep awake, `≡
//! set`) and a five-segment progress strip along the very bottom. None of them
//! are here, and they are absent by decision rather than by oversight: they are
//! cards F2, F3 and F4, and each carries a question this card cannot answer for
//! it — F2 has to say what happens to the swipe the handoff asks for on a
//! platform where a swipe produces no app-visible event at all (`setlist_detail`
//! §"Why there is no drag handle", and `src/bin/gesture_probe.rs` behind it),
//! and F4 needs a keep-awake call Rinch does not expose yet (K5).
//!
//! What that costs this screen is one thing worth naming: **there is no way to
//! move through the set from here yet.** The ✕ is the only control on the
//! screen. That is a smaller state than it sounds — the set opens on its first
//! song, which is the one you are about to play — and it is deliberately not
//! papered over with a temporary next button that F2 would then have to
//! remove from underneath the design.
//!
//! ## Theme: this screen is *not* the viewer
//!
//! The obvious thing to do, having just built `1k`, is to give `1o` the same
//! dark chrome regardless of theme. The handoff says otherwise in words:
//! *"performance mode defaults to following the app theme, with a Settings
//! option to force it dark"*. So nothing here re-declares a palette — the
//! screen inherits `crate::app`'s tokens like every other screen, and the
//! forced-dark setting already has a home in `SettingsStore::performance_theme`
//! waiting for the Settings screen to grow the switch that sets it.
//!
//! That distinction is why `Route::full_screen` and `Route::dark_chrome` are
//! two questions rather than one; see the note on the latter.
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

use crate::derive::{performance_meta, playing_at};
use crate::model::{AttachmentId, SetlistId, SongId};
use crate::store::{AttachmentsStore, NavStore, PlaybackStore, Route, SetlistsStore, SongsStore};
use crate::theme::{T_META, T_META_SMALL, T_ROW_TITLE};
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

    // How wide the chart column is. `crate::WIDTH` is the window the designs
    // assume; the safe-area insets come off it because `crate::app` pads the
    // root by them, so they are width this screen genuinely does not have.
    // Copied from the viewer deliberately rather than shared: the day Rinch
    // hands out a real viewport width, both call sites want replacing with it
    // and neither wants a helper standing in the way of that.
    let safe = crate::platform::safe_area();
    let column = (crate::WIDTH as f32 - safe.left - safe.right).max(1.0) as u32;

    // The page a PDF chart opens on, drawn before the first frame rather than
    // after it. Same self-healing job the viewer and `song_detail` both do at
    // mount: a chart imported before D4 landed, or one whose import-time render
    // was interrupted, has a `chart.pdf` and no pages, and this is where it
    // gets them. **In the component body, never in a render closure** — that is
    // `pages::cached_page`'s rule and `attachment_viewer`'s header explains it.
    //
    // Once F2 can move through the set, the song it moves *to* needs the same
    // call, and it belongs in that tap handler for exactly the same reason the
    // viewer's ‹ / › handlers carry it.
    if let Some(chart) = current(setlists, songs, playback, id).and_then(|now| now.chart)
        && let Some(directory) = attachments.directory(chart)
    {
        crate::pdf::pages::ensure_page(&directory, 1);
    }

    // The set itself is checked once, at mount, the way `setlist_detail` checks
    // it: nothing on this screen can delete the set it is playing. Everything
    // *inside* the set is read reactively below, because F2 will move through
    // it without leaving the route.
    if setlists.get(id).is_none() {
        return stranded(__scope, SET_GONE, close);
    }

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

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

            // ── The chart ─────────────────────────────────────────────────
            //
            // A one-element keyed `for` over the song being played, which is
            // the shape the whole rest of this screen already uses and which
            // gives F2 its seam for free: move `playback.index` and the key
            // changes, so the chart is rebuilt for the new song and the next
            // one starts at the top of its own page rather than wherever the
            // last one was scrolled to. The viewer wants that same property
            // badly enough to have a comment of its own about it.
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

/// Read the stores for the song being played, or `None` when there is not one.
///
/// `None` covers both a set that is gone and a set with nothing in it; the two
/// are told apart by the caller, which can still ask the store whether the set
/// exists. Everything else is handled rather than reported: a song id in the
/// set that no longer resolves is skipped by `filter_map`, exactly as
/// `setlist_detail` skips it, and an `index` past the end of what survives is
/// clamped by [`playing_at`] rather than blanking the screen.
fn current(
    setlists: SetlistsStore,
    songs: SongsStore,
    playback: PlaybackStore,
    id: SetlistId,
) -> Option<Now> {
    let setlist = setlists.get(id)?;
    let ordered: Vec<crate::model::Song> = setlist
        .song_ids
        .iter()
        .filter_map(|sid| songs.get(*sid))
        .collect();
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
