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
| ~~No writable-storage accessor~~ | Done (K3): `internal_data_path()` is threaded in at `android_main` | — |
| No wallpaper colours | Material You accent extraction, per the handoff's resolution order | `WallpaperManager.getWallpaperColors()` over JNI; until then `AccentChoice::FromSystem` falls back to Rust, which the handoff explicitly allows |
| ~~Status bar space is hard-coded 44px~~ | Done (K2): `src/platform.rs` | — |
| No way to register an app-bundled font | Newsreader and Karla are not on Android; the app falls back to Noto Serif and Roboto | `RinchApp::register_font_data` exists but neither `run_android` nor `ThemeProviderProps` reaches it — add a font-data field to `ThemeProviderProps`, or a `run_android_with_fonts` |
| `RinchActivity` never opts into edge-to-edge | Below Android 15 the system insets the window *and* the app reserves the same strip | `setDecorFitsSystemWindows(false)` (or the pre-30 flags) in `RinchActivity.onCreate` |
| `android_runtime.rs` passes `logical_size` as `handle_event`'s `window_size` | `ClickContext`'s viewport reads `physical / scale²`; popup placement, not tap targets | Pass `physical_size`. Desktop equivalent was joeleaver/rinch#246; this is a follow-up PR |

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
- `@on_delete(cascade)` / `@on_delete(remove)` do card J5's work *in the
  database*: deleting a song drops it from every set, unlinking leaves the song
  alone. Mind the direction — see the correction under Phase B.

**What we give up:** there is no substring or full-text filter — the operators
are `Eq/Ne/Lt/Le/Gt/Ge`. That is not confined to attachments: the library's own
search field has no database-side equivalent either. Both happen in Rust over
data already in memory (`derive::filter_songs`), which for a few hundred songs
is the right shape anyway. Card G2 is rescoped accordingly: searching inside
attachment bodies means our own index or a scan, not a database feature.

**Gotchas found the hard way**, both worth reporting upstream:

- An edge-field block must come *before* any directive on the same field.
  `songs: [Song] { position: u32 } @on_delete(remove)` parses; the other order
  fails with `expected identifier`. The docs only ever show an edge block with
  no directive.
- Closing a library and reopening it in the same process races the compaction
  worker, which can end up the last holder of the tree and release the directory
  lock on its own thread. Only tests restart in-process, and `db::restart`
  retries for them.

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
| B2 | ~~Repository functions per store — load-all at startup, write-through on mutation. Keep the store API as-is so screens do not change.~~ Done. | ✔ |
| B3 | ~~Persist `LibraryViewStore` and `SettingsStore`; restore on launch.~~ Done — one `Preferences` row, reasoning in `src/db/prefs.rs`. | ✔ |
| B4 | ~~Attachments directory: `<data>/attachments/<id>/`; delete on attachment removal.~~ Done. | ✔ |
| B5 | ~~Replace `src/seed.rs` with a first-run empty database; keep the seed behind a `--seed` flag.~~ Done. | ✔ |

**Done when** songs, setlists, settings and view state survive a restart, and
`src/seed.rs` is no longer in the startup path. ✔

Two things B1 got wrong, both corrected here:

- **The attachment cascade pointed the wrong way.** rhypedb applies an
  `@on_delete` policy to the relations pointing *at* the deleted object, acting
  on their source — so `Song.attachments @on_delete(cascade)` meant "deleting a
  chart deletes the song". The policy now lives on `Attachment.song`, with
  `Song.attachments` as its `@inverse`. `Setlist.songs @on_delete(remove)` was
  right by accident.
- **The delete policies do not do all of J5's work.** They keep the *database*
  consistent; the in-memory setlists a screen is reading do not hear about a
  cascade, so a song deleted mid-session lingers as an id in a setlist until the
  next launch. Nothing renders for it, and a membership write steps over it, so
  it is invisible — but J5 still has a job.

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

Sizes below are **revised after the E1 spike**, which is done. Its write-up —
the site table, the design, and the reasoning behind every number here — is
[docs/CAPTURE.md](CAPTURE.md). Read it before starting E2.

| # | Card | Wireframe | Size |
| --- | --- | --- | --- |
| E1 | ~~**Spike:** fetch a URL, strip scripts/ads, rewrite image URLs to local paths, write HTML + assets into the attachment directory. Prove it on five real chord sites.~~ **Done.** `src/capture/`, proven against eight. | ✓ |
| E2 | Capture UI: URL field, the progress checklist (fetched · stripped · downloading images N of M), determinate accent progress bar, `1.2 MB so far · will work with no signal`. | M |
| E3 | `Save as: Reader text ▾` — reader extraction vs full-page fidelity, with a preview of what gets kept. | ~~M~~ **S** |
| E4 | Failure states the handoff calls out: fetch failed, partial capture, paywalled/JS-only page. Each needs a real screen, not a toast. | ~~M~~ **S–M** |
| E5 | Render a captured page inside the attachment card and the viewer. | ~~M~~ **L** |
| E6 | Settings → "Re-check saved pages": re-fetch and diff, off by default. | S |
| E7 | *New.* Ultimate Guitar's chart is in a 134 KB JSON `data-content` attribute on the page we already fetch. A site-specific extractor brings the largest chord site on the internet from "impossible" to "works". | S–M |

**Phase E total: roughly 6–9 days.** E5 grew because a captured page is a
stranger's HTML and CSS and nothing in this app has yet asked Stylo and Parley
to lay that out. E3 and E4 shrank because the engine already implements both
capture modes and distinguishes five failure states, each carrying its own
user-facing sentence.

**Two things settled before E2 that were not on the card.**

*Reader mode is in.* It is a readability-style heuristic in pure Rust, and it
needed a chord-specific override to be worth having: generic readability
rewards commas and long sentences and so deletes chord charts. Before that
override, reader mode dropped the chart on three of the four capturable sites.

*Capture cannot run on Android.* `android.permission.INTERNET` is required to
open a socket, and this app promises a manifest with no permissions — a promise
that now has a test behind it. The engine is desktop-only until somebody
chooses between keeping the promise, rewriting it, and capturing through the
system share sheet. All three options are laid out in `docs/CAPTURE.md`. **This
is a decision, not a task, and it blocks E2 on Android only.**

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
| K1 | ~~Second crate target: `cdylib` + `android_main`, an `AndroidManifest.xml` (no permissions), a `build-apk.sh`.~~ Builds a signed APK; **not yet run on a device** — no hardware available. | ✔ |
| K2 | ~~Replace the hard-coded 44px status strip with `safe_area_insets()` and `density_dpi()`.~~ Done, behind `platform::safe_area()`. Unverified against a real notch. | ✔ |
| K3 | ~~Storage path from `AndroidApp::internal_data_path()` through a platform seam.~~ Done: `DataDir::install()` at the entry point, published as a context by `app()`. | ✔ |
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
