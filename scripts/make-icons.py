#!/usr/bin/env python3
"""Turn the three designed icon files in `assets/icon/` into `android/res/`.

Run it from anywhere:

    python3 scripts/make-icons.py            # regenerate android/res/

It needs Pillow and NumPy, and it is the only thing in this repo that does.
That is on purpose: `android/res/` is **committed**, so `build-apk.sh` — and
anyone who checks the tree out on a machine with nothing but a Rust toolchain
and the Android SDK — never runs this. Re-run it when the artwork changes.

────────────────────────────────────────────────────────────────────────────
What the three source files are, and why none of them can be shipped as-is
────────────────────────────────────────────────────────────────────────────

`assets/icon/maskable.png` (768², fully opaque) is the design: a radial
gradient in the app's Rust with the bracket-and-quaver glyph in white on top
of it. `assets/icon/mono.png` (768², transparent) is the same glyph on its
own, white, carrying the drop shadow the design gives it. `assets/icon/
whole.png` (512², transparent corners) is the rounded-square lockup.

Android wants an **adaptive icon**: two layers, each 108dp, of which the
system shows only the central 72dp and masks that to whatever shape the
launcher's theme is using — plus a third `monochrome` layer for the themed
("Material You") icons that Android 13 introduced. So there is no file here
that can be dropped in whole, and two separate problems to solve.

**One: the layers do not exist.** The design has the glyph baked into the
gradient. But the gradient is radial and unusually clean, so it can simply be
*re-derived*: fit a centre and a radius→colour profile against the pixels of
`maskable.png` that `mono.png` says the glyph and its shadow never touch, and
paint it back at any size. Measured on 2026-09-09, that reconstruction has an
RMS error of 0.28/255 and a worst pixel of 1.8/255 against the artwork —
`icon_gradient.check_fidelity` re-measures it on every run and refuses to
write anything if it drifts, because a background that is *nearly* the designed
gradient is a bug nobody would ever look for.

**Two: the glyph is drawn too big for Android's mask.** `maskable.png` is a
*web* maskable icon, and that spec's safe zone is a circle covering 80% of the
canvas. Android's is smaller: content is guaranteed visible only inside a 66dp
circle out of the 108dp layer, 61% of it. The glyph in `mono.png` reaches
279px from centre out of 384 — 39.2dp once mapped onto a 108dp layer, needing
a 78dp circle. Shipped at its drawn size, a circular launcher mask would clip
the corners off both brackets.

The fix chosen on 2026-09-09 was the conservative one: **scale the entire
design by 72/108, so that `maskable.png` *is* the visible 72dp square** and
the outer 18dp on each side is gradient bleed. Compositing this script's
foreground over its background and cropping to the visible region reproduces
the artwork pixel for pixel; the glyph lands at 27.4dp radius, comfortably
inside the 33dp safe radius, under any mask. The alternative — scaling the
glyph alone by 0.839 so it exactly fills the safe circle, bigger on screen but
no longer the designer's proportions — was considered and not taken.

────────────────────────────────────────────────────────────────────────────
The monochrome layer, and the shadow that must not be in it
────────────────────────────────────────────────────────────────────────────

`mono.png` looks like the obvious `monochrome` drawable and is the wrong file
for it. The system ignores that drawable's colour and tints its **alpha**, and
`mono.png`'s alpha contains the drop shadow — 53,435 pixels of it below 40%.
Tint that and the shadow stops being a shadow and becomes a soft halo in the
theme's accent colour, sitting a few pixels down and to the right of a glyph
it no longer looks attached to.

So the monochrome coverage is unmixed out of the artwork instead. Every pixel
of `maskable.png` is `white·a + gradient·(1-a)`, so `a = (maskable - gradient)
/ (255 - gradient)` recovers the glyph's own antialiased coverage — and the
shadow, which makes the artwork *darker* than the gradient rather than
lighter, comes out negative and clamps to nothing. That is the layer this
writes: the glyph, its edges intact, no shadow at all.

The full-colour `foreground` keeps the shadow, because there it is doing the
job it was drawn for.

────────────────────────────────────────────────────────────────────────────
Where the gradient fit went
────────────────────────────────────────────────────────────────────────────

`fit_gradient`, `paint_background` and `check_fidelity` used to be written out
below and now live in `scripts/icon_gradient.py`, imported. Nothing about what
they do changed with the move — this script regenerates `android/res/` byte for
byte across it, which was checked rather than assumed — and the reason for it
is entirely about the *other* caller. Card S2's `scripts/store-frame.py` paints
the Play listing on this same gradient, and the only alternative was to copy a
colour out of the artwork into a second file. That copy would be correct until
the artwork is redrawn and then wrong forever, silently, in a store listing
nobody re-opens; the module's own docstring argues it properly. The fit is a
measurement of the design, so it belongs somewhere both painters can ask.
"""

import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image

# A sibling file, found because Python puts a script's own directory at the
# front of `sys.path` — so this works from any working directory, the way the
# line at the top of this docstring promises, without either file knowing where
# the repository is checked out.
from icon_gradient import (
    LAYER_DP,
    VISIBLE_DP,
    check_fidelity,
    fit_gradient,
    load_artwork,
    paint_background,
)

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets" / "icon"
RES = ROOT / "android" / "res"

