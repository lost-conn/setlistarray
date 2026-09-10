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
`_check_fidelity` below re-measures it on every run and refuses to write
anything if it drifts, because a background that is *nearly* the designed
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
"""

import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets" / "icon"
RES = ROOT / "android" / "res"

# The five buckets Android sorts a screen into, and the multiple of the
# density-independent pixel each one means. A phone made this decade is xxhdpi
# or xxxhdpi; the small ones cost a few KB each and are what a tablet, an
# emulator or a launcher asking for a thumbnail will pick up.
DENSITIES = {"mdpi": 1.0, "hdpi": 1.5, "xhdpi": 2.0, "xxhdpi": 3.0, "xxxhdpi": 4.0}

# An adaptive icon's layers are 108dp; the system shows the central 72dp and
# masks that. The legacy bitmap under `android:icon` is a plain 48dp square.
LAYER_DP = 108
VISIBLE_DP = 72
LEGACY_DP = 48

# The radius, in dp out of the 108dp layer, inside which Android guarantees
# content survives every launcher mask. Asserted against the finished
# foreground rather than trusted.
SAFE_RADIUS_DP = 33.0


def _load(name):
    return np.asarray(Image.open(SRC / name).convert("RGBA")).astype(np.float64)


def fit_gradient(maskable, mono):
    """Recover the radial gradient underneath the glyph.

    Returns `(centre, profile)`: the centre in source pixels, and an array
    indexed by integer radius holding the RGB the artwork has at that distance.

    The centre is searched for rather than assumed, so that this still does the
    right thing if the artwork is redrawn — but the search is over the pixels
    the glyph and its shadow do not touch, which is what makes the answer mean
    anything. `_check_fidelity` is the part that decides whether the answer was
    good enough to use.
    """
    clean = mono[:, :, 3] < 1.0
    h, w, _ = maskable.shape
    yy, xx = np.mgrid[0:h, 0:w]
    max_r = int(np.hypot(h, w)) + 2

    def profile_for(cx, cy):
        d = np.hypot(xx - cx, yy - cy).astype(np.int32)
        d_clean = d[clean]
        counts = np.bincount(d_clean, minlength=max_r)
        prof = np.zeros((max_r, 3))
        for c in range(3):
            sums = np.bincount(d_clean, maskable[:, :, c][clean], minlength=max_r)
            band = np.where(counts > 0, sums / np.maximum(counts, 1), np.nan)
            # A radius no clean pixel fell in (the far corners, or a band the
            # glyph happens to cover entirely) carries the last value that was
            # measured. The profile is monotone out there; this is extending
            # it, not inventing it.
            last = 0.0
            for i in range(max_r):
                if np.isnan(band[i]):
                    band[i] = last
                else:
                    last = band[i]
            prof[:, c] = band
        residual = float(
            np.mean((prof[d[clean]] - maskable[:, :, :3][clean]) ** 2)
        )
        return prof, residual

    # Coarse pass over the whole canvas, then a fine pass around the winner.
    best = None
    for cx in range(0, 769, 32):
        for cy in range(0, 769, 32):
            prof, residual = profile_for(cx, cy)
            if best is None or residual < best[0]:
                best = (residual, cx, cy, prof)
    _, bx, by, _ = best
    for cx in range(bx - 24, bx + 25, 8):
        for cy in range(by - 24, by + 25, 8):
            prof, residual = profile_for(cx, cy)
            if residual < best[0]:
                best = (residual, cx, cy, prof)

    residual, cx, cy, prof = best
    print(f"    gradient centre ({cx}, {cy}), rms {residual ** 0.5:.3f}/255")
    return (cx, cy), prof


def paint_background(size, centre, profile):
    """The gradient, painted at `size`², framed so the central 72/108 of it is
    exactly the source artwork's canvas."""
    src_per_px = (LAYER_DP / VISIBLE_DP) * 768.0 / size
    inset = size * (LAYER_DP - VISIBLE_DP) / 2 / LAYER_DP
    yy, xx = np.mgrid[0:size, 0:size]
    sx = (xx + 0.5 - inset) * src_per_px
    sy = (yy + 0.5 - inset) * src_per_px
    d = np.clip(np.hypot(sx - centre[0], sy - centre[1]).astype(np.int32), 0, len(profile) - 1)
    rgb = profile[d]
    out = np.dstack([rgb, np.full((size, size), 255.0)])
    return Image.fromarray(np.clip(out + 0.5, 0, 255).astype(np.uint8), "RGBA")


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


def _check_fidelity(maskable, mono, centre, profile):
    """Refuse to write anything unless the split really is the artwork.

    Paints the background at the source's own scale, composites the source
    glyph back over it, and compares against `maskable.png` itself. A drift
    here means the gradient stopped being radial — a redrawn icon, most
    likely — and the honest answer is to say so rather than to ship a
    background that is *almost* the design.
    """
    yy, xx = np.mgrid[0:768, 0:768]
    d = np.clip(np.hypot(xx + 0.5 - centre[0], yy + 0.5 - centre[1]).astype(np.int32), 0, len(profile) - 1)
    bg = profile[d]
    a = mono[:, :, 3:4] / 255.0
    comp = mono[:, :, :3] * a + bg * (1 - a)
    worst = float(np.abs(comp - maskable[:, :, :3]).max())
    rms = float(np.mean((comp - maskable[:, :, :3]) ** 2)) ** 0.5
    print(f"    foreground over background reproduces the artwork: rms {rms:.3f}, worst {worst:.1f}")
    if worst > 6.0:
        sys.exit(
            f"ERROR: the fitted gradient is off by {worst:.1f}/255 at its worst pixel.\n"
            "       assets/icon/maskable.png is no longer a radial gradient with the glyph\n"
            "       on top of it, so it cannot be split into adaptive-icon layers this way.\n"
            "       Ask the designer for the two layers separately."
        )
    return np.dstack([bg, np.full((768, 768), 255.0)])


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
    maskable = _load("maskable.png")
    mono = _load("mono.png")
    whole = Image.open(SRC / "whole.png").convert("RGBA")

    print("--> fitting the gradient out of assets/icon/maskable.png")
    centre, profile = fit_gradient(maskable, mono)
    background_768 = _check_fidelity(maskable, mono, centre, profile)
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
