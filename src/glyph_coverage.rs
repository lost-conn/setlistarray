//! A guard for the fault cards K13 and K21 both found, on hardware, by
//! accident: a string this app draws contains a character no bundled font
//! carries, so on Android — which has no fontconfig behind the stack to hand
//! Parley a substitute the way a laptop's system fonts do — it comes back
//! `.notdef`, a tofu box, in the middle of a sentence. K13 was a chip label;
//! K21 was the chart editor's empty state, and it had never been seen on
//! hardware only because no screenshot session had happened to go down that
//! path. Nothing before this stopped the *next* one, which is the shape of
//! fault cards K15 and K20 already turned into a laptop-side `cargo test`:
//! this is that pattern applied here.
//!
//! ## Why this lexes instead of grepping
//!
//! A line-based scan over the raw source would trip on this repository's own
//! comments, which are long-form narrative prose (the house style — see
//! `CLAUDE.md`) and are full of exactly the punctuation this test cares
//! about: em dashes, mid-dots, curly quotes, arrows. Parsing each file into a
//! [`proc_macro2::TokenStream`] instead makes that a non-issue for ordinary
//! `//` and `/* */` comments, which never become tokens at all — but doc
//! comments (`///`, `//!`) are not free the same way: `proc_macro2`'s lexer
//! (matching what a real proc macro would see) rewrites them into
//! `#[doc = "…"]`/`#![doc = "…"]` attributes, literal string and all, so
//! `A♭ Major`/`A□ Major` sitting in `theme.rs`'s own doc comment about this
//! exact failure would otherwise be a literal this scan finds and has to be
//! taught to recognise as a doc attribute and skip, which [`is_doc_attribute`]
//! is. Lexing also reaches string literals that sit inside an `rsx!` macro
//! body — where most of this app's actual user-facing prose lives — which a
//! textual scan would have to know `rsx!`'s grammar to find at all.
//!
//! ## Why test modules are out
//!
//! A `#[cfg(test)] mod tests { … }` in this tree is where the capture
//! module keeps its fixtures (arbitrary web content, sitting in a literal on
//! purpose) and where the sort tests keep the Japanese kana that prove
//! `derive.rs`'s `highlight`/`runs` do the right thing on a script this app
//! never draws with a bundled face at all. Neither is a fault; both would be
//! a false positive here. [`skip_cfg_test_item`] detects the `#[cfg(test)]`
//! attribute on the token stream itself — the same shape [`is_cfg_test`]
//! checks — rather than hard-coding which files or line ranges to leave out,
//! so a new test module anywhere in the tree is exempt for free and a
//! non-test literal is never accidentally exempted by sitting near one.
//!
//! ## Why the bar is DejaVu Sans Mono
//!
//! A literal in `src/` doesn't know which of `FONT_DISPLAY`, `FONT_UI` or
//! `FONT_MONO` will end up drawing it — a `&'static str` sitting in a screen
//! module has no way to declare "I only ever land in body prose" — so the
//! only coverage guarantee that holds regardless of which stack answers is
//! the one face every stack now ends in (card K34, `theme::font_coverage`).
//! Checking against that face and nothing else is also why this doesn't need
//! to solve "is this string prose" in general: a route name, a CSS
//! declaration, a format string and a file path are all plain ASCII, and
//! DejaVu Sans Mono covers ASCII the same as every other bundled face, so
//! they pass without this test ever having to tell them apart from prose.
//! Deliberately not an `is_ascii()` shortcut, either — every character of
//! every literal is checked against the face's real `cmap`, ASCII included;
//! it happens that only non-ASCII characters ever fail, but that's a fact
//! about this font, not an assumption this test is allowed to make going in.
use std::path::{Path, PathBuf};
use std::str::FromStr;

use proc_macro2::{Delimiter, TokenStream, TokenTree};

use crate::theme::font_coverage;

