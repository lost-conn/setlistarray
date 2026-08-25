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

## The two Rinch faults (both fixed, not yet merged)

The app is pinned to Rinch `d25f646` (2026-03-17). Two faults on `main` kept it
there; both now have fixes on branches in a separate checkout, each with a
regression test that fails before and passes after.

### The paint regression

On `main` the library lost its attachment thumbs, row meta lines, hairlines and
confidence dots. Same DOM, same layout boxes — the boxes simply were not drawn.
`git bisect` over 196 commits (test: "is the first row's thumb painted") landed
on **a433811**, `fix(layout): flow inline content through display:contents in a
block parent`.

Cause: `mark_inline_descendants` marked *every* `display:contents` child of an
IFC root as IFC content, and `ifc_root` means "the IFC draws this, skip it in
the paint walk". Two details make that bite everywhere: `Node::is_inline()`
counts comment nodes, and rsx emits a comment marker for every `if`/`for`/
`match`, so a block container becomes an IFC root as soon as it contains any
control flow. Wrappers holding block content — every component — vanished with
their whole subtree. Only descendants creating their own stacking context
survived, which is exactly why row titles (`overflow: hidden`) painted and
nothing else did.

Fix: a wrapper is IFC content only when it wraps no block-level box.

### The viewport scale fault

The one I first wrote up as "a `flex: 1` child is sized from its content".
It was not a flex fault at all, and Taffy was innocent.

`rinch_runtime.rs` handed the layout engine `PlatformWindow::inner_size()` —
winit's **physical** surface size — while `paint_document` multiplies every
coordinate by the same window's scale factor. On this 1.25× display the page
was laid out 1.25× too wide and then drawn 1.25× larger again, so the rightmost
fifth fell off the surface. That is why the confidence dots were never visible,
why `flex-wrap` had nothing to wrap, and why only moving the growing child last
appeared to help. A 1× display is unaffected.

Fix: lay out at the logical size (`inner_size() / scale_factor`), through one
seam every window-backed layout and paint site uses.

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
