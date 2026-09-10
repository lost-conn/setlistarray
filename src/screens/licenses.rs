//! The Licenses screen: what SetListArray is licensed under, and the notice of
//! every crate it is built from. The data, and the two obligations that make
//! the screen necessary, are `crate::licenses`; this file is how it reads.
//!
//! Reached from the About section at the foot of Settings, and ← goes back to
//! Settings rather than through `NavStore::back`, which would drop somebody on
//! a tab root they did not come from.
//!
//! **One text open at a time.** Over a hundred license texts, the GPL among
//! them at 35 KB, are far more than anyone reads at once and a great deal for
//! the layout engine to shape on a phone. Every row starts closed, as a name
//! and a line saying who ships it; a tap opens that row's text and closes
//! whichever was open before. What the screen costs is then the list plus one
//! license — the one somebody actually wanted to read.
//!
//! The texts are shown reflowed; `licenses::paragraphs` has why a hard-wrapped
//! license file cannot simply be put on a phone as it is.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::licenses::{self, NOTICES};
use crate::store::{NavStore, Route};
use crate::theme::{SCREEN_PAD, T_BODY, T_META, T_META_SMALL, T_ROW_TITLE, T_SECTION_CAPS};
use crate::ui::{IconButton, icon};

/// Which text is open. At most one, for the reason the module header gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    App,
    ThirdParty(usize),
}

#[component]
pub fn Licenses() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let open = Signal::new(Option::<Open>::None);

    nav.register_back(move || nav.go(Route::Settings));

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

            // The same ← and title Settings wears, for the same reason: the
            // word names the screen, not anything on it.
            div { style: "padding: 2px 18px 8px; display: flex; align-items: center; gap: 6px;",
                IconButton { glyph: TablerIcon::ChevronLeft, size: 19, onclick: move || nav.go(Route::Settings) }
                div { style: {format!("{T_ROW_TITLE} font-size: 19px;")}, "Licenses" }
            }

            div { style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD} 20px;")},

                {section(__scope, "SetListArray")}

                div { style: {format!("{T_BODY} padding-top: 10px;")},
                    {format!("SetListArray {}", env!("CARGO_PKG_VERSION"))}
                }
                div { style: {T_META}, {licenses::COPYRIGHT} }
                div { style: {format!("{T_META} padding-top: 10px;")}, {licenses::NOTICE} }
                div { style: {format!("{T_META} padding: 10px 0 6px;")},
                    {format!("Source code: {}", licenses::SOURCE)}
                }

                {notice_row(
                    __scope,
                    "GNU General Public License v3.0",
                    "SetListArray's own license: version 3, or any later version".to_string(),
                    None,
                    licenses::GPL_TEXT,
                    Open::App,
                    open,
                )}

                {section(__scope, "Built with")}

                div { style: {format!("{T_META} padding: 10px 0 6px;")},
                    "SetListArray is built on open-source software. Each package is listed under \
                     the license it came with; tap one to read it."
                }

                for (i, notice) in NOTICES.iter().enumerate() {
                    div { key: i,
                        {notice_row(
                            __scope,
                            notice.license,
                            licenses::used_by_summary(notice.crates),
                            Some(notice.crates),
                            notice.text,
                            Open::ThirdParty(i),
                            open,
                        )}
                    }
                }
            }
        }
    }
}

/// A group heading, the same `section-caps` Settings uses for its own.
fn section(scope: &mut RenderScope, label: &'static str) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            style: {format!("{T_SECTION_CAPS} color: var(--sla-muted); padding: 26px 0 2px;")},
            {label}
        }
    }
}

/// One license: closed, a name and a line of who ships it; open, the full list
/// of crates with their versions and then the text itself. The whole header is
/// the tap target, and tapping an open row closes it.
fn notice_row(
    scope: &mut RenderScope,
    title: &'static str,
    summary: String,
    crates: Option<&'static [&'static str]>,
    text: &'static str,
    which: Open,
    open: Signal<Option<Open>>,
) -> NodeHandle {
    let __scope = scope;
    let is_open = move || open.get() == Some(which);
    rsx! {
        div { style: "display: flex; flex-direction: column; border-bottom: 1px solid var(--sla-hairline-soft);",
            div {
                onclick: move || open.update(|o| *o = if *o == Some(which) { None } else { Some(which) }),
                style: "display: flex; align-items: center; gap: 12px; padding: 12px 0; min-height: 44px;",
                div { style: "flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px;",
                    span { style: {T_BODY}, {title} }
                    span { style: {T_META_SMALL}, {summary} }
                }
                if is_open() {
                    span {
                        style: "display: flex; align-items: center; color: var(--sla-muted);",
                        {icon(__scope, TablerIcon::ChevronDown, 15)}
                    }
                } else {
                    span {
                        style: "display: flex; align-items: center; color: var(--sla-muted);",
                        {icon(__scope, TablerIcon::ChevronRight, 15)}
                    }
                }
            }
            if is_open() {
                div { style: "display: flex; flex-direction: column; gap: 8px; padding: 0 0 16px;",
                    if let Some(crates) = crates {
                        div { style: {T_META_SMALL}, {format!("Used by {}", crates.join(", "))} }
                    }
                    for (i, paragraph) in licenses::paragraphs(text).into_iter().enumerate() {
                        div { key: i, style: {format!("{T_META} color: var(--sla-ink);")}, {paragraph} }
                    }
                }
            }
        }
    }
}
