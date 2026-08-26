//! Setlists tab — WIREFRAME (`1m`), styled with the hi-fi token set.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::menu::{FULL_WIDTH_TARGET, MENU_SURFACE, SetlistMenuItems};
use crate::model::fmt_duration;
use crate::store::{NavStore, PlaybackStore, Route, SetlistsStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_ROW_TITLE, T_SCREEN_TITLE};
use crate::ui::{IconButton, icon};

#[component]
pub fn Setlists() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let setlists = use_store::<SetlistsStore>();
    let songs = use_store::<SongsStore>();
    let playback = use_store::<PlaybackStore>();

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0; position: relative;",

            div {
                style: {format!("padding: 6px {SCREEN_PAD} 14px; display: flex; align-items: baseline; gap: 12px;")},
                div { style: "flex: 1;",
                    div { style: {format!("{T_SCREEN_TITLE}")}, "Setlists" }
                    div { style: {format!("{T_META} margin-top: 7px;")},
                        {|| format!("{} sets", setlists.setlists.get().len())}
                    }
                }
                IconButton {
                    glyph: TablerIcon::Settings,
                    size: 18,
                    onclick: move || nav.go(Route::Settings),
                }
            }

            div {
                style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD} 90px; \
                                 display: flex; flex-direction: column; gap: 11px;")},
                for setlist in setlists.setlists.get() {
                    let id = setlist.id;
                    // The most recent set gets the heavier border.
                    let recent = setlist.last_played.is_some();
                    let titles: Vec<String> = setlist
                        .song_ids
                        .iter()
                        .filter_map(|sid| songs.get(*sid))
                        .map(|s| s.title.clone())
                        .collect();
                    let total: u32 = setlist
                        .song_ids
                        .iter()
                        .filter_map(|sid| songs.get(*sid))
                        .filter_map(|s| s.duration)
                        .sum();

                    let meta = {
                        let mut parts = vec![
                            format!("{} songs", setlist.song_ids.len()),
                            fmt_duration(total),
                        ];
                        if let Some(d) = setlist.last_played {
                            parts.push(format!("played {}", d.short()));
                        }
                        parts.join(" · ")
                    };
                    // A truncated peek at the running order.
                    let preview = {
                        let shown: Vec<String> = titles.iter().take(3).cloned().collect();
                        let rest = titles.len().saturating_sub(shown.len());
                        if rest > 0 {
                            format!("{} · +{rest}", shown.join(" · "))
                        } else {
                            shown.join(" · ")
                        }
                    };

                    let menu_open = Signal::new(false);
                    // Right-click stands in for long-press — see the note in
                    // `crate::menu`.
                    div {
                        key: id,
                        oncontextmenu: move || menu_open.set(true),
                        DropdownMenu {
                            opened_fn: move || menu_open.get(),
                            on_close: move || menu_open.set(false),
                            position: "bottom-start",
                            style: {FULL_WIDTH_TARGET},
                            DropdownMenuTarget {
                                style: {FULL_WIDTH_TARGET},
                                div {
                                    onclick: move || nav.go(Route::SetlistDetail(id)),
                                    style: {
                                        let border = if recent { "var(--sla-muted)" } else { "var(--sla-hairline)" };
                                        format!("border: 1px solid {border}; border-radius: 14px; padding: 14px 15px; \
                                                 display: flex; align-items: flex-start; gap: 12px;")
                                    },
                                    div { style: "flex: 1; min-width: 0;",
                                        if nav.renaming_setlist.get() == Some(id) {
                                            // A shield: without a click handler
                                            // of its own here, a click that
                                            // misses the input/buttons below
                                            // would walk up to the card's
                                            // `onclick` above and navigate away
                                            // mid-edit.
                                            div {
                                                onclick: move || {},
                                                style: "display: flex; align-items: center; gap: 6px;",
                                                TextInput {
                                                    value_fn: move || nav.rename_draft.get(),
                                                    oninput: move |v: String| nav.rename_draft.set(v),
                                                    onsubmit: move || {
                                                        let name = nav.rename_draft.get();
                                                        if !name.trim().is_empty() {
                                                            setlists.rename(id, name);
                                                        }
                                                        nav.renaming_setlist.set(None);
                                                    },
                                                    style: {format!(
                                                        "{T_ROW_TITLE} flex: 1; border: 1px solid var(--sla-hairline); \
                                                         border-radius: 8px; padding: 4px 8px; background: var(--sla-paper);"
                                                    )},
                                                }
                                                div {
                                                    onclick: move || {
                                                        let name = nav.rename_draft.get();
                                                        if !name.trim().is_empty() {
                                                            setlists.rename(id, name);
                                                        }
                                                        nav.renaming_setlist.set(None);
                                                    },
                                                    style: "color: var(--sla-accent); display: flex; flex-shrink: 0;",
                                                    {icon(__scope, TablerIcon::Check, 18)}
                                                }
                                                div {
                                                    onclick: move || nav.renaming_setlist.set(None),
                                                    style: "color: var(--sla-muted); display: flex; flex-shrink: 0;",
                                                    {icon(__scope, TablerIcon::X, 18)}
                                                }
                                            }
                                        } else {
                                            // Recomputed rather than reusing
                                            // the outer loop's `setlist`
                                            // binding — a nested `if` inside
                                            // `for` can't borrow a non-`Copy`
                                            // value captured by the closure
                                            // around it (see `songs_in_group`
                                            // in `library.rs`).
                                            div {
                                                style: {format!("{T_ROW_TITLE}")},
                                                {setlists.get(id).map(|s| s.name).unwrap_or_default()}
                                            }
                                        }
                                        div { style: {format!("{T_META} margin-top: 3px;")}, {meta.clone()} }
                                        div { style: {format!("{T_META_SMALL} margin-top: 7px;")}, {preview.clone()} }
                                    }
                                    div {
                                        onclick: move || {
                                            playback.start(id);
                                            nav.go(Route::Performance(id));
                                        },
                                        style: "width: 38px; height: 38px; border-radius: 12px; background: var(--sla-fill); \
                                                display: flex; align-items: center; justify-content: center; \
                                                color: var(--sla-ink-2); flex-shrink: 0;",
                                        {icon(__scope, TablerIcon::PlayerPlay, 17)}
                                    }
                                }
                            }
                            DropdownMenuDropdown {
                                style: {MENU_SURFACE},
                                SetlistMenuItems { id: id }
                            }
                        }
                    }
                }
            }

            // FAB — creates a setlist. `z-index` is load-bearing here for the
            // same reason it is on the library's FAB: without it the button
            // paints but cannot be tapped. The full explanation lives on that
            // one, in `crate::screens::library`.
            div {
                onclick: move || { setlists.add("New setlist"); },
                style: "position: absolute; right: 20px; bottom: 16px; z-index: 10; \
                        width: 60px; height: 60px; \
                        border-radius: 20px; background: var(--sla-accent); color: var(--sla-on-accent); \
                        display: flex; align-items: center; justify-content: center; \
                        box-shadow: 0 8px 18px -4px rgba(181,71,36,.5);",
                {icon(__scope, TablerIcon::Plus, 24)}
            }
        }
    }
}
