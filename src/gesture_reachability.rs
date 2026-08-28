//! A guard for the fault card K15 proved: the desktop backend answers every
//! rsx event handler there is, so a screen can be designed, built and
//! reviewed entirely against handlers Android's touch recogniser can never
//! deliver, and nothing about running the app — on the one backend anyone
//! runs it on day to day — will say so.
//!
//! `TouchGesture::process` (`rinch/src/shell/android_runtime.rs`) turns a
//! finger into events like this:
//!
//! | `MotionEvent`            | what the app gets                              |
//! | ------------------------ | ----------------------------------------------- |
//! | `Down`                   | `MouseMove` at the touch point. No `MouseDown`.  |
//! | `Move`, under 8px        | nothing                                          |
//! | `Move`, past 8px         | `MouseWheel` at the touch-down origin. No `MouseMove`. |
//! | `Up`, finger still       | `MouseDown` immediately followed by `MouseUp`    |
//! | `Up`, finger moved       | nothing at all                                   |
//!
//! Everything below is either a direct reading of that table, or — for
//! `oncontextmenu` — of what rinch#266 changed about it. `src/bin/
//! gesture_probe.rs` is the harness that established the desktop half
//! empirically and is where card C6 found this out; see its module doc and
//! the README's "Touch on Android is a tap and a scroll, and nothing else"
//! for the long version.
//!
//! ## The table
//!
//! Every rsx event handler the app could plausibly reach for, reachable on
//! Android or not, with the one-line reason:
//!
//! | handler          | Android?  | why                                                                 |
//! | ----------------- | :-------: | -------------------------------------------------------------------- |
//! | `onclick`         | yes       | a still finger's `Up` synthesises `MouseDown` immediately followed by `MouseUp` at the same point — that pair *is* a click, and it's the one gesture the recogniser has delivered from day one. |
//! | `oninput`         | yes       | driven by the IME/keyboard, which never goes through `TouchGesture::process` at all — typing isn't a touch gesture. |
//! | `onsubmit`        | yes       | fired by the form's own submit machinery (Enter/Done on the keyboard, or a submit-triggering click) — same reasoning as `oninput`. |
//! | `oncontextmenu`   | yes (#266) | a press held past the long-press timeout now synthesises a right-button press that `RinchApp` routes through `dispatch_oncontextmenu`, the same path a desktop right-click takes. Before rinch#266 nothing on Android produced this event at all — see `src/menu.rs`. |
//! | `onmousedown`     | no        | `Down` delivers only `MouseMove`; the only `MouseDown` Android ever emits arrives already paired with its `MouseUp` at `Up`, so a handler never observes a "pressed, not yet released" state. |
//! | `onmousemove`     | no        | `Down` gives one `MouseMove`, but a moving finger never repeats it — under 8px is silent, past 8px is `MouseWheel` instead. There is no move stream to track. |
//! | `onmouseup`       | no        | fires only at `Up` with a still finger, in the same batch as `MouseDown` (indistinguishable from a click), and not at all after a moving finger lifts. |
//! | `ondragstart`     | no        | rinch arms a pending drag on `MouseDown` and promotes it on the first `MouseMove` more than 5px away (`app/event_dispatch.rs`); Android's only `MouseDown` is already paired with its `MouseUp`, so the pending drag is armed and consumed in the same batch and becomes a click instead. Waits on stage 3 (real pointer events with capture) — currently parked. |
//! | `ondragmove`      | no        | never fires: nothing arms the drag it would promote — see `ondragstart`. |
//! | `ondragend`       | no        | never fires, for the same reason: a drag that never starts never ends. |
//! | `ondragenter`     | no        | fires only during an active native drag, which never begins on Android. |
//! | `ondragover`      | no        | same as `ondragenter` — no drag ever gets underway to be over anything. |
//! | `ondrop`          | no        | same root cause as `ondragstart` — nothing ever completes because nothing ever starts. |
//! | `onscroll`        | no        | the vertical half genuinely does fire `data-onscroll` when a scroll container moves — but the horizontal half a swipe needs dispatches no handler at all (`event_dispatch.rs` moves `scroll_offset.0` and stops there), so nothing here can tell a scroll from a swipe or catch where it ends. Kept with the parked family rather than promoted alone: the only reason this app would reach for `onscroll` today is to build the gesture the asymmetry breaks. rinch#267 is stage 2 (pointer-cancel semantics); stage 3 is what actually lets an app tell scroll and swipe apart. |
//!
//! Stage 3 — real pointer events with capture, where the scroll-or-not
//! decision is finally deferred to the DOM instead of decided by the shell 8
//! pixels in — is what drag and swipe both wait on, and it is not written
//! yet. Until it lands, every one of the ten handlers above stays off the
//! allowlist below no matter how tempting a screen makes it look on desktop.
//!
//! ## Why `src/bin/` is exempt
//!
//! `src/bin/gesture_probe.rs` exists *precisely* to write every handler in
//! the "no" column and drive it on both backends — that is the harness that
//! produced the table above, and card C6's whole point was to spike a
//! gesture there before designing a screen around it. A tool built to prove
//! an event is unreachable necessarily contains the event; exempting it is
//! not a loophole, it's the reason the tool is allowed to exist at all. The
//! other two binaries (`capture_probe`, `probe`) don't touch input handlers
//! and are excluded along with it only because the scan walks `src/bin/` as
//! a whole rather than naming files one at a time.
//!
//! ## What this guard is not
//!
//! It answers one question — *can Android deliver this handler at all* — and
//! card K22 is the reminder that it is not the only way a screen can work on
//! the desktop and be dead on a phone, nor even the most likely one.
//!
//! There, every menu item carried `onclick`, the one handler the table has
//! always called reachable, and every tap on one closed the menu without
//! running anything. The handler was never the problem: `DropdownMenu`'s
//! dismiss backdrop was `position: fixed`, which Rinch treats as viewport-level
//! content hoisted out of every ancestor clip and stacking context, so it sat
//! *above* the panel whose `z-index` was supposed to outrank it and swallowed
//! the tap before the item ever saw it. A handler that cannot be reached
//! because of where its box ended up in the paint order is invisible to a scan
//! over handler names, and no row here could have said so.
//!
//! It is also the reason there is no row: the fault was not a divergence.
//! K22 reproduced identically on the desktop with a mouse — the phone was
//! where it was noticed, not where it lived. The thing that would have caught
//! it is the one that did: driving the real app and watching the item not
//! fire.
//!
//! Card K23 is the same lesson from the far side, and worth stating because
//! it is the one shape where "works on the desktop, dead on the phone" is
//! *not* about handlers at all. Every bottom sheet in the app stopped opening
//! on Android. The chip's `onclick` fired, `nav.sort_sheet_open` flipped to
//! `true`, and every style closure re-ran — all of it provable from `adb
//! logcat`, and none of it visible on the glass. What was missing was the
//! frame clock: the Android shell never sent `PlatformEvent::AboutToWait`, so
//! CSS transitions never advanced, so the panel stayed parked 700px below the
//! fold and the scrim stayed at opacity 0 while the sheet root's
//! `pointer-events: auto` — not animatable, so applied at once — put an
//! invisible full-screen box over the app. The sheet was open, and the next
//! tap anywhere landed on its scrim and closed it. Fixed upstream in
//! rinch#325.
//!
//! So: a handler can be reachable, run, and change the state it was written to
//! change, and the screen can still be wrong, because between the state and
//! the pixels there is a layout, a paint order, and a clock — and this table
//! has an opinion about none of them. When a screen is dead on the phone,
//! ruling this file's question out is the first step, not the last. The
//! cheapest next one, twice now, has been to put a `log::info!` in the handler
//! and read `adb logcat`: it splits "the tap never arrived" from "the tap
//! arrived and the frame did not" in one build, and those two have nothing in
//! common but the symptom.
//!
//! ## Why an unknown handler fails instead of passing by omission
//!
//! [`is_reachable`] returns `None` for a name that isn't in the table at
//! all, and [`scan_shipping_src`]'s caller treats `None` as a failure just
//! like `Some(false)`. That's the entire value of this guard: if it only
//! failed on names it already recognised as unreachable, adding
//! `onpointerdown:` to a screen — a handler this table has never heard of —
//! would sail through, because nothing here has an opinion about it yet.
//! Failing closed forces the table to be updated with a reachable/not
//! decision (and the reasoning behind it) before the handler can ship,
//! which is exactly the conversation card K15 wants to force every time,
//! not just this once.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Handlers proven reachable on Android today, and why — see the table in
/// the module doc. This is the allowlist the scan checks shipping code
/// against.
const REACHABLE: &[(&str, &str)] = &[
    (
        "onclick",
        "a still finger's Up synthesises MouseDown immediately followed by \
         MouseUp at the same point — that pair is a click.",
    ),
    (
        "oninput",
        "driven by the IME/keyboard, which never goes through \
         TouchGesture::process.",
    ),
    (
        "onsubmit",
        "fired by the form's own submit machinery, not by the touch/drag \
         pipeline.",
    ),
    (
        "oncontextmenu",
        "reachable since rinch#266: a held press now synthesises a \
         right-button press routed through dispatch_oncontextmenu.",
    ),
];

