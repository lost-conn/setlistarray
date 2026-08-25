# Build plan

Everything between the current scaffold and the app the handoff describes.

The scaffold has the three hi-fi screens and the Setlists tab; state lives in
signals and dies with the process. What follows is ordered so that each phase
leaves the app usable, and so the two genuinely uncertain pieces — offline
webpage capture and PDF rendering — get proven early rather than discovered
late.

Sizes: **S** ≈ half a day · **M** ≈ 1–2 days · **L** ≈ 3+ days · **?** = spike
first, estimate after.

---

## Decisions needed before Phase C

These change what gets built, not just how. Everything up to Phase B is safe to
start now.

### 1. Android

The handoff targets Android. Rinch has no Android backend — `crates/rinch`
ships desktop (winit + software/GPU renderer) and wasm32, and `rinch-web`
renders through real browser DOM. Three ways out:

| Option | What it means | Cost |
| --- | --- | --- |
| **Desktop-shaped now** (recommended) | Keep building in the phone-sized window. Nothing in the UI assumes a desktop, so the screens carry over. | none |
| **WASM + installable PWA** | `rinch-web` mounts the same components in a browser; Android installs it to the home screen. Loses native file pickers and keep-awake; webpage capture must go through browser fetch and CORS. | M, plus ongoing divergence |
| **Native Android backend for Rinch** | winit supports Android; the renderer, font stack and event loop need porting. | L+, framework work |

The plan below assumes the first. It only bites at Phase K.

### 2. Where the data lives

The handoff says "local files/SQLite". Two shapes:

- **SQLite (recommended)** — `rusqlite` with the bundled feature, one file plus
  an attachments directory. Real queries for the search screen, a migration
  path, and full-text search over captured pages comes free with FTS5.
- **One JSON document** — simpler, no C dependency, but "search inside
  attachments" means loading every capture into memory, and every write
  rewrites the file.

Song counts of ~300 with multi-megabyte captures make SQLite the safer call.

### 3. PDF rendering

Rinch has no PDF support, and PDFs are one of the three attachment kinds.

- **`pdfium-render`** (recommended) — bundles Google's PDFium; rasterise each
  page to PNG on import, cache next to the attachment, display through the
  `Image` component. Big binary, permissive licence.
- **MuPDF** — smaller, but AGPL: it would force the whole app's licence.
- **Pure-Rust (`pdf`, `lopdf`)** — no rasteriser worth shipping yet.
- **Defer** — ship typed text and captured pages first; PDFs attach and open in
  the system viewer until a renderer lands.

---

## Phase A — Unblock rendering

Depends on the Rinch fixes in flight (see "The paint regression" in the README).

| # | Card | Size |
| --- | --- | --- |
| A1 | Move the `rinch`/`rinch-tabler-icons` pin to a revision with the paint fix; re-verify all four screens against the hi-fi. | S |
| A2 | Confirm the flex sizing fix lands the confidence dots inside the row; drop any layout hacks added meanwhile. | S |
| A3 | Fold the pixel checks into a repeatable script (`scripts/screenshot.sh`) — build, run under X11, capture the window, sample the known-good regions. This is the regression net for every phase after. | S |

**Done when** the library, song detail, setlist detail and setlists tab render
identically to the hi-fi on current `main`, and the script says so.

---

## Phase B — Persistence

Nothing else is worth building on top of in-memory state.

| # | Card | Size |
| --- | --- | --- |
| B1 | `src/db/mod.rs`: open/create the database under the platform data dir, schema v1 (`songs`, `setlists`, `setlist_songs`, `attachments`, `settings`, `library_view`), `PRAGMA user_version` migrations. | M |
| B2 | Repository functions per store — load-all at startup, write-through on mutation. Keep the store API as-is so screens do not change. | M |
| B3 | Persist `LibraryViewStore` (query, group-by, sort, density, collapsed groups) and `SettingsStore`; restore on launch. Each tab keeps its own state, per the handoff. | S |
| B4 | Attachments directory: `<data>/attachments/<id>/` holding the file plus derived assets; delete on attachment removal. | S |
| B5 | Replace `src/seed.rs` with a first-run empty database; keep the seed behind a `--seed` flag for screenshots and tests. | S |

