//! Setlist song picker — WIREFRAME (`1i`), styled with the hi-fi token set.
//!
//! The mirror image of [`add_to_setlist`](super::add_to_setlist): that sheet
//! puts one song into several sets, this one puts several songs into one set.
//! To the user they are the same idea from opposite ends, so they are built
//! from the same pieces — `sheet_*_style`, `SheetHandle`, `SheetFooter`, the
//! same 20px checkbox, the same "already in this set" wording.
//!
//! **Why a sheet and not a screen.** `1i` was chosen over the two-step wizard
//! (`1g`) and inline search (`1h`) because the set stays on screen behind the
//! picker: you can see what you are adding to while you add to it. That only
//! works if the picker is a layer over the setlist rather than a route, which
//! is why this is mounted at the app root beside the other two sheets and not
//! in the route switch.
//!
//! The slide and the parked-below-the-fold trick are the shared ones in
//! `crate::ui` — including the z-index-40 layer, without which the sheet would
//! paint over the setlist's scroll box and be untappable behind it.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{
    SongFilter, add_songs_label, library_tags, picked_note, picker_empty_note, picker_songs,
};
use crate::model::{Day, SetlistId, SongId};
use crate::store::{NavStore, SetlistsStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_ROW_TITLE};
use crate::ui::{
    Chip, SheetFooter, SheetHandle, icon, sheet_panel_style, sheet_root_style, sheet_scrim_style,
};

/// The sheet's share of the window. `1i` draws 492 of 720; on the 393×852
/// viewport the designs assume, that is 68% — the same height as the sort
/// sheet, and it leaves the top three rows of the set legible behind.
const SHEET_HEIGHT: u32 = 68;