/// Handlers proven *not* reachable on Android today (or not safely usable
/// for the only reason this app would want them — see `onscroll` in the
/// module doc), and why. Finding one of these in shipping code is exactly
/// what this guard exists to catch: it worked when it was written, on the
/// desktop backend, and would ship silently dead on a phone.
const UNREACHABLE: &[(&str, &str)] = &[
    (
        "onmousedown",
        "Down delivers only MouseMove; the one MouseDown Android emits is \
         already paired with its MouseUp at Up.",
    ),
    (
        "onmousemove",
        "a moving finger never produces MouseMove — under 8px is silent, \
         past 8px is MouseWheel instead.",
    ),
    (
        "onmouseup",
        "fires only at Up with a still finger, in the same batch as \
         MouseDown, and not at all after a moving finger lifts.",
    ),
    (
        "ondragstart",
        "the pending drag MouseMove would promote never gets a MouseDown \
         to arm on its own — waits on stage 3, currently parked.",
    ),
    (
        "ondragmove",
        "never fires: nothing arms the drag it would promote.",
    ),
    (
        "ondragend",
        "never fires: a drag that never starts never ends.",
    ),
    (
        "ondragenter",
        "fires only during an active native drag, which never begins.",
    ),
    (
        "ondragover",
        "fires only during an active native drag, which never begins.",
    ),
    (
        "ondrop",
        "same root cause as ondragstart — nothing completes because \
         nothing starts.",
    ),
    (
        "onscroll",
        "the vertical half fires, but the horizontal half a swipe needs \
         dispatches nothing, so nothing here can tell a scroll from a \
         swipe — parked with the rest pending stage 3.",
    ),
];