# The five buckets Android sorts a screen into, and the multiple of the
# density-independent pixel each one means. A phone made this decade is xxhdpi
# or xxxhdpi; the small ones cost a few KB each and are what a tablet, an
# emulator or a launcher asking for a thumbnail will pick up.
DENSITIES = {"mdpi": 1.0, "hdpi": 1.5, "xhdpi": 2.0, "xxhdpi": 3.0, "xxxhdpi": 4.0}

# An adaptive icon's layers are 108dp and the system shows the central 72dp of
# them; those two numbers are `icon_gradient`'s, imported above, because the
# background painter's framing is built out of them. The legacy bitmap under
# `android:icon` is a plain 48dp square and is this file's business alone.
LEGACY_DP = 48

# The radius, in dp out of the 108dp layer, inside which Android guarantees
# content survives every launcher mask. Asserted against the finished
# foreground rather than trusted.
SAFE_RADIUS_DP = 33.0


def place_glyph(glyph, size):
    """Scale a 768² source layer by 72/108 and centre it on a `size`² canvas."""
    inner = round(size * VISIBLE_DP / LAYER_DP)
    scaled = glyph.resize((inner, inner), Image.LANCZOS)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    off = (size - inner) // 2
    canvas.paste(scaled, (off, off))
    return canvas


def monochrome_source(maskable, background_768):
    """The glyph's own coverage, unmixed from the artwork, shadow and all
    discarded. See the module docstring."""
    bg = background_768[:, :, :3]
    a = (maskable[:, :, :3] - bg) / np.maximum(255.0 - bg, 1.0)
    a = np.clip(a.mean(axis=2), 0.0, 1.0)
    out = np.zeros((*a.shape, 4))
    out[:, :, 3] = a * 255.0
    return Image.fromarray(np.clip(out + 0.5, 0, 255).astype(np.uint8), "RGBA")


def circle_crop(img):
    size = img.size[0]
    yy, xx = np.mgrid[0:size, 0:size]
    r = np.hypot(xx + 0.5 - size / 2, yy + 0.5 - size / 2)
    # One pixel of feathering at the edge, so the circle is not a staircase.
    mask = np.clip(size / 2 - r, 0.0, 1.0) * 255.0
    out = np.asarray(img.convert("RGBA")).astype(np.float64)
    out[:, :, 3] = np.minimum(out[:, :, 3], mask)
    return Image.fromarray(np.clip(out + 0.5, 0, 255).astype(np.uint8), "RGBA")


def _check_safe_zone(foreground_768_alpha):
    """The reason the whole design is scaled by 72/108 — asserted, not assumed."""
    a = foreground_768_alpha
    size = a.shape[0]
    yy, xx = np.mgrid[0:size, 0:size]
    r = np.hypot(xx + 0.5 - size / 2, yy + 0.5 - size / 2)
    lit = a > 2.0
    reach_dp = float(r[lit].max()) / (size / 2) * (VISIBLE_DP / 2)
    print(f"    glyph reaches {reach_dp:.1f}dp from centre (safe circle is {SAFE_RADIUS_DP:.0f}dp)")
    if reach_dp > SAFE_RADIUS_DP:
        sys.exit(
            f"ERROR: the glyph reaches {reach_dp:.1f}dp from the centre of the 108dp layer,\n"
            f"       outside the {SAFE_RADIUS_DP:.0f}dp circle Android guarantees. A circular\n"
            "       launcher mask would clip it. Scale the artwork down before shipping it."
        )


def build():
    maskable, mono = load_artwork(SRC)
    whole = Image.open(SRC / "whole.png").convert("RGBA")

    print("--> fitting the gradient out of assets/icon/maskable.png")
    centre, profile = fit_gradient(maskable, mono)
    background_768 = check_fidelity(maskable, mono, centre, profile)
    _check_safe_zone(mono[:, :, 3])

    mono_glyph = monochrome_source(maskable, background_768)
    fg_glyph = Image.fromarray(np.clip(mono + 0.5, 0, 255).astype(np.uint8), "RGBA")
    round_source = circle_crop(Image.fromarray(np.clip(maskable + 0.5, 0, 255).astype(np.uint8), "RGBA"))

    written = []
    for bucket, scale in DENSITIES.items():
        out_dir = RES / f"mipmap-{bucket}"
        out_dir.mkdir(parents=True, exist_ok=True)
        layer = round(LAYER_DP * scale)
        legacy = round(LEGACY_DP * scale)

        files = {
            "ic_launcher_background.png": paint_background(layer, centre, profile),
            "ic_launcher_foreground.png": place_glyph(fg_glyph, layer),
            "ic_launcher_monochrome.png": place_glyph(mono_glyph, layer),
            "ic_launcher.png": whole.resize((legacy, legacy), Image.LANCZOS),
            "ic_launcher_round.png": round_source.resize((legacy, legacy), Image.LANCZOS),
        }
        for name, img in files.items():
            path = out_dir / name
            img.save(path, optimize=True)
            written.append(path)

    total = sum(p.stat().st_size for p in written)
    print(f"--> wrote {len(written)} files under android/res ({total / 1024:.0f} KiB)")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    build()
