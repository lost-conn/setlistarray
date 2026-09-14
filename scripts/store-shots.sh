#!/usr/bin/env bash
# Photograph SetListArray for its Play listing: boot a headless emulator, put
# the `--shots` build on it, and grab one PNG per screen as the app walks
# itself through them.
#
# Card S1. The pictures this produces are raw frames at the panel's own
# 1080x1920 — no caption, no device frame, no crop. Card S2's
# `scripts/store-frame.py` reads this directory and does the compositing; what
# this script owes it is a directory of correctly sized, correctly timed frames,
# a note of where the system bars were when they were taken, and a loud failure
# when it cannot produce one.
#
# Usage:
#   scripts/store-shots.sh                    # build, boot, install, capture
#   scripts/store-shots.sh --out DIR          # where the PNGs go (default .shots)
#   scripts/store-shots.sh --apk PATH         # use this APK instead of building
#   scripts/store-shots.sh --no-build         # use setlistarray.apk as it stands
#   scripts/store-shots.sh --software         # build the tiny-skia painter (see below)
#   scripts/store-shots.sh --avd NAME         # which AVD to boot (default sla-shots)
#   scripts/store-shots.sh --keep             # leave the emulator running afterwards
#
# ---------------------------------------------------------------------------
# The emulator is launched -no-window, and that is not a preference
# ---------------------------------------------------------------------------
#
# `CLAUDE.md`'s first rule is that nothing this repository runs may map a
# window on the developer's display, and it is a rule with a date on it: on
# 2026-08-28 an agent verifying the file picker launched the app on the real
# desktop and opened native file dialogs on it, repeatedly, while the machine's
# owner was working. An emulator window is that failure with a whole phone
# inside it — it takes focus, it takes a workspace, and it takes them for the
# two or three minutes a capture run lasts.
#
# So: `-no-window`, always, and `-no-audio` with it, because an emulator that
# is not being watched has even less business being heard. `-gpu host` is
# forbidden for the same reason wearing a different hat — it renders through
# the *host's* GL driver and, on this Wayland session, wants a host surface to
# do it on. `swiftshader_indirect` is a software rasteriser inside the emulator
# process and needs nothing from the session at all.
#
# Note that `scripts/with-display.sh` is not the answer here and is not used:
# it exists to move an *X11* client onto a private Xvfb, and a headless
# emulator is not an X11 client. `-no-window` is the emulator's own version of
# the same promise, made one layer down.
#
# ---------------------------------------------------------------------------
# Which painter these shots are taken with
# ---------------------------------------------------------------------------
#
# The same one users get: rinch's GPU shell, the default since card K41 and the
# default of `build-apk.sh`. That was not a given — the worry going in was that
# the emulator's SwiftShader could not bring up the Vulkan device the GPU shell
# asks for, in which case `--software` (the tiny-skia painter) was the
# documented fallback and these shots would have been taken with a painter no
# user runs. It brings it up:
#
#   rinch::shell::android_runtime: GPU: SwiftShader Device (Subzero) (Vulkan)
#
# so the fallback stays a flag rather than the default. If a future emulator
# image loses Vulkan, pass `--software` and *say so wherever the pictures go* —
# card K36 is on record that the two painters have differed in what they draw,
# not only in how fast they draw it, and a store page is the worst place to
# find that out.
#
# ---------------------------------------------------------------------------
# The status bar is put into a demo state, and the bars are measured
# ---------------------------------------------------------------------------
#
# Two things card S2 asked of this script, both about what is in the frame
# rather than about the app.
#
# **SysUI demo mode.** A raw capture carries whatever the emulator's status bar
# happened to be saying: the wall clock to the minute, a wifi glyph with the
# "no internet" exclamation on it because a swiftshader AVD's network is what it
# is, a battery at 51% with a charging bolt through it, and — on this image —
# the little bug-droid that means "a developer settings override is on". None of
# that is the app, all of it differs between one run and the next, and the two
# ways it shows up are both bad: seven screenshots on a store page with seven
# different clocks in them, or a reviewer's eye landing on a broken wifi icon
# instead of on a chord chart. `com.android.systemui.demo` is the platform's own
# answer, meant for exactly this, and it pins all of it: a settled 9:30, full
# signal, full battery, nothing else.
#
# It is entered before the tour and **exited afterwards, including on failure**,
# through the same `trap` that shuts the emulator down. A device left in demo
# mode looks fine and lies about everything in its status bar, and the person
# who finds that out is whoever next picks the device up for something
# unrelated — which on CI is nobody and on a laptop is somebody halfway through
# a different problem.
#
# **The insets, written down next to the PNGs.** The compositor crops the
# navigation bar off every frame (it has to: on a `full_screen` route like
# `chart-viewer` the nav bar is drawn *over* the chart's last lines rather than
# beside them) and keeps the status bar. Where to cut is a number that belongs
# to the device, so it is read off the device here — `dumpsys window displays`
# names both bars' frames — and saved as `insets.json` beside the frames.
#
# The alternative was two constants in the Python, and they would have been
# right on the day they were measured. `sla-shots` is 1080x1920 at 420dpi with
# three-button navigation, which makes the nav bar 48dp and so 126px; recreate
# the AVD from a system image that defaults to gesture navigation and the same
# bar is 24dp, and the compositor would crop 126px off a frame with 63px of bar
# in it and quietly shave a row of the app's own bottom chrome off all seven
# pictures. That is the class of bug this repository keeps turning into a test
# rather than a constant — see `the_apk_maps_its_native_library_instead_of_
# extracting_it` for the pattern — and the version of it available here is to
# measure rather than to assume.
#
# ---------------------------------------------------------------------------
# Why 1080x1920
# ---------------------------------------------------------------------------
#
# Because it is 16:9 exactly, and 16:9 is a ratio Google Play accepts without
# argument for a phone screenshot. The handset this project is otherwise
# developed against — the moto g stylus 5G, serial ZY22FD66GZ — has a 1080x2400
# panel, which is 2.22:1; Play rejects it outright, so the obvious "just plug
# the phone in" version of this script cannot work and an AVD shaped for the
# store is not a convenience but the requirement. Every capture is checked
# against those two numbers below before this script claims success, because a
# silently resized AVD would otherwise produce a directory of frames that are
# perfectly good pictures and are not usable for anything.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

