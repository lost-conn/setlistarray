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

## The flex regression

The app is pinned to Rinch `d25f646` (2026-03-17), not to `main`. Somewhere in
the 196 commits between that and `1f16bed` (2026-08-24) — the run of
flex/IFC/`display:contents` layout fixes — this broke:

> In a `display: flex` row, any sibling **after** a flex-grow child stops
> rendering.

`src/bin/probe.rs` is the minimal repro; `cargo run --release --bin probe`
draws ten numbered cases. Rows **9** and **10** are the regression: a fixed
box, a `flex: 1` middle child, another fixed box — the trailing box never
appears, with either the `flex: 1` shorthand or the `flex-grow`/`flex-basis`
longhands. Rows 2 and 7 are the same fault reached through a list row.

On `1f16bed` this costs the app its attachment thumbs, row meta lines,
confidence dots and row hairlines, because every list row is a flex row with a
`flex: 1` text block in the middle. On `d25f646` all of it renders.

One thing that only works on `main`: the search field's placeholder text. It
paints there and does not on the pinned revision.

Move the pin forward once the flex fault is fixed; nothing in the app code
works around it.

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
