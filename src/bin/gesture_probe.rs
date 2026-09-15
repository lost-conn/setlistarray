//! Spike harness for card C6 — do drag and swipe actually work?
//!
//! Card C6 asks for reorder-by-drag-handle and swipe-left-to-remove inside a
//! setlist. Three separate pointer/paint faults (K10, K11, K14) have already
//! been found in this framework by this project, so the gestures are proven
//! here, in the smallest thing that can hold them, before any setlist UI is
//! built on top of them.
//!
//! It reproduces the exact shape the real screen has and nothing else: rows
//! inside an `overflow-y: auto` scroll box, each with a drag handle. The two
//! questions are
//!
//!   (a) does a drag fire its events in the order the feature needs, with
//!       coordinates that can be turned into "row 3 goes above row 1"; and
//!   (b) can a horizontal swipe on a row be told apart from a vertical scroll
//!       of the list the row is sitting in.
//!
//! Every handler appends to a single text node (`.probe-log`), so the whole
//! event trace can be read back over the `--features devtools` IPC with
//! `query_selector` + `get_text_content` — no screenshots, and therefore none
//! of the HiDPI coordinate trouble `docs/NOTES.md` warns about. Drive it with
//! `scripts/gesture-probe.py`.
//!
//! ```text
//! cargo run --release --features devtools --bin gesture_probe
//! ```
//!
//! ## What it found
//!
//! On the **desktop** backend, both gestures work and the trace is unambiguous:
//!
//! ```text
//! mousedown r1 @29,415 elem 12,387 467x56 | dragstart r1 @29,436
//!   | dragenter r2 | dragover r2 @29,457 | … | dragenter r4
//!   | dragover r4 @29,583 | mouseup r4 | drop r4 @29,583 | dragend r1
//! ```
//!
//! — the source row from `dragstart`, the target row from `drop`, which is
//! exactly "row 1 goes after row 4". It still works with the list scrolled
//! (layout boxes carry the scroll offset), a plain tap on a `draggable` handle
//! is still a tap, and a horizontal drag across a row body reports its own
//! delta (`mouseup r6 @339,695 d=-120,0`). Nothing competes with it, because a
//! mouse drag does not scroll an `overflow-y: auto` box on the desktop at all —
//! only the wheel does, and the wheel never reaches the row's handlers.
//!
//! On **Android** neither gesture can fire, and the reason is in the shell
//! rather than in anything an app can write. See `docs/ANDROID.md`, "Touch on
//! Android is a tap and a scroll, and nothing else". The short version: `MouseDown` is
//! emitted only at finger-*up*, immediately followed by `MouseUp` at the same
//! point, and only for a finger that never travelled 8px — so the runtime's
//! pending drag is created and consumed in one batch and becomes a click. A
//! finger that moves produces `MouseWheel` deltas addressed at the scroll
//! container and no app-visible event whatsoever, including none when it lifts.
//!
//! Which is why `src/screens/setlist_detail.rs` reorders with buttons. Keep
//! this probe until the Android recogniser is fixed upstream: it is the thing
//! that turns "gestures feel broken on the phone" back into a five-minute
//! answer.

// Same rsx! lint artifact as main.rs: bindings used inside generated closures
// are reported as unused.
#![allow(unused_variables)]

use rinch::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// The trace every handler writes into. A `Vec<String>` behind a signal so the
/// log node re-renders, plus a plain counter of the mousemoves that were
/// folded away — a swipe produces dozens and the interesting part is the first
/// and the last, not the middle.
#[derive(Clone, Copy)]
struct Trace {
    lines: Signal<Vec<String>>,
    moves: Signal<u32>,
}

impl Trace {
    fn push(self, line: String) {
        self.lines.update(|l| l.push(line));
    }

    /// A mousemove, recorded in full for the first few of a gesture and only
    /// counted after that.
    fn mouse_move(self, row: usize, x: f32, y: f32) {
        let n = self.moves.get();
        self.moves.set(n + 1);
        if n < 3 {
            self.push(format!("mousemove r{row} @{x:.0},{y:.0}"));
        }
    }

    fn reset(self) {
        self.lines.set(Vec::new());
        self.moves.set(0);
    }
}

const ROWS: usize = 20;

/// A pointer position, remembered across handler calls so a swipe can measure
/// itself. `Rc<RefCell<..>>` rather than a signal: nothing renders from it, and
/// writing a signal from inside a mousemove would re-render the list on every
/// pixel of a drag.
type Press = Rc<RefCell<Option<(f32, f32, usize)>>>;

