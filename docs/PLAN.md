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

### 1. Android — settled, and better than I first reported

Rinch **does** have an Android backend. My earlier "desktop and wasm only" came
from grepping the March revision the app is pinned to; it landed after that.
On current `main`:

- `crates/rinch-android` — a JNI bridge (~1,300 lines) over Java companion
  classes (`RinchActivity`, `RinchInputView`, `RinchInputConnection`), covering
  clipboard, IME, file picker (`pick_file` / `save_file` / `read_content_uri`),
  share, camera, location, sensors, notifications, permissions, and display
  (`safe_area_insets()`, `density_dpi()`).
- `crates/rinch/src/shell/android_runtime.rs` — `run(android_app, title, w, h,
  component)` and `run_with_theme(...)`, mirroring the desktop entry points, on
  android-activity + softbuffer. Features: `android`, `android-gpu`.
- `examples/hello-android` — a `cdylib` with an `android_main`, an
  `AndroidManifest.xml`, and `build-apk.sh`: cargo-ndk → javac → d8 → aapt2 →
  zipalign → apksigner → adb install. minSdk 28, targetSdk 35.

So the port is: a second `cdylib` target with an `android_main`, a manifest, and
a build script. The UI code carries over unchanged.

Two more crates change earlier assumptions:

- `rinch-http` — one API over `ureq` natively and `fetch` on wasm. The webpage
  capture in Phase E should use it rather than taking a direct `ureq` dependency.
- `rinch-storage` — cross-platform durable key/blob store (atomic filesystem
  writes natively, IndexedDB on web). Not SQL, so it does not replace SQLite for
  search, but it is the right seam if cross-compiling SQLite to Android proves
  annoying.

**Gaps this app will hit** (all small, and all worth contributing upstream):

| Gap | Why we need it | Shape of the fix |
| --- | --- | --- |
| No keep-awake API | "Keep screen awake while playing" is a Settings toggle and a performance-mode control | `FLAG_KEEP_SCREEN_ON` on the activity window: a Java method plus a JNI wrapper in `rinch-android` |
| No writable-storage accessor | The database and attachments directory need a path | `AndroidApp::internal_data_path()` from android-activity is already in scope at `android_main`; thread it into the app, or expose it from `rinch-android` |
| No wallpaper colours | Material You accent extraction, per the handoff's resolution order | `WallpaperManager.getWallpaperColors()` over JNI; until then `AccentChoice::FromSystem` falls back to Rust, which the handoff explicitly allows |
| Status bar space is hard-coded 44px | `src/main.rs` reserves it as a fixed strip | Use `display::safe_area_insets()` / `density_dpi()` as `hello-android` does |

### 2. Where the data lives — settled: rhypedb

[rhypedb](https://github.com/joeleaver/rhypedb), in-process. Not SQLite, which
the K6 spike had already proven viable — the call is to dogfood the stack this
app's framework comes from, and the object/relationship model fits a library of
songs better than tables do.

Proven before committing to it:

- `rhypedb-engine` with `default-features = false` cross-compiles to
  `aarch64-linux-android` (pure-Rust LSM — WAL, memtable, SST, MVCC; no C
  toolchain). Default features pull the fastembed/ONNX stack, which this app
  can never ship on a phone — hence the explicit opt-out.
- `Database::open(schema, dir)` runs in-process. The documented path is a
  server with an HTTP/TCP client; embedding is undocumented but public.
- A song stores only the fields it has, and unset ones read back absent.
- Setlist order rides on the membership link as a `position` edge field.
- `@on_delete(cascade)` / `@on_delete(remove)` do card J5's work in the schema:
  deleting a song drops it from every set, unlinking leaves the song alone.

**What we give up:** there is no substring or full-text filter — the operators
are `Eq/Ne/Lt/Le/Gt/Ge`. That is not confined to attachments: the library's own
search field has no database-side equivalent either. Both happen in Rust over
data already in memory (`derive::filter_songs`), which for a few hundred songs
is the right shape anyway. Card G2 is rescoped accordingly: searching inside
attachment bodies means our own index or a scan, not a database feature.

**Gotcha found the hard way:** an edge-field block must come *before* any
directive on the same field. `songs: [Song] { position: u32 } @on_delete(remove)`
parses; the other order fails with `expected identifier`. Worth reporting
upstream — the docs only ever show an edge block with no directive.

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
| A2 | Confirm the viewport-scale fix lands the confidence dots inside the row on a HiDPI display; drop any layout hacks added meanwhile. | S |
| A3 | Fold the pixel checks into a repeatable script (`scripts/screenshot.sh`) — build, run under X11, capture the window, sample the known-good regions. This is the regression net for every phase after. | S |

**Done when** the library, song detail, setlist detail and setlists tab render
identically to the hi-fi on current `main`, and the script says so.

---

## Phase B — Persistence

Nothing else is worth building on top of in-memory state.

| # | Card | Size |
| --- | --- | --- |
| B1 | ~~`src/db/`: schema v1 (`Song`, `Setlist`, `Attachment` with ordered membership), `Database::open` behind a `DataDir` seam, and the domain↔object conversion.~~ Done. | ✔ |
| B2 | Repository functions per store — load-all at startup, write-through on mutation. Keep the store API as-is so screens do not change. | M |
| B3 | Persist `LibraryViewStore` (query, group-by, sort, density, collapsed groups) and `SettingsStore`; restore on launch. Small and singular — a settings blob rather than objects. | S |
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
| G2 | Search inside typed text and captured pages. rhypedb has no full-text or substring filter, so this is our own inverted index over extracted text, built at attachment import — or a scan, if the library stays small. Lower priority than the rest of Phase G. | M |
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

## Phase K — Android

No longer a question mark. Do it early enough that the phone is the reference
device rather than a port at the end — K1 and K2 are worth pulling forward to
sit alongside Phase C.

| # | Card | Size |
| --- | --- | --- |
| K1 | Second crate target: `cdylib` + `android_main` calling `run_android`, an `AndroidManifest.xml` (no camera/location permissions — this app needs none), and a `build-apk.sh` adapted from `hello-android`. Get the library screen onto a device. | M |
| K2 | Replace the hard-coded 44px status strip with `safe_area_insets()` and `density_dpi()`; check every screen against a notch and the gesture bar. | S |
| K3 | Storage path from `AndroidApp::internal_data_path()`; make the Phase B database and attachments directory take their root from a platform seam rather than a desktop path. | S |
| K4 | Attachment import through `rinch_android::file_picker::pick_file` + `read_content_uri`; backup export through `save_file`. Same trait the desktop `rfd` path implements. | M |
| K5 | Keep-awake: contribute `FLAG_KEEP_SCREEN_ON` to `rinch-android`, then wire F4 to it. | M |
| K6 | ~~Confirm SQLite cross-compiles under cargo-ndk~~ — done, see decision 2. | ✔ |
| K7 | Text input on device: the typed-lyrics editor and every text field through `RinchInputConnection`/IME. Most likely place for surprises. | M |
| K8 | Optional upstream contribution: wallpaper colours via `WallpaperManager`, completing the accent resolution order. | M |

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
