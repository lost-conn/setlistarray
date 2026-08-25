//! Library (Songs tab) — HI-FI. The app's home; answers "what can I actually
//! play?".

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::model::{AttachmentKind, Song};
use crate::derive::{GROUP_PREVIEW, visible_songs};
use crate::store::{
    AttachmentsStore, Density, LibraryViewStore, NavStore, Route, SettingsStore, SongsStore,
};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_SCREEN_TITLE};
use crate::ui::{Chip, GroupHeader, IconButton, SongRow, icon};

#[component]
pub fn Library() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let view = use_store::<LibraryViewStore>();
    let attachments = use_store::<AttachmentsStore>();
    let _settings = use_store::<SettingsStore>();

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0; position: relative;",

            // Title block. The gear is the only way into Settings from here —
            // there is no nav item for it.
            div {
                style: {format!("padding: 6px {SCREEN_PAD} 14px; display: flex; align-items: baseline; gap: 12px;")},
                div { style: "flex: 1;",
                    div { style: {format!("{T_SCREEN_TITLE}")}, "Songs" }
                    div { style: {format!("{T_META} margin-top: 7px;")},
                        {|| format!("{} in your book · ", songs.count())}
                        span {
                            style: "color: var(--sla-accent); font-weight: 600;",
                            {|| format!("{} solid", songs.solid_count())}
                        }
                    }
                }
                IconButton {
                    glyph: TablerIcon::Settings,
                    size: 18,
                    onclick: move || nav.go(Route::Settings),
                }
            }

            // Search — opens the search & filter screen on tap, but types in
            // place for now.
            div {
                style: {format!("margin: 0 {SCREEN_PAD}; background: var(--sla-fill); border-radius: 14px; \
                                 padding: 11px 14px; display: flex; align-items: center; gap: 9px;")},
                span { style: "color: var(--sla-muted); display: flex;", {icon(__scope, TablerIcon::Search, 17)} }
                input {
                    r#type: "text",
                    style: "flex: 1; border: none; outline: none; background: transparent; \
                            font-family: var(--sla-font-ui); font-size: 15px; color: var(--sla-ink);",
                    placeholder: "Search title, artist, tag…",
                    value: {|| view.query.get()},
                    oninput: move |value: String| view.query.set(value),
                }
            }

            // Chip row. Group is the active chip; sort shows field + direction.
            div {
                style: {format!("display: flex; flex-wrap: wrap; gap: 7px; padding: 12px {SCREEN_PAD} 2px;")},
                Chip {
                    label: {|| format!("Group: {}", view.group_by.get().label())},
                    active: true,
                    selected: false,
                    onclick: move || nav.sort_sheet_open.set(true),
                }
                Chip {
                    label: {|| format!("{} {}", view.sort_field.get().label(), view.sort_dir.get().arrow())},
                    active: false,
                    selected: true,
                    onclick: move || nav.sort_sheet_open.set(true),
                }
                Chip {
                    label: "Filter",
                    active: false,
                    selected: false,
                    onclick: move || nav.sort_sheet_open.set(true),
                }
                Chip {
                    label: "≣",
                    active: false,
                    selected: false,
                    onclick: move || view.toggle_density(),
                }
            }

            // The grouped list.
            div {
                style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD} 90px;")},
                for (index, group) in view.grouped(songs.songs.get()).into_iter().enumerate() {
                    // The `for` body re-runs as a closure, so everything it
                    // needs is cloned or copied up front.
                    let label = group.label.clone();
                    let total = group.songs.len();
                    let collapsed = view.is_collapsed(&label);
                    let expanded = view.is_expanded(&label);
                    let hidden = total.saturating_sub(GROUP_PREVIEW);

                    div { key: {label.clone()},
                        GroupHeader {
                            label: {label.clone()},
                            count: {total},
                            is_first: {index == 0},
                            collapsed: {collapsed},
                            onclick: {
                                let label = label.clone();
                                move || view.toggle_collapsed(label.clone())
                            },
                        }

                        if !collapsed {
                            div {
                                // Nested control flow re-runs as its own
                                // closure, so it takes only `Copy` inputs and
                                // recomputes the rows from the stores.
                                for song in songs_in_group(view, songs, attachments, index) {
                                    let id = song.id;
                                    let kind = primary_kind(attachments, &song);
                                    SongRow {
                                        key: id,
                                        song: {song.clone()},
                                        kind: {kind},
                                        compact: {view.density.get() == Density::Compact},
                                        onclick: move || nav.go(Route::SongDetail(id)),
                                    }
                                }
                            }

                            // Per-group truncation, until the user asks for the rest.
                            if total > GROUP_PREVIEW && !expanded {
                                div {
                                    onclick: move || expand_group(view, songs, index),
                                    style: "color: var(--sla-accent); font-weight: 500; font-size: 13px; padding: 11px 0;",
                                    {format!("Show {hidden} more")}
                                }
                            }
                        }
                    }
                }

                if songs.songs.get().is_empty() {
                    div { style: {format!("{T_META_SMALL} text-align: center; padding: 48px 0;")},
                        "Nothing here yet. Add the first song you know how to play."
                    }
                }
            }

            // FAB — creates a song.
            div {
                onclick: move || nav.go(Route::AddSong),
                style: "position: absolute; right: 20px; bottom: 16px; width: 60px; height: 60px; \
                        border-radius: 20px; background: var(--sla-accent); color: var(--sla-on-accent); \
                        display: flex; align-items: center; justify-content: center; \
                        box-shadow: 0 8px 18px -4px rgba(181,71,36,.5);",
                {icon(__scope, TablerIcon::Plus, 24)}
            }
        }
    }
}

/// The rows one group shows, recomputed from the stores. Takes only `Copy`
/// arguments so it can be called from inside a reactive closure.
fn songs_in_group(
    view: LibraryViewStore,
    songs: SongsStore,
    _attachments: AttachmentsStore,
    index: usize,
) -> Vec<Song> {
    match view.grouped(songs.songs.get()).into_iter().nth(index) {
        Some(group) => visible_songs(&group.songs, view.is_expanded(&group.label)),
        None => Vec::new(),
    }
}

/// Expand the group at `index` past its preview limit.
fn expand_group(view: LibraryViewStore, songs: SongsStore, index: usize) {
    if let Some(group) = view.grouped(songs.songs.get()).into_iter().nth(index) {
        view.expand(group.label);
    }
}

/// The badge a row's thumb shows, or `None` for the dashed empty thumb.
fn primary_kind(attachments: AttachmentsStore, song: &Song) -> Option<AttachmentKind> {
    song.primary_attachment
        .or_else(|| song.attachments.first().copied())
        .and_then(|id| attachments.get(id))
        .map(|a| a.kind)
}