#[component]
fn app() -> NodeHandle {
    let trace = Trace {
        lines: Signal::new(vec!["ready".to_string()]),
        moves: Signal::new(0),
    };
    let press: Press = Rc::new(RefCell::new(None));

    rsx! {
        div {
            style: "display: flex; flex-direction: column; height: 100vh; \
                    background: #FBF7F0; color: #1C1917; font-size: 12px;",

            // The trace, read back over the IPC. One text node on purpose:
            // `get_text_content` on a single node is the whole log.
            div {
                style: "flex-shrink: 0; height: 300px; overflow: hidden; padding: 8px; \
                        background: #F1E9DC; border-bottom: 2px solid #B54724; \
                        font-family: monospace; line-height: 1.35;",
                div {
                    class: "probe-log",
                    {move || {
                        let mut lines = trace.lines.get();
                        let moves = trace.moves.get();
                        if moves > 3 {
                            lines.push(format!("(+{} more mousemove)", moves - 3));
                        }
                        lines.join(" | ")
                    }}
                }
            }

            div {
                class: "probe-reset",
                onclick: move || trace.reset(),
                style: "flex-shrink: 0; padding: 6px 8px; background: #1C1917; color: #FBF7F0;",
                "reset trace"
            }

            // The shape that matters: rows inside a box that scrolls in the
            // *other* axis from the swipe. Question (b) lives right here.
            div {
                class: "probe-list",
                onscroll: move |top: f64| trace.push(format!("scroll {top:.0}")),
                style: "flex: 1; min-height: 0; overflow-y: auto; padding: 0 12px;",

                for i in 0..ROWS {
                    // Each `for` body is its own re-running closure, so it can
                    // only capture `Copy` values and owned clones (docs/NOTES.md).
                    let press = press.clone();
                    let press_move = press.clone();
                    let press_up = press.clone();
                    div {
                        key: i,
                        class: "probe-row",
                        style: "display: flex; align-items: center; gap: 10px; height: 56px; \
                                border-bottom: 1px solid #E7DFD4;",

                        onmousedown: move || {
                            let c = get_click_context();
                            *press.borrow_mut() = Some((c.mouse_x, c.mouse_y, i));
                            trace.push(format!(
                                "mousedown r{i} @{:.0},{:.0} elem {:.0},{:.0} {:.0}x{:.0}",
                                c.mouse_x, c.mouse_y,
                                c.element_x, c.element_y, c.element_width, c.element_height
                            ));
                        },
                        onmousemove: move || {
                            let c = get_click_context();
                            // Only trace moves that belong to a press that
                            // started on this row — a bare hover over the list
                            // would otherwise bury the gesture.
                            if press_move.borrow().is_some() {
                                trace.mouse_move(i, c.mouse_x, c.mouse_y);
                            }
                        },
                        onmouseup: move || {
                            let c = get_click_context();
                            let started = press_up.borrow_mut().take();
                            match started {
                                Some((sx, sy, _)) => trace.push(format!(
                                    "mouseup r{i} @{:.0},{:.0} d={:.0},{:.0}",
                                    c.mouse_x, c.mouse_y, c.mouse_x - sx, c.mouse_y - sy
                                )),
                                None => trace.push(format!("mouseup r{i} (no press)")),
                            }
                        },

                        // Drop target, so a drag over this row reports itself.
                        ondragenter: move || trace.push(format!("dragenter r{i}")),
                        ondragover: move || {
                            let c = get_click_context();
                            trace.push(format!("dragover r{i} @{:.0},{:.0}", c.mouse_x, c.mouse_y));
                        },
                        ondrop: move || {
                            let c = get_click_context();
                            trace.push(format!("drop r{i} @{:.0},{:.0}", c.mouse_x, c.mouse_y));
                        },

                        // The drag handle. `draggable="true"` is what puts the
                        // runtime into a pending drag on mousedown.
                        div {
                            class: "probe-handle",
                            draggable: "true",
                            ondragstart: move || {
                                let c = get_click_context();
                                trace.push(format!(
                                    "dragstart r{i} @{:.0},{:.0}", c.mouse_x, c.mouse_y
                                ));
                            },
                            ondragmove: move || {
                                let c = get_click_context();
                                trace.mouse_move(i, c.mouse_x, c.mouse_y);
                            },
                            ondragend: move || {
                                let c = get_click_context();
                                trace.push(format!(
                                    "dragend r{i} @{:.0},{:.0}", c.mouse_x, c.mouse_y
                                ));
                            },
                            style: "width: 34px; height: 40px; flex-shrink: 0; background: #E7DFD4; \
                                    display: flex; align-items: center; justify-content: center;",
                            "::"
                        }

                        div { style: "flex: 1;", {format!("row {i}")} }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
fn main() {
    run("gesture_probe", 393, 852, app);
}

// Desktop-only, like `probe.rs`. Kept compilable for Android so a whole-crate
// cross build still resolves.
#[cfg(target_os = "android")]
fn main() {}
