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
cargo run --release          # always --release; debug Stylo/Parley is slow
cargo run --release -- --seed   # ...with the demo library, if yours is empty
```

The typefaces are in the binary (`crate::FONTS`), so nothing has to be
installed first. `scripts/install-fonts.sh` still puts Newsreader and Karla on
the system for other tools that want them; the app no longer needs it.

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
├── rinch-fixes/      ← github.com/joeleaver/rinch, branch carrying #245, #246, #266, #267, #268, #270, #292
└── rhypedb-main/     ← github.com/joeleaver/rhypedb, main
```

Both pins are deliberate and temporary — see "The Rinch contributions"
below, and card A1. When those PRs land, `rinch` goes back to a git revision;
rhypedb stays a path dep while this app is its early in-process consumer.

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

The handoff targets Android, and Rinch has an Android backend, so the app runs
on both — it has been on a phone since. On the desktop it runs in a 393×852
phone-shaped window;
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
  primary attachment card, collapsed other attachments, footer actions. The card
  is fed by the attachment's own content rather than by skeleton bars: a typed
  chart renders monospaced with its chord alignment intact, and a kind with
  nothing to render yet (a PDF before D4, a capture before E2 has extracted its
  text) says so in one muted line. Tapping a collapsed row inlines it and does
  not promote it; long-pressing one opens **Set as primary · Remove
  attachment**. The first chart a song gets becomes its primary one, and
  removing the primary promotes the oldest chart still attached — see
  `Song::attach` / `detach` / `set_primary` in `src/model.rs`, which is where
  those three rules live.
- **Setlist detail** — hi-fi. Position numbers, the cumulative start-time
  column, the no-chart warning pill, the derived "Before you start" panel,
  Play set. Every value on it is derived inside a reactive closure, because the
  song picker adds to the set while the screen is still underneath it.
  **Reorder** turns the row list into an edit mode: Move up / Move down /
  Remove per song, with an Undo strip that puts a removed song back in the
  place it came out of. Not the drag handle and swipe-left the handoff asks
  for — see "Touch on Android is a tap and a scroll, and nothing else" below.
- **Setlists tab** — wireframe `1m`, styled with the hi-fi tokens.
- **Add to setlist sheet** — wireframe `2e`. Bottom sheet over the song,
  multi-select, `already in this set` on the sets that hold it, create inline.
- **Setlist song picker** — wireframe `1i`. Slides over setlist detail so the
  set stays readable behind it; search, `All / Solid / Recent / Tag` chips
  (`Tag` reveals a second row of the tags in your book), a checkbox list with
  `already in this set` on the songs the set holds, a running `2 picked` count
  and **Add 2 songs**. Picked songs join the end of the set in the order they
  were ticked, in one write.
- **Sort & group sheet** — wireframe `2c`. Group-by chips, every metadata field
  as a sort row, tap the active row to reverse, sparse fields greyed with a
  count but still selectable.
- **Add / edit song** — wireframe `1j`. One screen for both. Title and artist
  visible, everything else behind a collapsed **More details**; the artist field
  autocompletes against the artists already in the book, with the `use
  "<typed>"` escape hatch. Reached from the library FAB (add) and from song
  detail's pencil or a row's overflow menu (edit). Its **Attachments** section
  is not built — those are Phase D, and the one producer that exists lives on
  song detail instead (below).
