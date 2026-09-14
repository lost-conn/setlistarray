"""The radial gradient under `assets/icon/maskable.png`, recovered by measurement.

Imported by `scripts/make-icons.py`, which paints it behind the launcher icon's
foreground layer, and by `scripts/store-frame.py`, which paints it behind the
Play listing's screenshots and its feature graphic. It is not a script; it
writes nothing and has no `__main__`.

────────────────────────────────────────────────────────────────────────────
Why this is a module and not a colour somebody wrote down
────────────────────────────────────────────────────────────────────────────

`make-icons.py` has refused, since 2026-09-09, to name the icon's background
colour. It refits the gradient out of the artwork on every run and checks the
fit against the artwork before it writes anything, because a background that is
*nearly* the designed gradient is a bug nobody would ever look for — see that
script's own docstring for the whole argument.

Card S2 needed the same gradient behind the store screenshots, and the cheap
version of that is two hex values and a `linearGradient`: open the artwork in a
colour picker, read the middle, read the edge, type them into the compositor.
That would have been a second source of truth for the app's single most
recognisable surface, and it would have been *right* — right up until the
afternoon somebody redraws `assets/icon/maskable.png`. On that afternoon the
launcher icon changes, because `make-icons.py` measures; the store page does
not, because the compositor remembers; and the two drift apart silently, in a
place nobody re-checks, because a store listing is looked at once and then
lives for a year. That is precisely the failure `make-icons.py` already exists
to prevent, arriving through a door it was not watching.

So the measurement is lifted out here and both painters call it. The store
background is the launcher icon's actual gradient **by construction** rather
than by resemblance, and there is exactly one place where "what colour is this
app" is answered: the artwork, read at run time.

────────────────────────────────────────────────────────────────────────────
Where the centre sits when the frame is not a square
────────────────────────────────────────────────────────────────────────────

The gradient is measured on the artwork's own 768² canvas and comes back as a
centre in those pixels — (160, 120) as this is written, which is to say a light
source at 21% across and 16% down, up in the top-left. A launcher icon is a
square and `paint_background` simply reproduces that square. A Play screenshot
is 1080x1920 and a feature graphic is 1024x500, and neither of those is a
square, so somebody has to decide what "the same gradient" means on a frame
with a different shape. Left implied, that decision gets made accidentally by
whatever the first `np.mgrid` happened to do.

The decision, made for card S2 and written down here rather than discovered
later: **the artwork square is scaled to the frame's short side, and the centre
keeps its fractional position inside that square, measured from the frame's
top-left corner.** On 1080x1920 the 768px artwork becomes 1080px wide and the
light lands at (225, 169); on 1024x500 it becomes 500px and the light lands at
(104, 78). Two things follow, and both are why this rule was chosen over the
obvious alternative of centring the light in the frame:

* The fall-off is the same size next to the frame's short edge as it is next to
  the icon's, so an icon and a screenshot sitting beside each other in the Play
  listing are lit identically. Centring in the frame would stretch or squash
  the same colours over a different distance and the two would stop matching.
* The light stays in the top-left corner, which is where a caption usually
  goes, and the difference between the pale end of this gradient and the rust
  end is large enough that a caption's colour has to answer to it — which is
  why `store-frame.py` measures the band it is about to write in rather than
  being told a colour.

The profile runs out before a portrait frame does, and that is fine and
deliberate: `fit_gradient` extends its last measured band outwards (see its
comment), and the artwork's own gradient has flattened to its outer colour by
radius 600 anyway, so the bottom of a 1920px frame is the same rust the icon's
corners are. What it is *not* is invented — it is the last colour that was
actually measured, held.
"""

import sys

import numpy as np
from PIL import Image

# The artwork's own canvas, in pixels. All three files in `assets/icon/` are
# drawn on it and the fitted centre is expressed in it, so a painter working at
# any other size needs this number to scale by.
SOURCE_PX = 768

# An adaptive icon's layers are 108dp; the system shows the central 72dp and
# masks that. The whole design is scaled by 72/108 so that `maskable.png` *is*
# the visible square — `make-icons.py`'s docstring argues that at length — and
# `paint_background` below is the half of that framing which the background
# layer needs.
LAYER_DP = 108
VISIBLE_DP = 72


