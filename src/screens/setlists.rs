//! Setlists tab — WIREFRAME (`1m`), styled with the hi-fi token set.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::menu::{FULL_WIDTH_TARGET, MENU_SURFACE, SetlistMenuItems};
use crate::model::fmt_duration;
use crate::store::{NavStore, PlaybackStore, Route, SetlistsStore, SettingsStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_ROW_TITLE, T_SCREEN_TITLE};
use crate::ui::{IconButton, icon};

#[component]
pub fn Setlists() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let setlists = use_store::<SetlistsStore>();
    let songs = use_store::<SongsStore>();
    let playback = use_store::<PlaybackStore>();
    // Read for one thing: the keep-awake preference handed to `start` below.
    let settings = use_store::<SettingsStore>();

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
                                            // The keep-awake preference is the user's, so it is read at the
                        // moment the set starts rather than left at whatever the last
                        // gig's bottom bar was set to.
                        playback.start(id, settings.keep_awake.get());
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

                // Card J4's audit: the Songs tab has `1r` for an empty book
                // and this tab had nothing of its own — a fresh library with
                // songs already typed in but no set built yet drew a blank
                // column here with no more to say about it than the filtered-
                // library case `library.rs` already answers. No accent action
                // is repeated here: the FAB in the corner is already the one
                // control this state wants, and a second "New setlist" link
                // beside this text would be the same tap offered twice on one
                // screen, the thing `song_detail`'s header calls out as worse
                // than the gap it would fill.
                if setlists.setlists.get().is_empty() {
                    div { style: "padding: 60px 8px; text-align: center;",
                        div { style: "color: var(--sla-muted); display: flex; justify-content: center;",
                            {icon(__scope, TablerIcon::List, 26)}
                        }
                        div { style: {format!("{T_ROW_TITLE} margin-top: 14px;")}, "No sets yet" }
                        div { style: {format!("{T_META} margin-top: 6px;")},
                            "Group songs from your book into a set you can play front to back."
                        }
                    }
                }
            }

            // FAB — creates a setlist. `z-index` is load-bearing here for the
            // same reason it is on the library's FAB: without it the button is
            // neither drawn nor tappable. The full explanation lives on that
            // one, in `crate::screens::library`.
            div {
                onclick: move || { setlists.add("New setlist"); },
                style: "position: absolute; right: 20px; bottom: 16px; z-index: 10; \
                        width: 60px; height: 60px; \
                        border-radius: 20px; background: var(--sla-accent); color: var(--sla-on-accent); \
                        display: flex; align-items: center; justify-content: center; \
                        box-shadow: 0 8px 18px -4px var(--sla-accent-shadow);",
                {icon(__scope, TablerIcon::Plus, 24)}
            }
        }
    }
}
