//! Setlist detail — HI-FI. The cumulative column down the right edge is the
//! point: a musician reads down it to see where they'll be at any moment.
//!
//! Everything on this screen is derived from the two stores inside a reactive
//! closure rather than computed once at mount. That is not tidiness: the song
//! picker (`1i`) slides up *over* this screen and adds to the very setlist it
//! is showing, so a version of this file that read the store once would send
//! the user back to a set that still claimed five songs after they had added
//! two. See `songs_in_group` in `library.rs` for the same shape — a plain `fn`
//! over `Copy` stores, called from inside the closure that needs it. Reorder
//! mode leans on the same property: a move renumbers the positions and shifts
//! the running clock beneath your finger, because both are read from the store
//! every render rather than baked in at mount.
//!
//! ## Why there is no drag handle and no swipe (card C6)
//!
//! The handoff asks for drag-by-handle and swipe-left-to-remove. Both were
//! spiked first — `src/bin/gesture_probe.rs` and `scripts/gesture-probe.py` —
//! and both are **unreachable on Android**, which is the platform the handoff
//! is written for. Rinch's Android shell turns touch into mouse events through
//! one recogniser (`rinch/src/shell/android_runtime.rs`, `TouchGesture`) that
//! emits `MouseDown` only at *finger-up*, immediately followed by `MouseUp` at
//! the same coordinates, and only when the finger never travelled 8px. A
//! finger that moves becomes `MouseWheel` deltas addressed to the scroll
//! container and nothing else — no `MouseMove`, no button state, and no event
//! at all when it lifts.
//!
//! So a drag can never reach its 5px activation threshold (`ondragstart`
//! cannot fire), and a swipe produces no app-visible signal whatsoever:
//! horizontal wheel deltas move a scroll box without dispatching any handler.
//! Both gestures work on the desktop backend and neither works on a phone,
//! which is the worst possible place to leave a feature — so this screen uses
//! explicit controls instead, which are plain taps and work identically on
//! both. The evidence is in the probe's doc comment.
//!
//! Nothing here is `position: absolute`, deliberately: a lifted row is exactly
//! the shape that K10 kills (a positioned element over an `overflow: auto`
//! sibling paints on top but hit-tests underneath), and with no drag there is
//! nothing to lift.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{cumulative_starts, prep_facts, total_runtime};
use crate::model::{SetlistId, Song, SongId, fmt_duration};
use crate::store::{NavStore, PlaybackStore, Route, SetlistsStore, SettingsStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_SECTION_CAPS, T_SETLIST_TITLE};
use crate::ui::{IconButton, icon};

/// A 40×40 tap target for Move up / Move down. Nothing below 44 in the
/// handoff's own words, and the 40 box sits inside a padded row.
const MOVE_BUTTON: &str = "width: 40px; height: 40px; border-radius: 999px; \
    display: flex; align-items: center; justify-content: center; flex-shrink: 0;";
/// The dead state at either end of the set. Drawn, not hidden: a button that
/// disappears on the first row makes the two that remain change places, and a
/// control that moves while you are aiming at it is worse than one that is
/// visibly unavailable.
const MOVE_DEAD: &str = "background: transparent; color: var(--sla-hairline);";
const MOVE_LIVE: &str = "background: var(--sla-fill); color: var(--sla-ink-2);";