/// `Some(true)`/`Some(false)` for a handler the table has a decision on,
/// `None` for one it has never seen. `None` is deliberately not the same as
/// "assume it's fine" — see the module doc's "Why an unknown handler fails".
fn is_reachable(name: &str) -> Option<bool> {
    if REACHABLE.iter().any(|(n, _)| *n == name) {
        Some(true)
    } else if UNREACHABLE.iter().any(|(n, _)| *n == name) {
        Some(false)
    } else {
        None
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Blank out every comment and every string/char literal in `src`, replacing
/// their contents with spaces while leaving newlines (and everything else)
/// exactly where they were. The result is the same length, in the same
/// lines, as the input — only code positions still hold their original
/// characters — so a scan over the output can't be tripped by prose or by
/// literal text, and any position it does match is still reportable at the
/// original file's line numbers.
///
/// This is not a Rust lexer; it only needs to get comments and literals
/// right, because those are the only two things that can make ordinary
/// English (a doc comment) or arbitrary bytes (an HTML fixture in a test)
/// look like an rsx attribute. It handles line comments, nested block
/// comments, escaped string/char literals, and raw strings/byte-strings of
/// any `#` depth (`r"…"`, `r#"…"#`, `br##"…"##`, …) — everything this crate
/// actually uses (checked by hand before writing this).
fn mask(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut out = chars.clone();

    let blank = |out: &mut [char], start: usize, end: usize| {
        for c in &mut out[start..end] {
            if *c != '\n' {
                *c = ' ';
            }
        }
    };

    // Is `chars[i]` the start of a raw-string prefix (`r` or `br`), given
    // what came before it? Returns the position of the `r` itself (which is
    // `i`, or `i - 1` if `chars[i]` is the `b` of `br`).
    let raw_string_r_at = |chars: &[char], i: usize| -> Option<usize> {
        let boundary_before = |k: usize| k == 0 || !is_ident_char(chars[k - 1]);
        if chars[i] == 'r' && boundary_before(i) {
            Some(i)
        } else if chars[i] == 'b'
            && i + 1 < chars.len()
            && chars[i + 1] == 'r'
            && boundary_before(i)
        {
            Some(i + 1)
        } else {
            None
        }
    };

    let mut i = 0;
    while i < n {
        let c = chars[i];
        if c == '/' && i + 1 < n && chars[i + 1] == '/' {
            let start = i;
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            blank(&mut out, start, i);
        } else if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            // Rust block comments nest.
            let start = i;
            i += 2;
            let mut depth = 1usize;
            while i < n && depth > 0 {
                if chars[i] == '/' && i + 1 < n && chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && i + 1 < n && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            blank(&mut out, start, i);
        } else if let Some(r_at) = raw_string_r_at(&chars, i) {
            let mut j = r_at + 1;
            let mut hashes = 0usize;
            while j < n && chars[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < n && chars[j] == '"' {
                let start = i;
                j += 1; // past the opening quote
                loop {
                    if j >= n {
                        break;
                    }
                    if chars[j] == '"' {
                        let close_hashes = chars[j + 1..]
                            .iter()
                            .take(hashes)
                            .take_while(|&&c| c == '#')
                            .count();
                        if close_hashes == hashes {
                            j += 1 + hashes;
                            break;
                        }
                    }
                    j += 1;
                }
                blank(&mut out, start, j);
                i = j;
            } else {
                // `r`/`br` that wasn't actually a raw-string prefix (e.g. a
                // variable literally named `r`). Leave it as ordinary code.
                i += 1;
            }
        } else if c == '"' {
            let start = i;
            i += 1;
            while i < n && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < n {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i < n {
                i += 1; // closing quote
            }
            blank(&mut out, start, i);
        } else if c == '\'' {
            // A char literal ('x', '\n', '\u{2014}') or a lifetime ('a).
            // Only the former is a literal to blank; a lifetime is an
            // ordinary identifier and is left alone.
            let start = i;
            let mut j = i + 1;
            if j < n && chars[j] == '\\' {
                j += 1;
                if j < n {
                    j += 1;
                }
                if j > 0 && j - 1 < n && chars[j - 1] == 'u' && j < n && chars[j] == '{' {
                    while j < n && chars[j] != '}' {
                        j += 1;
                    }
                    if j < n {
                        j += 1;
                    }
                }
            } else if j < n {
                j += 1;
            }
            if j < n && chars[j] == '\'' {
                j += 1;
                blank(&mut out, start, j);
                i = j;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    out.into_iter().collect()
}

/// Every `on…:` that sits in rsx attribute position in `masked` code — that
/// is, a bare, all-lowercase identifier starting with `on`, at an identifier
/// boundary on both ends, followed (across any whitespace) by a single `:`
/// that isn't the start of a `::` path.
///
/// Restricting the identifier to `on` + lowercase letters only (no digits,
/// no underscore) is deliberate, not an oversight: every rsx/DOM event
/// handler this framework recognises is a single run-together lowercase
/// word, the same way HTML attribute names are (`onclick`, `onmousedown`,
/// `ondragstart`…). Rinch component props that merely start with `on`
/// don't follow that shape — `on_close` (a menu's dismiss callback) and
/// `on_tint`/`on_accent` (theme colour fields, not callbacks at all; see
/// `src/theme.rs`) all put an underscore right after the `on`, which stops
/// this scan cold before it ever reaches their colon. If a real Rinch
/// event handler is ever spelled with an underscore or a digit, this
/// pattern would miss it silently — the fallback is the same as for any
/// other unknown handler: `cargo test` catching a table this scan
/// stopped matching is a code-review conversation, not a runtime one.
fn find_handlers(masked: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = masked.chars().collect();
    let n = chars.len();
    let mut results = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;

    while i < n {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }

        let boundary_before = i == 0 || !is_ident_char(chars[i - 1]);
        if boundary_before && c == 'o' && i + 1 < n && chars[i + 1] == 'n' {
            let start = i;
            let mut j = i + 2;
            while j < n && chars[j].is_ascii_lowercase() {
                j += 1;
            }
            // Require at least one letter past "on", and that the word ends
            // at an identifier boundary (not merely where lowercase runs
            // out into a digit or underscore mid-identifier).
            let word_len_ok = j > i + 2;
            let end_boundary_ok = j >= n || !is_ident_char(chars[j]);
            if word_len_ok && end_boundary_ok {
                let mut k = j;
                while k < n && chars[k].is_whitespace() {
                    k += 1;
                }
                if k < n && chars[k] == ':' && (k + 1 >= n || chars[k + 1] != ':') {
                    let name: String = chars[start..j].iter().collect();
                    results.push((line, name));
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }

    results
}

/// Every `.rs` file under `src/`, depth first, except `src/bin/` — see the
/// module doc for why that directory is exempt.
fn collect_shipping_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("the crate's own source directory") {
        let path = entry.expect("a readable directory entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "bin") {
                continue;
            }
            collect_shipping_rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Scan the shipping source for every `on…:` in attribute position, paired
/// with the file and line it was found at.
fn scan_shipping_src() -> Vec<(PathBuf, usize, String)> {
    let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_shipping_rust_files(&src_root, &mut files);
    files.sort();

    let mut found = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).expect("a source file this crate compiles");
        for (line, name) in find_handlers(&mask(&text)) {
            found.push((path.clone(), line, name));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guard itself: every handler the shipping UI actually writes has
    /// to be one this crate has already decided is reachable on Android.
    /// A handler the table marks unreachable, or has never seen at all,
    /// fails the build with the file and line it was found at — see the
    /// module doc for why "never seen" is a failure and not a pass.
    #[test]
    fn shipping_ui_only_uses_handlers_reachable_on_android() {
        let found = scan_shipping_src();

        let failures: Vec<String> = found
            .iter()
            .filter_map(|(path, line, name)| match is_reachable(name) {
                Some(true) => None,
                Some(false) => {
                    let reason = UNREACHABLE
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, r)| *r)
                        .unwrap_or("");
                    Some(format!(
                        "{}:{line} — `{name}:` is not reachable on Android: {reason}",
                        path.display()
                    ))
                }
                None => Some(format!(
                    "{}:{line} — `{name}:` is not in this test's reachability table at all. \
                     Add a row to the table in src/gesture_reachability.rs with a reachable/not \
                     decision and a one-line reason before shipping it.",
                    path.display()
                )),
            })
            .collect();

        assert!(
            failures.is_empty(),
            "found {} handler(s) the shipping UI can't rely on on Android:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    /// The floor under the guard above: prove the scan itself only finds
    /// real rsx attributes, not the English this file is full of.
    ///
    /// A naive `\bon[a-z]+\s*:` over the raw source also matches "one:" and
    /// "only:" in doc comments and a format string (`src/lib.rs`,
    /// `src/capture/mod.rs`, `src/capture/detect.rs`,
    /// `src/screens/setlist_detail.rs`) — five false positives, found by
    /// running the unmasked scan against this tree before `mask` existed.
    /// This test is what keeps that regex change from regressing quietly:
    /// if masking ever breaks, the count below jumps from 4 distinct
    /// handlers back to 6.
    #[test]
    fn the_scan_ignores_prose_and_finds_exactly_the_shipping_handlers() {
        let found = scan_shipping_src();
        let names: BTreeSet<&str> = found.iter().map(|(_, _, name)| name.as_str()).collect();

        // Printed for a human reading `cargo test -- --nocapture`; the
        // assert below is what actually holds the line.
        println!("gesture_reachability: handlers found in shipping code: {names:?}");

        let expected: BTreeSet<&str> = REACHABLE.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names, expected,
            "the scan found a different handler set than the four the shipping UI is known \
             to use — either a new handler shipped without a table decision (see the other \
             test in this file) or the mask/scan regressed and started matching prose again"
        );
    }
}