- **Typed lyrics / chords** — card D2, and **the handoff never drew this
  screen**: `1j` names the row that opens it (`TXT · Type lyrics / chords · ›`)
  and stops, `1k` is the viewer. So it is the minimal reading of the card,
  wearing `1j`'s own chrome: full-screen `✕ · Lyrics / chords · Save` over one
  field in the app's mono face, writing a `Text` attachment. Reached from song
  detail's `+ Add attachment` — which is live now, and says underneath that
  typing is what it currently does, because PDFs (D3) and saved pages (E2) are
  not built. An existing typed chart reopens in the same screen from its
  long-press menu's **Edit lyrics / chords…**.

  Save is the only writer, as on the form above it. Nothing is created by
  *opening* the editor: a chart minted on arrival and abandoned would be an
  empty ghost holding the primary card. `✕` over changed text asks before
  discarding — an inline strip, not a dialog — and goes straight back when
  nothing was typed. An empty body is refused rather than written, and the
  refusal points at **Remove attachment** rather than inventing a second way to
  delete a chart. The chart is named after its first line of text, because the
  collapsed row is the only place a title shows and two rows reading
  `lyrics · typed` would be unusable.

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
- **Most of the gestures in the handoff.** Drag-to-reorder, swipe-to-remove and
  swipe between songs in performance mode cannot be built on this framework's
  Android backend today, and each has an explicit tap-driven stand-in instead.
  Long-press was the fourth of them until rinch#266, which the local branch now
  carries: it opens a context menu on a phone, and that has been watched
  happening. The finding is below; the affected cards are C6 (done, with
  buttons), F2, K15, and the long-press note in `src/menu.rs`, which the fix
  has left out of date in the app's favour.

## Android