PACKAGE="dev.lostconnection.setlistarray"
ACTIVITY="com.rinch.RinchActivity"

OUT_DIR="$ROOT/.shots"
AVD="sla-shots"
APK=""
BUILD=true
SOFTWARE=false
KEEP=false

# How long to wait for the next `SLA_SHOT_READY` before giving up on the run,
# and how long the whole tour is allowed to take. The app's own settle is six
# seconds and its dwell is four (see `src/shots.rs`), so ten is the expected
# gap; ninety is the same number with room for a cold emulator that is still
# finishing its own boot work behind the app.
SHOT_TIMEOUT=90
RUN_TIMEOUT=900

# What Play wants, and what every capture is measured against.
WANT_W=1080
WANT_H=1920

while [[ $# -gt 0 ]]; do
    case "$1" in
        --out) OUT_DIR="$2"; shift 2 ;;
        --avd) AVD="$2"; shift 2 ;;
        --apk) APK="$2"; BUILD=false; shift 2 ;;
        --no-build) BUILD=false; shift ;;
        --software) SOFTWARE=true; shift ;;
        --keep) KEEP=true; shift ;;
        -h|--help) sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 1 ;;
    esac
done

RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; BOLD=$'\033[1m'; RESET=$'\033[0m'
info() { printf '%s\n' "$*" >&2; }
ok()   { printf '%s✓%s %s\n' "$GREEN" "$RESET" "$*" >&2; }
warn() { printf '%s!%s %s\n' "$YELLOW" "$RESET" "$*" >&2; }

