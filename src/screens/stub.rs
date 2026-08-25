//! Placeholder for the wireframe screens still to be built. Each names the
//! wireframe it will be built from, so the next person knows where to look.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::store::NavStore;
use crate::theme::{SCREEN_PAD, T_META, T_SCREEN_TITLE};
use crate::ui::IconButton;

#[component]
pub fn Stub(title: String, wireframe: String) -> NodeHandle {
    let nav = use_store::<NavStore>();

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",
            div { style: "padding: 2px 18px 8px; display: flex; align-items: center;",
                IconButton { glyph: TablerIcon::ChevronLeft, size: 19, onclick: move || nav.back() }
            }
            div { style: {format!("padding: 0 {SCREEN_PAD};")},
                div { style: {format!("{T_SCREEN_TITLE}")}, {title.clone()} }
                div { style: {format!("{T_META} margin-top: 10px;")},
                    {format!("Not built yet. Wireframe {wireframe} in the handoff is the authority on this screen.")}
                }
            }
        }
    }
}
