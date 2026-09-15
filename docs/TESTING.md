# The visual net, and where the code lives

The pixel-level regression check that guards a commit, how its sampled regions
survive a window manager that will not grant the size the app asks for, and a
map of the source tree underneath it. Moved out of the README.

---

## Visual regression check

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
mean — this is the exact check that caught "The paint regression", in
[RINCH.md](RINCH.md)), the FAB and the first group header and the bottom-nav
strip (all sampled for the accent colour, `#B54724`), the screen background
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
| `src/screens/` | One file per screen, plus `captured_page.rs` — not a screen, but the one component the card and the full-screen viewer both mount to draw a saved page. |
| `src/db/` | The rhypedb schema, the domain↔object conversion, and the repository every store writes through. |
| `src/capture/` | Offline webpage capture: fetch, sanitise, rewrite images, judge what came back, and rebuild a saved page as markup Rinch can lay out (`render.rs`). No Signals, like `src/db/`. See [CAPTURE.md](CAPTURE.md). |
| `src/seed.rs` | Demo content, behind `--seed`. Not on the startup path. |
| `src/platform.rs` | Safe-area insets: real ones on Android, the phone stand-in on desktop. |
| `android/` | `AndroidManifest.xml`. One permission — INTERNET, for capture — and a test asserts nothing joins it. |
| `build-apk.sh` | cargo-ndk → javac → d8 → aapt2 → zipalign → apksigner → adb. |