#[component]
pub fn SetlistDetail(id: Option<SetlistId>) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let setlists = use_store::<SetlistsStore>();
    let songs = use_store::<SongsStore>();
    let playback = use_store::<PlaybackStore>();
    // Read for one thing: the keep-awake preference handed to `start` below.
    let settings = use_store::<SettingsStore>();

    let id = id.unwrap_or_default();

    // Reorder mode: the arrows and the Remove button only exist while it is
    // on. A screen-transient bool, so it lives here rather than in `NavStore` —
    // leaving the setlist ends it, which is what unmounting this component
    // already does.
    let reordering = Signal::new(false);

    // Checked once, at mount: nothing on this screen can delete the set it is
    // showing, so a set that existed when the route changed still does.
    //
    // A net, not the only answer, since card J7: `crate::app` carries an
    // effect that leaves `Route::SetlistDetail` (and `Route::Performance`)
    // the moment `setlists.get(id)` starts coming back `None`, wherever the
    // delete came from. What this still catches is the one frame between that
    // delete committing and the effect firing, and a route arrived at with the
    // setlist already gone.
    if setlists.get(id).is_none() {
        return rsx! {
            div { style: {format!("padding: {SCREEN_PAD}; {T_META}")}, "This setlist is gone." }
        };
    }

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

            div { style: "padding: 2px 18px 8px; display: flex; align-items: center; gap: 6px;",
                IconButton { glyph: TablerIcon::ChevronLeft, size: 19, onclick: move || nav.back() }
                div { style: "flex: 1;" }
                IconButton { glyph: TablerIcon::Pencil, size: 17, onclick: move || {} }
                IconButton { glyph: TablerIcon::DotsVertical, size: 17, onclick: move || {} }
            }

            div { style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD};")},

                div { style: {format!("{T_SETLIST_TITLE}")},
                    {move || setlists.get(id).map(|s| s.name).unwrap_or_default()}
                }
                div { style: {format!("{T_META} margin-top: 6px;")},
                    {move || header_meta(setlists, songs, id)}
                }

                div { style: "margin-top: 12px;",
                    for row in rows(setlists, songs, id) {
                        // The `for` body re-runs as its own closure, so it takes
                        // owned copies of everything it draws.
                        let song_id = row.id;
                        let has_chart = row.has_chart;
                        // Everything the reorder strip needs, as `Copy` scalars:
                        // the strip is a nested `if` closure and can capture
                        // nothing else from out here.
                        let index = row.index;
                        let is_first = row.is_first;
                        let is_last = row.is_last;
                        // Two boxes, not one: the hi-fi row is a baseline-aligned
                        // flex row, and a flex item that grows taller drags the
                        // baseline everything else is aligned to with it — put
                        // the reorder controls *inside* the title column and the
                        // position number and the running clock slide to the
                        // bottom of the row with them. So the controls are a
                        // sibling underneath, and the row above is untouched.
                        div {
                            key: song_id,
                            style: "padding: 12px 0; border-bottom: 1px solid var(--sla-hairline-soft);",
                            div {
                                style: "display: flex; align-items: baseline; gap: 10px;",
                                span {
                                    style: "width: 15px; flex-shrink: 0; color: var(--sla-accent); \
                                            font-weight: 600; font-size: 13px;",
                                    {row.position.clone()}
                                }
                                div { style: "flex: 1; min-width: 0;",
                                    div {
                                        style: "font-family: var(--sla-font-display); font-weight: 500; \
                                                font-size: 18px; line-height: 1.25;",
                                        {row.title.clone()}
                                    }
                                    div { style: {format!("{T_META} margin-top: 2px;")}, {row.meta.clone()} }
                                    // A song with no chart says so, here, before
                                    // it matters on stage.
                                    if !has_chart {
                                        div {
                                            style: "display: inline-flex; align-items: center; gap: 5px; \
                                                    margin-top: 6px; background: var(--sla-accent-tint); \
                                                    color: var(--sla-accent-on-tint); border-radius: 6px; \
                                                    padding: 3px 8px; font-weight: 600; font-size: 11.5px;",
                                            {icon(__scope, TablerIcon::AlertCircle, 13)}
                                            "No chart attached"
                                        }
                                    }
                                }
                                div { style: "text-align: right; flex-shrink: 0;",
                                    div { style: "font-weight: 500; font-size: 13px;", {row.duration.clone()} }
                                    div { style: {format!("{T_META_SMALL} margin-top: 2px;")}, {row.start.clone()} }
                                }
                            }
                            // The reorder controls, under the row rather
                            // than beside it: three 40px targets in the
                            // right-hand column would leave a phone-width
                            // title about 140px to live in, and the running
                            // clock beside it is the one thing this screen
                            // exists to show. Indented past the position
                            // number so they read as belonging to the song
                            // above them.
                            if reordering.get() {
                                div {
                                    style: "display: flex; align-items: center; gap: 4px; \
                                            margin: 8px 0 0 25px;",
                                    div {
                                        onclick: move || {
                                            if !is_first {
                                                setlists.reorder(id, index, index - 1);
                                            }
                                        },
                                        style: {format!(
                                            "{MOVE_BUTTON} {}",
                                            if is_first { MOVE_DEAD } else { MOVE_LIVE }
                                        )},
                                        {icon(__scope, TablerIcon::ChevronUp, 18)}
                                    }
                                    div {
                                        onclick: move || {
                                            if !is_last {
                                                setlists.reorder(id, index, index + 1);
                                            }
                                        },
                                        style: {format!(
                                            "{MOVE_BUTTON} {}",
                                            if is_last { MOVE_DEAD } else { MOVE_LIVE }
                                        )},
                                        {icon(__scope, TablerIcon::ChevronDown, 18)}
                                    }
                                    div { style: "flex: 1;" }
                                    // Destructive, and named rather than
                                    // drawn as a bare glyph: this is the
                                    // control that stands in for a gesture
                                    // nobody could have discovered anyway.
                                    div {
                                        onclick: move || setlists.remove_song(id, song_id),
                                        style: "display: flex; align-items: center; gap: 6px; \
                                                height: 40px; padding: 0 12px; border-radius: 999px; \
                                                background: var(--sla-fill); color: var(--sla-danger); \
                                                font-weight: 600; font-size: 13px;",
                                        {icon(__scope, TablerIcon::Trash, 15)}
                                        "Remove"
                                    }
                                }
                            }
                        }
                    }

                    // Card J4: a set holding no songs used to draw nothing
                    // here at all — the column between the header and
                    // "+ Add songs" simply had no rows in it, which reads as
                    // a screen that has not finished loading rather than as
                    // a set nobody has filled in yet. Reachable two ways: a
                    // freshly created set (the FAB on the Setlists tab makes
                    // one with nothing in it), and J5's own promise that
                    // deleting every song a set held leaves the set itself
                    // behind, empty rather than gone. `+ Add songs` sits one
                    // line below this and already is the way out, so this
                    // says the fact and stops rather than repeating that
                    // control a second time.
                    if ordered(setlists, songs, id).is_empty() {
                        div {
                            style: {format!("{T_META_SMALL} text-align: center; padding: 30px 0;")},
                            "Nothing in this set yet."
                        }
                    }
                }

                // Editing happens here rather than on a screen of its own: the
                // picker (`1i`) slides up over this list, so the set stays
                // readable behind while songs are chosen for it.
                div { style: "display: flex; gap: 22px; padding: 14px 0; align-items: center;",
                    span {
                        onclick: move || nav.picking_songs_for.set(Some(id)),
                        style: "color: var(--sla-accent); font-weight: 600; font-size: 14px;",
                        "+ Add songs"
                    }
                    // The handoff's own affordance, now load-bearing: it opens
                    // the mode the arrows and Remove live in. Hidden while the
                    // set is empty — there is nothing to order.
                    if !ordered(setlists, songs, id).is_empty() {
                        span {
                            onclick: move || reordering.set(!reordering.get()),
                            style: {move || {
                                if reordering.get() {
                                    "color: var(--sla-accent); font-weight: 600; font-size: 14px;"
                                } else {
                                    "color: var(--sla-muted); font-size: 14px;"
                                }
                            }},
                            {move || if reordering.get() { "Done" } else { "Reorder" }}
                        }
                    }
                }

                // One line, only while the mode is on, saying the thing a
                // musician about to press Remove wants to know. J5's rule is
                // the schema's promise; this is where it gets said out loud.
                if reordering.get() {
                    div {
                        style: {format!("{T_META_SMALL} margin: -6px 0 10px;")},
                        "Move songs with the arrows. Removing one here takes it out of this set only — the song stays in your book."
                    }
                }

                // Derived, not authored. Hidden for an empty set: `prep_facts`
                // returns an empty string for one rather than the misleading
                // "every chart is on this phone" that an empty tunings/capo/
                // missing-chart count used to add up to — see that function's
                // own header, which card J4 rewrote for exactly this case.
                if !prep_facts(&ordered(setlists, songs, id)).is_empty() {
                    div {
                        style: "background: var(--sla-fill); border-radius: 12px; padding: 13px 15px; margin-bottom: 20px;",
                        div { style: {format!("{T_SECTION_CAPS} color: var(--sla-muted);")}, "Before you start" }
                        div {
                            style: "font-size: 13.5px; line-height: 1.5; color: var(--sla-ink-2); margin-top: 7px;",
                            {move || prep_facts(&ordered(setlists, songs, id))}
                        }
                    }
                }
            }

            // The undo offer, outside the scroll box so it cannot be scrolled
            // away from. What a removal actually costs is the *position* — the
            // song was never at risk — and the picker can only put a song back
            // on the end, so without this, undoing a mis-tap on song two of
            // twenty is a dozen taps of Move up.
            if pending_undo(setlists, id) {
                div {
                    style: {format!(
                        "display: flex; align-items: center; gap: 12px; \
                         padding: 11px {SCREEN_PAD}; background: var(--sla-fill); \
                         border-top: 1px solid var(--sla-hairline);"
                    )},
                    div {
                        style: "flex: 1; min-width: 0; font-size: 13px; color: var(--sla-ink-2); \
                                overflow: hidden;",
                        {move || undo_label(setlists, songs, id)}
                    }
                    // `accent-on-tint`, not `accent` — card J3 found this one
                    // by widening the contrast audit to the whole token
                    // table. Rust's base on `fill` is 4.48:1, which misses
                    // the 4.5:1 the handoff commits to by two hundredths:
                    // invisible in a mockup, and still the one pairing in the
                    // app that broke a promise the design makes in writing.
                    //
                    // Neither hex was the thing to change. `#B54724` is the
                    // published accent and `fill` is an authored neutral, and
                    // the token set already has a colour for exactly this
                    // situation — `accent-on-tint` is defined as "readable
                    // accent-family text on the tint", which is what accent
                    // text on any tinted ground wants. It clears comfortably
                    // in both modes for all four accents. The other three
                    // accents passed on `accent` alone; using the right token
                    // rather than fixing only Rust keeps the four of them the
                    // same control.
                    span {
                        onclick: move || { setlists.undo_removal(); },
                        style: "color: var(--sla-accent-on-tint); font-weight: 600; font-size: 14px; \
                                padding: 6px 4px;",
                        "Undo"
                    }
                }
            }

            div {
                style: {format!("padding: 12px {SCREEN_PAD} 24px; border-top: 1px solid var(--sla-hairline);")},
                div {
                    onclick: move || {
                        // The keep-awake preference is the user's, so it is read at the
                        // moment the set starts rather than left at whatever the last
                        // gig's bottom bar was set to.
                        playback.start(id, settings.keep_awake.get());
                        nav.go(Route::Performance(id));
                    },
                    style: "background: var(--sla-accent); color: var(--sla-on-accent); border-radius: 16px; \
                            padding: 16px; display: flex; align-items: center; justify-content: center; gap: 8px; \
                            font-weight: 600; font-size: 16px; box-shadow: 0 8px 18px -4px rgba(181,71,36,.5);",
                    {icon(__scope, TablerIcon::PlayerPlay, 19)}
                    "Play set"
                }
            }
        }
    }
}

