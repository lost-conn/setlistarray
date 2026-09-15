# Android

What this app does on a phone, and what the phone did back. The record of
what hardware settled, what is still broken, what the one permission is for,
and why every affordance here is a tap. Moved out of the README, unedited
except for the cross-references. Building the APK is [BUILDING.md](BUILDING.md).

---

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

## One permission, on purpose

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
[CAPTURE.md](CAPTURE.md).

## What the phone settled

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
- **A dropdown menu's items answer the tap aimed at them.** They did not, on
  either platform: the menu opened, every tap inside it closed the menu, and
  no item's handler ever ran. Not a touch fault and not a divergence — an
  invisible `position: fixed` backdrop sitting above the panel it was written
  to sit under. [joeleaver/rinch#317](https://github.com/joeleaver/rinch/pull/317),
  and the paragraph under "The Rinch contributions" in [RINCH.md](RINCH.md)
  for the mechanism. On the moto: **Duplicate** turned one song into two,
  **Delete song** turned them
  back into one, and **Edit lyrics / chords…** off a long-pressed chart opened
  the editor with the chart in it. Card K22.
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
- **Every run of text in the app was being painted twice.** Card K20, found in
  the screenshots taken to verify K12 and not caused by it. The visible damage
  was in the two places the two copies disagree: a group header styled
  `text-transform: uppercase; letter-spacing: 0.16em` drew "SOLID" with "Solid"
  struck through it, at two widths; a chip with `padding: 6px 12px` drew its
  label twice, a line and a padding apart. Everywhere else the copies landed on
  top of each other and read as slightly heavy antialiasing. It looked
  Android-only and was not: the desktop had been doing it since long before the
  phone, and the visual net never caught it although one of its regions sat
  squarely on the fault: a check that samples *colour* cannot see a run painted
  twice in the same colour in the same place, so `group_header_accent` was green
  throughout, counting 156 accent pixels where it needed 40. The cause is one
  word in `rinch-dom`: `PositionValue` defaulted to `Relative` rather than the
  `Static` CSS says is the initial value, and text nodes never reach style
  resolution, so every text node in every document looked positioned, was
  hoisted out of its parent into the nearest
  stacking-context ancestor, and landed where the guard against painting an IFC
  root's children twice cannot see it.
  [joeleaver/rinch#342](https://github.com/joeleaver/rinch/pull/342); the check
  that would have caught it is
  `group_header_double_paint` in `scripts/screenshot-baseline.json`.
- **Nothing in the app animated, and the reason was not the animation code.**
  Card K24. The first frame after a tap took 316ms to present on the moto g
  stylus 5G, so a 220ms transition got exactly one tick — the one that finished
  it. The sheet was parked, and then it was arrived. None of that time was
  where it looked like it should be: presenting the pixels was a flat 12ms,
  re-resolving style was 0.0ms, glyphs were 7ms. It was all in paint, and all of
  it was work with no visible output — three always-mounted sheet scrims parked
  at `opacity: 0` being painted in full and composited back at alpha zero; every
  element's `background-color: transparent` rasterised as a real fill because
  that is the CSS initial value and Stylo hands it back as a colour; every
  nested clip intersecting its mask across all 2.66 million pixels of the
  surface; a blurred `box-shadow` filling eight layers across the whole sheet
  panel to darken pixels the panel then covered. **316ms → 63ms**, and the
  sheets now move through the positions in between.
  [joeleaver/rinch#344](https://github.com/joeleaver/rinch/pull/344). What is
  left is honest rasterisation — CPU tiny-skia at a full 1080×2460, where one
  opaque full-screen fill is 13ms — so roughly four frames per transition:
  animated, not yet smooth. The structural answer is the GPU path, which is
  card K27, not this one — and which is now what `./build-apk.sh` builds; see
  the next entry.
- **The phone gets the GPU painter now, and the number that mattered was not
  the median.** Cards K39 through K43 and K41. The owner said for three cards
  that the app felt like 30fps while an in-process probe reported 8.33ms p50,
  and the owner was right: a timer inside the process measures how long the app
  took to hand a frame over, not whether the compositor ever put it on the
  glass. `dumpsys SurfaceFlinger --latency` measures the second thing, and
  `scripts/frame-probe.sh` is now the house way to ask it — with a stock-app
  control on the same panel in the same minute, because a throttled handset
  makes any app look bad. It has two traps written into it that cost real time
  to find: SurfaceFlinger answers out of a **128-entry ring buffer**, so a long
  run silently reports only its last second (which is the cheap coasting tail,
  and flattered the app by 40fps), and `input swipe` against a list already at
  its end measures a screen that never moved. Measured with both fixed, two
  runs each, against stock Settings at 8.33ms p50 / 120.0fps / 0.0% missed:
  the GPU path is **8.37ms p50, 16.69ms p95, 84.8–85.7fps, 38.7–39.9% missed**
  and the software path **8.35ms p50, 25.18ms p95, 70.6–70.9fps, 43.9–45.6%**.
  The medians are identical and tell you nothing; the difference is that a
  missed frame costs two refreshes on the GPU path and three on the software
  one. K41 recorded the software path at 20.8fps and the GPU at 58fps a day
  earlier — both moved because K43's clip cuts live in `paint/mod.rs`, which
  feeds both painters, and every clip layer costs tiny-skia a full-surface
  pixmap. What allowed the flip was not speed but K36: until it landed, the
  faster path could silently throw away a shadow. `--software` is one flag
  away, because the GPU path is proven on exactly one driver.
- **INTERNET really is invisible.** `dumpsys package` lists it under *install
  permissions*, `granted=true`, with no runtime permissions at all — which is
  why there is nothing for the app's permission screen to show. The claim under
  "One permission, on purpose" holds on the device.

## Still open

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
- **No bottom sheet opens on Android.** Found while verifying K22 and unrelated
  to it. **Add to setlist**, the sort chip and the **Filter** chip each set a
  `NavStore` signal that a sheet reads, and on the phone nothing appears —
  including from the song screen's own full-width **Add to setlist** button,
  which is an ordinary tap on an ordinary element and never went near a menu.
  So the handler is not the question: every one of the three works on the
  desktop against the same build. Reproduced from a cold start with no menu
  ever opened, so it is not a stuck overlay either. Undiagnosed; it wants a
  card of its own.
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

## A third Rinch fault, found by reading

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

## Touch on Android is a tap and a scroll, and nothing else

**No gesture in the handoff could be built on the Android backend as it stood.**
Not drag-to-reorder, not swipe-to-remove, not swipe-between-songs in
performance mode, not long-press. This was found by card C6's spike before any
of it was built on, which is the only reason it did not cost a phase; card K15
carries it upstream. One of the four has since been fixed; that is the end of
this section, and the heading has been left alone because two other places —
`docs/PLAN.md` and `src/menu.rs` — point at it by name.

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