**Done when** songs, setlists, settings and view state survive a restart, and
`src/seed.rs` is no longer in the startup path.

---

## Phase C — Creating and organising

The flows that let someone actually build a book of songs.

| # | Card | Wireframe | Size |
| --- | --- | --- | --- |
| C1 | Add / edit song: title + artist visible, everything else behind **More details**; artist autocomplete with the `use "<typed>"` escape hatch. Must be usable in seconds. | `1j` | M |
| C2 | Overflow menu (⋮) on song detail and as the library row long-press: Add to setlist… · Set confidence · Mark played today · Duplicate · Delete. Order matters. | `2d` | S |
| C3 | Add-to-setlist bottom sheet: search, checkbox list with `9 songs · 32:04` sub-lines, "already in this set" disabled state, `+ New setlist…`, multi-select. | `2e` | M |
| C4 | Sort & group sheet: group-by chips, every metadata field as a sortable row with a human direction label, tap-active-to-reverse, greyed rows with counts for sparse fields. The store logic already exists — this is the sheet. | `2c` | M |
| C5 | Setlist editing: the song picker slides over the setlist so the set stays visible behind; running "N picked" count. | `1i` | M |
| C6 | Reorder by drag handle and swipe-left to remove inside a setlist. Rinch has drag support; swipe may need a gesture on top of pointer events. | — | M? |
| C7 | Setlist create/rename/duplicate/delete from the Setlists tab long-press. | `1m` | S |

**Done when** a song can go from nothing to being in two setlists without
touching the seed data.

---

## Phase D — Attachments, part one

| # | Card | Wireframe | Size |
| --- | --- | --- | --- |
| D1 | Attachment model plumbing: add/remove, first attachment becomes primary automatically, set-primary from the ⋮ menu, `card`-styled primary panel fed by real content. | — | S |
| D2 | Typed lyrics/chords: a full-screen editor writing a `Text` attachment; monospace-ish rendering in the card. | `1j` | M |
| D3 | Pick a PDF via `rfd` file dialog, copy into the attachments directory, record page count. | — | S |
| D4 | PDF rendering per decision 3 — rasterise pages on import, cache PNGs, show page one in the card. | L? |
| D5 | Attachment viewer: full-screen dark chrome, page prev/next, zoom, rotate, auto-hiding chrome that returns on tap. | `1k` | M |

**Done when** all three attachment kinds attach, display in the card, and open
full screen.

---

## Phase E — Offline webpage capture

The distinguishing feature, and the least certain. Spike before estimating the
rest. This flow is full of failure states — worth modelling as an explicit
state machine (`/smdp`) before writing it.

| # | Card | Wireframe | Size |
| --- | --- | --- | --- |
| E1 | **Spike:** fetch a URL with `ureq`, strip scripts/ads, rewrite image URLs to local paths, write HTML + assets into the attachment directory. Prove it on five real chord sites. | ? |
| E2 | Capture UI: URL field, the progress checklist (fetched · stripped · downloading images N of M), determinate accent progress bar, `1.2 MB so far · will work with no signal`. | M |
| E3 | `Save as: Reader text ▾` — reader extraction vs full-page fidelity, with a preview of what gets kept. | M |
| E4 | Failure states the handoff calls out: fetch failed, partial capture, paywalled/JS-only page. Each needs a real screen, not a toast. | M |
| E5 | Render a captured page inside the attachment card and the viewer. | M |
| E6 | Settings → "Re-check saved pages": re-fetch and diff, off by default. | S |

**Done when** a URL pasted on wifi still opens with the network off.

---

## Phase F — Performance mode

