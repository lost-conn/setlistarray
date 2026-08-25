# SetListArray

An offline-first book of the songs you know how to play. Attach charts to
them — PDFs, webpages captured for offline use, or typed text — and arrange
them into setlists. No account, no sync, nothing uploaded.

Built with [Rinch](https://github.com/joeleaver/rinch). The design handoff in
`design_handoff_setlistarray/` is the authority: the hi-fi file for visuals,
the wireframes (turn 2 wins over turn 1) for flow.

## Running it

```bash
./scripts/install-fonts.sh   # once — Newsreader and Karla, via fontconfig
cargo run --release          # always --release; debug Stylo/Parley is slow
```

Requires the Rust nightly toolchain (`rust-toolchain.toml` pins it). Rinch
comes from git, pinned to a revision — no local checkout needed.

**The pin is deliberately not on `main`.** See "The flex regression" below.

The handoff targets Android. Rinch currently ships desktop and wasm backends,
so this runs in a 393×852 phone-shaped desktop window; nothing in the UI code
assumes the desktop.

## Layout

| Path | What lives there |
| --- | --- |
| `src/theme.rs` | Every design token, as CSS custom properties. Nothing downstream hard-codes a hex. |
| `src/model.rs` | `Song`, `Setlist`, `Attachment`, `Confidence`, `Day`. Every field but id/title/artist/created_at is optional. |
| `src/store/` | One `Copy` struct of Signals per store, registered in `app()`. Derived values are computed on read, never stored. |
| `src/ui.rs` | Shared pieces: chips, confidence dots, attachment thumbs, list rows. |
| `src/screens/` | One file per screen. |
| `src/seed.rs` | Demo content, until persistence lands. |

## What is built

- **Library (Songs tab)** — hi-fi. Grouping (confidence by default), sorting in
  both directions, per-group collapse and truncation, comfortable/compact
  density, search, FAB, bottom nav.
- **Song detail** — hi-fi. Metadata chips with the tinted key chip, status line,
  primary attachment card, collapsed other attachments, footer actions.
- **Setlist detail** — hi-fi. Position numbers, the cumulative start-time
  column, the no-chart warning pill, the derived "Before you start" panel,
  Play set.
- **Setlists tab** — wireframe `1m`, styled with the hi-fi tokens.

See [docs/PLAN.md](docs/PLAN.md) for the phased plan to finish the rest.

## What is not

Each of these has a `Stub` screen naming its wireframe: Settings (`1q`),
Add/edit song (`1j`), Performance view (`1o`). Not yet started: attachment
viewer (`1k`), offline webpage capture (`1l`), add-to-setlist sheet (`2e`),
sort & group sheet (`2c`), search & filter (`1p`), first run (`1r`).

Also outstanding:

- **Persistence.** Everything is in memory; `src/seed.rs` stands in. The plan
  is SQLite plus an attachments directory, with export/import as a zip of both.
- **Accent from the system.** `AccentChoice::FromSystem` falls back to Rust
  until Rinch exposes the wallpaper colour.
- **Drag-to-reorder and swipe-to-remove** in setlists.

## The paint regression

The app is pinned to Rinch `d25f646` (2026-03-17), not to `main` (`1f16bed`,
2026-08-24). On `main` the library loses its attachment thumbs, row meta lines
and row hairlines — the confidence dots too, but those are a separate,
older fault (below).

It is a **paint** problem, not a layout one. Dumping the live DOM through the
debug IPC (`--features devtools`, then the `dom_tree` command) gives byte-for-byte
the same tree with the same boxes on both revisions:

```
row   box=(22, 251, 501x66)   style="… border-bottom: 1px solid var(--sla-hairline-soft)"
thumb box=(22, 264, 38x38)    style="… background: var(--sla-fill)"
text  box=(73, 262, 411x43)
title box=(73, 262, 411x22)   -> painted on both
meta  box=(73, 287, 411x19)   -> painted only on d25f646
```

Sampling the thumb on a screenshot: `#EFE7DA` (the fill token, painted) on
`d25f646`, exactly `#FBF7F0` (bare paper) on `1f16bed`.

`git bisect` over the 196 commits, using "is the first row's thumb painted" as
the test, lands on:

> **a433811** — `fix(layout): flow inline content through display:contents in a
> block parent (#61) (#84)`, 2026-07-01

That commit reworks `setup_inline_formatting_contexts` so a block container
detects inline content through `display:contents` wrappers, and marks
`ifc_root` on the wrapper "so IFC discovery finds the container and paint skips
the wrapper". Something in that skip takes the app's rows with it.

A reduced version of the row — thumb with an `if`-guarded span, a `flex: 1`
text block, a `for` of dots — paints correctly on `main` (`cargo run --release
--bin probe`, row E). It needs the whole screen to reproduce, so the app itself
is the repro: pin the two dependencies to `1f16bed` and run it.

One thing that only works on `main`: the search field's placeholder text.

## The flex sizing fault

Older, on both revisions, and not the reason for the pin: a `flex: 1` child is
sized from its content instead of the space its siblings leave, so a list row
measures 501px inside a 447px content box and the confidence dots are laid out
past the right edge of the window. `cargo run --release --bin probe` rows A–D
cover it: wrapping does not rescue the trailing box, and `flex-grow`/`flex-basis`
longhands, `min-width: 0`, a percentage width and `display: grid` with `1fr` all
behave the same. Only moving the growing child last works.

## Notes for the next person

- Rinch components run **once**. Anything dynamic goes in a `{|| ...}` closure.
- The `rsx!` macro builds component props with `..Default::default()`, so a
  prop type with no `Default` — an icon, a callback, a number — must be
  declared `Option<T>`.
- `if`, `for` and `match` inside `rsx!` become closures that re-run. A nested
  one cannot use a non-`Copy` value from the closure around it, which is why
  the screens pass `Copy` stores plus an index and recompute (see
  `songs_in_group` in `src/screens/library.rs`).
- Statements inside `rsx!` bodies get re-emitted into those closures, so rustc
  reports plainly-used bindings as unused. `#![allow(unused_variables)]` in
  `main.rs` covers it.
