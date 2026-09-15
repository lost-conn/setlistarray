# Notes for the next person

The things this codebase knows that are not obvious from reading it: how Rinch
components actually behave, the traps in the two frameworks, and where the
design the app was built to went. Moved out of the README.

---

## Where the design handoff went

The design arrived as a handoff — a hi-fi file for visuals, wireframes for
flow — and was the authority until the app had absorbed it; it was retired on
2026-09-10. It is still in history, and the
`design_handoff_setlistarray/README.md:NNN` line numbers cited in comments
refer to it there: `git show 6189dc7:design_handoff_setlistarray/README.md`.

## Working in Rinch

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
  Android is a tap and a scroll, and nothing else" in
  [ANDROID.md](ANDROID.md). `--features devtools` can press, move and release (`mouse_down`, `mouse_move`, `mouse_up`, `scroll`
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
- **Setting `DISPLAY` does not move a winit app on a Wayland desktop.** winit
  prefers the Wayland backend whenever `WAYLAND_DISPLAY` is set and never looks
  at `DISPLAY` at all, so a wrapper that exported `:99` and nothing else put the
  window on the developer's real screen — the exact thing
  `scripts/with-display.sh` exists to prevent. Found during card E3, by an agent
  that had used the wrapper as instructed and then found `xwininfo -root -tree`
  on `:99` reporting *0 children* while the app was plainly running. Both the
  wrapper and `scripts/screenshot.sh` now launch with `env -u WAYLAND_DISPLAY`.
  If you ever start the app by hand, do the same.
- `pgrep -f setlistarray` matches your own shell's command line. Kill the app by
  the PID you started, never by pattern.
- **`markup5ever_rcdom` empties nodes you are still holding.** Its hand-written
  `Drop` walks descendants iteratively and takes the `children` vector out of
  every node it reaches, whether or not something else still has a strong `Rc`
  to one. Detach an ancestor of a node you mean to keep and the node survives
  with its tag, its attributes and an empty subtree. `src/capture/reader.rs`
  moves the keeper before it sweeps, for that reason.

## And three about rhypedb

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