| # | Card | Wireframe | Size |
| --- | --- | --- | --- |
| F1 | Performance view: thin top bar (`2 / 5`, title, key · capo · bpm), full-bleed chart, larger type. | `1o` | M |
| F2 | Swipe between songs with edge chevrons that dim at the ends. | `1o` | M |
| F3 | Bottom bar: `up next`, keep-awake toggle, `set` button opening the running order; 5-segment progress strip. | `1o` | S |
| F4 | Keep-awake for real. No Rinch platform API exists — on Linux this is a D-Bus inhibit; on Android it is a window flag. Wrap it behind a trait with a no-op default. | M? |
| F5 | Performance theme setting: follow the app, or force dark. Design both. | S |

**Done when** a set plays end to end from the Play set button with the screen
staying awake.

---

## Phase G — Search

| # | Card | Wireframe | Size |
| --- | --- | --- | --- |
| G1 | Search & filter screen: live results grouped into Songs · Setlists · Inside attachments, matched substring highlighted. | `1p` | M |
| G2 | Full-text search over typed text and captured pages — SQLite FTS5 over extracted text, populated at attachment import. | M |
| G3 | Filter chips beyond sorting: confidence, tag, tuning, has-chart. | S |

---

## Phase H — Settings, first run, density

| # | Card | Wireframe | Size |
| --- | --- | --- | --- |
| H1 | Settings screen: Storage (sizes, saved-page count, re-check), Backup, Defaults (tuning, sort, density, keep-awake, accent, performance theme, dark mode follows system). | `1q` | M |
| H2 | Accent picker driving the existing token set; Material You extraction stays a stub until a platform API exists, and falls back to Rust. | S |
| H3 | First run: wordmark, the question, one field, Add song, "or import a backup file". Illustration slot stays empty — no artwork exists. | `1r` | S |
| H4 | Compact density everywhere it applies, plus the alphabet scrubber rail when grouping is First letter. | `1c`, `2b` | M |

---

## Phase I — Backup

| # | Card | Size |
| --- | --- | --- |
| I1 | Export: zip of the database plus the attachments directory, through a save dialog. | M |
| I2 | Import: validate, then replace or merge — decide which, and say so in the UI before it happens. | M |
| I3 | Show last-export date in Settings; nag never, mention once. | S |

---

## Phase J — Polish

| # | Card | Size |
| --- | --- | --- |
| J1 | Motion: sheets slide up 200–250ms ease-out, group collapse animates height. Nothing else animates. | M |
| J2 | Virtualise the library list using `rinch_core::virtual_list` — 300 songs is the stated target and the whole list currently builds every row. | M |
| J3 | Contrast audit: assert every text token clears 4.5:1 on its background, in both modes and for every accent, as a unit test over the token table. | S |
| J4 | Empty and error states for every screen — empty library, empty setlist, missing attachment file, database locked. | M |
| J5 | Delete/undo semantics: deleting a song removes it from every setlist; deleting from a setlist never touches the song. Cover with tests. | S |

---

## Phase K — Packaging

Blocked on decision 1. Desktop: an AppImage or a `.deb`, fonts bundled rather
than fontconfig-discovered (they are already vendored in `assets/fonts`).
Android: whatever decision 1 chose.

---

## Cross-cutting

**Testing.** Three levels, cheapest first:

1. Unit tests over the derived logic that already exists — grouping, sort order
   with missing fields, cumulative setlist times, the "Before you start" prep
   sentences, duration formatting. None of it needs a window.
2. Layout and paint tests in the Rinch style (`RinchDocument` → `resolve_layout`
   → `VelloPainter`) for anything that regresses visually.
3. The Phase A screenshot script for whole-screen checks, driven through the
   `devtools` debug IPC (`dom_tree`, `get_computed_styles`, `screenshot`,
   synthetic `click`) so screens can be walked without a human.

**Performance budget.** 300 songs, some with multi-megabyte captures. Watch
list build time (J2), attachment thumbnails, and never load an attachment body
to render a row.

**Offline promise.** One network call exists in the entire app: fetching a page
the user explicitly pasted. Anything else that reaches the network is a bug —
worth a test that asserts it.

**Rinch upstream.** Two faults are open (README). Others will surface; the
pattern that works is a headless repro in `rinch-dom`'s test style plus a
bisect when it is a regression.
