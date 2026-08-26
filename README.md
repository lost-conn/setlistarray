# SetListArray

An offline-first book of the songs you know how to play. Attach charts to
them — PDFs, webpages captured for offline use, or typed text — and arrange
them into setlists. No account, no sync, nothing uploaded. One outbound
network call exists in the whole app — fetching a webpage you have pasted in
yourself, so that it still opens with no signal.

Built with [Rinch](https://github.com/joeleaver/rinch). The design handoff in
`design_handoff_setlistarray/` is the authority: the hi-fi file for visuals,
the wireframes (turn 2 wins over turn 1) for flow.

## Running it

```bash
./scripts/install-fonts.sh   # once — Newsreader and Karla, via fontconfig
cargo run --release          # always --release; debug Stylo/Parley is slow
cargo run --release -- --seed   # ...with the demo library, if yours is empty
```

The library lives in `$XDG_DATA_HOME/setlistarray` (`~/.local/share/setlistarray`):
a rhypedb database in `db/`, and one directory per attachment under
`attachments/`. A first run opens an empty book; `--seed` fills it with the demo
content, and only ever into a library that has nothing in it.

Requires stable Rust (`rust-toolchain.toml` pins it, along with the two Android
targets). Rinch's docs ask for nightly; current main does not need it.

Also requires `mold` and `clang` on PATH: `.cargo/config.toml` links host builds
with mold, which turns a relink of this dependency tree into well under a second.
Package names are `mold` and `clang` on Debian/Ubuntu, Fedora and Arch alike. To
build without them, delete `.cargo/config.toml` — it only overrides the host
triple, so the Android build goes through the NDK's linker either way.

**This will not build from a fresh clone on its own.** Both frameworks are
local path dependencies, so `Cargo.toml` expects three checkouts side by side:

```
projects/personal/
├── setlistarray/     ← this
├── rinch-fixes/      ← github.com/joeleaver/rinch, on a branch carrying #245 + #246
└── rhypedb-main/     ← github.com/joeleaver/rhypedb, main
```

Both pins are deliberate and temporary — see "The two Rinch faults" below, and
card A1. When those PRs land, `rinch` goes back to a git revision; rhypedb stays
a path dep while this app is its early in-process consumer.

### Visual regression check

```bash
scripts/screenshot.sh            # build, run under X11, capture, check
scripts/screenshot.sh --update   # re-record thresholds from this run instead
scripts/screenshot.sh --self-test  # assert the region arithmetic; builds nothing
```

Builds release, launches the app under X11 (`-u WAYLAND_DISPLAY`; window
capture needs a real X window) against a **throwaway seeded library**, grabs
its window with ImageMagick's `import`, and samples six known-good regions
of the library screen against
`scripts/screenshot-baseline.json`: the first row's attachment thumb (grey
mean — this is the exact check that caught "The paint regression" below),
the FAB and the first group header and the bottom-nav strip (all
sampled for the accent colour, `#B54724`), the screen background
(`#FBF7F0`), and a negative control. Exits non-zero if any check fails, so it
can gate a commit.

The sixth is the negative control, and it is not about the app at all: it
asserts that the *other* corner of the same empty status-bar strip contains
**no** accent pixels, through the same crop-and-`compare` path the accent
checks use, at the same fuzz. Every other check here is of the form "this
colour is present", and a suite made only of those goes green just as happily
when the colour comparison has quietly started matching everything. Nothing
the app can do makes this one red; only the machinery can. `--update`
therefore refuses to re-record its ceiling — a control measured from the run
it is policing is not a control.

### Regions are anchored, not pinned

Every region says which corner or edge of the captured image it hangs off,
and is resolved against the capture's real dimensions at check time. The
window manager is not obliged to grant the size the app asks for, and this
one does not: the same `WM_NORMAL_HINTS` request produced a 491×1065 window
one morning and a 550×1065 one that afternoon, with no code change in
between. Absolute `WxH+X+Y` crops survive neither — the width grew by 59px,
the bottom-right FAB moved with the right edge, and its sample slid off the
button onto the paper beside it. The check went red while the FAB was
perfectly fine, which is the one thing a commit gate must never do.

