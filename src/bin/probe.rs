//! Minimal repro harness: a list row shaped like the library's, with a child
//! component on each side and two text children in the middle.

use rinch::prelude::*;

/// Background comes from a custom property set on an ancestor.
#[component]
fn VarBadge(label: String) -> NodeHandle {
    rsx! {
        div {
            style: "width: 38px; height: 38px; background: var(--probe-accent); color: #fff; \
                    display: flex; align-items: center; justify-content: center;",
            span { {label.clone()} }
        }
    }
}

#[component]
fn Badge(label: String) -> NodeHandle {
    rsx! {
        div {
            style: "width: 38px; height: 38px; background: #c04a24; color: #fff; \
                    display: flex; align-items: center; justify-content: center;",
            span { {label.clone()} }
        }
    }
}

/// Same shape as the library's attachment thumb: the style comes from a block
/// expression rather than a literal.
#[component]
fn BlockStyled(filled: bool) -> NodeHandle {
    rsx! {
        div {
            style: {
                match filled {
                    true => "width: 38px; height: 38px; background: #2f6f4e;".to_string(),
                    false => "width: 38px; height: 38px; border: 1px dashed #999;".to_string(),
                }
            },
        }
    }
}

#[component]
fn app() -> NodeHandle {
    rsx! {
        div {
            style: "padding: 20px; background: #ffffff; color: #111111; \
                    display: flex; flex-direction: column; gap: 16px;",

            // 1: two component siblings, nothing else.
            div { style: "display: flex; align-items: center; gap: 12px;",
                div { style: "width: 30px; font-size: 12px;", "1" }
                Badge { label: "AAA" }
                Badge { label: "BBB" }
            }

            // 2: component, plain div, component.
            div { style: "display: flex; align-items: center; gap: 12px;",
                div { style: "width: 30px; font-size: 12px;", "2" }
                Badge { label: "CCC" }
                div { style: "flex: 1;",
                    div { style: "font-size: 18px;", "Title line" }
                    div { style: "font-size: 12px; color: #666666;", {"meta line".to_string()} }
                }
                Badge { label: "DDD" }
            }

            // 3: a component after a plain div.
            div { style: "display: flex; align-items: center; gap: 12px;",
                div { style: "width: 30px; font-size: 12px;", "3" }
                div { style: "font-size: 12px;", "before" }
                Badge { label: "EEE" }
            }

            // 4: style from a block expression, filled and dashed.
            div { style: "display: flex; align-items: center; gap: 12px;",
                div { style: "width: 30px; font-size: 12px;", "4" }
                BlockStyled { filled: true }
                BlockStyled { filled: false }
            }

            // 6: a custom property set here, read inside a child component
            // and by a plain div.
            div {
                style: "display: flex; align-items: center; gap: 12px; --probe-accent: #3d5a9e;",
                div { style: "width: 30px; font-size: 12px;", "6" }
                VarBadge { label: "FFF" }
                div { style: "font-size: 14px; color: var(--probe-accent);", "var text" }
            }

            // 7: after a div with element children — a plain div, then a
            // component wrapped in a plain div.
            div { style: "display: flex; align-items: center; gap: 12px;",
                div { style: "width: 30px; font-size: 12px;", "7" }
                div { style: "flex: 1;",
                    div { style: "font-size: 18px;", "Title line" }
                    div { style: "font-size: 12px; color: #666666;", "meta line" }
                }
                div { style: "width: 38px; height: 38px; background: #7a4c86;" }
                div { Badge { label: "GGG" } }
            }

            // 8: same as 7, but the middle block is an explicit flex column
            // instead of a block container with block children.
            div { style: "display: flex; align-items: center; gap: 12px;",
                div { style: "width: 30px; font-size: 12px;", "8" }
                div { style: "flex: 1; display: flex; flex-direction: column;",
                    div { style: "font-size: 18px;", "Title line" }
                    div { style: "font-size: 12px; color: #666666;", "meta line" }
                }
                div { style: "width: 38px; height: 38px; background: #7a4c86;" }
                Badge { label: "HHH" }
            }

            // 9: `flex: 1` shorthand on a middle child with no children.
            div { style: "display: flex; align-items: center; gap: 12px;",
                div { style: "width: 30px; font-size: 12px;", "9" }
                div { style: "width: 38px; height: 38px; background: #c04a24;" }
                div { style: "flex: 1; font-size: 14px;", "grows" }
                div { style: "width: 38px; height: 38px; background: #7a4c86;" }
            }

            // 10: the same, written as longhands.
            div { style: "display: flex; align-items: center; gap: 12px;",
                div { style: "width: 30px; font-size: 12px;", "10" }
                div { style: "width: 38px; height: 38px; background: #c04a24;" }
                div { style: "flex-grow: 1; flex-shrink: 1; flex-basis: 0; font-size: 14px;", "grows" }
                div { style: "width: 38px; height: 38px; background: #7a4c86;" }
            }

            // 5: a keyed for loop of plain divs.
            div { style: "display: flex; align-items: center; gap: 6px;",
                div { style: "width: 30px; font-size: 12px;", "5" }
                for i in 0u8..3 {
                    div { key: i, style: "width: 8px; height: 8px; background: #111111;" }
                }
            }
        }
    }
}

fn main() {
    run("probe", 460, 620, app);
}
