//! Default tuning sheet — NO WIREFRAME. Card H5, modeled structurally on `2c`
//! (`src/screens/sort_sheet.rs`) the same way the filters sheet is: the
//! handoff draws `Default tuning · Standard ▾` on the Settings screen
//! (`design_handoff_setlistarray/README.md:258`) and never draws what the `▾`
//! opens, so this file is this card's answer to that.
//!
//! **A sheet, not a cycling `choice_row`.** `choice_row` — the shape Library
//! density and Performance mode theme already use — draws every option as an
//! inline chip, which works for two options and stops working well before
//! seven: seven chips wrapping onto two or three lines is a wall of text
//! under a label that used to be one line, and `1q`'s own row height budget
//! never anticipated it. A `▾` is also, in every other place this app draws
//! one, a promise that tapping the row opens something rather than cycling
//! it in place — the sort-and-group row and the filter row both keep that
//! promise with a sheet already. So this is a sixth sheet rather than a
//! seventh shape: one column of seven rows, the current tuning tinted the
//! way the sort sheet tints its active row, no separate `Done` — tapping a
//! row *is* choosing it, so it selects and closes in the one gesture, the
//! same immediacy `settings::accent_row`'s chips already have for a
//! same-screen pick. A sheet with only one thing to decide has no use for a
//! footer that exists to let you decide it and then also confirm it.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::store::{DefaultTuning, NavStore, SettingsStore};
use crate::theme::{SCREEN_PAD, T_BODY, T_ROW_TITLE};
use crate::ui::{SheetHandle, icon, sheet_panel_style, sheet_root_style, sheet_scrim_style};

/// Seven short rows never need the sort sheet's 68%; this is short enough
/// that the sheet hugs its own content instead of leaving a slab of empty
/// paper below the last tuning.
const SHEET_HEIGHT: u32 = 48;

#[component]
pub fn TuningSheet() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let settings = use_store::<SettingsStore>();

    rsx! {
        div {
            style: {move || sheet_root_style(nav.tuning_sheet_open.get())},

            div {
                onclick: move || nav.tuning_sheet_open.set(false),
                style: {move || sheet_scrim_style(nav.tuning_sheet_open.get())},
            }

            div {
                style: {move || sheet_panel_style(nav.tuning_sheet_open.get(), SHEET_HEIGHT)},

                SheetHandle {}

                div {
                    style: {format!("padding: 2px {SCREEN_PAD} 12px; flex-shrink: 0;")},
                    div { style: {format!("{T_ROW_TITLE} font-size: 20px;")}, "Default tuning" }
                }

                div {
                    style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD} 16px;")},

                    for tuning in DefaultTuning::ALL {
                        let chosen = move || settings.default_tuning.get() == tuning;
                        // The tint is the whole selected state, same rule the
                        // sort sheet's rows follow — see that file's header.
                        let band = if chosen() {
                            "background: var(--sla-accent-tint); color: var(--sla-accent-on-tint); \
                             border-radius: 10px; border-bottom-color: transparent;"
                        } else {
                            ""
                        };
                        let ink = if chosen() {
                            "var(--sla-accent-on-tint)"
                        } else {
                            "var(--sla-ink)"
                        };

                        div {
                            key: {tuning.label()},
                            // One tap both picks and dismisses — see the
                            // module header for why this sheet has no
                            // separate `Done`.
                            onclick: move || {
                                settings.set_default_tuning(tuning);
                                nav.tuning_sheet_open.set(false);
                            },
                            style: {format!("{band} display: flex; align-items: center; gap: 10px; \
                                             margin: 0 -10px; padding: 12px 10px; \
                                             border-bottom: 1px solid var(--sla-hairline-soft);")},
                            span { style: {format!("{T_BODY} flex: 1; color: {ink};")}, {tuning.label()} }
                            if chosen() {
                                span {
                                    style: "display: flex; align-items: center; color: var(--sla-accent-on-tint);",
                                    {icon(__scope, TablerIcon::Check, 16)}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