# Everything that gives up does it through here, and everything that gives up
# after the app has started prints the app's own logcat on the way out. That is
# the whole of the debugging story for a run nobody was watching: a shot that
# never arrived is either a screen that failed to build itself (`src/shots.rs`
# announces `SLA_SHOT_BROKEN` and stops, naming what it went looking for) or a
# process that is no longer there, and the difference is in those lines.
die() {
    printf '%s✗ %s%s\n' "$RED" "$*" "$RESET" >&2
    if [[ -n "${LOGCAT_FILE:-}" && -s "${LOGCAT_FILE:-}" ]]; then
        printf '%s--- the app said: ---%s\n' "$BOLD" "$RESET" >&2
        tail -n 60 "$LOGCAT_FILE" >&2
    fi
    # Only once there is an app to have said anything. Before that, a failure
    # is a missing file or a mis-shaped AVD and forty lines of `ActivityManager`
    # underneath the message that says so is forty lines of noise.
    if [[ -n "${ADB:-}" && "${APP_STARTED:-false}" == true ]]; then
        printf '%s--- and the platform said: ---%s\n' "$BOLD" "$RESET" >&2
        "$ADB" logcat -d -b crash 2>/dev/null | tail -n 40 >&2 || true
        "$ADB" logcat -d 2>/dev/null | grep -iE "$PACKAGE|AndroidRuntime|libc *:" | tail -n 30 >&2 || true
    fi
    exit 1
}

# ---------------------------------------------------------------------------
# The tools
# ---------------------------------------------------------------------------

# `adb` is deliberately not assumed to be on PATH: it is not, on the machine
# this was written on, and there are two SDKs installed there with an `adb`
# each. The env var wins, then PATH, then the two known locations in the order
# `CLAUDE.md` names them.
find_tool() {
    local name="$1"; shift
    local candidate
    for candidate in "$@"; do
        [[ -n "$candidate" && -x "$candidate" ]] && { printf '%s' "$candidate"; return 0; }
    done
    candidate="$(command -v "$name" 2>/dev/null || true)"
    [[ -n "$candidate" ]] && { printf '%s' "$candidate"; return 0; }
    return 1
}

ADB="$(find_tool adb "${ADB:-}" "$HOME/Android/Sdk/platform-tools/adb" "$HOME/android/sdk/platform-tools/adb")" \
    || die "no adb: set ADB, or put it on PATH"
EMULATOR="$(find_tool emulator "${EMULATOR:-}" "$HOME/Android/Sdk/emulator/emulator" "$HOME/android/sdk/emulator/emulator" || true)"

# ---------------------------------------------------------------------------
# A device: the one already here, or one we boot ourselves
# ---------------------------------------------------------------------------
#
# Attaching to what is already running is the case that matters on CI, where
# `reactivecircus/android-emulator-runner` has booted the AVD itself and this
# script is the command it runs inside it. Booting our own is the case that
# matters on a laptop. The difference is one variable, and only the second one
# gets torn down at the end — a run that adopted somebody's emulator must not
# take it away from them.
BOOTED_BY_US=false

# ---------------------------------------------------------------------------
# The status bar, pinned
# ---------------------------------------------------------------------------
#
# `DEMO_ON` is what tells the exit path whether there is anything to undo, and
# it is set *before* the first broadcast rather than after the last one: a run
# interrupted halfway through arming demo mode is still a run that has to leave
# it. The header argues why any of this happens at all.
DEMO_ON=false

demo() {
    "$ADB" shell am broadcast -a com.android.systemui.demo "$@" > /dev/null 2>&1
}

