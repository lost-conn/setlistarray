# Working in this repository

## Never open a window on the developer's display

This is a GUI app and verifying it means running it. **Run it through
`scripts/with-display.sh`, which puts it on a private, invisible Xvfb at `:99`,
never on `:0`.**

```bash
scripts/with-display.sh cargo run --release          # the app, invisibly
scripts/with-display.sh cargo run --release --seed   # ...with demo content
eval "$(scripts/with-display.sh --export)"           # point this shell at :99
scripts/with-display.sh --stop                       # tear the display down
```

`scripts/screenshot.sh` re-enters that wrapper itself, so the visual net is
already safe to run as-is.

Why the rule exists: on 2026-08-28 an agent verifying the file picker (card D3)
launched the app on the real desktop and opened native file dialogs on it,
repeatedly, while the machine's owner was working. Focus theft is not a neutral
side effect of a test. This applies to anything that maps a window —
`cargo run`, `rfd` dialogs, `xdg-open`, a probe binary.

The display is deliberately configured at a **1.25x scale factor**
(`WINIT_X11_SCALE_FACTOR`), because that is what
`scripts/screenshot-baseline.json` was measured at and several of its checks
count absolute pixels. Do not "fix" a red net by re-baselining at 1x.

## Do not run `cargo fmt`

103 pre-existing diffs are the baseline, and `cargo fmt -- <file>` rewrites the
whole crate rather than the file named. Match the surrounding style by hand.

## The frameworks are local path dependencies

`Cargo.toml` points at `../rinch-fixes` (an integration branch of
github.com/joeleaver/rinch carrying fixes not yet merged upstream) and
`../rhypedb-main`. A framework bug is fixed in `../rinch-fixes` and then PR'd
upstream — see the PR list at the bottom of README.md for the shape of that,
and card A1 for moving the pin once they land.

## The house style

Comments and commit messages here are long-form narrative prose that explain
*why*, including the failure that motivated the code. A new comment should read
like the ones around it. `scripts/screenshot.sh` and the `note` fields in
`scripts/screenshot-baseline.json` are the clearest examples.

## Android

A device is usually attached: `~/Android/Sdk/platform-tools/adb` (not on PATH),
serial `ZY22FD66GZ`, a moto g stylus 5G. `./build-apk.sh` builds the APK.
Anything found on hardware is worth turning into a `cargo test` that fails on a
laptop — see cards K15 and K20 for that pattern.
