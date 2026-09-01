#!/usr/bin/env bash
set -euo pipefail

# What the panel actually showed, measured from outside the app.
#
# Usage:
#   scripts/frame-probe.sh                    # this app, flinging the library list
#   scripts/frame-probe.sh --control          # stock Settings on the same panel
#   scripts/frame-probe.sh --flings 16        # a longer sample
#   scripts/frame-probe.sh --label "gpu"      # a name for the row it prints
#
# Card K39 measured frame timing inside the process and reported 8.33ms p50 on
# a screen the owner could see was not running at 120fps. It was wrong by 5x,
# and three cards were written on top of it before card K40 caught it. The
# reason is simple enough to state: an in-process timer measures how long the
# app took to hand a frame over, and says nothing about whether the compositor
# ever put it on the glass. A frame the app builds in 8ms and the panel shows
# every second refresh is 8ms of work and 60fps of experience.
#
# So this script never asks the app. `dumpsys SurfaceFlinger --latency <layer>`
# is SurfaceFlinger's own record of when each buffer for that layer was
# actually presented, and the interval between consecutive presents is what a
# person watching the screen sees. That is the number this prints.
#
# It also insists on a control. A phone that is thermally throttled, or busy
# installing something, or holding its panel at 60Hz because the battery is
# low, will make any app look bad — and the only way to know the difference is
# to measure a known-good app on the same panel in the same minute.
# `--control` flings stock Settings, which is what cards K42 and K43 compared
# against, and it holds 8.33ms p50 on this handset when the panel is at 120Hz.
#
# The workload is a fling rather than a still screen on purpose: a Rinch app
# with nothing moving parks in `ControlFlow::Wait` and presents no frames at
# all, so a measurement taken on a static screen samples an empty list and
# reports whatever noise is left. It also **verifies the list actually moved**
# — a swipe that lands on a dead area produces a perfectly convincing set of
# frame timings for a screen that never scrolled.

ADB="${ADB:-$HOME/Android/Sdk/platform-tools/adb}"
PACKAGE="dev.lostconnection.setlistarray"
ACTIVITY="com.rinch.RinchActivity"
LABEL=""
FLINGS=12
CONTROL=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --control) CONTROL=true; shift ;;
        --flings) FLINGS="$2"; shift 2 ;;
        --label) LABEL="$2"; shift 2 ;;
        -h|--help) sed -n '3,10p' "$0"; exit 0 ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

if [[ "$CONTROL" == true ]]; then
    PACKAGE="com.android.settings"
    ACTIVITY=".Settings"
    [[ -n "$LABEL" ]] || LABEL="stock Settings (control)"
fi
[[ -n "$LABEL" ]] || LABEL="$PACKAGE"

if [[ ! -x "$ADB" ]]; then
    echo "ERROR: adb not found at $ADB (set ADB=/path/to/adb)"
    exit 1
fi
if [[ -z "$("$ADB" devices | sed -n '2,$p' | grep -w device || true)" ]]; then
    echo "ERROR: no device attached"
    exit 1
fi

# Alternating direction, which is not cosmetic. A list flung one way twelve
# times reaches its end and then measures a screen that cannot move — the
# guard below catches that, but only after it has already happened. Going back
# and forth keeps the content under the finger for as long as the sample runs,
# whatever screen it was left on.
FLING_N=0
fling() {
    FLING_N=$((FLING_N + 1))
    # Deliberately not `input swipe`'s default duration. A 200ms drag over
    # 1100px is a fling a person would recognise; much faster and the gesture
    # recogniser reads it as something else, much slower and it is a drag that
    # ends where the finger stops and produces no inertia frames at all.
    if (( FLING_N % 2 )); then
        "$ADB" shell input swipe 540 1800 540 700 200
    else
        "$ADB" shell input swipe 540 700 540 1800 200
    fi
}

echo "==> launching $PACKAGE"
"$ADB" shell am start -n "$PACKAGE/$ACTIVITY" >/dev/null
"$ADB" shell sleep 3

LAYER="$("$ADB" shell dumpsys SurfaceFlinger --list 2>/dev/null \
    | grep "^$PACKAGE/" | tail -1 | tr -d '\r')"
if [[ -z "$LAYER" ]]; then
    echo "ERROR: no SurfaceFlinger layer for $PACKAGE — is it in the foreground?"
    exit 1
fi