demo_mode_enter() {
    # The one setting that gates the whole protocol. If SystemUI will not take
    # it, nothing below is doing anything and every frame carries the
    # emulator's real clock, so this is worth failing on rather than warning
    # about: the frames are the deliverable and a silently un-pinned status bar
    # is not visible until somebody compares seven of them side by side.
    "$ADB" shell settings put global sysui_demo_allowed 1 > /dev/null 2>&1 || true
    local allowed
    allowed="$("$ADB" shell settings get global sysui_demo_allowed 2>/dev/null | tr -d '\r')"
    [[ "$allowed" == "1" ]] \
        || die "this device will not allow SysUI demo mode (sysui_demo_allowed=${allowed:-unset}); every frame would carry its real status bar — see this script's header"

    DEMO_ON=true
    demo -e command enter

    # 9:30, and not the wall clock. A time is the one thing in a status bar
    # that a reader *reads*, and seven screenshots showing seven times three
    # minutes apart is a page that looks assembled rather than composed. The
    # value is arbitrary and deliberately unremarkable — morning, on the half
    # hour, nothing to think about.
    demo -e command clock -e hhmm 0930

    # `fully true` is the difference between a wifi glyph and a wifi glyph with
    # an exclamation mark beside it. Without it SystemUI draws the "connected
    # but no internet" variant, which is the truth about a swiftshader AVD and
    # is not the truth about anybody's phone.
    demo -e command network -e wifi show -e level 4 -e fully true
    demo -e command network -e mobile show -e level 4 -e datatype none -e fully true

    # A full battery with no bolt through it. An emulator is always "plugged
    # in", and a charging icon in a screenshot reads as a phone tethered to a
    # wall, which is the opposite of what this app is for.
    demo -e command battery -e level 100 -e plugged false

    # Everything else off: no notification icons, and none of the small status
    # glyphs that arrive from whatever the device was doing before this run.
    demo -e command notifications -e visible false
    demo -e command status -e volume hide -e bluetooth hide -e location hide \
        -e alarm hide -e sync hide -e tty hide -e eri hide -e mute hide -e speakerphone hide

    ok "status bar pinned (9:30, full signal, full battery)"
}

demo_mode_exit() {
    [[ "$DEMO_ON" == true ]] || return 0
    DEMO_ON=false
    demo -e command exit || true
    "$ADB" shell settings put global sysui_demo_allowed 0 > /dev/null 2>&1 || true
}

device_serial() {
    "$ADB" devices | sed -n '2,$p' | awk '$2 == "device" { print $1; exit }'
}

SERIAL="$(device_serial || true)"
if [[ -n "$SERIAL" ]]; then
    info "using the device already attached: $SERIAL"
else
    [[ -n "$EMULATOR" ]] || die "no emulator binary, and no device attached"
    "$EMULATOR" -list-avds 2>/dev/null | grep -qx "$AVD" \
        || die "no AVD named $AVD (make one 1080x1920: avdmanager create avd -n $AVD -k 'system-images;android-34;default;x86_64' -d pixel)"

    EMU_LOG="$(mktemp -t sla-emulator-XXXXXX.log)"
    info "booting $AVD headless (-no-window -no-audio, swiftshader) — log: $EMU_LOG"
    # `env -u WAYLAND_DISPLAY -u DISPLAY` belt and braces over `-no-window`:
    # an emulator with no way to reach a compositor cannot open a window even
    # if a future flag default changes its mind about wanting one.
    env -u WAYLAND_DISPLAY -u DISPLAY "$EMULATOR" -avd "$AVD" \
        -no-window -no-audio -no-boot-anim -no-snapshot \
        -gpu swiftshader_indirect > "$EMU_LOG" 2>&1 &
    EMULATOR_PID=$!
    BOOTED_BY_US=true

    info "waiting for it to come up…"
    deadline=$((SECONDS + 300))
    while :; do
        SERIAL="$(device_serial || true)"
        if [[ -n "$SERIAL" ]] \
            && [[ "$("$ADB" -s "$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == "1" ]]; then
            break
        fi
        kill -0 "$EMULATOR_PID" 2>/dev/null || { tail -n 30 "$EMU_LOG" >&2; die "the emulator exited during boot"; }
        (( SECONDS < deadline )) || { tail -n 30 "$EMU_LOG" >&2; die "the emulator never finished booting"; }
        sleep 2
    done
    ok "booted: $SERIAL"
