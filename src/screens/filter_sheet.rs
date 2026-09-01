//! Filters sheet — NO WIREFRAME. Card G3, and modeled structurally on `2c`
//! (`src/screens/sort_sheet.rs`), the sort & group sheet's own hi-fi register,
//! because the handoff never drew one of these: the wireframe's line for the
//! `Filter` chip in the library header stops at `background: fill, color:
//! muted` (`design_handoff_setlistarray/README.md:144`) and never says what
//! tapping it does. Everything below is this card's answer.
//!
//! **Four facets, OR within one and AND across them.** Confidence, Tag,
//! Tuning and "has a chart" each accept a set of values; a song has to clear
//! every facet that has anything picked in it, and a facet with nothing
//! picked constrains nothing. The rule itself lives on
//! [`crate::store::Filters::matches`], which is what
//! [`crate::derive::grouped`] calls before it buckets anything — this file is
//! only the sheet that edits the selection.
//!
//! **Confidence and Chart are fixed; Tag and Tuning are not.** The four
//! Confidence chips and the one Chart toggle are always drawn, because their
//! meaning does not depend on what happens to be in the book today. Tag and
//! Tuning are read off the library itself — [`crate::derive::library_tags`]
//! and [`crate::derive::library_tunings`] — and a facet with nothing in it
//! renders no section at all, the same "only fields that exist" rule the
//! library row already follows for a song's own metadata.
//!
//! **The footer is not `crate::ui::SheetFooter`, and that is the one place
//! this file departs from the shape the card asked it to borrow.**
//! `SheetFooter` is a note and one button, which is exactly right for a sheet
//! with one closing action — the sort sheet's `Done`. This sheet needs two:
//! a `Clear` that empties every facet without leaving, and a `Done` that
//! closes it, and there is nowhere on a note-plus-one-button footer to put a
//! second control that is not also the thing that dismisses the sheet. So the
//! footer here is hand-rolled to the same padding, border and type scale
//! `SheetFooter` uses, with `Clear` added to its left.

use rinch::prelude::*;

use crate::derive::{filter_sheet_note, library_tags, library_tunings};
use crate::model::Confidence;
use crate::store::{LibraryViewStore, NavStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_ROW_TITLE, T_META_SMALL};
use crate::ui::{Chip, SheetHandle, sheet_panel_style, sheet_root_style, sheet_scrim_style};

/// Same figure the sort sheet uses, for the same reason: nothing in the
/// handoff says otherwise, and a fixed percentage with its own internal
/// scroll degrades to a long tag list the same way `2c` degrades to ten sort
/// rows.
const SHEET_HEIGHT: u32 = 68;

/// The four confidence chips, in the fixed order
/// [`crate::derive::group_songs`]'s own Confidence buckets use — `None` is
/// `Unrated`, not "nothing picked".
const CONFIDENCE_SLOTS: [(Option<Confidence>, &str); 4] = [
    (Some(Confidence::Solid), "Solid"),
    (Some(Confidence::Rusty), "Rusty"),
    (Some(Confidence::Learning), "Learning"),
    (None, "Unrated"),
];

