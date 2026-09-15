#!/usr/bin/env bash
# Build, run and screenshot SetListArray, then sample known-good regions of
# the library screen and report pass/fail per check.
#
# This is the regression net card A3 asks for: the paint regression (see
# docs/RINCH.md, "The paint regression") produced a DOM and layout that were
# byte-for-byte identical to a good render — only pixels caught it. So pixels
# are what this script checks, sampled the same way the fix was verified by
# hand: `import` grabs the live X11 window, `convert`/`compare` sample it.
#
# The app is launched against a throwaway seeded library, never the real one
# in ~/.local/share/setlistarray, so the pixels it samples depend on nothing
# but the code in this repository. See the long note above the launch below.
#
# Every sampled region is anchored to an edge of the captured image rather
# than pinned to an absolute WxH+X+Y. The window manager is not obliged to
# grant the size the app asks for and this one does not: the same
# `WM_NORMAL_HINTS` request has produced a 491x1065 window and later a
# 550x1065 one, on the same machine, on the same day. Absolute coordinates
# survive neither, so each region says which corner or edge it hangs off and
# is resolved against the capture's real dimensions here. See
# `region_schema` in the baseline file.
#
# Usage:
#   scripts/screenshot.sh              # build, run, capture, check against
#                                       # scripts/screenshot-baseline.json
#   scripts/screenshot.sh --update     # same, but rewrite the baseline from
#                                       # this run's measurements instead of
#                                       # checking against it
#   scripts/screenshot.sh --self-test  # resolve every region against a table
#                                       # of capture sizes and assert the
#                                       # arithmetic; builds and launches
#                                       # nothing
#
# Exit status is non-zero if any check fails (or the window never appears),
# so this can gate a commit later. Every capture is kept under
# .screenshots/ (gitignored) for inspection; a run that fails prints exactly
# where to look.
set -euo pipefail

# ---------------------------------------------------------------------------
# Setup
# ---------------------------------------------------------------------------

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/release/setlistarray"
BASELINE_FILE="$ROOT/scripts/screenshot-baseline.json"
CAPTURE_DIR="$ROOT/.screenshots"

# How long to let the app render before the first capture attempt, then how
# many times to retry the capture itself (X11 can map the window a moment
# after the process starts) before giving up.
LAUNCH_WAIT_SECONDS=8
CAPTURE_ATTEMPTS=6
CAPTURE_RETRY_DELAY=2
# `import` on a not-yet-mapped window prints its error immediately but can
# then sit for several more seconds before actually exiting, so this needs
# to be generous enough to never truncate a real capture but tight enough
# that a genuinely missing window fails in well under a minute.
CAPTURE_TIMEOUT_SECONDS=6

MODE="check"
for arg in "$@"; do
  case "$arg" in
    --update|-u)
      MODE="update"
      ;;
    --self-test)
      MODE="self-test"
      ;;
    -h|--help)
      sed -n '2,38p' "${BASH_SOURCE[0]}"
      exit 0
      ;;
    *)
      echo "unknown option: $arg" >&2
      echo "usage: $(basename "${BASH_SOURCE[0]}") [--update | --self-test]" >&2
      exit 1
      ;;
  esac
done

RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; BOLD=$'\033[1m'; RESET=$'\033[0m'
info()  { printf '%s\n' "$*" >&2; }
ok()    { printf '%s✓%s %s\n' "$GREEN" "$RESET" "$*" >&2; }
warn()  { printf '%s!%s %s\n' "$YELLOW" "$RESET" "$*" >&2; }
die() {
  printf '%s✗ %s%s\n' "$RED" "$*" "$RESET" >&2
  exit 1
}

# Everything below this point maps a real X window and photographs it, and the
# display it does that on is not allowed to be the one somebody is working in.
# This script has always taken focus for the ten-odd seconds it takes to launch
# and capture, which was merely rude; on 2026-08-28 an agent verifying the file
# picker went further and opened native file dialogs on the developer's desktop
# while the developer was using it. So the run re-enters itself under
# `scripts/with-display.sh`, which puts it on a private, invisible Xvfb at the
# same 1.25x scale factor this file's numbers were measured at.
#
# `--self-test` is exempt because it never opens a window, and an explicit
# `SLA_HEADLESS_DISPLAY=1` is exempt because that is what with-display.sh sets
# on the way in — and because a human who genuinely wants to watch the capture
# happen on their own screen can set it by hand and get the old behaviour.
if [[ "$MODE" != self-test && -z "${SLA_HEADLESS_DISPLAY:-}" ]]; then
  WITH_DISPLAY="$ROOT/scripts/with-display.sh"
  [[ -x "$WITH_DISPLAY" ]] || die "missing $WITH_DISPLAY — it is what keeps this off :0"
  exec "$WITH_DISPLAY" "${BASH_SOURCE[0]}" "$@"
fi

# --self-test only reads the baseline and does arithmetic, so it must not
# refuse to run on a machine with no X server, no ImageMagick and no toolchain
# — that is most of the value of having it. Everything else needs the lot.
if [[ "$MODE" == self-test ]]; then
  REQUIRED_TOOLS=(jq awk)
else
  REQUIRED_TOOLS=(cargo import convert compare identify jq xwininfo xprop)
fi
for tool in "${REQUIRED_TOOLS[@]}"; do
  command -v "$tool" >/dev/null 2>&1 || die "missing dependency: $tool"
done

# ---------------------------------------------------------------------------
# Geometry: anchor-relative regions
#
# The window manager, not the app, decides how big the window is. This one
# grants a different width on different days for the same `WM_NORMAL_HINTS`
# request — 491x1065 once, 550x1065 later, no code change in between — so a
# region recorded as an absolute `WxH+X+Y` crop is a region that will one day
# quietly slide off the thing it was pointed at. That is worse than a crash:
# the FAB check went red while the FAB was perfectly fine, because its crop
# had drifted onto the paper beside it.
#
# So the baseline states, per region, which corner or edge of the *captured
# image* it hangs off, and the offsets are measured inward from there. A
# bottom-right region stays on the FAB however wide the window turns out to
# be; a top-left region stays on the content column, which starts at the left
# edge and does not move either.
#
# The offsets are written in CSS pixels — the units src/ is written in, so
# `right: 36` can be read against the FAB's `right: 20px` without arithmetic —
# and converted to the capture's physical pixels through one scale factor.
# ---------------------------------------------------------------------------