fi

export ANDROID_SERIAL="$SERIAL"

cleanup() {
    [[ -n "${LOGCAT_PID:-}" ]] && kill "$LOGCAT_PID" 2>/dev/null || true
    # Before the emulator goes, and *whatever* killed the run. `--keep` and a
    # run that adopted somebody's device both leave a device behind, and
    # leaving it behind in demo mode is the one outcome here that outlives the
    # script — see the header. `|| true` throughout because this is the exit
    # path: a device that has already gone away must not turn a real failure
    # into a confusing one.
    demo_mode_exit
    if [[ "$BOOTED_BY_US" == true && "$KEEP" != true ]]; then
        info "shutting the emulator down"
        "$ADB" emu kill > /dev/null 2>&1 || true
        wait "${EMULATOR_PID:-0}" 2>/dev/null || true
    fi
}
trap cleanup EXIT
# And on the two ways a run ends that are not the script's own decision. An
# untrapped SIGINT or SIGTERM kills bash outright and the `EXIT` trap above
# never runs — so a Ctrl-C halfway through the tour, which is the *ordinary*
# way somebody stops one of these, would leave the emulator up and the device
# in demo mode. Trapping them as a bare `exit` is the whole fix: `exit` from a
# signal handler runs the EXIT trap on its way out, once, so `cleanup` still
# happens exactly the way it does on a clean run. The codes are the
# conventional 128 + signal.
trap 'exit 130' INT
trap 'exit 143' TERM

# The panel, before anything is built or installed: a mis-shaped AVD is worth
# failing on in the first five seconds rather than after a two-minute build and
# seven captures that are all the wrong size.
SIZE="$("$ADB" shell wm size 2>/dev/null | sed -n 's/^Physical size: *//p' | tr -d '\r')"
[[ "$SIZE" == "${WANT_W}x${WANT_H}" ]] \
    || die "this device's panel is ${SIZE:-unknown}, and Play wants ${WANT_W}x${WANT_H} (16:9) — see this script's header"
ok "panel: $SIZE"

# ---------------------------------------------------------------------------
# The APK
# ---------------------------------------------------------------------------

if [[ "$BUILD" == true ]]; then
    # The ABI comes off the device rather than being assumed, because the same
    # script has to serve an x86_64 emulator on a laptop and whatever a CI
    # runner's image happens to be.
    ABI="$("$ADB" shell getprop ro.product.cpu.abi | tr -d '\r')"
    [[ -n "$ABI" ]] || die "could not read the device's ABI"
    case "$ABI" in
        arm64-v8a|x86_64|armeabi-v7a|x86) ;;
        *) die "build-apk.sh has no target for ABI $ABI" ;;
    esac
    BUILD_ARGS=(--shots --target "$ABI" --build-only)
    [[ "$SOFTWARE" == true ]] && BUILD_ARGS+=(--software)
    info "building the shots APK for $ABI…"
    ( cd "$ROOT" && ./build-apk.sh "${BUILD_ARGS[@]}" ) >&2 \
        || die "build-apk.sh ${BUILD_ARGS[*]} failed"
    APK="$ROOT/setlistarray.apk"
else
    APK="${APK:-$ROOT/setlistarray.apk}"
fi
[[ -f "$APK" ]] || die "no APK at $APK"

# Uninstall rather than `install -r`, and the demo library is why. `--seed`
# only writes itself into an *empty* library (see `crate::app`), so a shots
# build installed over a previous run's data would open on that run's library
# — which is the same content today and is one schema change away from not
# being. The uninstall is allowed to fail: on the first run there is nothing
# there to remove.
info "installing $(basename "$APK")…"
"$ADB" uninstall "$PACKAGE" > /dev/null 2>&1 || true
"$ADB" install -r "$APK" > /dev/null || die "adb install failed"

mkdir -p "$OUT_DIR"