#[component]
pub fn FilterSheet() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let view = use_store::<LibraryViewStore>();
    let songs = use_store::<SongsStore>();

    rsx! {
        div {
            style: {move || sheet_root_style(nav.filter_sheet_open.get())},

            div {
                onclick: move || nav.filter_sheet_open.set(false),
                style: {move || sheet_scrim_style(nav.filter_sheet_open.get())},
            }

            div {
                style: {move || sheet_panel_style(nav.filter_sheet_open.get(), SHEET_HEIGHT)},

                SheetHandle {}

                div {
                    style: {format!("padding: 2px {SCREEN_PAD} 12px; flex-shrink: 0;")},
                    div { style: {format!("{T_ROW_TITLE} font-size: 20px;")}, "Filters" }
                }

                div {
                    style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD} 8px;")},

                    // ── Confidence — fixed, always drawn ────────────────
                    div {
                        style: {format!("padding: 0 0 7px; flex-shrink: 0; {T_META_SMALL}")},
                        "Confidence"
                    }
                    div {
                        style: "display: flex; flex-wrap: wrap; gap: 7px; flex-shrink: 0; padding: 0 0 14px;",
                        for (confidence, label) in CONFIDENCE_SLOTS {
                            Chip {
                                key: {label},
                                label: {label.to_string()},
                                active: {view.filters.get().is_confidence_selected(confidence)},
                                selected: false,
                                onclick: move || view.toggle_confidence(confidence),
                            }
                        }
                    }

                    // ── Tag — dynamic, absent entirely with nothing to show ──
                    if !library_tags(&songs.songs.get()).is_empty() {
                        div {
                            style: {format!("padding: 10px 0 6px; flex-shrink: 0; {T_META_SMALL} \
                                             border-top: 1px solid var(--sla-hairline);")},
                            "Tag"
                        }
                        div {
                            style: "display: flex; flex-wrap: wrap; gap: 7px; flex-shrink: 0; padding: 0 0 14px;",
                            for tag in library_tags(&songs.songs.get()) {
                                Chip {
                                    key: {tag.clone()},
                                    label: {tag.clone()},
                                    active: {view.filters.get().is_tag_selected(&tag)},
                                    selected: false,
                                    onclick: {
                                        let tag = tag.clone();
                                        move || view.toggle_tag_filter(tag.clone())
                                    },
                                }
                            }
                        }
                    }

                    // ── Tuning — dynamic, same rule as Tag ──────────────
                    if !library_tunings(&songs.songs.get()).is_empty() {
                        div {
                            style: {format!("padding: 10px 0 6px; flex-shrink: 0; {T_META_SMALL} \
                                             border-top: 1px solid var(--sla-hairline);")},
                            "Tuning"
                        }
                        div {
                            style: "display: flex; flex-wrap: wrap; gap: 7px; flex-shrink: 0; padding: 0 0 14px;",
                            for tuning in library_tunings(&songs.songs.get()) {
                                Chip {
                                    key: {tuning.clone()},
                                    label: {tuning.clone()},
                                    active: {view.filters.get().is_tuning_selected(&tuning)},
                                    selected: false,
                                    onclick: {
                                        let tuning = tuning.clone();
                                        move || view.toggle_tuning_filter(tuning.clone())
                                    },
                                }
                            }
                        }
                    }

                    // ── Chart — fixed, one toggle ────────────────────────
                    div {
                        style: {format!("padding: 10px 0 6px; flex-shrink: 0; {T_META_SMALL} \
                                         border-top: 1px solid var(--sla-hairline);")},
                        "Chart"
                    }
                    div {
                        style: "display: flex; flex-wrap: wrap; gap: 7px; flex-shrink: 0; padding: 0 0 14px;",
                        Chip {
                            label: "Has a chart",
                            active: {|| view.filters.get().has_chart},
                            selected: false,
                            onclick: move || view.toggle_has_chart_filter(),
                        }
                    }
                }

                // Hand-rolled rather than `SheetFooter` — see the module header
                // for why a note-plus-one-button footer has no room for `Clear`.
                div {
                    style: {format!("display: flex; align-items: center; gap: 12px; flex-shrink: 0; \
                                     padding: 12px {SCREEN_PAD} 24px; border-top: 1px solid var(--sla-hairline);")},
                    span {
                        style: {format!("{T_META} flex: 1;")},
                        {|| filter_sheet_note(view.filters.get().count())}
                    }
                    div {
                        onclick: move || view.clear_filters(),
                        style: "color: var(--sla-muted); font-weight: 600; font-size: 14px; \
                                padding: 13px 6px; white-space: nowrap;",
                        "Clear"
                    }
                    div {
                        onclick: move || nav.filter_sheet_open.set(false),
                        style: "background: var(--sla-accent); color: var(--sla-on-accent); \
                                border-radius: 999px; padding: 13px 24px; font-weight: 600; \
                                font-size: 16px; white-space: nowrap;",
                        "Done"
                    }
                }
            }
        }
    }
}
