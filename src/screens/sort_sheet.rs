//! Sort & group sheet — WIREFRAME (`2c`), styled with the hi-fi token set.
//!
//! The rules this sheet drives already exist and are tested: `GroupBy`,
//! `SortField::direction_label`, `LibraryViewStore::choose_sort` (tap the
//! active row to reverse it, another to select it) and `derive::sort_songs`
//! (songs missing the field sort last). This file is only the sheet.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{direction_arrow, field_fill_count, fill_note, is_sparse_field};
use crate::store::{GroupBy, LibraryViewStore, NavStore, SongsStore, SortDir, SortField};
use crate::theme::{SCREEN_PAD, T_BODY, T_META_SMALL, T_ROW_TITLE};
use crate::ui::{
    Chip, SheetFooter, SheetHandle, icon, sheet_panel_style, sheet_root_style, sheet_scrim_style,
};

/// `2c` draws 562 of 720; on the 393×852 viewport the designs assume, that is
/// the handoff's "~68%".
const SHEET_HEIGHT: u32 = 68;

#[component]
pub fn SortGroupSheet() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let view = use_store::<LibraryViewStore>();
    let songs = use_store::<SongsStore>();

    rsx! {
        div {
            style: {move || sheet_root_style(nav.sort_sheet_open.get())},

            div {
                onclick: move || nav.sort_sheet_open.set(false),
                style: {move || sheet_scrim_style(nav.sort_sheet_open.get())},
            }

            div {
                style: {move || sheet_panel_style(nav.sort_sheet_open.get(), SHEET_HEIGHT)},

                SheetHandle {}

                div {
                    style: {format!("padding: 2px {SCREEN_PAD} 12px; flex-shrink: 0;")},
                    div { style: {format!("{T_ROW_TITLE} font-size: 20px;")}, "Sort & group" }
                }

                div {
                    style: {format!("padding: 0 {SCREEN_PAD} 7px; flex-shrink: 0; {T_META_SMALL}")},
                    "Group by"
                }
                div {
                    style: {format!("display: flex; flex-wrap: wrap; gap: 7px; flex-shrink: 0; \
                                     padding: 0 {SCREEN_PAD} 14px;")},
                    for group_by in GroupBy::ALL {
                        Chip {
                            key: {group_by.label()},
                            label: {group_by.label().to_string()},
                            active: {view.group_by.get() == group_by},
                            selected: false,
                            onclick: move || view.group_by.set(group_by),
                        }
                    }
                }

                div {
                    style: {format!("padding: 10px {SCREEN_PAD} 6px; flex-shrink: 0; {T_META_SMALL} \
                                     border-top: 1px solid var(--sla-hairline);")},
                    "Sort within group"
                }

                div {
                    style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD} 8px;")},

                    for row in rows(view, songs) {
                        // The `for` body re-runs as its own closure, so every
                        // value it draws is computed and owned up front.
                        let field = row.field;
                        // The tint is the whole selected state — nothing else on
                        // the row changes weight, so the eye finds it at a glance.
                        let band = if row.active {
                            "background: var(--sla-accent-tint); color: var(--sla-accent-on-tint); \
                             border-radius: 10px; border-bottom-color: transparent;"
                        } else {
                            ""
                        };
                        // A field almost nobody has filled in is greyed, and says
                        // how many songs would actually sort — but stays tappable.
                        let name_color = if row.active {
                            "var(--sla-accent-on-tint)"
                        } else if row.sparse {
                            "var(--sla-muted)"
                        } else {
                            "var(--sla-ink)"
                        };
                        let side_color = if row.active {
                            "var(--sla-accent-on-tint)"
                        } else {
                            "var(--sla-muted)"
                        };
                        let arrow_color = if row.active {
                            "var(--sla-accent-on-tint)"
                        } else {
                            "var(--sla-hairline)"
                        };

                        div {
                            key: {field.label()},
                            // Tapping the active row reverses it; tapping
                            // another selects it. `choose_sort` owns that rule.
                            onclick: move || view.choose_sort(field),
                            style: {format!("{band} display: flex; align-items: center; gap: 10px; \
                                             margin: 0 -10px; padding: 10px; \
                                             border-bottom: 1px solid var(--sla-hairline-soft);")},
                            span {
                                style: {format!("{T_BODY} flex: 1; color: {name_color}; \
                                                 overflow: hidden; text-overflow: ellipsis; white-space: nowrap;")},
                                {field.label()}
                            }
                            span {
                                style: {format!("font-size: 12px; color: {side_color};")},
                                {row.side.clone()}
                            }
                            span {
                                style: {format!("display: flex; align-items: center; color: {arrow_color};")},
                                if let Some(glyph) = row.arrow {
                                    {icon(__scope, glyph, 15)}
                                }
                            }
                        }
                    }
                }

                SheetFooter {
                    note: "Songs missing a field sort last",
                    action: "Done",
                    enabled: true,
                    onclick: move || nav.sort_sheet_open.set(false),
                }
            }
        }
    }
}

/// One drawn row.
#[derive(Clone, PartialEq)]
struct Row {
    field: SortField,
    active: bool,
    sparse: bool,
    /// The words to the right of the name: the direction this row would sort
    /// in, or — for a field hardly anyone has filled in — why it will not help.
    side: String,
    arrow: Option<TablerIcon>,
}

/// The ten metadata rows, recomputed from the stores. Takes only `Copy`
/// arguments so it can be called from inside a reactive closure.
fn rows(view: LibraryViewStore, songs: SongsStore) -> Vec<Row> {
    let all = songs.songs.get();
    let active_field = view.sort_field.get();
    let active_dir = view.sort_dir.get();

    SortField::ALL
        .into_iter()
        .map(|field| {
            let active = field == active_field;
            let sparse = is_sparse_field(&all, field);
            // An inactive row previews the direction it would take if chosen —
            // `choose_sort` starts every new field ascending.
            let dir = if active { active_dir } else { SortDir::Asc };
            Row {
                field,
                active,
                sparse,
                side: if sparse && !active {
                    fill_note(field_fill_count(&all, field))
                } else {
                    field.direction_label(dir).to_string()
                },
                arrow: if sparse && !active {
                    None
                } else {
                    Some(direction_arrow(field, dir))
                },
            }
        })
        .collect()
}