# Which offset keys each anchor requires, and whether it spans the capture
# width. Anything not listed here is rejected, which is the point: a future
# check cannot be added without stating where it hangs from.
anchor_offsets() {
  case "$1" in
    top-left)     printf 'left top\n' ;;
    top-right)    printf 'right top\n' ;;
    bottom-left)  printf 'left bottom\n' ;;
    bottom-right) printf 'right bottom\n' ;;
    top)          printf 'top\n' ;;
    bottom)       printf 'bottom\n' ;;
    *)            return 1 ;;
  esac
}

anchor_spans_width() {
  case "$1" in
    top|bottom) return 0 ;;
    *)          return 1 ;;
  esac
}

# CSS pixels to this capture's physical pixels. Rounds half away from zero,
# the same way the old absolute-coordinate scaler did.
phys() {
  awk -v v="$1" -v s="$SCALE" 'BEGIN { printf "%d", (v * s) + 0.5 }'
}

# Resolve one check's `region` object against ACTUAL_W/ACTUAL_H/SCALE.
#
# On success sets REGION_W/REGION_H/REGION_X/REGION_Y and returns 0. On a
# schema fault it sets REGION_ERROR and returns 1 rather than calling `die`,
# so that --self-test can assert the faults are caught as well as the
# arithmetic; every other caller treats a non-zero return as fatal.
resolve_region() {
  local check="$1" name="$2"
  REGION_ERROR=""
  REGION_W=""; REGION_H=""; REGION_X=""; REGION_Y=""

  local anchor keys key value spans w_spec h
  anchor="$(jq -r '.region.anchor // "«missing»"' <<<"$check")"
  if ! keys="$(anchor_offsets "$anchor")"; then
    REGION_ERROR="check \"$name\": region.anchor is \"$anchor\"; every region must declare one of top-left, top-right, bottom-left, bottom-right, top, bottom"
    return 1
  fi

  w_spec="$(jq -r '.region.w // "«missing»"' <<<"$check")"
  h="$(jq -r '.region.h // "«missing»"' <<<"$check")"
  if ! [[ "$h" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
    REGION_ERROR="check \"$name\": region.h must be a number, got \"$h\""
    return 1
  fi

  if anchor_spans_width "$anchor"; then
    spans=yes
    if [[ "$w_spec" != "full" ]]; then
      REGION_ERROR="check \"$name\": anchor \"$anchor\" spans the capture width, so region.w must be the string \"full\", got \"$w_spec\""
      return 1
    fi
  else
    spans=no
    if ! [[ "$w_spec" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
      REGION_ERROR="check \"$name\": region.w must be a number for anchor \"$anchor\", got \"$w_spec\""
      return 1
    fi
  fi

  # Every offset the anchor names has to be there and has to be a number. A
  # region that leaves one out would otherwise resolve to 0 and land in a
  # corner, which is exactly the silent-wrong-answer this whole rewrite is
  # about.
  local left="" top="" right="" bottom=""
  for key in $keys; do
    value="$(jq -r --arg k "$key" '.region[$k] // "«missing»"' <<<"$check")"
    if ! [[ "$value" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
      REGION_ERROR="check \"$name\": anchor \"$anchor\" requires a numeric region.$key, got \"$value\""
      return 1
    fi
    case "$key" in
      left)   left="$value" ;;
      top)    top="$value" ;;
      right)  right="$value" ;;
      bottom) bottom="$value" ;;
    esac
  done

  REGION_H="$(phys "$h")"
  if [[ "$spans" == yes ]]; then
    REGION_W="$ACTUAL_W"
    REGION_X=0
  else
    REGION_W="$(phys "$w_spec")"
    if [[ -n "$left" ]]; then
      REGION_X="$(phys "$left")"
    else
      REGION_X=$(( ACTUAL_W - $(phys "$right") - REGION_W ))
    fi
  fi
  if [[ -n "$top" ]]; then
    REGION_Y="$(phys "$top")"
  else
    REGION_Y=$(( ACTUAL_H - $(phys "$bottom") - REGION_H ))
  fi

  # A region that has fallen off the capture is never what was meant, and
  # sampling it would either crash `convert` or — worse — silently return the
  # clamped remainder. Say so instead.
  if (( REGION_W <= 0 || REGION_H <= 0 || REGION_X < 0 || REGION_Y < 0 \
        || REGION_X + REGION_W > ACTUAL_W || REGION_Y + REGION_H > ACTUAL_H )); then
    REGION_ERROR="check \"$name\": region resolves to ${REGION_W}x${REGION_H}+${REGION_X}+${REGION_Y}, which does not fit inside the ${ACTUAL_W}x${ACTUAL_H} capture"
    return 1
  fi
  return 0
}

# Read the logical window size once — the size the app *asks* for, which is a
# constant of the source (WIDTH/HEIGHT in src/lib.rs) and not a measurement.
LOGICAL_W="$(jq -r '.logical_window.width' "$BASELINE_FILE")"
LOGICAL_H="$(jq -r '.logical_window.height' "$BASELINE_FILE")"

# Derive the display scale factor from the capture's height and never its
# width. The window manager has been observed granting 491x1065 and 550x1065
# for the same request: it stretched the width and left the height exactly at
# 852 x 1.25. So the height is the dimension that still carries the scale
# factor honestly, and the width is a number the compositor made up. There is
# no other source of truth here — winit knows the real scale factor but the
# app does not print it — so this is derived, not read, and it is derived from
# the trustworthy half. If a window manager ever starts stretching the height
# too, this assumption breaks loudly (every region shifts at once) rather than
# quietly, which is the failure mode to prefer.
derive_scale() {
  awk -v a="$1" -v l="$2" 'BEGIN { printf "%.6f", a / l }'
}

# ---------------------------------------------------------------------------
# --self-test: the region arithmetic, with no app and no X server
#
# The whole point of the anchor rewrite is that it survives a window size
# nobody can arrange on demand. There is no way to make this window manager
# grant a chosen geometry (no Xvfb, no wmctrl, no xdotool on this machine), so
# the arithmetic is asserted directly instead, over the real regions from the
# real baseline file at five capture sizes: the two this machine has actually
# produced, an absurdly wide grant, a 2x display, and a 1x one. A resolver
# that is right at all five is right for the reason the checks need it to be.
#
# It also asserts the schema faults, because "every region must state its
# anchor" is only true if leaving it out is an error rather than a default.
# ---------------------------------------------------------------------------

if [[ "$MODE" == self-test ]]; then
  SELF_TEST_FAILURES=0

  expect_region() {
    local name="$1" want="$2" check got
    check="$(jq -c --arg n "$name" '.checks[] | select(.name == $n)' "$BASELINE_FILE")"
    [[ -n "$check" ]] || die "self-test: no check named \"$name\" in $BASELINE_FILE"
    if ! resolve_region "$check" "$name"; then
      SELF_TEST_FAILURES=$((SELF_TEST_FAILURES + 1))
      printf '%s✗ %s@%sx%s%s — %s\n' "$RED" "$name" "$ACTUAL_W" "$ACTUAL_H" "$RESET" "$REGION_ERROR" >&2
      return 0
    fi
    got="${REGION_W}x${REGION_H}+${REGION_X}+${REGION_Y}"
    if [[ "$got" == "$want" ]]; then
      ok "$(printf '%-26s @ %sx%-5s -> %s' "$name" "$ACTUAL_W" "$ACTUAL_H" "$got")"
    else
      SELF_TEST_FAILURES=$((SELF_TEST_FAILURES + 1))
      printf '%s✗ %-26s @ %sx%s%s -> %s, expected %s\n' "$RED" "$name" "$ACTUAL_W" "$ACTUAL_H" "$RESET" "$got" "$want" >&2
    fi
  }

  expect_schema_fault() {
    local label="$1" region="$2" check
    check="$(jq -cn --argjson r "$region" '{name: "synthetic", region: $r}')"
    if resolve_region "$check" "synthetic"; then
      SELF_TEST_FAILURES=$((SELF_TEST_FAILURES + 1))
      printf '%s✗ schema fault not caught%s — %s resolved to %sx%s+%s+%s\n' \
        "$RED" "$RESET" "$label" "$REGION_W" "$REGION_H" "$REGION_X" "$REGION_Y" >&2
    else
      ok "$(printf '%-26s -> rejected: %s' "$label" "$REGION_ERROR")"
    fi
  }

  at_size() {
    ACTUAL_W="$1"; ACTUAL_H="$2"
    SCALE="$(derive_scale "$ACTUAL_H" "$LOGICAL_H")"
    info ""
    info "${BOLD}capture ${ACTUAL_W}x${ACTUAL_H}${RESET} (scale $SCALE)"
  }

  # 491x1065 — the size this window manager granted when the baseline numbers
  # below were first measured, and the size the app still asks for (393 x 1.25).
  at_size 491 1065
  expect_region thumb_painted            "40x50+30+330"
  expect_region fab_solid_accent         "65x58+396+882"
  expect_region confidence_dots_on_screen "43x20+425+353"
  expect_region group_header_accent      "55x20+20+290"
  expect_region group_header_double_paint "55x25+20+285"
  expect_region bottom_nav_accent        "491x70+0+975"
  expect_region screen_background        "40x30+0+0"
  expect_region background_has_no_accent "40x30+451+0"
  expect_region screen_title_glyph       "200x53+28+60"

  # 550x1065 — the size the same window manager grants now, for the same
  # request. Only the two right-anchored regions and the full-width one move,
  # and they move by exactly the 59px the compositor added.
  at_size 550 1065
  expect_region thumb_painted            "40x50+30+330"
  expect_region fab_solid_accent         "65x58+455+882"
  expect_region confidence_dots_on_screen "43x20+484+353"
  expect_region group_header_accent      "55x20+20+290"
  expect_region group_header_double_paint "55x25+20+285"
  expect_region bottom_nav_accent        "550x70+0+975"
  expect_region screen_background        "40x30+0+0"
  expect_region background_has_no_accent "40x30+510+0"
  expect_region screen_title_glyph       "200x53+28+60"

  # 700x1065 — no window manager has done this yet; the arithmetic should not
  # care that it is unreasonable.
  at_size 700 1065
  expect_region thumb_painted            "40x50+30+330"
  expect_region fab_solid_accent         "65x58+605+882"
  expect_region confidence_dots_on_screen "43x20+634+353"
  expect_region bottom_nav_accent        "700x70+0+975"
  expect_region background_has_no_accent "40x30+660+0"
  expect_region screen_title_glyph       "200x53+28+60"

  # 786x1704 — a 2x display. Everything scales, including the vertical
  # offsets, which is what the height-derived scale factor is for.
  at_size 786 1704
  expect_region thumb_painted            "64x80+48+528"
  expect_region fab_solid_accent         "104x92+634+1412"
  expect_region confidence_dots_on_screen "68x32+682+564"
  expect_region group_header_accent      "88x32+32+464"
  expect_region group_header_double_paint "88x40+32+456"
  expect_region bottom_nav_accent        "786x112+0+1560"
  expect_region screen_background        "64x48+0+0"
  expect_region background_has_no_accent "64x48+722+0"
  expect_region screen_title_glyph       "320x84+44+96"

  # 393x852 — a 1x display, where CSS pixels and physical pixels are the same
  # thing and the numbers in the baseline should appear unchanged.
  at_size 393 852
  expect_region thumb_painted            "32x40+24+264"
  expect_region fab_solid_accent         "52x46+317+706"
  expect_region confidence_dots_on_screen "34x16+341+282"
  expect_region group_header_accent      "44x16+16+232"
  expect_region group_header_double_paint "44x20+16+228"
  expect_region bottom_nav_accent        "393x56+0+780"
  expect_region screen_background        "32x24+0+0"
  expect_region background_has_no_accent "32x24+361+0"
  expect_region screen_title_glyph       "160x42+22+48"

  info ""
  info "${BOLD}schema faults${RESET} (a region that does not say where it hangs from is an error, not a default)"
  ACTUAL_W=550; ACTUAL_H=1065; SCALE="$(derive_scale 1065 "$LOGICAL_H")"
  expect_schema_fault "anchor missing"          '{"w":10,"h":10,"left":0,"top":0}'
  expect_schema_fault "anchor unknown"          '{"anchor":"middle","w":10,"h":10}'
  expect_schema_fault "corner missing offset"   '{"anchor":"bottom-right","w":10,"h":10,"right":4}'
  expect_schema_fault "corner wrong offset"     '{"anchor":"bottom-right","w":10,"h":10,"left":4,"top":4}'
  expect_schema_fault "edge anchor numeric w"   '{"anchor":"bottom","w":10,"h":10,"bottom":4}'
  expect_schema_fault "corner anchor full w"    '{"anchor":"top-left","w":"full","h":10,"left":0,"top":0}'
  expect_schema_fault "h missing"               '{"anchor":"top-left","w":10,"left":0,"top":0}'
  expect_schema_fault "region off the capture"  '{"anchor":"top-left","w":10,"h":10,"left":9000,"top":0}'

  info ""
  if [[ "$SELF_TEST_FAILURES" -eq 0 ]]; then
    ok "${BOLD}region resolver self-test passed${RESET}"
    exit 0
  else
    die "${BOLD}$SELF_TEST_FAILURES region resolver assertions failed${RESET}"
  fi
fi

mkdir -p "$CAPTURE_DIR"

WINDOW_TITLE="$(jq -r '.window_title' "$BASELINE_FILE")"
STAMP="$(date +%Y%m%d-%H%M%S)"
SHOT_PATH="$CAPTURE_DIR/${STAMP}.png"
LATEST_PATH="$CAPTURE_DIR/latest.png"
APP_LOG="$CAPTURE_DIR/${STAMP}.app.log"

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

info "building (cargo build --release)…"
( cd "$ROOT" && cargo build --release ) || die "build failed"
[[ -x "$BIN" ]] || die "build succeeded but $BIN is not there — check the [[bin]] name in Cargo.toml"

# ---------------------------------------------------------------------------
# Run + capture
#
# X11, not Wayland: window capture needs a real X window, and `-u
# WAYLAND_DISPLAY` is what makes winit pick the X11 backend on a system that
# has both running. Killed strictly by PID — never `pkill -f setlistarray`,
# which also matches this script's own command line in `ps` and has taken
# down the wrong process before.
#
# The run gets a library of its own — a throwaway $XDG_DATA_HOME, seeded with
# the demo content by `--seed` — and never touches the real book in
# ~/.local/share/setlistarray. Three separate reasons, all of them learned the
# hard way:
#
#   1. Two of the five checks sample *content*: the first row's attachment
#      thumb and the first group header. Reading those out of whatever the
#      developer happens to have in their own library means the baseline is a
#      measurement of one particular person's songs on one particular day.
#      Empty that library — or add a song that sorts above Landslide — and the
#      net goes red with nothing whatsoever wrong with the app. That is exactly
#      how it went red before this comment existed, and the failure reads
#      convincingly like a paint regression: an empty library draws bare paper
#      where the thumb should be, whose grey mean (~0.954) sits right next to
#      the ~0.970 this file records for "laid out but never painted".
#   2. A gate that can fail for a reason outside the repository is not a gate.
#      Every input to a red run should be something `git diff` can show you.
#   3. rhypedb takes a directory lock. Pointing this at the real library means
#      the check cannot run while you have the app open to look at the very
#      thing you are checking.
#
# Card A4 raised a fourth thing about this same `XDG_DATA_HOME` override, worth
# recording here because it looked like a problem and turned out not to be
# one. `XDG_DATA_HOME` is also where fontconfig looks for user-installed
# fonts, and `scripts/install-fonts.sh` puts Newsreader and Karla there — so
# every run of this script was, in principle, also blinding fontconfig to the
# app's own typefaces, on top of blinding it to the developer's library.
# Before card K17 (`f8db611`) that would have mattered: the app asked for
# those faces by name and depended on the platform's font list to supply them,
# so every capture this harness ever took was rendered in whatever fontconfig
# fell back to — DejaVu Serif and DejaVu Sans — not the app's own type, and
# nothing here could have told you. As of K17 the desktop binary carries the
# font files (`crate::FONTS` in src/lib.rs, passed straight into
# `run_with_fonts`), and this was checked rather than assumed: a capture taken
# with `XDG_DATA_HOME` pointed at a directory fontconfig can search (so it
# *can* find the real Newsreader/Karla files) is byte-identical to one with it
# pointed here, at a directory with nothing in it — 0 of 585,750 pixels differ
# — which means fontconfig finding or not finding those names makes no
# difference to what gets painted; the bundled faces answer regardless. And
# removing the serif/sans-serif entries from `FONTS` and rebuilding — forcing
# the same DejaVu fallback fontconfig used to hand back silently — moves
# 31,468 of those same 585,750 pixels, which is what proves the bundled faces
# are what the un-modified build paints rather than DejaVu coincidentally
# matching them. So this override needs no seeded font directory: it already
# gets the real typefaces, for a reason that has nothing to do with
# fontconfig. See `ink_extent` below for the check that would now catch it if
# that ever stopped being true. One rough edge found along the way and not
# fixed here, because it is a different failure than the one this card is
# about: `fontique`'s fontconfig backend (`SystemSource::new`, in the
# `parley` dependency tree) still runs unconditionally at startup regardless
# of `FONTS`, to build its own system-wide generic-family map, and it
# `.unwrap()`s a `font_sort` call that can return `NoMatch` — confirmed by
# pointing `FONTCONFIG_FILE` at a config with no usable font directories at
# all, which panics the app before a window ever opens. That is a real
# fragility, but it takes a fontconfig with literally nothing in it to trigger
# — every real machine, including one with Newsreader and Karla never
# installed, has system fonts fontconfig can find — so it is out of scope
# here and left for whoever next has reason to touch that code path.
#
# `--seed` only ever fills a library that has nothing in it, so a fresh
# directory each run gives the same fourteen songs in the same order every
# time, and there is no state carried between runs to drift.
# ---------------------------------------------------------------------------

LIBRARY_DIR="$(mktemp -d "${TMPDIR:-/tmp}/setlistarray-screenshot-XXXXXX")"

APP_PID=""
kill_app() {
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill "$APP_PID" 2>/dev/null || true
    for _ in 1 2 3 4 5; do
      kill -0 "$APP_PID" 2>/dev/null || break
      sleep 0.5
    done
    kill -0 "$APP_PID" 2>/dev/null && kill -9 "$APP_PID" 2>/dev/null || true
  fi
}
# The throwaway library goes with it. Unlike the captures, there is nothing to
# learn from it after the fact — the seed content is in src/seed.rs.
cleanup() {
  kill_app
  # Trailing `|| true` because this is the last command of an EXIT trap under
  # `set -e`: a false test here must not become the script's exit status and
  # turn a failing run into a passing one.
  { [[ -n "${LIBRARY_DIR:-}" ]] && rm -rf "$LIBRARY_DIR"; } || true
}
trap cleanup EXIT

info "launching under X11 (window title \"$WINDOW_TITLE\", seeded library in $LIBRARY_DIR)…"
env -u WAYLAND_DISPLAY XDG_DATA_HOME="$LIBRARY_DIR" "$BIN" --seed >"$APP_LOG" 2>&1 &
APP_PID=$!

sleep "$LAUNCH_WAIT_SECONDS"

if ! kill -0 "$APP_PID" 2>/dev/null; then
  info "--- app log ($APP_LOG) ---"
  cat "$APP_LOG" >&2 || true
  die "the app exited before it could be captured (pid $APP_PID) — see the log above"
fi

# Matching by title alone is not safe: this machine runs several worktrees
# of this same repo side by side (see the top-level task notes), and two of
# them can easily have a window titled "SetListArray" open at once. Instead,
# find the specific window whose EWMH _NET_WM_PID matches the PID this
# script just launched, so a sibling agent's window is never captured by
# mistake — silently sampling someone else's screen would be a worse failure
# than any of this script's other edge cases.
#
# The id is taken as the first field of the `xwininfo -root -tree` line and
# not by grepping the line for something that looks like a hex literal. Those
# lines carry the window geometry too, and a geometry of `550x1065` contains
# the perfectly good hex literal `0x1065` — `grep -oE '0x[0-9a-fA-F]+'` finds
# it and hands back a second, entirely fictional window id. Today the
# `_NET_WM_PID` test below throws it away and nothing goes wrong, which is the
# only reason this was never noticed; it is one unlucky window height
# (`…x1234`, say, next to a real window whose id happens to match) away from
# capturing something else. xwininfo puts the id first on every line, indented
# by depth, so the first field is the id and nothing else ever is.
find_own_window_id() {
  local id wmpid
  while IFS= read -r id; do
    [[ "$id" =~ ^0x[0-9a-fA-F]+$ ]] || continue
    wmpid="$(xprop -id "$id" _NET_WM_PID 2>/dev/null | grep -oE '[0-9]+$')" || true
    if [[ "$wmpid" == "$APP_PID" ]]; then
      printf '%s\n' "$id"
      return 0
    fi
  done < <(xwininfo -root -tree 2>/dev/null | grep -F "\"$WINDOW_TITLE\"" | awk '{print $1}')
  return 1
}

WINDOW_ID=""
for attempt in $(seq 1 "$CAPTURE_ATTEMPTS"); do
  if WINDOW_ID="$(find_own_window_id)"; then
    break
  fi
  warn "attempt $attempt/$CAPTURE_ATTEMPTS: no window titled \"$WINDOW_TITLE\" owned by pid $APP_PID yet, retrying…"
  WINDOW_ID=""
  sleep "$CAPTURE_RETRY_DELAY"
done

if [[ -z "$WINDOW_ID" ]]; then
  info "--- app log ($APP_LOG) ---"
  cat "$APP_LOG" >&2 || true
  die "window \"$WINDOW_TITLE\" owned by pid $APP_PID never appeared after $CAPTURE_ATTEMPTS attempts. Confirm DISPLAY is set (currently \"${DISPLAY:-<unset>}\") and this isn't running headless."
fi

# The window is confirmed to exist and to belong to us; capture by numeric
# id (not title) so there is no window-lookup race left at all. Still
# wrapped in a timeout defensively — `import` on a window that vanishes
# mid-capture (e.g. this app crashing right after mapping) doesn't always
# fail promptly.
grab() {
  timeout "${CAPTURE_TIMEOUT_SECONDS}s" import -window "$WINDOW_ID" "$1" 2>>"$APP_LOG" || true
}

# Keep grabbing until two consecutive grabs are pixel-identical, then sample
# that one.
#
# A window being mapped is not the same thing as a window that has stopped
# moving. The window manager places and animates the new window, and `import`
# on an unredirected X11 window reads the screen, so a grab taken during that
# animation comes back the right *size* — the geometry is settled long before
# the pixels are — with the app's own frame drawn inset by a dozen pixels and
# clipped at the far edge. Every check then samples coordinates that are off
# by that inset, and the run fails with an assortment of red that looks for
# all the world like a paint bug. That is a spurious red on a commit gate,
# which is the one failure mode a gate cannot afford: it teaches whoever hits
# it to re-run until green, and after that the gate is decoration.
#
# The library screen is completely static once it has settled — no animation,
# no caret, no clock — so "two grabs in a row that agree" is a cheap and
# content-agnostic proof that nothing is still moving. Refusing to sample an
# unsettled frame is deliberate: a run that says "the window never stopped
# changing" is worth far more than one that guesses.
STABLE_ATTEMPTS=5
STABLE_SETTLE_DELAY=1
PREV_GRAB="$CAPTURE_DIR/.tmp-previous-grab.png"

grab "$PREV_GRAB"
CAPTURE_SETTLED=""
for attempt in $(seq 1 "$STABLE_ATTEMPTS"); do
  sleep "$STABLE_SETTLE_DELAY"
  grab "$SHOT_PATH"
  if [[ -s "$PREV_GRAB" && -s "$SHOT_PATH" ]]; then
    # `compare` exits non-zero whenever the images differ at all, and prints
    # a parse error rather than a count if they are not even the same size,
    # so neither its status nor a non-numeric result means anything here
    # beyond "not settled yet".
    differing="$(compare -metric AE "$PREV_GRAB" "$SHOT_PATH" null: 2>&1 | awk '{print $1}' || true)"
    if [[ "$differing" == "0" ]]; then
      CAPTURE_SETTLED="yes"
      break
    fi
    warn "attempt $attempt/$STABLE_ATTEMPTS: the window is still changing (${differing} px differ), waiting for it to settle…"
  fi
  cp -f "$SHOT_PATH" "$PREV_GRAB" 2>/dev/null || true
done
rm -f "$PREV_GRAB"

if [[ ! -s "$SHOT_PATH" ]]; then
  info "--- app log ($APP_LOG) ---"
  cat "$APP_LOG" >&2 || true
  die "found window $WINDOW_ID but \`import\` produced no image — see the log above"
fi

if [[ -z "$CAPTURE_SETTLED" ]]; then
  cp "$SHOT_PATH" "$LATEST_PATH"
  die "window $WINDOW_ID never stopped changing across $STABLE_ATTEMPTS grabs ${STABLE_SETTLE_DELAY}s apart — the last one is at $SHOT_PATH. Sampling a moving window would fail the checks for the wrong reason, so this run asserts nothing."
fi

cp "$SHOT_PATH" "$LATEST_PATH"

# The app is captured; no reason to keep it running while we sample pixels.
# Only the process goes — the EXIT trap stays armed so the throwaway library
# is still removed however this script ends, including down the `die` paths
# that every failing check takes.
kill_app
APP_PID=""

CAPTURE_DIMS="$(identify -format '%w %h' "$SHOT_PATH")"
ACTUAL_W="${CAPTURE_DIMS%% *}"
ACTUAL_H="${CAPTURE_DIMS##* }"
info "captured ${ACTUAL_W}x${ACTUAL_H} -> $SHOT_PATH"

SCALE="$(derive_scale "$ACTUAL_H" "$LOGICAL_H")"

# A scale factor outside this range means the capture is not the app's window
# — a stray grab of the whole root window, say, or a `import` that came back
# with a placeholder. Every region would then resolve somewhere absurd and the
# checks would report a paint regression that isn't there.
if ! awk -v s="$SCALE" 'BEGIN { exit !(s >= 0.5 && s <= 8) }'; then
  die "capture is ${ACTUAL_W}x${ACTUAL_H}, i.e. ${SCALE}x the ${LOGICAL_W}x${LOGICAL_H} window the app asks for — that is not a plausible display scale factor, so this run refuses to guess where anything is."
fi
info "scale factor ${SCALE} (captured height ${ACTUAL_H} / logical height ${LOGICAL_H}; the width the window manager granted is deliberately not used)"

# ---------------------------------------------------------------------------
# Sampling helpers
# ---------------------------------------------------------------------------

# Mean of ImageMagick's Gray colorspace conversion over a crop, 0..1. This is
# exactly `convert shot.png -crop WxH+X+Y +repage -colorspace Gray -format
# "%[fx:mean]" info:`, the command the paint regression was originally
# bisected with — kept byte-for-byte so the recorded baseline stays valid.
gray_mean() {
  local w="$1" h="$2" x="$3" y="$4"
  convert "$SHOT_PATH" -crop "${w}x${h}+${x}+${y}" +repage -colorspace Gray -format '%[fx:mean]' info:
}

# Pixels within `fuzz_percent` of `color` inside a WxH+X+Y crop, as
# "matched total" — computed by diffing the crop against a synthetic
# solid-color reference the same size and reading ImageMagick's Absolute
# Error pixel count back out.
matched_pixels() {
  local w="$1" h="$2" x="$3" y="$4" color="$5" fuzz="$6"
  local crop="$CAPTURE_DIR/.tmp-crop.png" ref="$CAPTURE_DIR/.tmp-ref.png"
  convert "$SHOT_PATH" -crop "${w}x${h}+${x}+${y}" +repage "$crop"
  convert -size "${w}x${h}" "xc:$color" "$ref"
  local ae total
  ae="$(compare -metric AE -fuzz "${fuzz}%" "$ref" "$crop" null: 2>&1 | awk '{print $1}' || true)"
  rm -f "$crop" "$ref"
  # A non-numeric result means `compare`'s output didn't parse — that's an
  # environment problem (ImageMagick version skew), not "0 pixels differ".
  # This runs inside a process substitution, so `exit`/`die` here would only
  # kill that subshell, not the script — print a sentinel instead and let
  # the caller (in the main shell) fail loudly.
  if [[ ! "$ae" =~ ^[0-9]+$ ]]; then
    echo "ERROR: couldn't read a pixel count from \`compare\` (got: \"$ae\")"
    return 0
  fi
  total=$((w * h))
  echo $((total - ae)) "$total"
}

# Ink shape over a crop, as three scale-invariant fractions: how much of the
# region's width and height the glyph ink actually spans, and what fraction of
# the region's pixels are ink at all. This exists because of card A4 (see the
# long note above the launch, "The typefaces come from FONTS") — every other
# check in this file samples colour, and colour cannot tell Newsreader from
# DejaVu Serif: the accent swatch, the paper and the fill are the same six hex
# codes whichever face drew the letters over them. Six colour checks passed on
# every capture this harness ever took, including ones that turned out to be
# rendered in the system's DejaVu fallback rather than the app's own type —
# nothing here would have gone red for that. Shape is the axis colour cannot
# see, so this measures shape.
#
# The pipeline: crop, convert to greyscale, `-threshold` to pure black/white
# at `threshold_percent`, then read the binary image two ways:
#
#   * `-trim` finds the bounding box of the non-background pixels — the
#     tightest rectangle containing every ink pixel — and `identify` reads its
#     width and height back out. Dividing by the crop's own W and H turns that
#     into a fraction of the crop the ink spans, which is what makes the
#     number comparable across display scale factors: a 2x capture has a
#     bounding box twice the pixel size but the same fraction, the same way
#     `gray_mean`'s 0..1 mean is comparable regardless of how many pixels went
#     into it. An empty crop makes `-trim` print "geometry does not contain
#     image" and hand back a 1x1 placeholder rather than failing outright, so
#     an unpainted region reads back as a fraction near zero and fails the
#     check for the right reason instead of aborting the script.
#   * `-negate` then the same `%[fx:mean]` trick `gray_mean` uses reads the
#     fraction of the *whole* crop that is ink, independent of its shape — two
#     faces can share a bounding box but differ in stroke weight, and this
#     catches that axis too.
#
# Ink and paper are `#1C1917` on `#FBF7F0` in light mode — nowhere near each
# other in lightness — so a 50% threshold sits comfortably in the gap without
# needing to track the theme's actual hex values.
#
# Measured on the library screen title ("Songs", `T_SCREEN_TITLE`, Newsreader
# at 34px) at this file's recorded region: the bundled face gives a 0.495
# width fraction, 0.774 height fraction, 0.086 ink fraction. Forcing the
# DejaVu Serif fallback — deleting the `AppFont::serif` and `AppFont::new`
# (italic) entries from `crate::FONTS` in src/lib.rs and rebuilding, so
# `font-family: Newsreader, Georgia, serif` falls through both named faces to
# the bare generic — moves every one of those numbers: 0.56 / 0.75 / 0.113.
# DejaVu Serif's "Songs" is both wider and heavier-stroked than Newsreader's
# at the same point size, and the width and ink fractions below are tight
# enough to catch that on their own; the height fraction is left loose
# because cap-height happens to be close between these two particular faces,
# but a face that failed to render at all — 1x1 bounding box — would still
# blow through it.
ink_extent() {
  local w="$1" h="$2" x="$3" y="$4" threshold="$5"
  local bin="$CAPTURE_DIR/.tmp-ink-bin.png" trimmed="$CAPTURE_DIR/.tmp-ink-trim.png"
  convert "$SHOT_PATH" -crop "${w}x${h}+${x}+${y}" +repage -colorspace Gray -threshold "${threshold}%" "$bin"
  local ink_fraction
  ink_fraction="$(convert "$bin" -negate -format '%[fx:mean]' info:)"
  # `-trim` warns rather than fails on an all-background image (see the note
  # above), but it is wrapped in `|| true` anyway: a future ImageMagick that
  # makes that a hard error must not take the whole script down with it, only
  # this one check, which will then read a 0x0 trim and fail loudly on its own.
  convert "$bin" -trim +repage "$trimmed" 2>/dev/null || true
  local dims tw th
  dims="$(identify -format '%w %h' "$trimmed" 2>/dev/null || echo "0 0")"
  tw="${dims%% *}"; th="${dims##* }"
  rm -f "$bin" "$trimmed"
  # The trailing \n matters: `read` (below, at the call site) reports failure
  # on a line with no terminating newline even though it fills the variables
  # correctly, and that failure is enough to abort the whole script under
  # `set -e` — silently, since nothing here catches it. `matched_pixels`
  # avoids this the same way, with `echo` rather than `printf`.
  awk -v tw="$tw" -v th="$th" -v w="$w" -v h="$h" -v ink="$ink_fraction" \
    'BEGIN { printf "%.4f %.4f %.4f\n", tw / w, th / h, ink }'
}

within_tolerance() {
  awk -v a="$1" -v b="$2" -v tol="$3" 'BEGIN { d = a - b; if (d < 0) d = -d; exit !(d <= tol) }'
}

at_least_fraction() {
  awk -v matched="$1" -v total="$2" -v frac="$3" 'BEGIN { exit !(matched / total >= frac) }'
}

# ---------------------------------------------------------------------------
# Run checks
# ---------------------------------------------------------------------------

CHECK_COUNT="$(jq '.checks | length' "$BASELINE_FILE")"
FAILURES=0
UPDATED_JSON="$BASELINE_FILE"

for i in $(seq 0 $((CHECK_COUNT - 1))); do
  check="$(jq -c ".checks[$i]" "$BASELINE_FILE")"
  name="$(jq -r '.name' <<<"$check")"
  desc="$(jq -r '.description' <<<"$check")"
  type="$(jq -r '.type' <<<"$check")"
  resolve_region "$check" "$name" || die "$REGION_ERROR"
  w="$REGION_W"; h="$REGION_H"; x="$REGION_X"; y="$REGION_Y"

  case "$type" in
    gray_mean)
      expected="$(jq -r '.expected' <<<"$check")"
      tolerance="$(jq -r '.tolerance' <<<"$check")"
      measured="$(gray_mean "$w" "$h" "$x" "$y")"
      if [[ "$MODE" == update ]]; then
        UPDATED_JSON="$(jq --argjson i "$i" --argjson v "$measured" '.checks[$i].expected = ($v | tonumber)' <<<"$(cat "$UPDATED_JSON")")"
        echo "$UPDATED_JSON" > "$CAPTURE_DIR/.tmp-baseline.json" && UPDATED_JSON="$CAPTURE_DIR/.tmp-baseline.json"
        printf '%supdate%s %-24s expected %.3f -> %.3f\n' "$YELLOW" "$RESET" "$name" "$expected" "$measured" >&2
        continue
      fi
      if within_tolerance "$measured" "$expected" "$tolerance"; then
        ok "$name: $measured (expected $expected ± $tolerance)"
      else
        FAILURES=$((FAILURES + 1))
        printf '%s✗ %s%s — %s\n' "$RED" "$name" "$RESET" "$desc" >&2
        printf '    expected %.3f ± %.3f, measured %.3f\n' "$expected" "$tolerance" "$measured" >&2
        printf '    region: %sx%s+%s+%s in %s\n' "$w" "$h" "$x" "$y" "$SHOT_PATH" >&2
      fi
      ;;
    solid_color|accent_presence|accent_absence)
      color="$(jq -r '.color' <<<"$check")"
      fuzz="$(jq -r '.fuzz_percent' <<<"$check")"
      read -r matched total < <(matched_pixels "$w" "$h" "$x" "$y" "$color" "$fuzz")
      [[ "$matched" == "ERROR:" ]] && die "check \"$name\": $total"
      case "$type" in
        solid_color)
          threshold_kind="fraction"
          min_fraction="$(jq -r '.min_match_fraction' <<<"$check")"
          measured_frac="$(awk -v m="$matched" -v t="$total" 'BEGIN{printf "%.3f", m/t}')"
          ;;
        accent_presence)
          threshold_kind="count"
          min_matched="$(jq -r '.min_matched' <<<"$check")"
          ;;
        accent_absence)
          threshold_kind="absence"
          max_matched="$(jq -r '.max_matched' <<<"$check")"
          ;;
      esac

      if [[ "$MODE" == update ]]; then
        if [[ "$threshold_kind" == absence ]]; then
          # Never re-recorded. A negative control exists to prove the sampling
          # path can still say "no"; a control whose ceiling is measured from
          # the same run it is policing proves nothing at all — if `compare`
          # started matching every pixel, --update would dutifully raise the
          # ceiling to fit and the suite would go green forever.
          printf '%skeep%s   %-24s max_matched %s (negative control — never re-recorded; measured %s/%s)\n' \
            "$YELLOW" "$RESET" "$name" "$max_matched" "$matched" "$total" >&2
          continue
        fi
        if [[ "$threshold_kind" == fraction ]]; then
          # A small safety margin below a fresh full/near-full match, so
          # ordinary anti-aliasing jitter doesn't start failing the day
          # after a re-record.
          new_min="$(awk -v f="$measured_frac" 'BEGIN { v = f - 0.03; if (v < 0) v = 0; printf "%.2f", v }')"
          UPDATED_JSON="$(jq --argjson i "$i" --argjson v "$new_min" '.checks[$i].min_match_fraction = $v' <<<"$(cat "$UPDATED_JSON")")"
          echo "$UPDATED_JSON" > "$CAPTURE_DIR/.tmp-baseline.json" && UPDATED_JSON="$CAPTURE_DIR/.tmp-baseline.json"
          printf '%supdate%s %-24s min_match_fraction %s -> %s (measured %s)\n' "$YELLOW" "$RESET" "$name" "$min_fraction" "$new_min" "$measured_frac" >&2
        else
          new_min="$(awk -v m="$matched" 'BEGIN { v = int(m * 0.85); print v }')"
          UPDATED_JSON="$(jq --argjson i "$i" --argjson v "$new_min" '.checks[$i].min_matched = $v' <<<"$(cat "$UPDATED_JSON")")"
          echo "$UPDATED_JSON" > "$CAPTURE_DIR/.tmp-baseline.json" && UPDATED_JSON="$CAPTURE_DIR/.tmp-baseline.json"
          printf '%supdate%s %-24s min_matched %s -> %s (measured %s/%s)\n' "$YELLOW" "$RESET" "$name" "$min_matched" "$new_min" "$matched" "$total" >&2
        fi
        continue
      fi

      case "$threshold_kind" in
        fraction)
          if at_least_fraction "$matched" "$total" "$min_fraction"; then
            ok "$name: $matched/$total near $color (>= $min_fraction)"
          else
            FAILURES=$((FAILURES + 1))
            printf '%s✗ %s%s — %s\n' "$RED" "$name" "$RESET" "$desc" >&2
            printf '    expected >= %s of %d px near %s, measured %s/%s\n' "$min_fraction" "$total" "$color" "$matched" "$total" >&2
            printf '    region: %sx%s+%s+%s in %s\n' "$w" "$h" "$x" "$y" "$SHOT_PATH" >&2
          fi
          ;;
        count)
          if [[ "$matched" -ge "$min_matched" ]]; then
            ok "$name: $matched/$total px near $color (>= $min_matched)"
          else
            FAILURES=$((FAILURES + 1))
            printf '%s✗ %s%s — %s\n' "$RED" "$name" "$RESET" "$desc" >&2
            printf '    expected >= %s px near %s, measured %s/%s\n' "$min_matched" "$color" "$matched" "$total" >&2
            printf '    region: %sx%s+%s+%s in %s\n' "$w" "$h" "$x" "$y" "$SHOT_PATH" >&2
          fi
          ;;
        absence)
          if [[ "$matched" -le "$max_matched" ]]; then
            ok "$name: $matched/$total px near $color (<= $max_matched)"
          else
            FAILURES=$((FAILURES + 1))
            printf '%s✗ %s%s — %s\n' "$RED" "$name" "$RESET" "$desc" >&2
            printf '    expected <= %s px near %s, measured %s/%s\n' "$max_matched" "$color" "$matched" "$total" >&2
            printf '    region: %sx%s+%s+%s in %s\n' "$w" "$h" "$x" "$y" "$SHOT_PATH" >&2
            printf '    this region is empty paper at every window size, so the app cannot have caused this. Suspect the sampling path: ImageMagick, the -fuzz argument, or resolve_region.\n' >&2
          fi
          ;;
      esac
      ;;
    ink_extent)
      threshold="$(jq -r '.threshold_percent' <<<"$check")"
      exp_w="$(jq -r '.ink_width_fraction' <<<"$check")"
      tol_w="$(jq -r '.ink_width_tolerance' <<<"$check")"
      exp_h="$(jq -r '.ink_height_fraction' <<<"$check")"
      tol_h="$(jq -r '.ink_height_tolerance' <<<"$check")"
      exp_i="$(jq -r '.ink_pixel_fraction' <<<"$check")"
      tol_i="$(jq -r '.ink_pixel_tolerance' <<<"$check")"
      read -r meas_w meas_h meas_i < <(ink_extent "$w" "$h" "$x" "$y" "$threshold")

      if [[ "$MODE" == update ]]; then
        UPDATED_JSON="$(jq --argjson i "$i" --argjson vw "$meas_w" --argjson vh "$meas_h" --argjson vi "$meas_i" \
          '.checks[$i].ink_width_fraction = ($vw | tonumber)
           | .checks[$i].ink_height_fraction = ($vh | tonumber)
           | .checks[$i].ink_pixel_fraction = ($vi | tonumber)' \
          <<<"$(cat "$UPDATED_JSON")")"
        echo "$UPDATED_JSON" > "$CAPTURE_DIR/.tmp-baseline.json" && UPDATED_JSON="$CAPTURE_DIR/.tmp-baseline.json"
        printf '%supdate%s %-24s width %.3f -> %.3f, height %.3f -> %.3f, ink %.3f -> %.3f\n' \
          "$YELLOW" "$RESET" "$name" "$exp_w" "$meas_w" "$exp_h" "$meas_h" "$exp_i" "$meas_i" >&2
        continue
      fi

      ink_w_ok=1; within_tolerance "$meas_w" "$exp_w" "$tol_w" || ink_w_ok=0
      ink_h_ok=1; within_tolerance "$meas_h" "$exp_h" "$tol_h" || ink_h_ok=0
      ink_i_ok=1; within_tolerance "$meas_i" "$exp_i" "$tol_i" || ink_i_ok=0

      if [[ "$ink_w_ok" == 1 && "$ink_h_ok" == 1 && "$ink_i_ok" == 1 ]]; then
        ok "$name: ink width $meas_w height $meas_h pixels $meas_i (expected $exp_w/$exp_h/$exp_i ± $tol_w/$tol_h/$tol_i)"
      else
        FAILURES=$((FAILURES + 1))
        printf '%s✗ %s%s — %s\n' "$RED" "$name" "$RESET" "$desc" >&2
        [[ "$ink_w_ok" == 1 ]] || printf '    ink width fraction: expected %s ± %s, measured %s\n' "$exp_w" "$tol_w" "$meas_w" >&2
        [[ "$ink_h_ok" == 1 ]] || printf '    ink height fraction: expected %s ± %s, measured %s\n' "$exp_h" "$tol_h" "$meas_h" >&2
        [[ "$ink_i_ok" == 1 ]] || printf '    ink pixel fraction: expected %s ± %s, measured %s\n' "$exp_i" "$tol_i" "$meas_i" >&2
        printf '    region: %sx%s+%s+%s in %s\n' "$w" "$h" "$x" "$y" "$SHOT_PATH" >&2
      fi
      ;;
    *)
      die "unknown check type in baseline: $type"
      ;;
  esac