So `fab_solid_accent` anchors bottom-right, `bottom_nav_accent` anchors to the
bottom and spans the width (the two nav items are `flex: 1`, so the active one
re-centres and there is no fixed x to sample), and the content-column checks
anchor top-left and say so rather than relying on it. The offsets are written
in **CSS pixels** — the units `src/` is written in, so `right: 36` reads
against the FAB's own `right: 20px` without arithmetic — and converted through
one scale factor, derived as `captured_height / 852`. Height and never width:
the window manager stretched the width and left the height at exactly
852 × 1.25, so the height is the dimension still carrying the scale factor
honestly. A region that omits its anchor is an error, not a default.

`--self-test` resolves every region against five capture sizes (both the ones
this machine has produced, an absurdly wide one, a 2× display and a 1× one)
and asserts the results, plus that eight kinds of malformed region are
rejected. It builds nothing, launches nothing and needs no X server, so it is
the part of this net that can run anywhere.

The library it samples is its own: a fresh `$XDG_DATA_HOME` under `$TMPDIR`,
filled by `--seed` and deleted on the way out. Never
`~/.local/share/setlistarray`. Two of the five checks — the first row's thumb
and the first group header — sample *content*, so pointing them at whatever
happens to be in your own book makes the baseline a measurement of your songs
rather than of this repository, and a gate that can go red for a reason
`git diff` cannot show you is not a gate. It has already happened once: an
emptied library drew bare paper where the first thumb should be, and the two
content checks failed in a way that reads exactly like a fresh paint
regression. It also means the check runs fine while you have the real app
open — rhypedb takes a directory lock, and previously the two collided.

The window is found by matching its title *and* its `_NET_WM_PID` X property
against the PID this script just launched — not by title alone, because this
machine routinely runs several worktrees of this repo side by side and two
of them can have a window titled "SetListArray" open at once. Matching by
title only risks silently sampling a sibling's window instead of your own.