# Prove the workload does something before trusting a single frame time. The
# cheapest evidence available over adb is the screen itself: fling, and check
# the pixels changed. `screencap -p` is a PNG, so identical bytes mean an
# identical picture.
# Both directions before giving up. A list sitting at its end cannot move the
# way the next fling pushes it, and that is a scroll position rather than a
# broken workload — the app persists where it was left (card B3), so whichever
# end the last run stopped at is where this one starts.
moved=false
for _ in 1 2; do
    before="$("$ADB" exec-out screencap -p | md5sum | cut -d' ' -f1)"
    fling
    "$ADB" shell sleep 1
    after="$("$ADB" exec-out screencap -p | md5sum | cut -d' ' -f1)"
    if [[ "$before" != "$after" ]]; then moved=true; break; fi
done
if [[ "$moved" == false ]]; then
    echo "ERROR: the screen did not change across a fling in either direction —"
    echo "       this workload is measuring a screen that never scrolled. Check"
    echo "       the app is on a scrollable view and the coordinates suit this"
    echo "       panel."
    exit 1
fi

echo "==> warming up"
for _ in 1 2 3 4; do fling; done

# One dump per fling, not one dump at the end.
#
# `--latency` answers out of a **128-entry ring buffer**, so a run of twelve
# flings reports the last twelve-hundred-odd milliseconds of it and silently
# discards everything before. The first version of this script did exactly
# that, and gave itself away by reporting "125 frames" for two different apps
# and two different fling counts — a sample size that does not move with the
# workload is not a sample size, it is a buffer length.
#
# So each fling is cleared, flung, and drained on its own, and the blocks are
# concatenated with a marker between them. The marker matters: the gap from
# the end of one fling to the start of the next is not a frame interval, and
# joining two blocks would invent a slow frame that nobody saw.
echo "==> sampling $FLINGS flings"
: > /tmp/frame-probe-$$.txt
for _ in $(seq "$FLINGS"); do
    "$ADB" shell "dumpsys SurfaceFlinger --latency-clear '$LAYER'" >/dev/null
    fling
    "$ADB" shell sleep 0.7   # let the inertia frames land before draining
    echo "=== block ===" >> /tmp/frame-probe-$$.txt
    "$ADB" shell "dumpsys SurfaceFlinger --latency '$LAYER'" >> /tmp/frame-probe-$$.txt
done

awk -v label="$LABEL" '
    # The first line is the panel refresh period in nanoseconds; every line
    # after it is one presented buffer as three timestamps, of which the
    # second — when SurfaceFlinger actually put it on the glass — is the only
    # one this cares about. Pending frames carry INT64_MAX and zeroes; both
    # are dropped rather than smoothed over.
    /^=== block ===$/ { prev = 0; expect_period = 1; next }
    expect_period { period = $1 / 1e6; expect_period = 0; next }
    NF == 3 && $2 > 0 && $2 != 9223372036854775807 {
        if (prev > 0) {
            d = ($2 - prev) / 1e6
            # A gap between flings is not a dropped frame, it is the app
            # correctly going to sleep with nothing to draw. 200ms is far
            # longer than any janky frame and far shorter than the pause
            # between two `input swipe` calls.
            if (d > 0 && d < 200) intervals[++n] = d
        }
        prev = $2
    }
    END {
        if (n < 20) {
            printf "ERROR: only %d frame intervals — too few to report\n", n
            exit 1
        }
        asort(intervals)
        p50 = intervals[int(n * 0.50)]
        p95 = intervals[int(n * 0.95)]
        p99 = intervals[int(n * 0.99)]
        total = 0
        missed = 0
        for (i = 1; i <= n; i++) {
            total += intervals[i]
            # "Missed" is a frame that did not land on the next refresh. Half
            # a period of slack keeps ordinary jitter out of the count.
            if (intervals[i] > period * 1.5) missed++
        }
        printf "\n  %-28s %7s %7s %7s %7s %8s\n", "", "p50", "p95", "p99", "fps", "missed"
        printf "  %-28s %6.2fms %6.2fms %6.2fms %7.1f %7.1f%%\n", \
            label, p50, p95, p99, n / (total / 1000), missed * 100 / n
        printf "  %d frames over %.2fs, panel refresh %.2fms\n\n", n, total / 1000, period
    }
' /tmp/frame-probe-$$.txt

rm -f /tmp/frame-probe-$$.txt