#[component]
pub fn SetlistPicker() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let setlists = use_store::<SetlistsStore>();

    // Mounted for the life of the app so it has somewhere to slide from, which
    // means its state cannot reset on mount. `picked_for` records which set the
    // ticks belong to, so a picker reopened over a different set reads as empty
    // however the last one was dismissed — the same guard `2e` uses.
    let picked: Signal<Vec<SongId>> = Signal::new(Vec::new());
    let picked_for: Signal<Option<SetlistId>> = Signal::new(None);
    let query = Signal::new(String::new());
    let filter = Signal::new(SongFilter::All);
    // Which tag the `Tag` chip has been narrowed to. `None` while the chip is
    // active means "any tag at all".
    let chosen_tag: Signal<Option<String>> = Signal::new(None);

    let close = move || {
        nav.picking_songs_for.set(None);
        query.set(String::new());
        filter.set(SongFilter::All);
        chosen_tag.set(None);
    };

    rsx! {
        div {
            style: {move || sheet_root_style(nav.picking_songs_for.get().is_some())},

            div {
                onclick: close,
                style: {move || sheet_scrim_style(nav.picking_songs_for.get().is_some())},
            }

            div {
                style: {move || sheet_panel_style(nav.picking_songs_for.get().is_some(), SHEET_HEIGHT)},

                SheetHandle {}

                // `1i` puts the running count on the title row, where it is in
                // the eye's way on the journey from a tick to the button. The
                // set's own name goes underneath, the way `2e` names the song
                // it is filing — the header behind is only half visible once
                // the sheet is up.
                div {
                    style: {format!("padding: 2px {SCREEN_PAD} 12px; flex-shrink: 0;")},
                    // The count shares the title's row, as `1i` draws it — a
                    // row of its own rather than a third flex item beside the
                    // whole header block, because `align-items: baseline` would
                    // otherwise line it up with the set name underneath.
                    div {
                        style: "display: flex; align-items: baseline; gap: 10px;",
                        div { style: {format!("{T_ROW_TITLE} font-size: 20px; flex: 1; min-width: 0;")}, "Add to set" }
                        div {
                            style: {format!("{T_META} flex-shrink: 0;")},
                            {move || picked_note(current_picks(picked, picked_for, nav).len())},
                        }
                    }
                    div {
                        style: {format!("{T_META} margin-top: 3px; \
                                         overflow: hidden; text-overflow: ellipsis; white-space: nowrap;")},
                        {move || set_name(setlists, nav)},
                    }
                }

                div {
                    style: {format!("margin: 0 {SCREEN_PAD} 8px; background: var(--sla-fill); border-radius: 14px; \
                                     padding: 10px 13px; display: flex; align-items: center; gap: 9px; flex-shrink: 0;")},
                    span { style: "color: var(--sla-muted); display: flex;", {icon(__scope, TablerIcon::Search, 16)} }
                    input {
                        r#type: "text",
                        style: "flex: 1; border: none; outline: none; background: transparent; \
                                font-family: var(--sla-font-ui); font-size: 14px; color: var(--sla-ink);",
                        placeholder: "Search your library",
                        value: {|| query.get()},
                        oninput: move |value: String| query.set(value),
                    }
                }

                // All · Solid · Recent · Tag, exactly the four `1i` draws.
                div {
                    style: {format!("display: flex; gap: 7px; flex-shrink: 0; padding: 0 {SCREEN_PAD} 8px;")},
                    for choice in SongFilter::ALL {
                        Chip {
                            key: {choice.label()},
                            label: {choice.label().to_string()},
                            active: {filter.get() == choice},
                            selected: false,
                            onclick: move || filter.set(choice),
                        }
                    }
                }

                // Which tag is a second question, and it only exists once the
                // `Tag` chip is on — so it gets a second row rather than a
                // picker over a picker. `1i` shows four chips at rest and this
                // still does; the row below appears only after a tap.
                if filter.get() == SongFilter::Tag {
                    div {
                        style: {format!("display: flex; flex-wrap: wrap; gap: 7px; flex-shrink: 0; \
                                         padding: 0 {SCREEN_PAD} 8px;")},
                        for (tag, active) in tag_chips(songs, chosen_tag) {
                            Chip {
                                key: {tag.clone()},
                                label: {tag.clone()},
                                active: {active},
                                selected: false,
                                // Tapping the chosen tag again widens back out
                                // to every tagged song.
                                onclick: move || chosen_tag.set(if active { None } else { Some(tag.clone()) }),
                            }
                        }
                        if library_tags(&songs.songs.get()).is_empty() {
                            span {
                                style: {format!("{T_META_SMALL}")},
                                "No song in your book carries a tag yet."
                            }
                        }
                    }
                }

                div {
                    style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD};")},

                    for row in rows(songs, setlists, nav, query, filter, chosen_tag, picked, picked_for) {
                        // The `for` body re-runs as its own closure, so it
                        // takes owned copies of everything it draws.
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
                        let title_color = if already { "var(--sla-muted)" } else { "var(--sla-ink)" };

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
                                    style: {format!("{T_ROW_TITLE} color: {title_color}; \
                                                     overflow: hidden; text-overflow: ellipsis; white-space: nowrap;")},
                                    {row.title.clone()}
                                }
                                div { style: {format!("{T_META_SMALL} margin-top: 2px;")}, {row.sub.clone()} }
                            }
                        }
                    }

                    // A list with nothing in it says which of the two things
                    // the user just did emptied it.
                    if rows(songs, setlists, nav, query, filter, chosen_tag, picked, picked_for).is_empty() {
                        div {
                            style: {format!("{T_META} padding: 22px 0;")},
                            // A closure, not a plain expression: the `if` above
                            // re-runs when the list empties, but the text inside
                            // an already-rendered body does not, so a bare call
                            // here would keep naming whichever chip emptied the
                            // list first.
                            {move || picker_empty_note(filter.get(), chosen_tag.get().as_deref(), &query.get())}
                        }
                    }

                    div { style: "height: 12px;" }
                }

                SheetFooter {
                    note: "added at the end of the set",
                    action: {|| add_songs_label(current_picks(picked, picked_for, nav).len())},
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

/// The set being added to, or nothing while the sheet is sliding away.
fn set_name(setlists: SetlistsStore, nav: NavStore) -> String {
    match nav.picking_songs_for.get().and_then(|id| setlists.get(id)) {
        Some(setlist) => setlist.name,
        None => String::new(),
    }
}

/// One drawn row. Computed outside `rsx!` so the loop body has only owned
/// values to hand around.
#[derive(Clone, PartialEq)]
struct Row {
    id: SongId,
    title: String,
    sub: String,
    already: bool,
    ticked: bool,
}

/// The rows the picker shows, recomputed from the stores. Takes only `Copy`
/// arguments so it can be called from inside a reactive closure.
#[allow(clippy::too_many_arguments)]
fn rows(
    songs: SongsStore,
    setlists: SetlistsStore,
    nav: NavStore,
    query: Signal<String>,
    filter: Signal<SongFilter>,
    chosen_tag: Signal<Option<String>>,
    picked: Signal<Vec<SongId>>,
    picked_for: Signal<Option<SetlistId>>,
) -> Vec<Row> {
    let Some(setlist_id) = nav.picking_songs_for.get() else {
        return Vec::new();
    };
    let members = setlists
        .get(setlist_id)
        .map(|s| s.song_ids)
        .unwrap_or_default();
    let picks = current_picks(picked, picked_for, nav);
    let tag = chosen_tag.get();

    picker_songs(
        songs.songs.get(),
        &query.get(),
        filter.get(),
        tag.as_deref(),
        Day::today(),
    )
    .into_iter()
    .map(|song| {
        // Songs already in the set are shown, disabled, rather than hidden.
        // The sheet covers the set, so the picker is the only thing that can
        // still answer "is this one in already?" — and a hidden row makes the
        // search field lie, reporting no match for a song plainly in the book.
        // `2e` disables rather than hides for the same reason from the other
        // end.
        let already = members.contains(&song.id);
        Row {
            id: song.id,
            title: song.title.clone(),
            sub: if already {
                "already in this set".to_string()
            } else {
                // `Artist · key`, the sub-line `1i` draws — never `meta_line`'s
                // tempo as well, which is detail this list has no room for.
                match &song.key {
                    Some(key) => format!("{} · {key}", song.artist),
                    None => song.artist.clone(),
                }
            },
            already,
            ticked: picks.contains(&song.id),
        }
    })
    .collect()
}

/// Every tag in the book, and whether it is the one narrowed to.
fn tag_chips(songs: SongsStore, chosen_tag: Signal<Option<String>>) -> Vec<(String, bool)> {
    let chosen = chosen_tag.get();
    library_tags(&songs.songs.get())
        .into_iter()
        .map(|tag| {
            let active = chosen
                .as_deref()
                .is_some_and(|c| c.eq_ignore_ascii_case(&tag));
            (tag, active)
        })
        .collect()
}

/// The ticks, but only while they still belong to the set on screen.
fn current_picks(
    picked: Signal<Vec<SongId>>,
    picked_for: Signal<Option<SetlistId>>,
    nav: NavStore,
) -> Vec<SongId> {
    let setlist = nav.picking_songs_for.get();
    if setlist.is_some() && picked_for.get() == setlist {
        picked.get()
    } else {
        Vec::new()
    }
}

fn toggle(
    picked: Signal<Vec<SongId>>,
    picked_for: Signal<Option<SetlistId>>,
    nav: NavStore,
    id: SongId,
) {
    let setlist = nav.picking_songs_for.get();
    if picked_for.get() != setlist {
        picked_for.set(setlist);
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

/// The picked songs join the end of the set, in the order they were ticked,
/// in one write — existing positions are not disturbed.
fn commit(
    setlists: SetlistsStore,
    picked: Signal<Vec<SongId>>,
    picked_for: Signal<Option<SetlistId>>,
    nav: NavStore,
) {
    let Some(setlist) = nav.picking_songs_for.get() else {
        return;
    };
    setlists.add_songs(setlist, &current_picks(picked, picked_for, nav));
    picked.set(Vec::new());
    picked_for.set(None);
}