/// The set's songs, in the running order. Takes only `Copy` arguments so it
/// can be called from inside a reactive closure.
fn ordered(setlists: SetlistsStore, songs: SongsStore, id: SetlistId) -> Vec<Song> {
    let Some(setlist) = setlists.get(id) else {
        return Vec::new();
    };
    setlist
        .song_ids
        .iter()
        .filter_map(|sid| songs.get(*sid))
        .collect()
}

/// `5 songs · 17:15 · played Aug 14`, skipping a date the set has never had.
fn header_meta(setlists: SetlistsStore, songs: SongsStore, id: SetlistId) -> String {
    let ordered = ordered(setlists, songs, id);
    let mut parts = vec![
        format!("{} songs", ordered.len()),
        fmt_duration(total_runtime(&ordered)),
    ];
    if let Some(day) = setlists.get(id).and_then(|s| s.last_played) {
        parts.push(format!("played {}", day.short()));
    }
    parts.join(" · ")
}

/// Whether this setlist has a removal still waiting to be undone.
///
/// Scoped to `id` on purpose: the offer lives on the store, one at a time for
/// the whole app, and a strip on this screen must not offer to undo something
/// that happened to a different set.
fn pending_undo(setlists: SetlistsStore, id: SetlistId) -> bool {
    setlists
        .last_removal
        .get()
        .is_some_and(|removal| removal.setlist == id)
}

