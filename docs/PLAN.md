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
| No way to register an app-bundled font | Newsreader and Karla are not on Android, and the APK carries no font files; on the phone the app falls back to the system serif and Roboto | `RinchApp::register_font_data` exists but neither `run_android` nor `ThemeProviderProps` reaches it — add a font-data field to `ThemeProviderProps`, or a `run_android_with_fonts` |
| `RinchActivity` never opts into edge-to-edge | Predicted double insets below Android 15; **disproved on Android 13** — the window spans the whole display and `safe_area()` applies the strip once. Still untested on 15, which enforces edge-to-edge for an SDK 35 target | `setDecorFitsSystemWindows(false)` (or the pre-30 flags) in `RinchActivity.onCreate`, if 15 turns out to need it |
| `android_runtime.rs` passes `logical_size` as `handle_event`'s `window_size` | `ClickContext`'s viewport reads `physical / scale²`; popup placement, not tap targets. No longer theoretical — the long-press overflow menu flips up and lands outside the list on the phone | Pass `physical_size`. Desktop equivalent was joeleaver/rinch#246; this is a follow-up PR |

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

### 3. PDF rendering — settled: hayro, pure Rust

Rinch has no PDF support, and PDFs are one of the three attachment kinds.
Card D4 spiked every option and built a real signed APK for each; the numbers
are in [PDF.md](PDF.md). **Decided 2026-08-26: `hayro`.**

| | APK (`arm64-v8a`) | Δ | Blob | Licence |
| --- | ---: | ---: | --- | --- |
| Today | 6,799,883 B | — | — | — |
| **hayro** | **9,413,131 B** | **+2.49 MiB** | none | MIT / Apache-2.0 |
| `pdfium-render` | 10,920,555 B | +3.93 MiB | 6.4 MB `libpdfium.so` | BSD-3 + 15 notices |
| Defer | 6,799,883 B | 0 | none | — |

**The bullet this list used to carry — "pure Rust: no rasteriser worth
shipping yet" — expired.** It was true when it was written. `hayro` 0.7.1
rasterises this app's test documents to within antialiasing noise of PDFium
(mean absolute difference 2.0/255; 0.14 % of pixels differ by more than half a
level), at comparable speed, in one third fewer APK bytes, with no binary blob
and no attribution burden. Its README's "no encrypted PDF support" is also out
of date: 0.7 ships RC4, AES-128 and AES-256.

Rasterise each page on import, cache the PNG next to the attachment, display
through the `Image` component — the shape the plan always proposed, now with
measured costs: 6.0 ms and 104 KiB per page at 1080 px, so a 30-page chart book
imports in 180 ms. **Encode the cache as a 16-colour palette PNG**: the same 30
pages are 8.35 MiB as RGB8, 3.05 MiB as Luma8 and 1.30 MiB paletted. The
encoding choice is worth more than the renderer choice.

Not chosen, and why:

- **`pdfium-render`** — Google's renderer, and the safer bet on correctness for
  hostile real-world PDFs. Costs 1.5 MiB more than hayro, ships an unauditable
  6.4 MB C++ blob that parses untrusted input in-process, and adds a BSD-3
  attribution obligation: 16 licence texts and a notices screen this app does
  not have.
- **MuPDF** — AGPL. Would force the whole app's licence. Unchanged.
- **Defer to the system viewer** — zero APK cost but not zero work, and the
  cost lands in the wrong place: Android needs a FileProvider (no new
  permission, so that promise survives) plus roughly 60 lines of this project's
  first Java and a JNI `startActivity` bridge, because `rinch-android` has
  none. Worse, PDFs would be absent from **performance mode** entirely — the
  most common chart format, missing from the screen this app exists for.
- **`android.graphics.pdf.PdfRenderer`** — an option decision 3 never listed:
  zero APK bytes, no permission, and PDFium underneath. Rejected because it is
  JNI plus a Java shim and does nothing at all for the desktop, where this app
  is also developed and run.

**The risk accepted:** hayro is young (0.7.1), and it is now on the critical
path for one of the three attachment kinds. An escape-hatch trait with a
swappable renderer was offered and declined — if hayro mangles a chart someone
actually owns, that is the moment to revisit, and PDF.md keeps the pdfium
measurements so the comparison does not have to be redone.

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
| C6 | ~~Reorder by drag handle and swipe-left to remove inside a setlist.~~ **Spiked, and both gestures refused.** Shipped as an explicit **Reorder** mode: Move up · Move down · Remove per row, plus an Undo strip. See below. | — | ✔ |
| C7 | Setlist create/rename/duplicate/delete from the Setlists tab long-press. | `1m` | S |

**Done when** a song can go from nothing to being in two setlists without
touching the seed data.

### C6: the gestures do not exist on Android, and that is not a C6 problem

