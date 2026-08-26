//! Add to setlist sheet — WIREFRAME (`2e`), styled with the hi-fi token set.
//!
//! One song into several setlists at once. Reachable from song detail's footer
//! button (and, once they exist, from the ⋮ menu and a library long-press) —
//! all of them just set `NavStore::add_to_setlist_for`.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{filter_setlists, setlist_summary};
use crate::model::{SetlistId, SongId};
use crate::store::{NavStore, SetlistsStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_ROW_TITLE};
use crate::ui::{
    SheetFooter, SheetHandle, icon, sheet_panel_style, sheet_root_style, sheet_scrim_style,
};

/// The sheet's share of the window. `2e` draws 472 of 720; on the 393×852
/// viewport the designs assume, that is the handoff's "~57%".
const SHEET_HEIGHT: u32 = 57;

#[component]
pub fn AddToSetlistSheet() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let setlists = use_store::<SetlistsStore>();

    // The sheet is mounted for the life of the app so that it has somewhere to
    // slide from, which means its own state cannot reset on mount. `picked_for`
    // records which song the ticks belong to, so a sheet reopened for a
    // different song reads as empty however it was dismissed.
    let picked: Signal<Vec<SetlistId>> = Signal::new(Vec::new());
    let picked_for: Signal<Option<SongId>> = Signal::new(None);
    let query = Signal::new(String::new());
    // `+ New setlist…` turns into a field in place rather than pushing a screen.
    let naming = Signal::new(false);
    let new_name = Signal::new(String::new());

    let close = move || {
        nav.add_to_setlist_for.set(None);
        query.set(String::new());
        naming.set(false);
        new_name.set(String::new());
    };

    rsx! {
        div {
            style: {move || sheet_root_style(nav.add_to_setlist_for.get().is_some())},

            div {
                onclick: close,
                style: {move || sheet_scrim_style(nav.add_to_setlist_for.get().is_some())},
            }

            div {
                style: {move || sheet_panel_style(nav.add_to_setlist_for.get().is_some(), SHEET_HEIGHT)},

                SheetHandle {}

                div {
                    style: {format!("padding: 2px {SCREEN_PAD} 12px; flex-shrink: 0;")},
                    div { style: {format!("{T_ROW_TITLE} font-size: 20px;")}, "Add to setlist" }
                    div {
                        style: {format!("{T_META} margin-top: 3px;")},
                        {move || song_line(songs, nav)},
                    }
                }

                // Search. Long books have long shelves of sets.
                div {
                    style: {format!("margin: 0 {SCREEN_PAD} 6px; background: var(--sla-fill); border-radius: 14px; \
                                     padding: 10px 13px; display: flex; align-items: center; gap: 9px; flex-shrink: 0;")},
                    span { style: "color: var(--sla-muted); display: flex;", {icon(__scope, TablerIcon::Search, 16)} }
                    input {
                        r#type: "text",
                        style: "flex: 1; border: none; outline: none; background: transparent; \
                                font-family: var(--sla-font-ui); font-size: 14px; color: var(--sla-ink);",
                        placeholder: "Find a setlist",
                        value: {|| query.get()},
                        oninput: move |value: String| query.set(value),
                    }
                }

                div {
                    style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD};")},

                    for row in rows(setlists, songs, nav, query, picked, picked_for) {
                        // The `for` body re-runs as its own closure, so it takes
                        // owned copies of everything it draws.
                        let id = row.id;
                        let already = row.already;
                        let ticked = row.ticked;
                        let box_style = if already {
                            "background: transparent; border: 1.5px solid var(--sla-hairline); color: var(--sla-muted);"
                        } else if ticked {
                            "background: var(--sla-accent); border: 1.5px solid var(--sla-accent); color: var(--sla-on-accent);"
                        } else {
                            "background: transparent; border: 1.5px solid var(--sla-ink-2); color: transparent;"
                        };
                        let name_color = if already { "var(--sla-muted)" } else { "var(--sla-ink)" };

                        div {
                            key: id,
                            onclick: move || {
                                if !already {
                                    toggle(picked, picked_for, nav, id);
                                }
                            },
                            style: "display: flex; align-items: center; gap: 11px; padding: 11px 0; \
                                    border-bottom: 1px solid var(--sla-hairline-soft);",
                            div {
                                style: {format!("{box_style} width: 20px; height: 20px; border-radius: 5px; \
                                                 display: flex; align-items: center; justify-content: center; flex-shrink: 0;")},
                                {icon(__scope, TablerIcon::Check, 13)}
                            }
                            div { style: "flex: 1; min-width: 0;",
                                div {
                                    style: {format!("{T_ROW_TITLE} color: {name_color}; \
                                                     overflow: hidden; text-overflow: ellipsis; white-space: nowrap;")},
                                    {row.name.clone()}
                                }
                                div { style: {format!("{T_META_SMALL} margin-top: 2px;")}, {row.sub.clone()} }
                            }
                        }
                    }

                    // Create inline. The new set is ticked, so one Add puts the
                    // song in it alongside anything else already chosen.
                    if naming.get() {
                        div {
                            style: "display: flex; align-items: center; gap: 11px; padding: 12px 0;",
                            span { style: "color: var(--sla-accent); display: flex;", {icon(__scope, TablerIcon::Plus, 17)} }
                            input {
                                r#type: "text",
                                style: "flex: 1; border: none; outline: none; background: transparent; \
                                        font-family: var(--sla-font-display); font-weight: 500; font-size: 18px; \
                                        color: var(--sla-ink);",
                                placeholder: "Name this set",
                                value: {|| new_name.get()},
                                oninput: move |value: String| new_name.set(value),
                            }
                            div {
                                onclick: move || create(setlists, picked, picked_for, nav, naming, new_name),
                                style: "color: var(--sla-accent); font-weight: 600; font-size: 14px;",
                                "Create"
                            }
                        }
                    } else {
                        div {
                            onclick: move || naming.set(true),
                            style: "display: flex; align-items: center; gap: 9px; padding: 13px 0 20px; \
                                    color: var(--sla-accent); font-weight: 600; font-size: 15px;",
                            {icon(__scope, TablerIcon::Plus, 17)}
                            "New setlist…"
                        }
                    }
                }

                SheetFooter {
                    note: "added at the end of each",
                    action: {|| add_label(picked, picked_for, nav)},
                    enabled: {|| !current_picks(picked, picked_for, nav).is_empty()},
                    onclick: move || {
                        commit(setlists, picked, picked_for, nav);
                        close();
                    },
                }
            }
        }
    }
}

