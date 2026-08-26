//! Setlist detail — HI-FI. The cumulative column down the right edge is the
//! point: a musician reads down it to see where they'll be at any moment.
//!
//! Everything on this screen is derived from the two stores inside a reactive
//! closure rather than computed once at mount. That is not tidiness: the song
//! picker (`1i`) slides up *over* this screen and adds to the very setlist it
//! is showing, so a version of this file that read the store once would send
//! the user back to a set that still claimed five songs after they had added
//! two. See `songs_in_group` in `library.rs` for the same shape — a plain `fn`
//! over `Copy` stores, called from inside the closure that needs it.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{cumulative_starts, prep_facts, total_runtime};
use crate::model::{SetlistId, Song, fmt_duration};
use crate::store::{NavStore, PlaybackStore, Route, SetlistsStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_SECTION_CAPS, T_SETLIST_TITLE};
use crate::ui::{IconButton, icon};

#[component]
pub fn SetlistDetail(id: Option<SetlistId>) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let setlists = use_store::<SetlistsStore>();
    let songs = use_store::<SongsStore>();
    let playback = use_store::<PlaybackStore>();

    let id = id.unwrap_or_default();

    // Checked once, at mount: nothing on this screen can delete the set it is
    // showing, so a set that existed when the route changed still does.
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
                        div {
                            key: song_id,
                            style: "display: flex; align-items: baseline; gap: 10px; padding: 12px 0; \
                                    border-bottom: 1px solid var(--sla-hairline-soft);",
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
                    }
                }

                // Editing happens here rather than on a screen of its own: the
                // picker (`1i`) slides up over this list, so the set stays
                // readable behind while songs are chosen for it.
                div { style: "display: flex; gap: 22px; padding: 14px 0;",
                    span {
                        onclick: move || nav.picking_songs_for.set(Some(id)),
                        style: "color: var(--sla-accent); font-weight: 600; font-size: 14px;",
                        "+ Add songs"
                    }
                    span { style: "color: var(--sla-muted); font-size: 14px;", "Reorder" }
                }

                // Derived, not authored.
                div {
                    style: "background: var(--sla-fill); border-radius: 12px; padding: 13px 15px; margin-bottom: 20px;",
                    div { style: {format!("{T_SECTION_CAPS} color: var(--sla-muted);")}, "Before you start" }
                    div {
                        style: "font-size: 13.5px; line-height: 1.5; color: var(--sla-ink-2); margin-top: 7px;",
                        {move || prep_facts(&ordered(setlists, songs, id))}
                    }
                }
            }

            div {
                style: {format!("padding: 12px {SCREEN_PAD} 24px; border-top: 1px solid var(--sla-hairline);")},
                div {
                    onclick: move || {
                        playback.start(id);
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

/// One drawn row, every field already a string so the loop body has only owned
/// values to hand around.
#[derive(Clone, PartialEq)]
struct Row {
    id: u32,
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
    ordered
        .iter()
        .enumerate()
        .map(|(index, song)| Row {
            id: song.id,
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