The spike (`src/bin/gesture_probe.rs`, driven by `scripts/gesture-probe.py`)
proved both gestures on the **desktop** backend — a handle drag fires
`dragstart → dragenter/dragover → drop → dragend` with usable coordinates, and
a horizontal drag on a row reports its delta — and then proved, by reading the
one function every Android touch passes through, that **neither can ever fire
on a phone**. The table and the three consequences are in the README under
"Touch on Android is a tap and a scroll, and nothing else". In one line:
Android's `TouchGesture` emits `MouseDown` only at finger-*up*, immediately
followed by `MouseUp`, and only for a finger that never moved; a finger that
moves becomes wheel deltas and nothing else.

So C6 ships tap-driven controls, which work identically on both platforms:

- **Reorder** on the action row toggles an edit mode. Each row grows a Move up
  and a Move down (dead, not hidden, at the ends) and a named **Remove**.
  Positions renumber and the cumulative clock re-runs on every move, because
  `SetlistDetail` derives both on read.
- **Removal is undoable.** The handoff does not say, so: yes. Removing never
  touches the song — that is J5's rule and `@on_delete(remove)` — so what it
  actually destroys is the *place in the running order*, and the picker can
  only put a song back on the end. `SetlistsStore::last_removal` holds one
  removal, any other write to any running order spends it, and it does not
  survive the process.

**This lands on two other cards, and neither has been rescoped yet:**

- **F2** — "Swipe between songs with edge chevrons" in performance mode is the
  same dead gesture. It needs an explicit control (the chevrons themselves,
  made tappable) or an upstream fix first.
- **`src/menu.rs`'s long-press** already stands in `oncontextmenu` for
  press-and-hold, on the grounds that the app "only runs in a desktop window
  today". That was wrong in a second way — `oncontextmenu` was never synthesised
  from touch at all — and has since been fixed upstream by joeleaver/rinch#266,
  which turns a 500ms still press into the same right-button dispatch a desktop
  right-click takes. A long press on a library row opens the menu on the phone
  and does not also navigate. The file's own note has not caught up. What is
  still wrong is where the menu *lands*: it flips upward and off the list,
  because `ClickContext`'s viewport on Android is short by one scale factor.

---

## Phase D — Attachments, part one

| # | Card | Wireframe | Size |
| --- | --- | --- | --- |
| D1 | ~~Attachment model plumbing: add/remove, first attachment becomes primary automatically, set-primary from the ⋮ menu, `card`-styled primary panel fed by real content.~~ Done — see below. | — | ✔ |
| D2 | ~~Typed lyrics/chords: a full-screen editor writing a `Text` attachment; monospace-ish rendering in the card.~~ Done — see below. | `1j` | ✔ |
| D3 | Pick a PDF via `rfd` file dialog, copy into the attachments directory, record page count. | — | S |
| D4 | PDF rendering per decision 3 — rasterise pages on import, cache PNGs, show page one in the card. | L? |
| D5 | Attachment viewer: full-screen dark chrome, page prev/next, zoom, rotate, auto-hiding chrome that returns on tap. | `1k` | M |

**Done when** all three attachment kinds attach, display in the card, and open
full screen.

### D1: where the rules live, and what D2–D5 inherit

- **`Song::attach` / `detach` / `set_primary` / `primary`** (`src/model.rs`) are
  the three rules and their read, as pure functions over a `Song`. One private
  `settle_primary` ends every mutation, which is what makes "a song with charts
  and no primary" unreachable rather than merely unlikely.
- **Removing the primary chart promotes the oldest chart still attached.** The
  card did not say; leaving a dangling pointer and clearing it while charts
  remain are both out, and "the oldest one left" is the first-becomes-primary
  rule applied a second time — the row directly under the card is the row that
  moves into it.