Captures land in `.screenshots/` (gitignored) — `latest.png` plus one
timestamped PNG and app log per run, so a failure leaves something to look
at. The app is killed by PID on the way out, never `pkill -f setlistarray`
(that also matches this script's own command line).

`--update` re-measures every check from a fresh run and rewrites the
baseline file; it does not overwrite it blindly — it prints a `name: old ->
new` line per check, and that diff (`git diff scripts/screenshot-baseline.json`)
is what to review before committing. A diff that isn't explained by a
deliberate visual change means something regressed, not that the baseline
needed updating.

**The pin is deliberately not on `main`.** See "The flex regression" below.

The handoff targets Android, and Rinch has an Android backend, so the app
builds for both. On the desktop it runs in a 393×852 phone-shaped window;
nothing in the UI code assumes either platform. See "Android" below.

## Layout

| Path | What lives there |
| --- | --- |
| `src/lib.rs` | The app root: the stores, the route switch, the bottom nav. Both entry points run this. |
| `src/main.rs` | The desktop binary. Three lines. |
| `src/android.rs` | `android_main`. Picks the data directory and starts the Android shell. |
| `src/platform.rs` | The safe-area seam: real insets on Android, a phone stand-in on the desktop. |
| `src/theme.rs` | Every design token, as CSS custom properties. Nothing downstream hard-codes a hex. |
| `src/model.rs` | `Song`, `Setlist`, `Attachment`, `Confidence`, `Day`. Every field but id/title/artist/created_at is optional. |
| `src/store/` | One `Copy` struct of Signals per store, registered in `app()`. Derived values are computed on read, never stored. Every mutation writes through `Storage` to the database before it reaches a signal. |
| `src/ui.rs` | Shared pieces: chips, confidence dots, attachment thumbs, list rows. |
| `src/screens/` | One file per screen. |
| `src/db/` | The rhypedb schema, the domain↔object conversion, and the repository every store writes through. |
| `src/capture/` | Offline webpage capture: fetch, sanitise, rewrite images, judge what came back. No Signals, like `src/db/`. See [docs/CAPTURE.md](docs/CAPTURE.md). |
| `src/seed.rs` | Demo content, behind `--seed`. Not on the startup path. |
| `src/platform.rs` | Safe-area insets: real ones on Android, the phone stand-in on desktop. |
| `android/` | `AndroidManifest.xml`. One permission — INTERNET, for capture — and a test asserts nothing joins it. |
| `build-apk.sh` | cargo-ndk → javac → d8 → aapt2 → zipalign → apksigner → adb. |

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
- **Add to setlist sheet** — wireframe `2e`. Bottom sheet over the song,
  multi-select, `already in this set` on the sets that hold it, create inline.
- **Sort & group sheet** — wireframe `2c`. Group-by chips, every metadata field
  as a sort row, tap the active row to reverse, sparse fields greyed with a
  count but still selectable.
- **Add / edit song** — wireframe `1j`. One screen for both. Title and artist
  visible, everything else behind a collapsed **More details**; the artist field
  autocompletes against the artists already in the book, with the `use
  "<typed>"` escape hatch. Reached from the library FAB (add) and from song
  detail's pencil or a row's overflow menu (edit). Its **Attachments** section
  is not built — those are Phase D.

See [docs/PLAN.md](docs/PLAN.md) for the phased plan to finish the rest.

## What is not

Each of these has a `Stub` screen naming its wireframe: Settings (`1q`),
Performance view (`1o`). Not yet started: attachment viewer (`1k`), search &
filter (`1p`), first run (`1r`).

Offline webpage capture (`1l`) has its **engine** — `src/capture/`, tested and
proven against eight real chord sites — and none of its UI. It runs on both
targets: the manifest declares `android.permission.INTERNET` for it, which is
the decision written up in [docs/CAPTURE.md](docs/CAPTURE.md). What it captures
is narrower than "any chord site", and the site table there says which kinds of
page survive.

Also outstanding:

- **Backup.** Export/import as a zip of the database and the attachments
  directory (Phase I).
- **Accent from the system.** `AccentChoice::FromSystem` falls back to Rust
  until Rinch exposes the wallpaper colour.
- **Drag-to-reorder and swipe-to-remove** in setlists.

## Android

> **Unverified on hardware.** There is no device and no emulator on the machine
> this was built on. `./build-apk.sh --build-only` produces a signed,
> `apksigner`-verified APK and that is the whole of what has been checked. The
> app has never been launched on a phone. Every claim below about how it
> *behaves* on device is a reading of the code, not an observation.

One crate, two targets. `src/main.rs` is the desktop binary; the same crate also
builds as a `cdylib` whose `android_main` (`src/android.rs`) starts Rinch's
Android shell with the same `app()` component. There is no `#[cfg]` in any
screen — the two things that genuinely differ each have a seam:

- **`src/platform.rs`** — `safe_area()`. On Android, `safe_area_insets()` and
  `density_dpi()` from `rinch-android`, converted from physical pixels to CSS
  pixels. On the desktop, the numbers a phone would report, because the desktop
  window is a preview of one. This replaced a hard-coded 44px status strip.
- **`src/db/DataDir`** — installed once by the entry point. `android_main` uses
  `AndroidApp::internal_data_path()`, i.e. `/data/data/<package>/files`:
  app-private, needs no permission, removed with the app. The desktop keeps
  `$XDG_DATA_HOME/setlistarray`. `app()` publishes it as a context, so a
  repository can reach it without knowing which platform it is on.

### Prerequisites

| What | Default the script looks in | Override |
| --- | --- | --- |
| NDK r27c | `~/android/android-ndk-r27c` | `ANDROID_NDK_HOME` |
| SDK build-tools 35 | `~/android/sdk/build-tools/35.0.0` | `ANDROID_SDK_BUILD_TOOLS` |
| `android.jar` (android-35) | `~/android/sdk/platforms/android-35/android.jar` | `ANDROID_SDK_PLATFORM` |
| The Rinch checkout (for `RinchActivity.java`) | `../rinch-fixes` | `RINCH_DIR` |
| Signing key | `target/debug.keystore`, generated on first run | `ANDROID_DEBUG_KEYSTORE` |

Also `cargo-ndk` on `PATH` (`cargo install cargo-ndk`), a JDK for `javac`, and
the Android targets — which `rust-toolchain.toml` already declares. `adb` is
needed only to install.

### Building

```bash
export ANDROID_NDK_HOME=$HOME/android/android-ndk-r27c
./build-apk.sh --build-only          # signed APK at ./setlistarray.apk
./build-apk.sh --target x86_64       # for an emulator instead of a phone
./build-apk.sh                       # the above, then adb install and launch
```

Defaults to `arm64-v8a` and the release profile. It builds `--lib` only: the
desktop binary and the probe are not part of the APK. Roughly 6.5 MiB, almost
all of it `libsetlistarray.so` — it was 5.8 MiB before the capture engine
brought html5ever and its friends in.

### One permission, on purpose

`android/AndroidManifest.xml` declares exactly one:
`android.permission.INTERNET`. `the_android_manifest_asks_only_for_internet`
(in `src/lib.rs`) fails the build if a second one appears, whatever it is.

It is there for one feature. Offline webpage capture fetches a page you pasted
in yourself, and Android refuses the socket without it — the installer puts a
package in the `inet` group only when the manifest asks, and there is no way
round that from app code. INTERNET is a *normal* permission: granted at
install, never prompted for, absent from the app's permission screen, not
revocable. It gives the app no reach into anything of yours — no files, no
contacts, no location, no identifiers.

So the promise is no longer "no permissions". It is narrower and it is
checkable: **one permission, one call site, nothing else reaches the network.**
The call site is `src/capture/fetch.rs`, and
`the_http_client_is_named_in_exactly_one_file` holds it to being the only file
under `src/` that names the HTTP client at all. That is a floor rather than a
proof — card X2 is the real assertion — but it is what stands between one call
and a few.

Everything else still needs nothing. App-private storage needs no permission,
and attachment import (card K4) goes through the system file picker, which
grants access per file without one either. Camera, location and external
storage are not asked for and should not be.
`android:allowBackup="false"` is unchanged and is not in tension with any of
this: the permission lets the app reach out, while cloud backup would let
Google's servers reach in and copy the library off the device. Opposite
directions, and only one of them is a feature you asked for.

This section used to read "No permissions, on purpose", and until card E1 that
was both true and untested. E1 wrote the test and, on the way, found the one
feature that could not live inside it. Adding the line was chosen over dropping
capture from Android and over routing it through the system share sheet; the
reasoning, and the two options not taken, are in
[docs/CAPTURE.md](docs/CAPTURE.md).

### Known unknowns

Things that cannot be settled without a device:

- **Insets may double up below Android 15.** `RinchActivity` never opts into
  edge-to-edge. Android 15 enforces it for anything targeting SDK 35, so there
  the insets are ours to apply and `safe_area()` is right. On Android 14 and
  below the system already insets the window, and the strip this app reserves
  would be added on top of that. The fix is upstream (`setDecorFitsSystemWindows`
  in `RinchActivity`), not here.
- **The fonts do not ship.** Newsreader and Karla are picked up from the system
  font list, which on Android does not have them; the app will fall back to
  Noto Serif and Roboto. Rinch can register font bytes
  (`RinchApp::register_font_data`) but neither `run_android` nor
  `ThemeProviderProps` exposes a way to reach it, so `assets/fonts/` cannot get
  into the APK yet. Small upstream fix; the app is already carrying the files.
- **Touch, IME and the soft keyboard** are all untried. Card K7.
- **Keep-awake** does not exist in Rinch's Android backend at all. Card K5.
- **`ClickContext`'s viewport is wrong by one scale factor on Android** — see
  below. Popup placement, not tap targets.

### A third Rinch fault, found by reading

`shell/android_runtime.rs` passes its `logical_size` as `handle_event`'s
`window_size`. That parameter is documented on the desktop side as *physical*
("`RinchApp::handle_event` divides it by the scale factor itself"), and
`handle_event` does exactly that:

```rust
let vp_w = window_size.0 as f32 / scale_factor as f32;
```

So on Android the viewport handed to every `ClickContext` is
`physical / scale²` — 143×310 where it should read 393×852 on a 2.75× phone.

It is narrower than it sounds. Pointer coordinates are *separately* divided by
the scale factor in `collect_input_events`, and the layout tree is resolved at
the logical size, so hit-testing agrees with itself and **taps land where they
should**. What is wrong is `ClickContext::viewport_width` / `viewport_height`,
which is what decides whether a `<select>` popup or a dropdown flips up or down
and how it is clamped to the screen edge. This app has no such control yet, so
nothing visible is broken today.

Found while fixing the desktop equivalent
([joeleaver/rinch#246](https://github.com/joeleaver/rinch/pull/246)), which left
it out of scope. The fix is one line — pass `physical_size` — and a follow-up
PR upstream, not a change here.

## The two Rinch faults (fixed upstream, awaiting review)

The app is pinned to Rinch `d25f646` (2026-03-17). Two faults on `main` kept it
there; both now have PRs, each with a regression test that fails before and
passes after:

- [joeleaver/rinch#245](https://github.com/joeleaver/rinch/pull/245) — the paint regression
- [joeleaver/rinch#246](https://github.com/joeleaver/rinch/pull/246) — the viewport scale fault

Move the pin once they land.

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
- **A `position: absolute` child needs an explicit `z-index` to be tappable**
  when it overlaps a scrolling sibling. `Node::creates_stacking_context` counts
  `overflow: auto`, so the scroll box is hoisted into its ancestor's
  z-index-0 stacking phase — which hit-testing walks before the plain
  non-stacking children, where a `z-index: auto` positioned element sits.
  Paint disagrees with that and draws the positioned element on top, so the
  symptom is a button that looks right and does nothing. The library FAB was
  dead for exactly this reason until card C1 gave it a layer.
- Rinch interpolates a `transform` transition through its **matrix**, and the
  matrix does not carry percentage translations (`rinch-dom`'s
  `transition/apply.rs` zeroes them). `translateY(100%) → translateY(0)` snaps
  instead of sliding; the bottom sheets park themselves in pixels for that
  reason. A node also has to already be in the tree to slide, so the sheets stay
  mounted and go transparent to taps rather than unmounting.
- `--features devtools` is the only way to drive the app without a human, but on
  a HiDPI display it also re-lays the page out at the physical surface size
  (491×1065 here, not 393×852) after a `screenshot` command. Hit-test with the
  boxes from a `dom_tree` dump rather than with coordinates read off a picture,
  and take visual measurements from a build without the feature.
- `pgrep -f setlistarray` matches your own shell's command line. Kill the app by
  the PID you started, never by pattern.
- **`markup5ever_rcdom` empties nodes you are still holding.** Its hand-written
  `Drop` walks descendants iteratively and takes the `children` vector out of
  every node it reaches, whether or not something else still has a strong `Rc`
  to one. Detach an ancestor of a node you mean to keep and the node survives
  with its tag, its attributes and an empty subtree. `src/capture/reader.rs`
  moves the keeper before it sweeps, for that reason.

### And two about rhypedb

- **`@on_delete` reads backwards from how it looks.** The policy is applied to
  the relations pointing *at* the object being deleted, and it acts on their
  *source*. `Song.attachments: [Attachment] @on_delete(cascade)` therefore means
  "deleting a chart deletes the song" — which is how schema v1 had it. The
  cascade that takes a chart with its song has to live on `Attachment.song`,
  with `Song.attachments` as the `@inverse`. `Setlist.songs @on_delete(remove)`
  happens to read correctly under the same rule.
- **Reopening a library the same process just closed races.** rhypedb's
  compaction worker holds a `Weak` to the tree and upgrades it while it works;
  if the last external handle goes during that window, the tree — and the
  directory lock with it — is released on the worker's thread. A second
  *process* gets a clean refusal, but a test that restarts the app in-process
  has to retry (`db::restart`). The app itself opens the library once.
