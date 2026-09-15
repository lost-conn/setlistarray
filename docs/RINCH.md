# The Rinch contributions

Every fault this app found in the GUI framework underneath it, what each one
looked like from up here, and which pull request carries the fix. This is why
`Cargo.toml` points at `../rinch-fixes` rather than at a git revision. Moved
out of the README.

---

## The Rinch contributions (upstream)

This app depends on Rinch through the `../rinch-fixes` path dependency (see
"Running it" in [BUILDING.md](BUILDING.md)) rather than a pinned git revision,
because `main` was missing fixes this app needed and several features it
wanted.
Each carries a PR with a regression test that fails before and passes after,
and two of them add something that was never there at all. Three have since
landed on `main`; one was superseded there, its diagnosis credited but its
diff no longer needed; the rest are still waiting on review.

**Landed on `main`:**

- [joeleaver/rinch#245](https://github.com/joeleaver/rinch/pull/245) — the paint regression
- [joeleaver/rinch#246](https://github.com/joeleaver/rinch/pull/246) — the viewport scale fault
- [joeleaver/rinch#270](https://github.com/joeleaver/rinch/pull/270) — the empty-block line-height floor, which is what blanked the search field

**Closed, superseded:**

- [joeleaver/rinch#268](https://github.com/joeleaver/rinch/pull/268) — the Android `ClickContext` viewport, which is what put the overflow menu off screen. Filed against #246 while it was still a narrower mount-time fix; #246's own review widened it into a unified `window_size` contract across all three shells (`RinchApp::layout_viewport`, a shared guarded `rinch_platform::to_logical`) before merging, which fixed the same fault for a different reason. The maintainer credited the diagnosis on closing it, and filed [issue #300](https://github.com/joeleaver/rinch/issues/300) for the two loose ends that widening left behind — addressed by #306, below.

**In `../rinch-fixes`, not yet filed upstream:**

- **A decoded image never reaches the screen on its own.** Card D4 rasterises a
  PDF page to a PNG beside the attachment and shows it with an `<img>`, and on
  both platforms the picture simply did not appear: the file was on disk, the
  `src` was right, the box was the right size, and the card was blank until the
  user happened to touch something. Three separate misses, each in a different
  crate, all with the same shape — *a finished image decode dirties no DOM node,
  and every layer assumed something dirty is the only reason to do work.*
  1. `rinch-dom`'s loader thread pushed the decode onto the pending queue and
     returned. The desktop event loop is `ControlFlow::Wait`, so nothing woke it
     and the queue sat there. It now calls `rinch_core::run_on_main_thread`.
  2. `RinchApp::resolve_and_repaint` short-circuits when no node is dirty, and
     the drain lives *inside* the layout it was skipping. It now also asks the
     new `rinch_dom::image_cache::has_pending`.
  3. `RinchDocument::resolve_layout` calls `drain_pending_images` and threw away
     its `bool`. A decoded image changes a Taffy node's *context*, not its style,
     so `layout_dirty` stayed false and the `<img>` kept the 0x0 intrinsic size
     it was created with. It now sets `layout_dirty` when the drain returns true.
  4. The Android loop decides for itself whether to call `resolve_and_repaint`,
     from `frame.pending_layout` — which a decode does not set either. It now
     also asks the new `RinchApp::has_pending_images`.

  All four are one commit on the `../rinch-fixes` branch, and upstream as
  [joeleaver/rinch#353](https://github.com/joeleaver/rinch/pull/353).
  Anything drawing a local image will hit them; `docs/PDF.md` §7 listed
  "whether Rinch's `Image` will display a cached PNG from app-private storage at
  all" as the largest unknown in card D4 and it was right to.

- **An `<img>` inside an `<a>` never appears.** Found on card E5, showing a
  captured web page — every site's logo and half its chord diagrams are wrapped
  in an anchor, so hymnal.net's masthead was simply absent from the card and the
  viewer. The image had the right `src`, a computed width and height from its
  own style, and a **0x0 layout box**; the identical `<img>` as a direct child
  of the block, or beside text inside a `<p>`, laid out correctly. Rinch does
  support atomic inlines — `ifc.rs` pushes a Parley `InlineBox` for an
  inline-block and measures it in `compute_inline_block_layouts` — but
  `mark_inline_descendants` set `ifc_root` on an inline child and did not
  recurse into it, so an inline-block *descendant* was never measured and the
  `InlineBox` read a `layout` that was still zero. The fix is to recurse,
  exactly as the `display: contents` branch beside it already does. One line
  plus a regression test in `crates/rinch-dom/tests/layout_tests.rs`
  (`test_inline_block_inside_an_inline_element_is_measured`), which fails
  without it with `(0.0, 0.0)` where `(90.0, 30.0)` is expected.

- **An `<img>` with a percentage width is laid out at its bitmap's height.**
  Not fixed, and worked around in the app instead — `song_detail::page_image`
  states both axes in pixels. `width: 100%` on a 1080x1398 page in a 393 px
  window produced a 373x1398 box: Rinch's Taffy measure closure derives the
  missing axis from the aspect ratio only when the other arrives as a
  `known_dimension`, and a percentage does not survive the content-sizing pass
  as one. Stating both dimensions skips the measure function entirely, which is
  why the workaround is reliable rather than lucky, but a page whose size the
  app has to compute for itself is a page Rinch could have sized.

- **A container stays scrolled after its content stops overflowing.** Found on
  the phone building card D5. Zoom a page in the attachment viewer to 200 %,
  drag it sideways to read the end of a staff, then zoom back to 100 %: the page
  sits half off the left edge with no way to bring it back, because a drag on
  content that no longer overflows does nothing. `scroll_offset` is clamped
  against `scroll_width - client_width` when a *scroll event* arrives and never
  when the content it is measured against shrinks, so the stale offset survives
  a relayout. A browser clamps after layout. Worked around in the app —
  `screens::attachment_viewer` puts the scrolling box inside a one-element `for`
  keyed on `(page, zoom, rotation)`, so any change of geometry builds a new node
  with a fresh offset — because nothing in an app can reach `set_scroll_left`:
  the handle belongs to the node, not to the component that declared it.

- **An open `DropdownMenu`'s target escapes an ancestor's `display: none`.**
  Also D5. The viewer's chrome hides itself after four seconds, and once the
  overflow menu had been opened, the ⋮ **stayed painted** in the corner of the
  chart with a 40x40 layout box at the window origin — the same hoisting that
  put the dismiss backdrop above its own panel in K22, seen from the other side.
  Closing the menu first is not enough; the target stays hoisted. Unmounting the
  bar is worse: remounted, the dropdown resolves `bottom-end` against a target
  box it no longer has and draws its panel half off the right edge, tall enough
  to push the bottom bar out of the `overflow: hidden` root. The app's answer is
  that the ⋮ carries its own `display: none` — after the hoist it is the hoisted
  node's own child rather than the hidden bar's descendant, so its own style
  still reaches it.

**Still open, unreviewed:**

- [joeleaver/rinch#266](https://github.com/joeleaver/rinch/pull/266) — a long press on Android is a context menu, stage 1 of three
- [joeleaver/rinch#267](https://github.com/joeleaver/rinch/pull/267) — pointer-cancel semantics, stage 2 of three
- [joeleaver/rinch#274](https://github.com/joeleaver/rinch/pull/274) — the Android IME's composing region, so autocorrect and swipe reach the document
- [joeleaver/rinch#281](https://github.com/joeleaver/rinch/pull/281) — a `<textarea>` takes a line break from Enter, and Android's keyboard offers one
- [joeleaver/rinch#286](https://github.com/joeleaver/rinch/pull/286) — an app can ship its own typefaces and say which CSS names they answer to
- [joeleaver/rinch#292](https://github.com/joeleaver/rinch/pull/292) — one paint sequence for the painter and the finger, which is what made both FABs dead
- [joeleaver/rinch#298](https://github.com/joeleaver/rinch/pull/298) — an app can tell Android its system bars sit over a light background, which is what made the clock invisible
- [joeleaver/rinch#306](https://github.com/joeleaver/rinch/pull/306) — the two loose ends issue #300 named after the #268 review: an inline, unrounded viewport division `dispatch_oncontextmenu` still did, and an architecture doc that never named `window_size`'s unit
- [joeleaver/rinch#317](https://github.com/joeleaver/rinch/pull/317) — a dropdown menu's dismiss backdrop sits under the panel it belongs to, which is what made every menu item dead. Based on #292's branch rather than `main`, because #292 is what makes the fault visible and #292 should not ship without it
- [joeleaver/rinch#353](https://github.com/joeleaver/rinch/pull/353) — four gates between a finished image decode and the screen, each enough on its own to leave an `<img>` permanently blank: the loader never woke a `ControlFlow::Wait` loop, `resolve_and_repaint` returned early on an undirty tree, `resolve_layout` discarded `drain_pending_images`'s `bool`, and the Android loop gated on a `pending_layout` a decode never sets. Card D4; found showing a rasterised PDF page on the phone
- [joeleaver/rinch#344](https://github.com/joeleaver/rinch/pull/344) — four things the software painter drew that could not be seen: an `opacity: 0` subtree painted in full, a fully transparent `background-color` rasterised as a fill, a clip mask intersected across the whole surface rather than the clip's own bounds, and a blurred `box-shadow` filled under the element instead of around it. Card K24; 316ms to 63ms on the device
- [joeleaver/rinch#417](https://github.com/joeleaver/rinch/pull/417) — a Rinch
  app on Android had no way to ask that the screen stay on. `RinchActivity`
  gains `setKeepScreenOn(boolean)`, posted through `runOnUiThread` like the
  keyboard and system-bar calls beside it, behind a `rinch_android::screen`
  wrapper. Deliberately `FLAG_KEEP_SCREEN_ON` on the window rather than a
  `PowerManager.WakeLock`: the flag is scoped to the activity and the system
  drops it when that activity stops, so the worst it can leak is a lit screen
  somebody is looking at, where a leaked wake lock is a flat battery in a bag.
  Card K5, for F4 — a phone on a music stand that sleeps in the middle of a
  song is the failure this app exists to avoid. Named `screen` and not `wake`
  because `local/both-fixes` already carries a `wake.rs` about waking the
  *frame loop*; upstream `main` has no such file, so the PR argues the naming
  on its own terms.
- [joeleaver/rinch#402](https://github.com/joeleaver/rinch/pull/402) — the two
  painters disagreed about what an `opacity` layer clips, and only one of them
  was right. `paint/mod.rs` handed `push_layer` the element's own border box as
  the layer bounds; `skia_painter.rs` named that parameter `_bounds` and never
  read it, while `vello_painter.rs` passed it to `vello::Scene::push_layer`,
  which clips every command after it. So a `box-shadow`, an overflowing child
  or a `transform` that leaves the box was drawn by the software painter and
  silently thrown away by the GPU one — which is why the GPU path could not
  become the default, however much faster card K35 made it. CSS is not
  ambiguous here: a stacking context does not clip its descendants, so Vello
  was the one in the wrong. The cheap fix was to pass an unbounded rect the way
  the zero-area path a few hundred lines up already does, and it was not the
  fix taken: a new `paint/layer_bounds.rs` walks the subtree and returns the
  union of what it actually paints — border box, outset `box-shadow`, outline,
  text-shadow reach, every descendant with its own transform applied, shrunk
  where an `overflow` ancestor inside the subtree genuinely clips — so Vello's
  clip becomes an optimisation hint that can never cut anything off. Anything
  the walk cannot measure exactly falls back to the unbounded rect rather than
  to a guess, because bounds that are too large cost a little fill and bounds
  that are too small are the bug. Measured at ~20ns a node with no allocation,
  so there is no cache to invalidate wrongly. Card K36; the disagreement was
  suspected in K24 and proven in K35. Cut from `main`, which means it does not
  carry K43's clip-skip work — the two touch different parts of `paint_node`
  and apply cleanly either way, but whoever lands both will reconcile them.
- [joeleaver/rinch#342](https://github.com/joeleaver/rinch/pull/342) — the double paint behind card K20: `PositionValue`'s `#[default]` in
  `crates/rinch-dom/src/computed_style/values.rs`, moved from `Relative` to the
  `Static` that CSS gives `position` as its initial value. Style resolution runs
  on elements only, so every text node in every Rinch document keeps
  `ComputedStyle::default()` for its whole life; with `Relative` there,
  `stacking::is_positioned_z_auto` answered `true` for all of them and each one
  was hoisted out of its parent into the nearest stacking-context ancestor's
  paint sequence. The guard that stops an inline formatting context's children
  being drawn a second time only recognises a child of the node it is called on,
  so a hoisted text node arrived somewhere it could not be skipped and was drawn
  again — this time by the standalone text path, which knows nothing of
  `text-transform`, `letter-spacing` or any inline styling and draws the raw DOM
  string at the IFC root's own box origin. Two copies of every run in the
  framework, at two widths and two offsets. `to_taffy` maps `Static` and
  `Relative` to the same Taffy position and `is_positioned_z_auto` is the only
  place in the tree that asks whether a position is non-static, so the change is
  one predicate wide. Two regression tests in
  `crates/rinch-dom/tests/stacking_tests.rs` fail before and pass after. Cut
  from `main` rather than from the integration branch, because unlike #317 it
  needs none of the others to be visible: every Rinch app has been painting its
  text twice for as long as the default has been wrong.

The `../rinch-fixes` integration branch carries the still-open fixes above
(plus the already-landed and superseded ones it was built from), which is why
the long press works in an APK built here and would not in one built against
`main`. Move the pin once they land — card A1.

**Filed 2026-09-09**, cut from `main` for the same reason the rest below were —
it needs nothing else on the integration branch:

- [joeleaver/rinch#575](https://github.com/joeleaver/rinch/pull/575) — an app
  can be told what it was launched with. `share.rs` could fire an `ACTION_SEND`
  at the chooser; nothing could be on the receiving end of one, so an app whose
  manifest declares an `<intent-filter>` was a share target that appeared in
  every chooser and then opened on whatever screen it would have opened on
  anyway. `on_incoming_intent` and a drain beside the others in
  `android_runtime` are the plumbing; the two things that are not plumbing are
  the cold-start handshake — `onCreate` queues rather than calling a native,
  because `RinchActivity.java` already warns that a native called from a
  lifecycle override races the thread registering them, and `bridge::init`
  calls `flushPendingIntents()` once registration is known to be done — and the
  rule that the drain *keeps* what it cannot deliver, without which the launch
  share would be discarded on every cold start by a handler that had not
  registered yet. Card "Share intents"; the PDF half was driven through Files
  by Google's real share sheet, because `am start --grant-read-uri-permission`
  does not actually confer the grant it names.

**Filed 2026-09-09**, both found while making the app follow the system's dark
mode and its Material You accent, and both cut from `main` for the same reason
the pair below were — neither needs anything else on the integration branch:

- [joeleaver/rinch#571](https://github.com/joeleaver/rinch/pull/571) — a
  configuration-change handler, so a platform reading can stop being frozen at
  mount. Nothing surfaced `MainEvent::ConfigChanged`, so dark mode flipping at
  sunset, a new accent, or insets changing on a fold were all things Android
  knew and nothing carried across. `set_configuration_change_handler` is the
  slot, shaped after `set_keyboard_interceptor` and living in `rinch-core` for
  a reason worth reading: every module in `rinch-android` is
  `cfg(target_os = "android")`, so a hook there would be *absent* on desktop
  rather than inert, and every call site would pay for it with a `#[cfg]` and a
  stub. It dispatches on the main thread because the handler's whole job is to
  write signals, and `Signal::set` panics off it. Card K53, which had been
  parked precisely because there was nothing to listen to.
- [joeleaver/rinch#572](https://github.com/joeleaver/rinch/pull/572) —
  `night_mode`, `wallpaper_primary` and `system_accent`, the three readings
  that say what the system's theme *is*. All three need
  `bridge::with_activity`, which is private to that crate, which is why none of
  them could live here. `system_accent` is the one with the argument behind it:
  `theme_customization_overlay_packages` carries a `color_source` that reads
  `preset` whenever somebody picked a basic colour in Wallpaper & style, and in
  that configuration the wallpaper's own primary is exactly the thing they
  overrode — so extracting it returns a confidently wrong answer rather than a
  missing one. Cards K8 and the system half of the theme work.

**Filed 2026-09-03**, both found the day before while building the backup and
the Settings screen, and both cut from `main` rather than from the
integration branch because neither needs anything else on it:

- [joeleaver/rinch#514](https://github.com/joeleaver/rinch/pull/514) —
  `write_content_uri`, the other half of the pair `read_content_uri` had been
  half of since Android bring-up. `save_file` fires `ACTION_CREATE_DOCUMENT`
  and hands back the `content://` URI of a document it just created, and
  nothing in `rinch-android` could put bytes into one — the only
  `openOutputStream` in `RinchActivity.java` was buried inside `shareImage`,
  hard-coded to a MediaStore JPEG. Card K4, so that card I1's backup could
  reach a file at all. Driven through the real SAF dialog on the moto g before
  it was believed.
- [joeleaver/rinch#515](https://github.com/joeleaver/rinch/pull/515) —
  removing a `display: contents` node freed its own Taffy slot, which it never
  had, instead of its children's, which it did. `sync_display_contents`
  polyfills the property by hiding the wrapper's Taffy node and splicing its
  *children's* ids into the grandparent's Taffy child list; `remove_node` then
  asked Taffy to remove the wrapper's own id, which was a silent no-op, and the
  child that actually occupied the slot was never asked to leave. It stayed a
  permanent invisible `flex: 1` sibling of the app root.
  `Route::Library`'s arm is the only one the RSX macro wraps that way — it is
  the only one whose body is a reactive `if` rather than one element — so the
  ghost appeared the first time anybody navigated away from the library and
  then halved the height of every screen reached afterwards, for the rest of
  the process. Settings drew four of its eleven rows and the accent picker card
  H2 had just shipped could not be reached on a phone at all. Card K48; a
  host-side regression test in `crates/rinch-dom/tests/layout_tests.rs` fails
  before and passes after, so the next one of these is caught on a laptop.

#292 was found here and filed late. Rinch derives paint order and
hit-test order twice, by different rules, and implements no CSS painting step 8
— so a `position: absolute; z-index: auto` element over an `overflow: auto`
sibling loses to it both ways. The fault was reported here as a dead FAB, on the
belief that the painter got it right and only the finger did not; writing the
PR's readback test disproved that. The scrolling list painted *over* the FAB as
well. It never looked wrong only because the `z-index: 10` workaround both FABs
carry had been hiding the visual half from the day it was added. It is not
Android-specific and it reproduces on the desktop.

#317 is what #292 turned up next, and the same sentence covers it: it looked
Android-only and it was not. Every dropdown menu in the app opened correctly
and then swallowed the tap on its own items — the menu closed and nothing ran.
`DropdownMenu`'s dismiss backdrop is `position: fixed`, which Rinch treats as
viewport-level content hoisted out of every ancestor clip and, because an
overflow clip *is* a stacking context there, out of every ancestor stacking
context with it. The `z-index: 99` that was supposed to keep it under the
panel's `100` was being compared across two stacking contexts, which is to say
not compared at all, and behind this app's `overflow: hidden` root the backdrop
was simply on top. Before #292 the painter hoisted fixed boxes and hit testing
did not, so the backdrop painted over the panel invisibly while taps still
found the item underneath; #292 makes one sequence answer both, and this idiom
could not survive the answer it kept. `Select` had the identical fault. The fix
puts the backdrop back in the panel's own stacking context, where the two
z-indexes mean something. It reproduces on the desktop with a mouse — that is
where it was diagnosed, driving the real app through rinch's debug IPC.

## The paint regression

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

## The viewport scale fault

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
