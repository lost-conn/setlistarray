#!/usr/bin/env bash
# Run a command against a private, invisible X display instead of the one the
# developer is sitting in front of.
#
# Why this exists: on 2026-08-28 an agent verifying the file picker (card D3)
# launched the app on `:0` and opened a native file dialog on it, repeatedly,
# while the machine's owner was using the machine. A GUI app that takes focus
# every time something is verified is not a neutral act on a desktop somebody
# is working in — and the visual net does a milder version of the same thing
# on every single run, since `scripts/screenshot.sh` has to map a real window
# to photograph it.
#
# Xvfb, not Xephyr: Xephyr draws its nested screen inside a window on the real
# desktop, which is most of the problem again. Xvfb draws nowhere at all, and
# `import`, `xwininfo` and `xprop` are as happy against it as against a screen
# with a monitor attached.
#
# **Setting DISPLAY is not enough on a Wayland desktop, and this machine is
# one.** winit prefers the Wayland backend whenever `WAYLAND_DISPLAY` is set
# and never looks at `DISPLAY` at all, so a wrapper that exported `:99` and
# nothing else handed the app straight back to the session compositor — the
# window opened on the developer's real desktop, exactly what this script is
# for. It was found on 2026-08-28 during card E3, by an agent that had used
# this wrapper as instructed and then could not find the window it had just
# driven anywhere on `:99`. `scripts/screenshot.sh` already knew — it launches
# with `env -u WAYLAND_DISPLAY` and says why — so the knowledge existed and was
# in the wrong file. It is here now, which is the only place that makes the
# rule in CLAUDE.md true.
#
# Usage:
#   scripts/with-display.sh cargo run --release        # run anything on :99
#   scripts/with-display.sh scripts/screenshot.sh      # (screenshot.sh does
#                                                      #  this to itself)
#   scripts/with-display.sh --stop                     # tear the display down
#   eval "$(scripts/with-display.sh --export)"         # DISPLAY for this shell
#
# The display is left running between calls. Starting one costs about a second
# and nothing is watching it, so paying that on every invocation would be a
# waste; `--stop` is there for when a run has wedged something and a clean X
# server is easier to reason about than a dirty one.
set -euo pipefail

DISPLAY_NUM="${SLA_DISPLAY_NUM:-99}"
DISPLAY_NAME=":${DISPLAY_NUM}"

# Big enough for the app's window with room around it, and 24-bit because the
# net compares colours to the hex codes in the theme and a 16-bit visual would
# quantise them into failing.
SCREEN_GEOMETRY="${SLA_SCREEN_GEOMETRY:-1400x1400x24}"

# The scale factor the app renders at. This matters more than it looks: every
# number in scripts/screenshot-baseline.json was measured on a display granting
# 1.25x, and several checks count absolute pixels — `confidence_dots_on_screen`
# wants at least 99 of them. A display that hands back 1.0 resolves every region
# a quarter smaller, and the net goes red with nothing whatsoever wrong with the
# app. Measured, not assumed: the first run of this script on a bare Xvfb did
# exactly that, 72 dot pixels against a floor of 99.
#
# `WINIT_X11_SCALE_FACTOR` is winit's own override and is what gets used, after
# the obvious route turned out not to work here. X clients normally learn the
# scale from `Xft.dpi` in the resource database, but on this Xvfb no root-window
# property write persists at all — `xrdb -load` exits 0 and `xrdb -query` comes
# back empty, and so does an `xprop -root -set` of any property whatsoever. Xvfb
# `-dpi 120` does make `xdpyinfo` report 120x120, but winit reads the resource
# database rather than the server's own idea of its physical size, so it ignores
# that and falls back to 1.0. The env var goes straight to the code that would
# have read the resource, and needs no property to survive.
SCALE_FACTOR="${SLA_SCALE_FACTOR:-1.25}"

RED=$'\033[31m'; RESET=$'\033[0m'
die() { printf '%s✗ %s%s\n' "$RED" "$*" "$RESET" >&2; exit 1; }

command -v Xvfb >/dev/null 2>&1 || die "missing dependency: Xvfb (apt install xvfb)"

# Is a server already listening on this display? The lock file alone is not the
# answer — a killed Xvfb can leave one behind — so the socket is what gets
# asked, and a stale lock is cleared rather than believed.
display_is_up() {
    xdpyinfo -display "$DISPLAY_NAME" >/dev/null 2>&1
}

stop_display() {
    if display_is_up; then
        # Strictly by the PID that owns this display's lock file, never by
        # name: `pkill Xvfb` on a machine where something else is using one is
        # exactly the kind of thing this script exists to avoid doing to
        # somebody's session.
        local pid
        pid="$(cat "/tmp/.X${DISPLAY_NUM}-lock" 2>/dev/null || true)"
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
            for _ in 1 2 3 4 5; do
                display_is_up || break
                sleep 0.4
            done
        fi
    fi
    rm -f "/tmp/.X${DISPLAY_NUM}-lock" 2>/dev/null || true
}

start_display() {
    display_is_up && return 0

    # A lock with no server behind it stops Xvfb dead ("server already
    # running"), which is a confusing way to fail three cards later.
    if [[ -e "/tmp/.X${DISPLAY_NUM}-lock" ]]; then
        local stale
        stale="$(cat "/tmp/.X${DISPLAY_NUM}-lock" 2>/dev/null || true)"
        if [[ -z "$stale" ]] || ! kill -0 "$stale" 2>/dev/null; then
            rm -f "/tmp/.X${DISPLAY_NUM}-lock"
        fi
    fi

    # `-dpi` does not reach winit (see the note on SCALE_FACTOR above) but it
    # does make `xdpyinfo` and anything else that asks the server agree with
    # the scale factor the app is being told to use, which is worth more than
    # leaving the two saying different things.
    Xvfb "$DISPLAY_NAME" -screen 0 "$SCREEN_GEOMETRY" -dpi 120 -nolisten tcp >/dev/null 2>&1 &
    for _ in $(seq 1 40); do
        display_is_up && break
        sleep 0.25
    done
    display_is_up || die "Xvfb never came up on $DISPLAY_NAME"
}

case "${1:-}" in
    --stop)
        stop_display
        exit 0
        ;;
    --export)
        start_display
        # For `eval "$(scripts/with-display.sh --export)"`, which is how a
        # human gets a shell pointed at the private display without wrapping
        # every command in this script. `unset WAYLAND_DISPLAY` for the reason
        # in the header: a shell that still has it exported will put the next
        # GUI app it starts on the real desktop, whatever DISPLAY says.
        printf 'unset WAYLAND_DISPLAY; export DISPLAY=%s WINIT_X11_SCALE_FACTOR=%s SLA_HEADLESS_DISPLAY=1\n' \
            "$DISPLAY_NAME" "$SCALE_FACTOR"
        exit 0
        ;;
    -h|--help|"")
        sed -n '2,29p' "${BASH_SOURCE[0]}"
        exit 0
        ;;
esac

start_display

# `SLA_HEADLESS_DISPLAY` is the flag that stops screenshot.sh re-entering this
# script when it is already inside it. Without it the two would call each other
# until the shell ran out of processes.
# `env -u WAYLAND_DISPLAY` is load-bearing, not tidiness — see the header.
exec env -u WAYLAND_DISPLAY \
    DISPLAY="$DISPLAY_NAME" \
    WINIT_X11_SCALE_FACTOR="$SCALE_FACTOR" \
    SLA_HEADLESS_DISPLAY=1 \
    "$@"