/// Every `.rs` file this crate ships, `src/` walked depth-first with nothing
/// excluded by path — unlike `gesture_reachability`'s scan, `src/bin/` is not
/// exempt here. That scan carves out `src/bin/` because its whole subject is
/// event handlers a *shipping screen* writes; this one's subject is any
/// string a probe binary could just as easily get wrong, and the whole point
/// of lexing over hand-picking directories (see the module doc) is never
/// having to decide that a particular corner of the tree is safe by
/// inspection.
fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("the crate's own source directory") {
        let path = entry.expect("a readable directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// One string or char literal this scan found outside a `#[cfg(test)]` item,
/// decoded to the characters it actually denotes (see [`literal_text`]).
struct Found {
    line: usize,
    text: String,
}

/// Is this attribute's token stream (the contents of the `[...]` after `#` or
/// `#!`) a doc comment's `doc = "…"`? Checking only the first token is
/// deliberate: every doc comment `proc_macro2` rewrites takes this exact
/// shape, and a hand-written `#[doc(hidden)]` — the one other `doc` attribute
/// this crate could plausibly grow — carries no literal either, so treating
/// any `doc`-headed attribute as "nothing here to check" is correct either
/// way, not just for the common case.
fn is_doc_attribute(tokens: &[TokenTree]) -> bool {
    matches!(tokens.first(), Some(TokenTree::Ident(id)) if id == "doc")
}

/// Is this attribute's token stream exactly `cfg(test)` — the shape every
/// `#[cfg(test)]` in this tree actually takes (checked by hand before writing
/// this; nothing here uses a compound predicate like `cfg(any(test, …))`)?
fn is_cfg_test(tokens: &[TokenTree]) -> bool {
    let Some(TokenTree::Ident(id)) = tokens.first() else {
        return false;
    };
    if id != "cfg" {
        return false;
    }
    let Some(TokenTree::Group(group)) = tokens.get(1) else {
        return false;
    };
    group.delimiter() == Delimiter::Parenthesis && group.stream().to_string().trim() == "test"
}

/// Having just seen a `#[cfg(test)]` (or `#![cfg(test)]`), consume tokens
/// from `iter` without looking at any of them until the item it applies to
/// is fully behind us: either a bare `;` at this nesting level (a semicolon
/// mod, e.g. `pub mod test_support;`, or any other item with no body) or a
/// brace-delimited `Group` (`mod tests { … }`, `pub fn scratch(…) { … }`,
/// …). The brace group is never recursed into — its contents, however deep,
/// are exactly what this function exists to keep out of the scan.
///
/// A second attribute between the `#[cfg(test)]` and the item it decorates
/// (`#[cfg(test)]\n#[allow(dead_code)]\nfn …`) is handled without special
/// casing: its own `[...]` arrives as a `Group` too, but a *bracket*
/// delimiter, which this loop only ever treats as ordinary tokens to skip
/// past — only a *brace* group ends the search.
fn skip_cfg_test_item(iter: &mut proc_macro2::token_stream::IntoIter) {
    for tt in iter.by_ref() {
        match tt {
            TokenTree::Punct(p) if p.as_char() == ';' => return,
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => return,
            _ => {}
        }
    }
}

/// The text a string or char literal's source spelling actually denotes —
/// good enough for this scan, not a full literal parser (the same trade this
/// tree's `gesture_reachability::mask` makes, and for the same reason: this
/// only has to get the shapes this crate actually uses right, checked by
/// hand before writing it). Returns `None` for a literal kind that can never
/// carry a display character in the first place — a number, or a byte
/// string/byte char (`b"…"`, `b'x'`), which Rust rejects a literal
/// multi-byte UTF-8 character in at compile time, so neither can smuggle
/// anything this test needs to check.
///
/// A raw string/char (`r"…"`, `r#"…"#`) has no escapes at all by definition,
/// so its content between the delimiters is used unmodified. A cooked
/// string/char can carry a character two ways — typed directly as UTF-8 (the
/// common case in this tree; K13's and K21's own fixes both typed the glyph
/// itself, and so does `theme.rs`'s doc comment describing them) or spelled
/// `\u{XXXX}` (`derive.rs`'s curly-quote prose, `capture/reader.rs`'s
/// typography table) — and only the second form would slip past a scan that
/// read the raw source characters, so [`decode_cooked_escapes`] expands
/// `\u{…}` runs to the codepoint they name (and also unwinds Rust's
/// string-continuation escape — see that function's own doc for why that
/// turned out to matter here too). Every other escape this crate writes
/// (`\n \t \\ \" \' \r`) is itself ASCII, so leaving it un-expanded can never
/// manufacture a character that needs checking — only, at worst, fail to
/// notice one that was never there.
fn literal_text(raw: &str) -> Option<String> {
    if let Some(inner) = raw.strip_prefix('r') {
        let hashes = inner.chars().take_while(|&c| c == '#').count();
        let body = &inner[hashes..];
        let quoted = body.strip_prefix('"')?;
        let close = format!("\"{}", "#".repeat(hashes));
        return quoted.strip_suffix(&close).map(str::to_string);
    }
    if let Some(inner) = raw.strip_prefix('"') {
        return inner.strip_suffix('"').map(decode_cooked_escapes);
    }
    if let Some(inner) = raw.strip_prefix('\'') {
        return inner.strip_suffix('\'').map(decode_cooked_escapes);
    }
    None
}

/// Two escapes decoded out of `s`, the rest passed through unchanged:
///
/// * `\u{XXXX}` expands to the character it names. `derive.rs`'s curly-quote
///   prose and `capture/reader.rs`'s typography table both spell a character
///   this way rather than typing it, and only this form would slip past a
///   scan that just read the raw source characters — see [`literal_text`].
/// * A `\` immediately followed by a line break, and every whitespace
///   character after it up to the next non-whitespace one, vanishes
///   entirely — Rust's string-continuation escape, which is how this
///   codebase writes one long inline style split across several source
///   lines without a real newline landing in the value (e.g.
///   `screens/filter_sheet.rs`'s multi-line `style:` strings). Found by
///   running this scan before this case existed: every continued string in
///   the tree came back flagged for containing U+000A, a control character
///   with no glyph in any typeface, bundled or not — not because a
///   continuation ever reaches the screen, but because leaving the escape
///   undecoded left the *next line's* raw newline and leading spaces sitting
///   in the literal's supposed value, which is simply the wrong text to be
///   checking in the first place.
///
/// Every other escape this crate writes (`\n \t \\ \" \' \r` as two-character
/// escapes, not a raw control byte) is itself ASCII on both characters, so
/// leaving it un-decoded can never manufacture a character that needs
/// checking — only, at worst, fail to notice one that was never there.
fn decode_cooked_escapes(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && matches!(chars.get(i + 1), Some('\n') | Some('\r')) {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            i = j;
            continue;
        }
        if chars[i] == '\\' && chars.get(i + 1) == Some(&'u') && chars.get(i + 2) == Some(&'{') {
            let mut j = i + 3;
            let mut hex = String::new();
            while j < chars.len() && chars[j] != '}' {
                hex.push(chars[j]);
                j += 1;
            }
            if chars.get(j) == Some(&'}') {
                if let Some(ch) =
                    u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                {
                    out.push(ch);
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Walk `tokens` collecting every string/char literal outside a
/// `#[cfg(test)]` item, at every nesting depth — an ordinary brace/paren/
/// bracket `Group` is descended into like any other code, which is how this
/// reaches a literal sitting inside an `rsx!` macro body without knowing
/// `rsx!`'s grammar at all: by the time this scan sees it, `rsx! { … }` is
/// just a `Group` like any other, its macro name already consumed as a
/// preceding `Ident` this function never has to inspect.
fn scan_stream(tokens: TokenStream, out: &mut Vec<Found>) {
    let mut iter = tokens.into_iter();
    while let Some(tt) = iter.next() {
        match tt {
            TokenTree::Punct(p) if p.as_char() == '#' => {
                // `#!…` (inner attribute, e.g. a module's own `#![cfg(test)]`)
                // vs. `#…` (outer, the shape every attribute in this tree
                // that matters here actually takes) — either way the
                // bracket group right after it is the attribute's body.
                let mut peeked = iter.clone();
                if let Some(TokenTree::Punct(bang)) = peeked.next() {
                    if bang.as_char() == '!' {
                        iter = peeked;
                    }
                }
                let mut peeked = iter.clone();
                let Some(TokenTree::Group(group)) = peeked.next() else {
                    // Not actually followed by an attribute body — a bare
                    // `#` never appears outside one in valid Rust, but there
                    // is nothing to skip if it somehow does.
                    continue;
                };
                if group.delimiter() != Delimiter::Bracket {
                    continue;
                }
                iter = peeked;
                let attr_tokens: Vec<TokenTree> = group.stream().into_iter().collect();
                if is_doc_attribute(&attr_tokens) {
                    // The doc text itself is a Literal one level down inside
                    // this very group — deliberately not recursed into.
                    continue;
                }
                if is_cfg_test(&attr_tokens) {
                    skip_cfg_test_item(&mut iter);
                    continue;
                }
                // Some other attribute (`#[allow(dead_code)]`, `#[derive(…)]`,
                // …) — recursed into on the off chance one ever carries a
                // literal; none in this tree do today.
                scan_stream(group.stream(), out);
            }
            TokenTree::Group(g) => scan_stream(g.stream(), out),
            TokenTree::Literal(lit) => {
                if let Some(text) = literal_text(&lit.to_string()) {
                    out.push(Found { line: lit.span().start().line, text });
                }
            }
            _ => {}
        }
    }
}

/// Every non-test string/char literal in `path`, paired with the line it
/// starts on.
fn scan_file(path: &Path) -> Vec<Found> {
    let text = std::fs::read_to_string(path).expect("a source file this crate compiles");
    let tokens = TokenStream::from_str(&text)
        .unwrap_or_else(|e| panic!("{} failed to lex as Rust: {e}", path.display()));
    let mut found = Vec::new();
    scan_stream(tokens, &mut found);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Characters allowed to fail the coverage check below, each with the
    /// narrative reason its literal never reaches a screen. Kept short on
    /// purpose — see the module doc's warning against reaching for this to
    /// make the test pass, and the card K26 report for what was actually run
    /// against an empty version of this list before anything was added to
    /// it. Declared inside `mod tests` rather than above it so that this
    /// scan's own walk over `src/` (it does not exempt itself) treats any
    /// non-ASCII character a future reason string quotes the same way it
    /// treats the rest of this crate's test fixtures — out, on the strength
    /// of the same `#[cfg(test)]` detection the module doc describes, not a
    /// second exemption.
    const ALLOWLIST: &[(char, &str)] = &[];

    /// The guard itself. See the module doc for what it walks, what it
    /// skips, and why the bar is DejaVu Sans Mono specifically.
    #[test]
    fn every_shipped_literal_is_covered_by_the_bundled_coverage_tail() {
        let by_name = font_coverage::by_name();

        let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rust_files(&src_root, &mut files);
        files.sort();

        let mut failures = Vec::new();
        for path in &files {
            for found in scan_file(path) {
                for ch in found.text.chars() {
                    if ALLOWLIST.iter().any(|(allowed, _)| *allowed == ch) {
                        continue;
                    }
                    if !font_coverage::covers(&by_name, font_coverage::DEJAVU_SANS_MONO, ch) {
                        failures.push(format!(
                            "{}:{} — {:?} contains U+{:04X} {:?}, which DejaVu Sans Mono (the \
                             coverage tail every stack in theme.rs ends in) has no glyph for — \
                             on a phone with no fontconfig behind it, that's tofu in the middle \
                             of a sentence, the way `A♭ Major` came back `A□ Major` on \
                             hymnal.net (card K34) and the chart editor's empty state read \
                             wrong on the moto g stylus 5G before anyone had screenshotted that \
                             path (card K21)",
                            path.display(),
                            found.line,
                            found.text,
                            ch as u32,
                            ch
                        ));
                    }
                }
            }
        }

        assert!(
            failures.is_empty(),
            "found {} character(s) with no bundled glyph, in shipped source:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