> **Run on two Android 13 targets, and only those.** A moto g stylus 5G (2022)
> — arm64-v8a, 1080×2460 at density 400, so a 2.5× scale and a logical 432×984
> — and Waydroid on x86_64, where the app gets a freeform 609×1059 window at
> density 225 (1.41×, logical 432×752). Both report SDK 33. It launches, it
> renders, it takes taps and text, and what follows names the target whenever
> the target is the point.
>
> That is one handset and one container on one Android version. No Android 14
> and no Android 15 — and 15 is where the inset question below actually lives —
> no tablet, no fold, no low-density screen, no second manufacturer's skin.
> Every gesture below was driven through `adb`; none of it has been under a
> human finger.

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
./build-apk.sh --target x86_64       # for an emulator or Waydroid, not a phone
./build-apk.sh                       # the above, then adb install and launch
```

Defaults to `arm64-v8a` and the release profile. It builds `--lib` only: the
desktop binary and the probe are not part of the APK. Roughly 6.6 MiB on either
target — 6,935,051 bytes for arm64-v8a, 6,976,008 for x86_64 — almost all of it
`libsetlistarray.so`, a stripped 21 MiB shared object that the zip squeezes to
about a third. It was 5.8 MiB before the capture engine brought html5ever and
its friends in.

### One permission, on purpose

`android/AndroidManifest.xml` declares exactly one:
`android.permission.INTERNET`. `the_android_manifest_asks_only_for_internet`
(in `src/lib.rs`) fails the build if a second one appears, whatever it is.

It is there for one feature. Offline webpage capture fetches a page you pasted
in yourself, and Android refuses the socket without it — the installer puts a
package in the `inet` group only when the manifest asks, and there is no way
round that from app code. INTERNET is a *normal* permission: granted at
install, never prompted for, absent from the app's permission screen, not
revocable. The phone bears that out: `dumpsys package` lists
`android.permission.INTERNET: granted=true` under *install permissions* and no
runtime permissions whatever, which is exactly why the permission screen has
nothing to show. It gives the app no reach into anything of yours — no files, no
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

### What the phone settled

- **Insets do not double up on Android 13.** This section used to predict that
  they would below Android 15, reasoning that `RinchActivity` never opts into
  edge-to-edge and the system would therefore inset the window itself. It does
  not. The shell logs `InitWindow: 1080x2460 physical` — the whole display,
  status bar and cutout included — so the window is never inset and
  `safe_area()` applies the strip exactly once. The phone has a real punch-hole
  cutout (`DisplayCutout{insets=Rect(0, 115 - 0, 0)}`) and the title lands 140
  physical pixels down: 115 for the cutout, the header's own 6 CSS px, and the
  serif's leading. Doubled, it would have started past 245. Card K2's
  "unverified against a real notch" can go with it. **Android 15 is still
  untested**, and it is the version that enforces edge-to-edge for an SDK 35
  target, so the question is open there and nowhere else.
- **The icon set renders. Some Unicode does not.** Every Tabler icon draws
  correctly — the gear, the FAB's plus, both nav glyphs, the search magnifier,
  the trash in the overflow menu, the chevron on **More details**. The tofu is
  in the two places a *text* glyph stands in for an icon: the sort chip's
  `↑`/`↓` (U+2191/2193, `SortDir::arrow`) and the density chip's `≣` (U+2263).
  `·` and `…` come through fine, so this is not "no Unicode" — it is two
  characters the fallback font does not carry. Card K13.
- **Touch, focus and the IME work.** Taps land where they are aimed, the search
  field focuses and raises the soft keyboard, and typed characters reach the
  store *and are drawn* — the last of those took rinch#270. Card K7 is no
  longer "untried".
- **A text field used to vanish on the tap that focused it.** Card K11, and it
  was never about the IME or about paint scheduling: the value attribute was
  correct on every frame. Rinch gives a childless block container a one-line
  `min-height` floor — the only thing that gives an `<input>` a height at all,
  since its value lives in an attribute rather than in a child — and wrote that
  floor straight onto the node's Taffy style from a pass that runs only on a
  *structural* change. The pass that runs on every *style* change rebuilt the
  style from the computed values and dropped it. Focusing a field re-resolves
  its style (`data-focused`, `data-cursor-pos`, DOM `:focus`) and nothing
  structural happens beside it, so the input collapsed to zero height, and a
  zero-size box is skipped whole by `paint_node`: no background, no value, no
  caret, for the life of the process. On the desktop the same thing happens and
  self-heals within a frame, because filtering the list as you type is a
  structural change that re-runs the pass — which is why it looked like an
  Android fault, and was not one. [joeleaver/rinch#270](https://github.com/joeleaver/rinch/pull/270).
- **INTERNET really is invisible.** `dumpsys package` lists it under *install
  permissions*, `granted=true`, with no runtime permissions at all — which is
  why there is nothing for the app's permission screen to show. The claim under
  "One permission, on purpose" holds on the device.

### Still open

- **The status bar icons are drawn white on the app's cream paper** and are
  close to illegible — the clock especially. The app never tells Android that
  its bars sit over a light background, so the system keeps the light-content
  icons it starts with. Card K12.
- ~~**The fonts do not ship.**~~ Fixed upstream and consumed here. Rinch's
  shells now take an `&[AppFont]` — the file plus the CSS names it answers to —
  and register it before the first layout pass; `crate::FONTS` in `src/lib.rs`
  carries Newsreader, Karla and DejaVu Sans Mono into the `.so` on both
  platforms. On the moto, `Songs` is Newsreader rather than Noto Serif (the
  title's ink is 196px wide where Noto Serif's was 226px) and the body is Karla
  rather than Roboto.
- **Keep-awake** does not exist in Rinch's Android backend at all. Card K5.
- **`ClickContext`'s viewport is wrong by one scale factor on Android** — and
  it is no longer harmless. See below.
- **A `<textarea>` cannot scroll to its caret**, on either platform. Whatever
  falls past its `rows` height is clipped at the border and unreachable, and
  there is no scroll inside the control at all. The typed-chart editor works
  around it by driving `rows` from the value — `chart_editor::field_rows`, which
  is deliberately generous, because over-guessing costs blank paper and
  under-guessing eats a line — so the box grows and the *screen* scrolls
  instead. Watched on the moto: 28 lines drawn in full, all reachable with the
  keyboard up. It is a stopgap and it is approximate for soft-wrapped lines.
- **Tapping to place the caret in a multi-line field lands about a line off.**
  Real, and the first thing a hand hits when correcting a chord — a tap aimed at
  the end of line four put the caret mid-line and the next 22 lines went in
  there. Nothing the app can do about it from here.
- **No IME inset is exposed**, so a focused field can sit under the soft
  keyboard with nothing telling the app it happened. The editor carries 320px of
  scrollable emptiness below its field so the screen can always be scrolled far
  enough by hand. Also a stopgap.
- ~~**`font-family: monospace` does not resolve on Android**~~ — and it was
  worse than "the app's font is missing". Every name in `--sla-font-mono`
  resolved to *nothing* on the phone, `monospace` included: the platform's
  generic map looks up a font family literally named `monospace`, and no font
  file is called that, so the whole stack fell through to the proportional
  script fallback. Fixed by shipping DejaVu Sans Mono and declaring it as
  `monospace`. On the moto, in the chart editor's own field, the chord `D` in
  column 18 sat 112px left of the syllable it belongs over; it now sits on it
  exactly (both at x=515), and the two lines finally share one column width
  (20.3px against 20.3px, where the chord line's spaces used to measure 9.2px
  against the lyric line's 15.5px).

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

It is narrower than it sounds, and it is no longer invisible. Pointer
coordinates are *separately* divided by the scale factor in
`collect_input_events`, and the layout tree is resolved at the logical size, so
hit-testing agrees with itself and **taps land where they should**. What is
wrong is `ClickContext::viewport_width` / `viewport_height`, which is what
decides whether a `<select>` popup or a dropdown flips up or down and how it is
clamped to the screen edge.

`src/menu.rs` is exactly such a control — `DropdownMenu` from `rinch-components`
does the viewport-edge flipping this app declined to reimplement — and now that
a long press can reach it on a phone (below), the misplacement is on screen.
Long-press a library row on the moto and the overflow menu opens **upward**:
everything but its last item lands outside the list's scroll box, painted under
the chip row. The arithmetic accounts for it exactly. The row sits about 300 CSS
pixels down a screen that is really 984 tall, which leaves room for the menu
below it — but the menu believes the screen is 393 tall, and 300 down a 393-tall
screen is a row with nothing under it, so it flips. On the desktop the same menu
opens downward. That is the first user-visible symptom of this fault, and it
moves it from a note to a thing to fix.

Found while fixing the desktop equivalent
([joeleaver/rinch#246](https://github.com/joeleaver/rinch/pull/246)), which left
it out of scope. The fix is one line — pass `physical_size` — and a follow-up
PR upstream, not a change here.

### Touch on Android is a tap and a scroll, and nothing else

**No gesture in the handoff could be built on the Android backend as it stood.**
Not drag-to-reorder, not swipe-to-remove, not swipe-between-songs in
performance mode, not long-press. This was found by card C6's spike before any
of it was built on, which is the only reason it did not cost a phase; card K15
carries it upstream. One of the four has since been fixed; that is the end of
this section, and the heading has been left alone because three other places —
"What is built" above, `docs/PLAN.md` and `src/menu.rs` — point at it by name.

Every touch on Android goes through one recogniser — `TouchGesture::process` in
`rinch/src/shell/android_runtime.rs` — and on `main`, which is still what a
build against upstream gets, it emits:

| MotionEvent | What the app gets |
| --- | --- |
| `Down` | `MouseMove` at the touch point. **No `MouseDown`.** |
| `Move`, under 8px | nothing |
| `Move`, past 8px | `MouseWheel { x, y }` at the **touch-down origin**, carrying the frame's delta. No `MouseMove`. |
| `Up` after a still finger | `MouseDown` **immediately followed by** `MouseUp`, at the down position |
| `Up` after a moving finger | **nothing at all** |

Three consequences, each of which kills a feature:

- **`ondragstart` can never fire.** Rinch's DOM drag arms a *pending* drag on
  `MouseDown` and promotes it on the first `MouseMove` more than 5px away
  (`app/event_dispatch.rs`). On Android the only `MouseDown` ever emitted is
  the one paired with the `MouseUp` beside it, so the pending drag is created
  and consumed in the same event batch and dispatches an ordinary click.
- **A swipe is invisible to the app.** A moving finger produces wheel deltas
  and nothing else. The vertical half at least fires `data-onscroll` when a
  scroll container actually moves; the *horizontal* half fires no handler at
  all — `event_dispatch.rs` scrolls `scroll_offset.0` and dispatches nothing.
  So there is no signal to hang "swipe left to remove" on, and no event when
  the finger lifts to commit it either.
- **There was no press-and-hold.** Already written up in `src/menu.rs` for a
  different reason (`onclick` fires synchronously inside the `MouseDown`
  handler, so no timer can get between a tap and its navigation); this was the
  second, independent reason, and `oncontextmenu` — the desktop stand-in that
  file uses — was never synthesised from touch at all. This is the one that has
  been fixed.

All of it works on the desktop backend, which is what makes it dangerous: a
gesture written and tested in the phone-shaped window here is dead on the
device and nothing says so. `src/bin/gesture_probe.rs` plus
`scripts/gesture-probe.py` are the harness that established the desktop half
empirically — press, eight moves, release, driven through the debug IPC's
`mouse_down`/`mouse_move`/`mouse_up`, which go through the same
`RinchApp::handle_event` a real mouse does. On the desktop a handle drag fires
`dragstart → dragenter/dragover per row → drop on the target → dragend` with
usable coordinates, taps still work on `draggable` elements, and a horizontal
drag on a row reports its delta — none of which transfers.

The whole fix is upstream and is a real piece of work, not a one-liner: the
recogniser has to emit a genuine down/move/up stream and let the DOM decide
what claims it, rather than deciding "this is a scroll" on the app's behalf
8 pixels in.

**One of the four is done.**
[joeleaver/rinch#266](https://github.com/joeleaver/rinch/pull/266) makes a press
held still past `ViewConfiguration.getLongPressTimeout()` — 500ms, the deadline
Android's own widgets use — synthesise a right-button press, which `RinchApp`
already routes through `dispatch_oncontextmenu`, the same and only dispatch a
desktop right-click takes. Crossing the 8px slop still makes it a scroll and
lifting early still makes it a tap; once the context event has fired, the lift
emits only the matching right-button release, so a long press cannot also
activate what it was held over. Watched on the moto: holding a library row for
900ms opens the overflow menu, and the app does not navigate to the song
underneath. Where the menu *lands* is a different fault — see "A third Rinch
fault" above.

It is stage 1 of three. Stage 2 is pointer-cancel semantics; stage 3 is real
pointer events with capture, where the scroll decision is finally deferred to
the DOM. Drag and swipe wait on stage 3, so until then every affordance in this
app except the long-press menu is a tap.

## The Rinch contributions (upstream, awaiting review)

The app is pinned to Rinch `d25f646` (2026-03-17). Faults on `main` kept it
there; each has a PR with a regression test that fails before and passes after,
and two of them add something that was never there at all:

- [joeleaver/rinch#245](https://github.com/joeleaver/rinch/pull/245) — the paint regression
- [joeleaver/rinch#246](https://github.com/joeleaver/rinch/pull/246) — the viewport scale fault
- [joeleaver/rinch#266](https://github.com/joeleaver/rinch/pull/266) — a long press on Android is a context menu, stage 1 of three
- [joeleaver/rinch#267](https://github.com/joeleaver/rinch/pull/267) — pointer-cancel semantics, stage 2 of three
- [joeleaver/rinch#268](https://github.com/joeleaver/rinch/pull/268) — the Android `ClickContext` viewport, which is what put the overflow menu off screen
- [joeleaver/rinch#270](https://github.com/joeleaver/rinch/pull/270) — the empty-block line-height floor, which is what blanked the search field
- [joeleaver/rinch#274](https://github.com/joeleaver/rinch/pull/274) — the Android IME's composing region, so autocorrect and swipe reach the document
- [joeleaver/rinch#281](https://github.com/joeleaver/rinch/pull/281) — a `<textarea>` takes a line break from Enter, and Android's keyboard offers one
- [joeleaver/rinch#286](https://github.com/joeleaver/rinch/pull/286) — an app can ship its own typefaces and say which CSS names they answer to
- [joeleaver/rinch#292](https://github.com/joeleaver/rinch/pull/292) — one paint sequence for the painter and the finger, which is what made both FABs dead
- [joeleaver/rinch#298](https://github.com/joeleaver/rinch/pull/298) — an app can tell Android its system bars sit over a light background, which is what made the clock invisible

The `../rinch-fixes` integration branch carries all of them, which is why the
long press works in an APK built here and would not in one built against
`main`. Move the pin once they land.

The last of those was found here and filed late. Rinch derives paint order and
hit-test order twice, by different rules, and implements no CSS painting step 8
— so a `position: absolute; z-index: auto` element over an `overflow: auto`
sibling loses to it both ways. The fault was reported here as a dead FAB, on the
belief that the painter got it right and only the finger did not; writing the
PR's readback test disproved that. The scrolling list painted *over* the FAB as
well. It never looked wrong only because the `z-index: 10` workaround both FABs
carry had been hiding the visual half from the day it was added. It is not
Android-specific and it reproduces on the desktop.

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
- **Spike a gesture before you design around it.** Everything the desktop
  backend does with a pointer, the Android backend does not — see "Touch on
  Android is a tap and a scroll, and nothing else" above. `--features devtools`
  can press, move and release (`mouse_down`, `mouse_move`, `mouse_up`, `scroll`
  — all routed through the same `handle_event` a real mouse is), so a gesture
  can be proven without a hand. `src/bin/gesture_probe.rs` is the pattern:
  write every handler's name and coordinates into one text node and read it
  back with `query_selector` + `text_content`, rather than trying to see what
  happened in a picture.
- **A flex item that grows taller drags the row's baseline with it.** Setlist
  detail's rows are `align-items: baseline`, and putting the reorder controls
  inside the title column slid the position number and the whole cumulative
  clock down to the bottom of the row. The controls are a sibling *underneath*
  the baseline row for that reason.
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

### And three about rhypedb

- **`@on_delete` reads backwards from how it looks.** The policy is applied to
  the relations pointing *at* the object being deleted, and it acts on their
  *source*. `Song.attachments: [Attachment] @on_delete(cascade)` therefore means
  "deleting a chart deletes the song" — which is how schema v1 had it. The
  cascade that takes a chart with its song has to live on `Attachment.song`,
  with `Song.attachments` as the `@inverse`. `Setlist.songs @on_delete(remove)`
  happens to read correctly under the same rule.
- **A background thread could lose the app a write, at random.** rhypedb spawns
  a cover-refresh worker by default: an `update()` on an object something links
  *to* queues a rewrite of the covering blobs embedded in those inbound edges,
  and the worker commits it on its own thread. The transaction manager detects
  write-write conflicts by comparing a transaction's snapshot against everything
  committed since, so a foreground write touching the same edge keys a moment
  later loses and comes back `write conflict`. Card D1 walked straight into it:
  attaching a chart writes the `Attachment → Song` link and then updates the
  song, and deleting that song immediately afterwards failed **roughly half the
  time**, differently on every run. `src/db/mod.rs` now opens with
  `background_cover_refresh: false` — this app has exactly one writer and no use
  for a housekeeper that can make a delete fail — and
  `attaching_a_chart_and_deleting_its_song_is_repeatable` runs the sequence
  forty times, because one round passed perfectly well on the bad build. The
  alternative, retrying a conflicted write in `Storage`, was not taken: it would
  work, and it would hide the next race instead of removing it.
- **Reopening a library the same process just closed races.** rhypedb's
  compaction worker holds a `Weak` to the tree and upgrades it while it works;
  if the last external handle goes during that window, the tree — and the
  directory lock with it — is released on the worker's thread. A second
  *process* gets a clean refusal, but a test that restarts the app in-process
  has to retry (`db::restart`). The app itself opens the library once.
