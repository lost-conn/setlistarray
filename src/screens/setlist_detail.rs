//! Setlist detail — HI-FI. The cumulative column down the right edge is the
//! point: a musician reads down it to see where they'll be at any moment.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

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

    let Some(setlist) = setlists.get(id) else {
        return rsx! {
            div { style: {format!("padding: {SCREEN_PAD}; {T_META}")}, "This setlist is gone." }
        };
    };

    let ordered: Vec<Song> = setlist
        .song_ids
        .iter()
        .filter_map(|sid| songs.get(*sid))
        .collect();
    let total: u32 = ordered.iter().filter_map(|s| s.duration).sum();
    let prep = prep_facts(&ordered);
    let header_meta = {
        let mut parts = vec![format!("{} songs", ordered.len()), fmt_duration(total)];
        if let Some(d) = setlist.last_played {
            parts.push(format!("played {}", d.short()));
        }
        parts.join(" · ")
    };

    // Cumulative start times, computed alongside the rows.
    let mut running = 0u32;
    let rows: Vec<(usize, Song, u32)> = ordered
        .iter()
        .enumerate()
        .map(|(i, song)| {
            let start = running;
            running += song.duration.unwrap_or(0);
            (i, song.clone(), start)
        })
        .collect();

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

            div { style: "padding: 2px 18px 8px; display: flex; align-items: center; gap: 6px;",
                IconButton { glyph: TablerIcon::ChevronLeft, size: 19, onclick: move || nav.back() }
                div { style: "flex: 1;" }
                IconButton { glyph: TablerIcon::Pencil, size: 17, onclick: move || {} }
                IconButton { glyph: TablerIcon::DotsVertical, size: 17, onclick: move || {} }
            }

            div { style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD};")},

                div { style: {format!("{T_SETLIST_TITLE}")}, {setlist.name.clone()} }
                div { style: {format!("{T_META} margin-top: 6px;")}, {header_meta.clone()} }

                div { style: "margin-top: 12px;",
                    for (index, song, start) in rows.clone() {
                        // `Artist · key · capo`, skipping whatever is unset.
                        let has_chart = song.has_chart();
                        let row_meta = {
                            let mut parts = vec![song.artist.clone()];
                            if let Some(k) = &song.key { parts.push(k.clone()); }
                            if let Some(c) = song.capo { parts.push(format!("capo {c}")); }
                            parts.join(" · ")
                        };
                        div {
                            key: {song.id},
                            style: "display: flex; align-items: baseline; gap: 10px; padding: 12px 0; \
                                    border-bottom: 1px solid var(--sla-hairline-soft);",
                            span {
                                style: "width: 15px; flex-shrink: 0; color: var(--sla-accent); \
                                        font-weight: 600; font-size: 13px;",
                                {format!("{}", index + 1)}
                            }
                            div { style: "flex: 1; min-width: 0;",
                                div {
                                    style: "font-family: var(--sla-font-display); font-weight: 500; \
                                            font-size: 18px; line-height: 1.25;",
                                    {song.title.clone()}
                                }
                                div { style: {format!("{T_META} margin-top: 2px;")}, {row_meta.clone()} }
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
                                div { style: "font-weight: 500; font-size: 13px;",
                                    {song.duration.map(fmt_duration).unwrap_or_default()}
                                }
                                div { style: {format!("{T_META_SMALL} margin-top: 2px;")},
                                    {fmt_duration(start)}
                                }
                            }
                        }
                    }
                }

                div { style: "display: flex; gap: 22px; padding: 14px 0;",
                    span { style: "color: var(--sla-accent); font-weight: 600; font-size: 14px;", "+ Add songs" }
                    span { style: "color: var(--sla-muted); font-size: 14px;", "Reorder" }
                }

                // Derived, not authored.
                div {
                    style: "background: var(--sla-fill); border-radius: 12px; padding: 13px 15px; margin-bottom: 20px;",
                    div { style: {format!("{T_SECTION_CAPS} color: var(--sla-muted);")}, "Before you start" }
                    div {
                        style: "font-size: 13.5px; line-height: 1.5; color: var(--sla-ink-2); margin-top: 7px;",
                        {prep.clone()}
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

/// The prep facts: which tunings the set needs, how many songs want a capo,
/// and whether everything is available offline.
fn prep_facts(songs: &[Song]) -> String {
    let mut tunings: Vec<String> = Vec::new();
    for song in songs {
        if let Some(t) = &song.tuning
            && !tunings.contains(t)
        {
            tunings.push(t.clone());
        }
    }
    let capos = songs.iter().filter(|s| s.capo.is_some()).count();
    let missing = songs.iter().filter(|s| !s.has_chart()).count();

    let mut sentences = Vec::new();
    match tunings.len() {
        0 => {}
        1 => sentences.push(format!("Everything is in {}.", tunings[0])),
        _ => sentences.push(format!("You'll need {}.", tunings.join(" and "))),
    }
    match capos {
        0 => {}
        1 => sentences.push("One song wants a capo.".to_string()),
        n => sentences.push(format!("{n} songs want a capo.")),
    }
    match missing {
        0 => sentences.push("Every chart is on this phone and works with no signal.".to_string()),
        1 => sentences.push("One song has no chart attached.".to_string()),
        n => sentences.push(format!("{n} songs have no chart attached.")),
    }
    sentences.join(" ")
}