done

if [[ "$MODE" == update ]]; then
  # Only thresholds are re-recorded. The geometry is not, and there is no
  # longer a captured size stored anywhere to re-record either: regions are
  # anchored to the edges of whatever the window manager grants and written in
  # the app's own CSS pixels, so the one thing --update used to write back —
  # "the capture was this many pixels wide that day" — was precisely the
  # transient machine state that had no business in the repository. Writing it
  # was how a window-manager mood swing turned into a baseline diff.
  jq . "$UPDATED_JSON" > "$CAPTURE_DIR/.tmp-baseline2.json"
  mv "$CAPTURE_DIR/.tmp-baseline2.json" "$BASELINE_FILE"
  rm -f "$CAPTURE_DIR/.tmp-baseline.json"
  info ""
  info "${BOLD}baseline thresholds rewritten from this run's measurements${RESET} -> $BASELINE_FILE"
  info "diff it before committing — every changed number above should be explained by an intentional change."
  info "region geometry is never re-recorded; if a region has drifted off its target, move it by hand and say why."
  exit 0
fi

echo >&2
if [[ "$FAILURES" -eq 0 ]]; then
  ok "${BOLD}all $CHECK_COUNT checks passed${RESET}"
  exit 0
else
  die "${BOLD}$FAILURES/$CHECK_COUNT checks failed${RESET} — screenshot kept at $SHOT_PATH ($LATEST_PATH)"
fi