/// `Blackbird — The Beatles`, or nothing while the sheet is sliding away.
fn song_line(songs: SongsStore, nav: NavStore) -> String {
    match nav.add_to_setlist_for.get().and_then(|id| songs.get(id)) {
        Some(song) => format!("{} — {}", song.title, song.artist),
        None => String::new(),
    }
}

/// One drawn row. Computed outside `rsx!` so the loop body has only owned
/// values to hand around.
#[derive(Clone, PartialEq)]
struct Row {
    id: SetlistId,
    name: String,
    sub: String,
    already: bool,
    ticked: bool,
}

/// The rows the sheet shows, recomputed from the stores. Takes only `Copy`
/// arguments so it can be called from inside a reactive closure.
fn rows(
    setlists: SetlistsStore,
    songs: SongsStore,
    nav: NavStore,
    query: Signal<String>,
    picked: Signal<Vec<SetlistId>>,
    picked_for: Signal<Option<SongId>>,
) -> Vec<Row> {
    let Some(song_id) = nav.add_to_setlist_for.get() else {
        return Vec::new();
    };
    let all = songs.songs.get();
    let picks = current_picks(picked, picked_for, nav);
    filter_setlists(setlists.setlists.get(), &query.get())
        .into_iter()
        .map(|setlist| {
            let already = setlist.song_ids.contains(&song_id);
            Row {
                id: setlist.id,
                name: setlist.name.clone(),
                // A set that already holds the song says so instead of
                // repeating a count the user cannot act on.
                sub: if already {
                    "already in this set".to_string()
                } else {
                    setlist_summary(&setlist, &all)
                },
                already,
                ticked: picks.contains(&setlist.id),
            }
        })
        .collect()
}

/// The ticks, but only while they still belong to the song on screen.
fn current_picks(
    picked: Signal<Vec<SetlistId>>,
    picked_for: Signal<Option<SongId>>,
    nav: NavStore,
) -> Vec<SetlistId> {
    let song = nav.add_to_setlist_for.get();
    if song.is_some() && picked_for.get() == song {
        picked.get()
    } else {
        Vec::new()
    }
}

fn toggle(
    picked: Signal<Vec<SetlistId>>,
    picked_for: Signal<Option<SongId>>,
    nav: NavStore,
    id: SetlistId,
) {
    let song = nav.add_to_setlist_for.get();
    if picked_for.get() != song {
        picked_for.set(song);
        picked.set(vec![id]);
        return;
    }
    picked.update(|list| match list.iter().position(|s| *s == id) {
        Some(i) => {
            list.remove(i);
        }
        None => list.push(id),
    });
}

/// `Add`, or `Add to 3` once there is more than one set chosen — the sheet is
/// multi-select and the button is the only place that shows it.
fn add_label(
    picked: Signal<Vec<SetlistId>>,
    picked_for: Signal<Option<SongId>>,
    nav: NavStore,
) -> String {
    match current_picks(picked, picked_for, nav).len() {
        0 | 1 => "Add".to_string(),
        n => format!("Add to {n}"),
    }
}

/// Create the named set, tick it, and leave the sheet open so more can follow.
fn create(
    setlists: SetlistsStore,
    picked: Signal<Vec<SetlistId>>,
    picked_for: Signal<Option<SongId>>,
    nav: NavStore,
    naming: Signal<bool>,
    new_name: Signal<String>,
) {
    let name = new_name.get().trim().to_string();
    if name.is_empty() {
        naming.set(false);
        return;
    }
    let id = setlists.add(name);
    toggle(picked, picked_for, nav, id);
    naming.set(false);
    new_name.set(String::new());
}

/// Songs join at the end of each chosen set. `add_song` is already a no-op for
/// a set that holds the song.
fn commit(
    setlists: SetlistsStore,
    picked: Signal<Vec<SetlistId>>,
    picked_for: Signal<Option<SongId>>,
    nav: NavStore,
) {
    let Some(song) = nav.add_to_setlist_for.get() else {
        return;
    };
    for id in current_picks(picked, picked_for, nav) {
        setlists.add_song(id, song);
    }
    picked.set(Vec::new());
    picked_for.set(None);
}