# ---------------------------------------------------------------------------
# Where the system bars are, written down beside the pictures
# ---------------------------------------------------------------------------
#
# One `frame=[left,top][right,bottom]` per bar, straight out of
# `dumpsys window displays`, which on API 34 lists every inset source on the
# display by name:
#
#   InsetsSource id=6aa50000  type=statusBars      frame=[0,0][1080,63]      visible=true
#   InsetsSource id=27370001  type=navigationBars  frame=[0,1794][1080,1920] visible=true
#
# Read once, before the tour, because these belong to the display rather than
# to the app: the app never asks for them to change, and the one screen that
# goes full screen (`chart-viewer`) does not hide the nav bar — it draws
# *underneath* it, which is the whole reason the compositor has to crop rather
# than keep. The header argues why this is measured at all instead of written
# into the Python as a pair of constants.
#
# `navigation_mode` goes in too, and it is the thing most likely to explain a
# surprising number later: 0 is three-button, 1 is two-button, 2 is gestures,
# and the bar's height follows from it.
bar_frame() {
    printf '%s\n' "$WINDOW_DUMP" \
        | grep -oE "type=$1 frame=\[[0-9]+,[0-9]+\]\[[0-9]+,[0-9]+\] visible=true" \
        | head -n 1 \
        | sed -E 's/.*frame=\[([0-9]+),([0-9]+)\]\[([0-9]+),([0-9]+)\].*/\1 \2 \3 \4/'
}

WINDOW_DUMP="$("$ADB" shell dumpsys window displays 2>/dev/null | tr -d '\r')"
STATUS_FRAME="$(bar_frame statusBars)"
NAV_FRAME="$(bar_frame navigationBars)"
[[ -n "$STATUS_FRAME" && -n "$NAV_FRAME" ]] \
    || die "could not read the system bar insets off this device; scripts/store-frame.py crops by them and has nothing to crop by"
read -r SB_L SB_T SB_R SB_B <<< "$STATUS_FRAME"
read -r NB_L NB_T NB_R NB_B <<< "$NAV_FRAME"

NAV_MODE="$("$ADB" shell settings get secure navigation_mode 2>/dev/null | tr -d '\r')"
SDK="$("$ADB" shell getprop ro.build.version.sdk | tr -d '\r')"
FINGERPRINT="$("$ADB" shell getprop ro.build.fingerprint | tr -d '\r')"

cat > "$OUT_DIR/insets.json" <<JSON
{
  "note": "Where the system bars were when the PNGs beside this file were taken. Written by scripts/store-shots.sh, read by scripts/store-frame.py, which keeps the status bar and crops the navigation bar off every frame. Measured rather than assumed: see store-shots.sh's header.",
  "panel": { "width": $WANT_W, "height": $WANT_H },
  "status_bar": { "left": $SB_L, "top": $SB_T, "right": $SB_R, "bottom": $SB_B, "height": $((SB_B - SB_T)) },
  "navigation_bar": { "left": $NB_L, "top": $NB_T, "right": $NB_R, "bottom": $NB_B, "height": $((NB_B - NB_T)) },
  "navigation_mode": "${NAV_MODE:-unknown}",
  "sdk": "$SDK",
  "fingerprint": "$FINGERPRINT"
}
JSON
ok "insets: status bar $((SB_B - SB_T))px, nav bar $((NB_B - NB_T))px (navigation_mode ${NAV_MODE:-unknown})  ->  $OUT_DIR/insets.json"

demo_mode_enter

# ---------------------------------------------------------------------------
# The tour
# ---------------------------------------------------------------------------
#
# The app announces itself and this follows along; the list of shots lives in
# `src/shots.rs` and deliberately does not live here too. A second copy of it
# in shell would be a copy that can disagree with the Rust one, and the way it
# would disagree is the way that costs the most: a shot added in Rust and not
# here is a picture nobody notices is missing.
LOGCAT_FILE="$(mktemp -t sla-shots-XXXXXX.log)"
"$ADB" logcat -c > /dev/null 2>&1 || true
"$ADB" logcat -s rinch:I > "$LOGCAT_FILE" 2>&1 &
LOGCAT_PID=$!