def fit_gradient(maskable, mono):
    """Recover the radial gradient underneath the glyph.

    Returns `(centre, profile)`: the centre in source pixels, and an array
    indexed by integer radius holding the RGB the artwork has at that distance.

    The centre is searched for rather than assumed, so that this still does the
    right thing if the artwork is redrawn — but the search is over the pixels
    the glyph and its shadow do not touch, which is what makes the answer mean
    anything. `check_fidelity` is the part that decides whether the answer was
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


def sample(width, height, centre, profile, src_per_px, origin=(0.0, 0.0)):
    """The gradient's RGB over a `width`x`height` grid, as a float array.

    The one piece of arithmetic every painter here shares, so that there is one
    place where a half-pixel offset can be wrong. A destination pixel's centre
    maps to the source coordinate `(px + 0.5 - origin) * src_per_px`, its
    distance from `centre` is rounded down to a whole source pixel, and that
    indexes the profile — clamped at the end, which is the monotone extension
    the module docstring describes.
    """
    yy, xx = np.mgrid[0:height, 0:width]
    sx = (xx + 0.5 - origin[0]) * src_per_px
    sy = (yy + 0.5 - origin[1]) * src_per_px
    d = np.clip(
        np.hypot(sx - centre[0], sy - centre[1]).astype(np.int32), 0, len(profile) - 1
    )
    return profile[d]


def paint_background(size, centre, profile):
    """The gradient, painted at `size`², framed so the central 72/108 of it is
    exactly the source artwork's canvas."""
    src_per_px = (LAYER_DP / VISIBLE_DP) * float(SOURCE_PX) / size
    inset = size * (LAYER_DP - VISIBLE_DP) / 2 / LAYER_DP
    rgb = sample(size, size, centre, profile, src_per_px, (inset, inset))
    out = np.dstack([rgb, np.full((size, size), 255.0)])
    return Image.fromarray(np.clip(out + 0.5, 0, 255).astype(np.uint8), "RGBA")


def paint_panel(width, height, centre, profile):
    """The gradient over a frame of any shape, by the rule in the module
    docstring: the artwork square scaled to the short side, the light left where
    the artwork puts it.

    This is what the store listing is painted on. It deliberately does *not* go
    through `paint_background`'s 108/72 framing — that inset exists because a
    launcher masks the outer 18dp off an adaptive icon's layers, and nothing
    masks a screenshot.
    """
    src_per_px = float(SOURCE_PX) / min(width, height)
    rgb = sample(width, height, centre, profile, src_per_px)
    return Image.fromarray(np.clip(rgb + 0.5, 0, 255).astype(np.uint8), "RGB")


def check_fidelity(maskable, mono, centre, profile):
    """Refuse to write anything unless the split really is the artwork.

    Paints the background at the source's own scale, composites the source
    glyph back over it, and compares against `maskable.png` itself. A drift
    here means the gradient stopped being radial — a redrawn icon, most
    likely — and the honest answer is to say so rather than to ship a
    background that is *almost* the design.

    Both callers run this, and the second one is the less obvious of the two:
    `store-frame.py` has no foreground layer to composite and no fidelity of
    its own at stake, but it is painting this gradient across a 1080x1920 store
    page, and "the fit stopped meaning anything" is a worse thing to discover
    there than on an icon 48dp across. Same measurement, same refusal.
    """
    yy, xx = np.mgrid[0:SOURCE_PX, 0:SOURCE_PX]
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
    return np.dstack([bg, np.full((SOURCE_PX, SOURCE_PX), 255.0)])


def load_artwork(src_dir):
    """The two source files the fit reads, as float RGBA arrays.

    Both callers want the same pair off disk in the same shape, and
    `store-frame.py` has no other reason to know that `mono.png` exists.
    """
    def one(name):
        return np.asarray(Image.open(src_dir / name).convert("RGBA")).astype(np.float64)

    return one("maskable.png"), one("mono.png")
