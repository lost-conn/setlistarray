#!/usr/bin/env bash
# Build, run and screenshot SetListArray, then sample known-good regions of
# the library screen and report pass/fail per check.
#
# This is the regression net card A3 asks for: the paint regression (see
# README, "The paint regression") produced a DOM and layout that were
# byte-for-byte identical to a good render — only pixels caught it. So pixels
# are what this script checks, sampled the same way the fix was verified by
# hand: `import` grabs the live X11 window, `convert`/`compare` sample it.
#
# Usage:
#   scripts/screenshot.sh              # build, run, capture, check against
#                                       # scripts/screenshot-baseline.json
#   scripts/screenshot.sh --update     # same, but rewrite the baseline from
#                                       # this run's measurements instead of
#                                       # checking against it
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
    -h|--help)
      sed -n '2,20p' "${BASH_SOURCE[0]}"
      exit 0
      ;;
    *)
      echo "unknown option: $arg" >&2
      echo "usage: $(basename "${BASH_SOURCE[0]}") [--update]" >&2
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

for tool in cargo import convert compare identify jq xwininfo xprop; do
  command -v "$tool" >/dev/null 2>&1 || die "missing dependency: $tool"
done

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
# ---------------------------------------------------------------------------

APP_PID=""
cleanup() {
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill "$APP_PID" 2>/dev/null || true
    for _ in 1 2 3 4 5; do
      kill -0 "$APP_PID" 2>/dev/null || break
      sleep 0.5
    done
    kill -0 "$APP_PID" 2>/dev/null && kill -9 "$APP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

info "launching under X11 (window title \"$WINDOW_TITLE\")…"
env -u WAYLAND_DISPLAY "$BIN" >"$APP_LOG" 2>&1 &
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
find_own_window_id() {
  local id wmpid
  while IFS= read -r id; do
    wmpid="$(xprop -id "$id" _NET_WM_PID 2>/dev/null | grep -oE '[0-9]+$')" || true
    if [[ "$wmpid" == "$APP_PID" ]]; then
      printf '%s\n' "$id"
      return 0
    fi
  done < <(xwininfo -root -tree 2>/dev/null | grep -F "\"$WINDOW_TITLE\"" | grep -oE '0x[0-9a-fA-F]+')
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
timeout "${CAPTURE_TIMEOUT_SECONDS}s" import -window "$WINDOW_ID" "$SHOT_PATH" 2>>"$APP_LOG" || true

if [[ ! -s "$SHOT_PATH" ]]; then
  info "--- app log ($APP_LOG) ---"
  cat "$APP_LOG" >&2 || true
  die "found window $WINDOW_ID but \`import\` produced no image — see the log above"
fi

cp "$SHOT_PATH" "$LATEST_PATH"

# The app is captured; no reason to keep it running while we sample pixels.
cleanup
trap - EXIT
APP_PID=""

CAPTURE_DIMS="$(identify -format '%w %h' "$SHOT_PATH")"
ACTUAL_W="${CAPTURE_DIMS%% *}"
ACTUAL_H="${CAPTURE_DIMS##* }"
info "captured ${ACTUAL_W}x${ACTUAL_H} -> $SHOT_PATH"

REF_W="$(jq -r '.reference_capture.width' "$BASELINE_FILE")"
REF_H="$(jq -r '.reference_capture.height' "$BASELINE_FILE")"

# ---------------------------------------------------------------------------
# Sampling helpers
# ---------------------------------------------------------------------------

# Scale a reference-capture coordinate to this run's actual capture size, so
# the baseline geometry doesn't have to be re-measured every time this runs
# on a display with a different scale factor. Rounds half away from zero.
scale() {
  local value="$1" ref="$2" actual="$3"
  awk -v v="$value" -v ref="$ref" -v act="$actual" 'BEGIN { printf "%d", (v * act / ref) + 0.5 }'
}

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
  w="$(scale "$(jq -r '.crop.w' <<<"$check")" "$REF_W" "$ACTUAL_W")"
  h="$(scale "$(jq -r '.crop.h' <<<"$check")" "$REF_H" "$ACTUAL_H")"
  x="$(scale "$(jq -r '.crop.x' <<<"$check")" "$REF_W" "$ACTUAL_W")"
  y="$(scale "$(jq -r '.crop.y' <<<"$check")" "$REF_H" "$ACTUAL_H")"

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
    solid_color|accent_presence)
      color="$(jq -r '.color' <<<"$check")"
      fuzz="$(jq -r '.fuzz_percent' <<<"$check")"
      read -r matched total < <(matched_pixels "$w" "$h" "$x" "$y" "$color" "$fuzz")
      [[ "$matched" == "ERROR:" ]] && die "check \"$name\": $total"
      if [[ "$type" == solid_color ]]; then
        threshold_kind="fraction"
        min_fraction="$(jq -r '.min_match_fraction' <<<"$check")"
        measured_frac="$(awk -v m="$matched" -v t="$total" 'BEGIN{printf "%.3f", m/t}')"
      else
        threshold_kind="count"
        min_matched="$(jq -r '.min_matched' <<<"$check")"
      fi

      if [[ "$MODE" == update ]]; then
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

      if [[ "$threshold_kind" == fraction ]]; then
        if at_least_fraction "$matched" "$total" "$min_fraction"; then
          ok "$name: $matched/$total near $color (>= $min_fraction)"
        else
          FAILURES=$((FAILURES + 1))
          printf '%s✗ %s%s — %s\n' "$RED" "$name" "$RESET" "$desc" >&2
          printf '    expected >= %s of %d px near %s, measured %s/%s\n' "$min_fraction" "$total" "$color" "$matched" "$total" >&2
          printf '    region: %sx%s+%s+%s in %s\n' "$w" "$h" "$x" "$y" "$SHOT_PATH" >&2
        fi
      else
        if [[ "$matched" -ge "$min_matched" ]]; then
          ok "$name: $matched/$total px near $color (>= $min_matched)"
        else
          FAILURES=$((FAILURES + 1))
          printf '%s✗ %s%s — %s\n' "$RED" "$name" "$RESET" "$desc" >&2
          printf '    expected >= %s px near %s, measured %s/%s\n' "$min_matched" "$color" "$matched" "$total" >&2
          printf '    region: %sx%s+%s+%s in %s\n' "$w" "$h" "$x" "$y" "$SHOT_PATH" >&2
        fi
      fi
      ;;
    *)
      die "unknown check type in baseline: $type"
      ;;
  esac
done

if [[ "$MODE" == update ]]; then
  jq --arg w "$ACTUAL_W" --arg h "$ACTUAL_H" '.reference_capture.width = ($w | tonumber) | .reference_capture.height = ($h | tonumber)' "$UPDATED_JSON" > "$CAPTURE_DIR/.tmp-baseline2.json"
  mv "$CAPTURE_DIR/.tmp-baseline2.json" "$BASELINE_FILE"
  rm -f "$CAPTURE_DIR/.tmp-baseline.json"
  info ""
  info "${BOLD}baseline rewritten from this run's measurements${RESET} -> $BASELINE_FILE"
  info "diff it before committing — every changed number above should be explained by an intentional change."
  exit 0
fi

echo >&2
if [[ "$FAILURES" -eq 0 ]]; then
  ok "${BOLD}all $CHECK_COUNT checks passed${RESET}"
  exit 0
else
  die "${BOLD}$FAILURES/$CHECK_COUNT checks failed${RESET} — screenshot kept at $SHOT_PATH ($LATEST_PATH)"
fi