/// `Removed Blackbird`, or just `Removed a song` if the song has since gone
/// from the library entirely — which a cascade can do while this screen is up.
fn undo_label(setlists: SetlistsStore, songs: SongsStore, id: SetlistId) -> String {
    let Some(removal) = setlists.last_removal.get().filter(|r| r.setlist == id) else {
        return String::new();
    };
    match songs.get(removal.song) {
        Some(song) => format!("Removed {}", song.title),
        None => "Removed a song".to_string(),
    }
}

/// One drawn row, every field already a string so the loop body has only owned
/// values to hand around.
#[derive(Clone, PartialEq)]
struct Row {
    id: SongId,
    /// Where this row sits in the running order — what `reorder` moves.
    index: usize,
    /// The arrows read these rather than comparing against a length the nested
    /// closure cannot see.
    is_first: bool,
    is_last: bool,
    position: String,
    title: String,
    meta: String,
    duration: String,
    /// Seconds from the top of the set — the running clock a musician reads
    /// down to see where they'll be.
    start: String,
    has_chart: bool,
}

fn rows(setlists: SetlistsStore, songs: SongsStore, id: SetlistId) -> Vec<Row> {
    let ordered = ordered(setlists, songs, id);
    let starts = cumulative_starts(&ordered);
    let last = ordered.len().saturating_sub(1);
    ordered
        .iter()
        .enumerate()
        .map(|(index, song)| Row {
            id: song.id,
            index,
            is_first: index == 0,
            is_last: index == last,
            position: format!("{}", index + 1),
            title: song.title.clone(),
            // `Artist · key · capo`, skipping whatever is unset.
            meta: {
                let mut parts = vec![song.artist.clone()];
                if let Some(k) = &song.key {
                    parts.push(k.clone());
                }
                if let Some(c) = song.capo {
                    parts.push(format!("capo {c}"));
                }
                parts.join(" · ")
            },
            duration: song.duration.map(fmt_duration).unwrap_or_default(),
            start: fmt_duration(starts[index]),
            has_chart: song.has_chart(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::store::AttachmentsStore;

    /// Stores wired the way `crate::app` wires them: `SongsStore` holds the
    /// very `SetlistsStore` this screen reads, which is what lets
    /// `SongsStore::delete` reach in and drop a song's id out of the running
    /// order the instant the delete lands (card J5). `SongsStore::new` will
    /// not do here — it mints its own private `SetlistsStore` nobody else can
    /// see, which is fine for a screen that only ever reads one store at a
    /// time and wrong for this one.
    fn linked(songs: Vec<Song>) -> (SongsStore, SetlistsStore) {
        let storage = crate::store::Storage::in_memory();
        let attachments = AttachmentsStore::restored(storage, Vec::new());
        let setlists = SetlistsStore::restored(storage, Vec::new());
        let songs = SongsStore::restored(storage, attachments, setlists, songs);
        (songs, setlists)
    }

    /// A three-song set, open the way this screen would have it open when the
    /// delete in each test below arrives — not a contrived state, but the
    /// Delete two taps away on the song's own overflow menu, reachable while
    /// this exact screen is on top.
    fn a_set_of_three() -> (SongsStore, SetlistsStore, SetlistId) {
        let (songs, setlists) = linked(vec![
            Song::new(1, "Carolina", "M. Ward"),
            Song::new(2, "Ripple", "Grateful Dead"),
            Song::new(3, "Blackbird", "The Beatles"),
        ]);
        let id = setlists.add("Friday set");
        setlists.add_songs(id, &[1, 2, 3]);
        (songs, setlists, id)
    }

    /// The screen's own row list, not just the store's `song_ids` — this is
    /// what would still have shown three rows before J5, since `rows` already
    /// filtered a stale id out of the *positions* it drew even while
    /// `song_ids` itself still held it underneath.
    #[test]
    fn deleting_a_song_mid_session_removes_its_row_from_the_open_screen() {
        let (songs, setlists, id) = a_set_of_three();
        assert_eq!(rows(setlists, songs, id).len(), 3);

        songs.delete(2);

        let after = rows(setlists, songs, id);
        let titles: Vec<SongId> = after.iter().map(|r| r.id).collect();
        assert_eq!(titles, vec![1, 3]);
        // And it is not merely unrenderable: the running order the store
        // holds is one song shorter, not three ids with one that draws
        // nothing.
        assert_eq!(setlists.get(id).unwrap().song_ids, vec![1, 3]);
    }

    /// The bug a stale id actually caused, and the reason "nothing renders
    /// for it" was not the same thing as "harmless": `rows` numbers a row by
    /// its position in the *filtered* list, but the reorder arrows call
    /// `SetlistsStore::reorder` with that same number against the *raw*
    /// `song_ids`. A dead id sitting between two live ones made those two
    /// numbers disagree — row 1's arrow would have moved whatever `song_ids`
    /// held at index 1, which was the dead id, not the song drawn there.
    #[test]
    fn a_reorder_after_a_mid_session_delete_moves_the_song_the_screen_actually_shows() {
        let (songs, setlists, id) = a_set_of_three();
        songs.delete(2);

        let after = rows(setlists, songs, id);
        assert_eq!(after[1].id, 3, "Blackbird is drawn second now");
        assert_eq!(after[1].index, 1, "and its index agrees with song_ids");

        setlists.reorder(id, after[1].index, 0);

        assert_eq!(
            setlists.get(id).unwrap().song_ids,
            vec![3, 1],
            "the move landed on Blackbird, the song under the arrow that was tapped"
        );
    }

    /// The set-of-nothing case: this screen's own early return, exercised
    /// with the linked stores so the empty state still comes from a real
    /// delete rather than an empty set built by hand.
    #[test]
    fn deleting_every_song_in_a_set_leaves_it_open_but_empty() {
        let (songs, setlists) = linked(vec![Song::new(1, "Carolina", "M. Ward")]);
        let id = setlists.add("Friday set");
        setlists.add_song(id, 1);

        songs.delete(1);

        assert!(rows(setlists, songs, id).is_empty());
        assert!(setlists.get(id).is_some(), "the set itself is untouched, only empty");
    }
}