- **`SongsStore::attach` / `detach` / `set_primary` are the store seam.** A song
  owns its charts (the schema's cascade says so), so the song store holds the
  attachments store and `AttachmentsStore`'s mutating half is `pub(super)`.
  There is no second door: a producer cannot write a chart the song does not
  list. `AttachmentsStore::update` stays public for the metadata a producer
  learns later — D4's page count, E2's real size on disk.
- **Expanding a collapsed row is view state**, a `Signal<Vec<AttachmentId>>` on
  the screen, and cannot reach a write.
- **Set-primary is not in wireframe `2d`.** It is in the handoff's sentence
  about the collapsed rows, and it has no unambiguous object at song level, so
  it lives on a per-attachment long-press menu — `menu::AttachmentMenuItems`,
  which explains the reasoning in full. `2d` keeps its five entries.
- **The card degrades honestly.** No skeleton bars: a kind with nothing to
  render says which of the three reasons it is. D4 and E2/E5 replace those
  sentences with content, and `song_detail::preview` is the one place to change.
- **There is still no way to add an attachment from the UI.** D1 is the
  plumbing; D2, D3 and E2 are the three producers, and `+ Add attachment` stays
  inert until one exists.

### D2: a screen with no wireframe, and four decisions

`1j` names the row that opens this editor and never draws what is behind it;
`1k` is the viewer. So D2 is the minimal reading of the card — full-screen
`✕ / title / Save`, one field, a `Text` attachment — wearing `1j`'s own chrome,
because `song_form.rs` is the closest sibling and the only full-screen-form
pattern the app has. `src/screens/chart_editor.rs` opens with the reasoning.

- **Save creates the attachment; opening the screen does not.** `attach` mints
  a row *and* a directory, and the first chart a song gets becomes its primary
  one — so create-on-open would litter a directory per abandoned edit and let
  an empty ghost take the card away from a real chart.
- **`✕` asks, but only over changed text.** `song_form` discards silently and
  is right to for two short fields; a verse someone typed out, with no undo
  anywhere in this app, is not that. An untouched editor still closes straight
  back.
- **An empty body is refused, not written.** Emptying an existing chart and
  saving is a *removal*, and there is already one, on the long-press menu,
  where a destructive action looks like one. The refusal says so.
- **An existing typed chart edits in the same screen**, reached from
  `menu::AttachmentMenuItems`, which grows an **Edit lyrics / chords…** entry
  for `AttachmentKind::Text` only. The primary card's own tap is spoken for —
  it opens the viewer, which is D5.

`+ Add attachment` on song detail is live and goes straight here, with a muted
sub-line saying so. It becomes the three-row chooser `1j` draws when D3 or E2
gives it a second thing to choose.

**Two framework gaps shape this screen and are not fixable from it** — a
`<textarea>` cannot scroll to its caret, and tapping to place the caret lands
about a line off. The first is worked around by growing the field with its
value; the second is not worked around at all. Both, plus the missing Android
monospace font and the missing IME inset, are written up under "Still open" in
the README.

**One latent fault fell out of it.** `AttachmentsStore::update` projected the
body away *inside* `Signal::update`'s closure, and `strip_body` reads another
signal to do it — which panics with "RefCell already mutably borrowed" the
moment anything calls it on a persistent library. D2 is the first caller;
`insert` had the order right all along.

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

*Capture runs on Android — decided 2026-08-26.* `android.permission.INTERNET`
is required to open a socket, and this app promised a manifest with no
permissions at all. The permission is now declared and the promise is
rewritten: **one permission, one call site, nothing else reaches the network.**
The manifest test narrowed from "no permissions" to an allowlist of exactly
one (`the_android_manifest_asks_only_for_internet`), and the two options not
taken — desktop-only capture, and capturing through the system share sheet —
are recorded with the reasoning in `docs/CAPTURE.md`. **E2 is no longer blocked
on either platform**, though its UI now has to exist on both. On the phone,
`dumpsys package` shows INTERNET granted at install with no runtime permissions
beside it; the capture path itself has still never been exercised on a device.

**Done when** a URL pasted on wifi still opens with the network off.

---

## Phase F — Performance mode

| # | Card | Wireframe | Size |
| --- | --- | --- | --- |
| F1 | Performance view: thin top bar (`2 / 5`, title, key · capo · bpm), full-bleed chart, larger type. | `1o` | M |
| F2 | ~~Swipe between songs~~ with edge chevrons that dim at the ends. **The swipe cannot be built** — see C6 above; the chevrons have to be the control, not the hint. | `1o` | S–M |
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
| K1 | ~~Second crate target: `cdylib` + `android_main`, an `AndroidManifest.xml`, a `build-apk.sh`.~~ Builds a signed APK, and it runs: a moto g stylus 5G (2022) and Waydroid, both Android 13 / SDK 33. No other version, and nothing bigger than a handset. The manifest declares one permission, INTERNET, for capture (Phase E). | ✔ |
| K2 | ~~Replace the hard-coded 44px status strip with `safe_area_insets()` and `density_dpi()`.~~ Done, behind `platform::safe_area()`. Verified against a real punch-hole cutout on Android 13; the insets are applied once, not twice. | ✔ |
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
worth a test that asserts it. That is card X2, and it is still open. There is
now a floor under it: `the_http_client_is_named_in_exactly_one_file` (in
`src/lib.rs`) fails if any file under `src/` other than `src/capture/fetch.rs`
names the HTTP client. It catches a second call site being written by hand,
which is the likely way this breaks; it does not catch a dependency dialling
out on its own, or this crate growing a different client. X2 is what would.
The promise is also what `android.permission.INTERNET` in the manifest is
justified by — see Phase E — so it is now load-bearing rather than decorative.

**Rinch upstream.** Two faults are open (README). Others will surface; the
pattern that works is a headless repro in `rinch-dom`'s test style plus a
bisect when it is a regression.