info "launching $PACKAGE/$ACTIVITY"
APP_STARTED=true
"$ADB" shell am start -n "$PACKAGE/$ACTIVITY" > /dev/null || die "am start failed"

# A PNG's width and height, out of its own IHDR chunk: big-endian u32 at byte
# 16 and byte 20. Done here with `od` rather than with ImageMagick's `identify`
# on purpose — this is the one check that decides whether the run succeeded, so
# it should not be the one thing that makes the script refuse to run on a
# machine (or a CI image) with no ImageMagick on it.
png_size() {
    local hex
    hex="$(od -An -tx1 -j16 -N8 "$1" | tr -d ' \n')"
    [[ ${#hex} -eq 16 ]] || return 1
    printf '%d %d' "0x${hex:0:8}" "0x${hex:8:8}"
}

capture() {
    local id="$1" path="$OUT_DIR/$1.png"
    "$ADB" exec-out screencap -p > "$path" || die "screencap failed for $id"
    [[ -s "$path" ]] || die "screencap produced an empty file for $id"
    local size
    size="$(png_size "$path")" || die "$path is not a PNG this script can measure"
    read -r w h <<< "$size"
    [[ "$w" -eq "$WANT_W" && "$h" -eq "$WANT_H" ]] \
        || die "$id captured at ${w}x${h}, not ${WANT_W}x${WANT_H} — see this script's header for why that matters"
    ok "$id  ${w}x${h}  $(du -h "$path" | cut -f1)"
}

CAPTURED=()
cursor=0
last_shot=$SECONDS
run_deadline=$((SECONDS + RUN_TIMEOUT))
finished=false

while [[ "$finished" != true ]]; do
    total="$(wc -l < "$LOGCAT_FILE")"
    if (( total > cursor )); then
        # Process substitution rather than a pipe, so that what the loop body
        # appends to CAPTURED is still there when the loop ends.
        while IFS= read -r line; do
            case "$line" in
                *SLA_SHOT_READY*)
                    id="${line##*SLA_SHOT_READY }"
                    id="${id%%[[:space:]]*}"
                    capture "$id"
                    CAPTURED+=("$id")
                    last_shot=$SECONDS
                    ;;
                *SLA_SHOT_BROKEN*)
                    die "the app could not set a shot up: ${line#*SLA_SHOT_BROKEN }"
                    ;;
                *SLA_SHOTS_DONE*)
                    finished=true
                    ;;
            esac
        done < <(tail -n "+$((cursor + 1))" "$LOGCAT_FILE" | head -n "$((total - cursor))")
        cursor=$total
    fi
    [[ "$finished" == true ]] && break

    # Three ways for this to end badly, and each says something different.
    if ! "$ADB" shell pidof "$PACKAGE" > /dev/null 2>&1; then
        sleep 2   # a moment, in case it is simply between the fork and the log
        "$ADB" shell pidof "$PACKAGE" > /dev/null 2>&1 \
            || die "the app is no longer running after ${#CAPTURED[@]} shot(s)"
    fi
    (( SECONDS - last_shot < SHOT_TIMEOUT )) \
        || die "no shot in ${SHOT_TIMEOUT}s after ${#CAPTURED[@]} (${CAPTURED[*]:-none})"
    (( SECONDS < run_deadline )) \
        || die "the tour did not finish inside ${RUN_TIMEOUT}s"
    sleep 1
done

(( ${#CAPTURED[@]} > 0 )) || die "the app said it was done without announcing a single shot"

printf '\n%s✓ %d shot(s) in %s%s\n' "$GREEN$BOLD" "${#CAPTURED[@]}" "$OUT_DIR" "$RESET" >&2
for id in "${CAPTURED[@]}"; do
    printf '    %s\n' "$OUT_DIR/$id.png" >&2
done
