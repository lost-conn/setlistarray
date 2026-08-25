//! Layout probe for a Rinch flex fault the app has to live with.
//!
//! A `flex: 1` child is not shrunk to the space left over by its siblings; it
//! takes its content width, so the row overflows its container and anything
//! after it is laid out past the viewport. Row A is the plain case, B shows it
//! is not a wrap/overflow-visible question, C is the same children with
//! nothing after the growing child (fine), D is a percentage width instead of
//! flex-grow (same fault).
//!
//! Present on both d25f646 and 1f16bed, so this is long-standing rather than a
//! regression. In the app it is why a library row measures 501px inside a
//! 447px content box and the confidence dots sit off the right edge.
//!
//! `cargo run --release --bin probe`

// Same rsx! lint artifact as main.rs: bindings used inside generated closures
// are reported as unused.
#![allow(unused_variables)]

use rinch::prelude::*;

/// The library's attachment thumb, reduced: a block box whose only content is
/// an inline span, behind an `if let`.
#[component]
fn Thumb(label: String, show: bool) -> NodeHandle {
    rsx! {
        div {
            style: "width: 38px; height: 38px; background: #c04a24; \
                    display: flex; align-items: center; justify-content: center;",
            if show {
                span { style: "font-size: 9px; color: #ffffff;", {label.clone()} }
            }
        }
    }
}

/// The library row, reduced.
#[component]
fn Row(title: String, meta: String) -> NodeHandle {
    rsx! {
        div {
            style: "display: flex; align-items: center; gap: 13px; padding: 11px 0; \
                    border-bottom: 2px solid #2f6f4e;",
            Thumb { label: "TXT", show: true }
            div { style: "flex: 1; min-width: 0;",
                div { style: "font-size: 18px;", {title.clone()} }
                div { style: "font-size: 13px; color: #3d5a9e; margin-top: 2px;", {meta.clone()} }
            }
            div { style: "display: flex; gap: 4px; flex-shrink: 0;",
                for i in 0u8..3 {
                    div { key: i, style: "width: 6px; height: 6px; background: #7a4c86;" }
                }
            }
        }
    }
}

const ROW: &str = "display: flex; align-items: center; gap: 12px;";
const LABEL: &str = "width: 40px; font-size: 12px;";
const ORANGE: &str = "width: 38px; height: 38px; background: #c04a24;";
const PURPLE: &str = "width: 38px; height: 38px; background: #7a4c86;";

#[component]
fn app() -> NodeHandle {
    rsx! {
        div {
            style: "padding: 20px; background: #ffffff; color: #111111; \
                    display: flex; flex-direction: column; gap: 18px;",

            // A — fixed, grows, fixed. The purple box never appears.
            div { style: {ROW},
                div { style: {LABEL}, "A" }
                div { style: {ORANGE} }
                div { style: "flex: 1; font-size: 14px;", "grows" }
                div { style: {PURPLE} }
            }

            // B — the same, wrapping. It does not wrap to a second line.
            div { style: {format!("{ROW} flex-wrap: wrap;")},
                div { style: {LABEL}, "B" }
                div { style: {ORANGE} }
                div { style: "flex: 1; font-size: 14px;", "grows" }
                div { style: {PURPLE} }
            }

            // C — nothing after the growing child. Fine.
            div { style: {ROW},
                div { style: {LABEL}, "C" }
                div { style: {ORANGE} }
                div { style: {PURPLE} }
                div { style: "flex: 1; font-size: 14px;", "grows" }
            }

            // E — the library row, reduced. On 1f16bed the thumb fill, the
            // "TXT" badge, the meta line, the border and the dots are all
            // absent while the title paints; on d25f646 every part draws.
            Row { title: "Landslide", meta: "Fleetwood Mac · Eb" }

            // D — a percentage width instead of flex-grow. Same fault.
            div { style: {ROW},
                div { style: {LABEL}, "D" }
                div { style: {ORANGE} }
                div { style: "width: 70%; font-size: 14px;", "70%" }
                div { style: {PURPLE} }
            }
        }
    }
}

fn main() {
    run("probe", 460, 300, app);
}
