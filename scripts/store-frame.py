#!/usr/bin/env python3
"""Turn the raw emulator frames into the pictures Google Play is handed.

    scripts/store-frame.py .shots store/metadata/en-US/images
    scripts/store-frame.py --check store/metadata/en-US/images
    scripts/store-frame.py .shots site/img --preset site

`scripts/store-shots.sh` (card S1) leaves one raw 1080x1920 PNG per screen in
`.shots/`, plus an `insets.json` saying where the system bars were. This reads
those, together with the captions and the per-shot layout in `store/shots.json`,
and writes the finished listing:

    <out>/phoneScreenshots/1_library.png … 7_performance.png
    <out>/featureGraphic.png      1024x500, which Play requires and nothing else makes
    <out>/icon.png                512x512

Those three names are `fastlane supply`'s own, so card S4's upload step can point
at the directory and not at a list of files.

────────────────────────────────────────────────────────────────────────────
The background is the launcher icon's gradient, measured, not matched
────────────────────────────────────────────────────────────────────────────

`scripts/icon_gradient.py` fits the radial gradient out of
`assets/icon/maskable.png` on every run, and this script paints the listing on
the result. There is no colour written down here, deliberately, and that module's
docstring makes the argument: a hex copied into this file would be correct until
the artwork is redrawn and then wrong forever, in a store listing nobody
re-opens. It also settles where the light sits when the frame is not a square,
which is the one decision a portrait canvas forces and the one thing easiest to
leave implied.

────────────────────────────────────────────────────────────────────────────
Why the navigation bar is cropped and the status bar is not
────────────────────────────────────────────────────────────────────────────

The status bar is part of the picture: a screenshot with no clock at the top of
it does not read as a phone, it reads as a mockup, and the emulator's real one
is already pinned to a clean 9:30 by `store-shots.sh`'s demo mode. The
navigation bar is not part of the picture — it is three grey glyphs from another
application, and on `chart-viewer` it is worse than surplus, because that route
is `full_screen` and the app draws the last lines of the chord chart *underneath*
it. Keeping it would put a back arrow on top of a lyric.

**How far to crop is read out of `.shots/insets.json`, not written here.** That
file is measured off the device at capture time, and the reason is worth keeping
in front of whoever edits this next: `sla-shots` is 420dpi with three-button
navigation, so its bar is 48dp and 126px, and a constant `126` in this file would
be silently wrong the first time the AVD is recreated from an image that defaults
to gesture navigation — the bar is 24dp then, and every one of the seven pictures
would quietly lose 63px of the app's own bottom chrome instead. That failure
produces seven plausible-looking PNGs and is invisible until somebody compares
one against the app. This repository's habit with that shape of bug is to measure
it or test it rather than to assume it.

────────────────────────────────────────────────────────────────────────────
`--check`, and where a rejection is cheap
────────────────────────────────────────────────────────────────────────────

Play refuses assets that break its shape rules, and it refuses them at the end
of an upload — after a workflow has checked out three repositories, installed an
NDK, built the bundle, signed it with the upload key and started talking to the
API. Every rule it applies to these files is arithmetic on a PNG header, so it is
applied here instead, on every run and on demand. `--check` exists so the same
assertions can be run against a directory somebody else produced, or run twice,
without re-rendering anything.

Note that the three asset classes have *different* rules and that a single
"Play's limits" check would be wrong for at least one of them. A phone screenshot
may be 320–3840px on a side with its long side at most twice its short side; the
feature graphic must be exactly 1024x500, which is 2.048:1 and would fail the
screenshot rule it looks like it should pass; the icon must be exactly 512x512.

────────────────────────────────────────────────────────────────────────────
`--preset site`, which is the same crop and none of the rest
────────────────────────────────────────────────────────────────────────────

The landing page in `site/` wants the same screens, and wants almost nothing
this file does to them. It has its own paper behind the picture, its own prose
beside it saying what the screen is, and its own CSS to round the corners — so
the gradient, the plate and the caption are all noise there, and the caption is
the same sentence printed twice. What both presets do want is the crop, for the
reason two sections up: the navigation bar is the emulator's furniture and
belongs in neither picture.

So `--preset site` is `crop_to_content` and a resize, out to WebP. It is in
this file rather than in a script of its own because the crop is the part worth
sharing and it is the part that is subtle — a second script would sooner or
later measure the navigation bar its own way. `build_site` has the rest.
"""

import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

# A sibling file, found because Python puts a script's own directory at the
# front of `sys.path`, so this works from any working directory.
from icon_gradient import (
    LAYER_DP,
    VISIBLE_DP,
    check_fidelity,
    fit_gradient,
    load_artwork,
    paint_background,
    paint_panel,
)

ROOT = Path(__file__).resolve().parent.parent
ICON_SRC = ROOT / "assets" / "icon"
FONTS = ROOT / "assets" / "fonts"
LAYOUT_FILE = ROOT / "store" / "shots.json"

# The app's own two text colours, in light mode, read off `src/theme.rs`'s
# LIGHT_INK and LIGHT_PAPER. A caption is drawn in whichever of them reads
# better against the gradient actually behind it — see `pick_ink`.
INK = (0x1C, 0x19, 0x17)
PAPER = (0xFB, 0xF7, 0xF0)

# The app's four bundled faces are the listing's faces, so that the page and the
# app read as one object rather than as an app and an advertisement for it.
# These are variable fonts and every axis is set explicitly below: the default
# instance of `Newsreader[opsz,wght]` is 400 weight at 18pt optical size, which
# is a body face, and a caption set at 58px in a body optical size looks like
# body text that has been enlarged — because that is exactly what it is.
DISPLAY = "Newsreader[opsz,wght].ttf"

# Play's numbers, all three sets of them.
SHOT_MIN_SIDE, SHOT_MAX_SIDE = 320, 3840
SHOT_MAX_RATIO = 2.0
SHOT_MIN_COUNT, SHOT_MAX_COUNT = 2, 8
FEATURE_SIZE = (1024, 500)
ICON_SIZE = (512, 512)
MAX_BYTES = 8 * 1024 * 1024

# The listing's own margins, in canvas pixels. Not in `shots.json`, because a
# margin that differs between two screenshots on the same store page is a
# mistake rather than a choice; what varies per shot is where the plate sits
# inside them, and that is what `plate_offset` is for.
MARGIN = 72
CAPTION_GAP = 52
CAPTION_SIZE = 58
CAPTION_WEIGHT = 500
CAPTION_OPSZ = 72
CAPTION_LEADING = 1.22

# The landing page's numbers, which are not Play's and are not in shots.json.
# `site/style.css` shows a plate in a 300px slot, so 600 is the two-times
# asset; the aspect is the captured 1080x1920 frame with the emulator's 126px
# navigation bar taken off it. The page reserves exactly this shape before an
# image arrives, so `build_site` refuses a capture that is not it rather than
# letting the page load crooked. Change either number and change the
# `aspect-ratio` on `.plate img` with it — `src/site.rs` is the test that
# notices when only one of them moves.
SITE_PLATE_WIDTH = 600
SITE_PLATE_ASPECT = (1080, 1794)
# Shown at 38px in the masthead, so twice that, and large enough to be the
# favicon a browser scales down for a tab.
SITE_ICON_SIZE = 96

RED = "\033[31m"
GREEN = "\033[32m"
RESET = "\033[0m"


def die(message):
    sys.exit(f"{RED}✗ {message}{RESET}")


def ok(message):
    print(f"{GREEN}✓{RESET} {message}")


# ---------------------------------------------------------------------------
# Type
# ---------------------------------------------------------------------------


def face(filename, size, weight, optical=None):
    """One instance of a variable font, with every axis stated.

    Pillow's `set_variation_by_axes` takes the axes in the order the font
    declares them, which for `Newsreader[opsz,wght]` is weight first and optical
    size second — the order of the file's *name* is the opposite, and reading
    the name is how you get a 200-weight caption at 72pt and wonder why it went
    thin.
    """
    font = ImageFont.truetype(str(FONTS / filename), size)
    axes = [weight] if optical is None else [weight, optical]
    font.set_variation_by_axes(axes)
    return font


def wrap(draw, text, font, max_width):
    """Break `text` into the fewest lines that each fit `max_width`, evenly.

    Two passes, and the second one is the whole point. A plain greedy wrap
    fills each line to the measure and puts whatever is left on the last one,
    which is fine for a paragraph and bad for a two-line heading: the caption
    `You remember the verse, not the title` came out as thirty-one characters
    over `title`, a single orphaned word under a full line, which reads as a
    mistake at the size these are set.

    So: greedy once to learn how many lines the caption needs, then find the
    narrowest measure that still produces that many lines and greedy again at
    that width. Squeezing the measure forces the break earlier and the lines
    even out, and because the line count is held fixed it can never cost a
    line. A binary search over integer widths settles it in a dozen passes
    over a sentence of eight words.

    This is deliberately not the hyphenating, penalty-minimising line breaker
    an earlier draft of this comment argued against, and that argument still
    holds — that would be a great deal of machinery for eight lines of prose.
    Balancing at a line count you already know is arithmetic.
    """
    lines = _greedy(draw, text, font, max_width)
    if len(lines) < 2:
        return lines
    lo, hi, best = 1, int(max_width), lines
    while lo <= hi:
        mid = (lo + hi) // 2
        trial = _greedy(draw, text, font, mid)
        if len(trial) <= len(lines):
            best, hi = trial, mid - 1
        else:
            lo = mid + 1
    return best


def _greedy(draw, text, font, max_width):
    """Fill each line to `max_width` and start a new one when the next word
    will not fit. A word wider than the measure gets a line of its own and
    overhangs it, which is the only sane thing to do without hyphenation."""
    words, lines, current = text.split(), [], ""
    for word in words:
        trial = f"{current} {word}".strip()
        if current and draw.textlength(trial, font=font) > max_width:
            lines.append(current)
            current = word
        else:
            current = trial
    if current:
        lines.append(current)
    return lines


def _relative_luminance(rgb):
    """WCAG's, so that `pick_ink` below is choosing by the same measure the rest
    of this project argues contrast in (`theme::contrast_ratio`)."""
    c = np.asarray(rgb, dtype=np.float64) / 255.0
    lin = np.where(c <= 0.03928, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)
    return float(lin[0] * 0.2126 + lin[1] * 0.7152 + lin[2] * 0.0722)


def contrast(a, b):
    la, lb = _relative_luminance(a), _relative_luminance(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def pick_ink(canvas, band):
    """Whichever of the app's two text colours reads against what is actually
    behind the caption.

    This could have been a per-shot field in `shots.json` and should not be.
    The gradient runs from a pale terracotta at the top-left to a deep rust
    everywhere else, which means a caption above the plate wants the near-black
    and a caption below it wants the cream — and which of those a given shot
    gets is not a matter of taste, it is a matter of where its plate sits. A
    field would be a second place to remember when `plate_offset` moves by 120px
    and a caption that was on the light end stops being. Measuring the band
    cannot be forgotten, and it keeps working the day the artwork is redrawn in
    a different colour.
    """
    patch = np.asarray(canvas.convert("RGB")).astype(np.float64)
    left, top, right, bottom = band
    mean = patch[max(top, 0):max(bottom, 1), max(left, 0):max(right, 1)].reshape(-1, 3).mean(axis=0)
    return max((INK, PAPER), key=lambda ink: contrast(ink, mean))


# ---------------------------------------------------------------------------
# The plate
# ---------------------------------------------------------------------------


def crop_to_content(frame, insets):
    """The captured frame with the navigation bar taken off it.

    The bar's own rectangle comes out of `insets.json`. Only the case that
    actually arises is handled — a bar spanning the full width along the bottom
    edge, which is every phone in portrait — and anything else stops the run
    rather than being cropped by a guess. A side navigation bar would mean this
    was pointed at a tablet or a landscape capture, and in either case the rest
    of the layout below is wrong too.
    """
    panel_w, panel_h = frame.size
    bar = insets["navigation_bar"]
    if bar["height"] <= 0:
        # Nothing to crop, which is a legitimate answer: a device in gesture
        # navigation with the bar hidden outright reports an empty frame.
        return frame
    if not (bar["left"] == 0 and bar["right"] == panel_w and bar["bottom"] == panel_h):
        die(
            f"the navigation bar in insets.json is at {bar}, which is not a full-width bar along "
            f"the bottom of a {panel_w}x{panel_h} frame — this compositor's whole layout assumes "
            "a phone in portrait"
        )
    return frame.crop((0, 0, panel_w, panel_h - bar["height"]))


def rounded_mask(size, radius, supersample=4):
    """An antialiased rounded-rectangle mask.

    Drawn at four times the size and scaled down, because `ImageDraw.
    rounded_rectangle` is not antialiased and a hard-edged corner on a 780px
    plate is visible as a set of steps at the size Play shows these at.
    """
    w, h = size
    big = Image.new("L", (w * supersample, h * supersample), 0)
    ImageDraw.Draw(big).rounded_rectangle(
        (0, 0, w * supersample - 1, h * supersample - 1),
        radius=radius * supersample,
        fill=255,
    )
    return big.resize((w, h), Image.LANCZOS)


def cast_shadow(canvas, rect, radius, blur, offset, opacity):
    """A real gaussian shadow under the plate.

    Not a stack of translucent rectangles and not a pre-baked PNG: the plate's
    corner radius and the shadow's blur have to agree, and the only way to keep
    them agreeing when either is edited in `shots.json` is to blur the actual
    silhouette. The layer is built canvas-sized so the blur has somewhere to
    spread into rather than being clipped at the shadow's own edge.
    """
    x, y, w, h = rect
    layer = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    silhouette = Image.new("RGBA", (w, h), (0, 0, 0, round(255 * opacity)))
    layer.paste(silhouette, (x + offset[0], y + offset[1]), rounded_mask((w, h), radius))
    return Image.alpha_composite(canvas, layer.filter(ImageFilter.GaussianBlur(blur)))


# ---------------------------------------------------------------------------
# The three kinds of picture
# ---------------------------------------------------------------------------


def render_shot(frame, caption, layout, canvas_size, centre, profile):
    """One finished screenshot: gradient, plate, shadow, caption."""
    width, height = canvas_size
    canvas = paint_panel(width, height, centre, profile).convert("RGBA")

    plate = crop_to_content(frame, layout["insets"])
    plate_w = round(layout["plate_scale"] * width)
    plate_h = round(plate_w * plate.size[1] / plate.size[0])
    plate_x = (width - plate_w) // 2 + layout["plate_offset"][0]
    plate_y = (height - plate_h) // 2 + layout["plate_offset"][1]
    plate = plate.resize((plate_w, plate_h), Image.LANCZOS).convert("RGBA")

    canvas = cast_shadow(
        canvas,
        (plate_x, plate_y, plate_w, plate_h),
        layout["corner_radius"],
        layout["shadow_blur"],
        layout["shadow_offset"],
        layout["shadow_opacity"],
    )
    canvas.paste(plate, (plate_x, plate_y), rounded_mask((plate_w, plate_h), layout["corner_radius"]))

    position = layout["caption_position"]
    if position != "none":
        if position == "above":
            band = (MARGIN, MARGIN, width - MARGIN, plate_y - CAPTION_GAP)
        elif position == "below":
            band = (MARGIN, plate_y + plate_h + CAPTION_GAP, width - MARGIN, height - MARGIN)
        else:
            die(f"caption_position {position!r} is not one of above, below, none")
        draw_caption(canvas, caption, band, layout["type_scale"])

    return canvas.convert("RGB")


def draw_caption(canvas, text, band, type_scale):
    """The caption, wrapped, centred in `band`, in the app's display face.

    Vertically centred rather than pinned to the top or the bottom of the band,
    so that a one-line caption and a two-line caption both sit in the same
    optical place relative to the plate. Pinning would move the gap between the
    words and the phone by a whole line when a caption is edited, which is a
    thing that happens to captions.
    """
    left, top, right, bottom = band
    if bottom - top < 40:
        die(
            f"the caption band is only {bottom - top}px tall; the plate has been offset into the "
            "space its own caption needs. Move plate_offset, or shrink plate_scale, in "
            "store/shots.json"
        )

    draw = ImageDraw.Draw(canvas)
    size = round(CAPTION_SIZE * type_scale)
    font = face(DISPLAY, size, CAPTION_WEIGHT, CAPTION_OPSZ)
    lines = wrap(draw, text, font, right - left)
    leading = round(size * CAPTION_LEADING)
    block = leading * len(lines)
    if block > bottom - top:
        die(
            f"the caption {text!r} needs {block}px on {len(lines)} lines and its band is only "
            f"{bottom - top}px tall. Shorten it, drop its type_scale, or move the plate in "
            "store/shots.json"
        )

    ink = pick_ink(canvas, band)
    y = (top + bottom) // 2 - block // 2 + leading // 2
    for line in lines:
        draw.text(((left + right) // 2, y), line, font=font, fill=ink, anchor="mm")
        y += leading


def render_feature_graphic(wordmark, type_scale, centre, profile):
    """1024x500: the gradient and the wordmark, and nothing else.

    Play shows this small, cropped, and sometimes with a play button over the
    middle of it, so everything that is not the name is a liability. The name is
    set in the display face at the same weight the app's own screen titles use.
    """
    width, height = FEATURE_SIZE
    canvas = paint_panel(width, height, centre, profile).convert("RGBA")
    draw = ImageDraw.Draw(canvas)

    size = round(height * 0.24 * type_scale)
    font = face(DISPLAY, size, CAPTION_WEIGHT, CAPTION_OPSZ)
    band = (0, height // 2 - size, width, height // 2 + size)
    draw.text((width // 2, height // 2), wordmark, font=font, fill=pick_ink(canvas, band), anchor="mm")
    return canvas.convert("RGB")


def render_icon(centre, profile):
    """512x512, which is the adaptive launcher icon flattened.

    The background is painted with the same 108/72 framing `make-icons.py` uses,
    which means the artwork's own 768² canvas lands as the central 72dp square
    and the rest is gradient bleed — so pasting `maskable.png` back into that
    square reproduces exactly what a launcher composites, at the size Play wants
    it. Written with no alpha: Play masks this itself, and an icon that arrives
    with its own transparent corners gets them masked twice.
    """
    size = ICON_SIZE[0]
    canvas = paint_background(size, centre, profile).convert("RGBA")
    inner = round(size * VISIBLE_DP / LAYER_DP)
    artwork = Image.open(ICON_SRC / "maskable.png").convert("RGBA").resize((inner, inner), Image.LANCZOS)
    offset = (size - inner) // 2
    canvas.paste(artwork, (offset, offset), artwork)
    return canvas.convert("RGB")


# ---------------------------------------------------------------------------
# Play's rules
# ---------------------------------------------------------------------------


def png_size(path):
    """A PNG's dimensions out of its own IHDR, and a refusal if it is not one.

    Read by hand rather than by opening it, because "is this actually a PNG" is
    one of the things being checked and `Image.open` would happily answer the
    question for a JPEG somebody renamed.
    """
    with path.open("rb") as handle:
        header = handle.read(24)
    if header[:8] != b"\x89PNG\r\n\x1a\n":
        die(f"{path} is not a PNG, and Play takes PNG or JPEG for these — not a renamed one")
    return int.from_bytes(header[16:20], "big"), int.from_bytes(header[20:24], "big")


def check(out_dir):
    """Every rule Play applies to these three asset classes, applied here."""
    shots_dir = out_dir / "phoneScreenshots"
    if not shots_dir.is_dir():
        die(f"{shots_dir} does not exist; there is nothing to check")
    shots = sorted(shots_dir.glob("*.png"))

    if not SHOT_MIN_COUNT <= len(shots) <= SHOT_MAX_COUNT:
        die(
            f"{len(shots)} phone screenshot(s) in {shots_dir}; Play takes between "
            f"{SHOT_MIN_COUNT} and {SHOT_MAX_COUNT} and publishes none of them if there are more"
        )

    for path in shots:
        width, height = png_size(path)
        short, long = min(width, height), max(width, height)
        if not SHOT_MIN_SIDE <= short or not long <= SHOT_MAX_SIDE:
            die(
                f"{path.name} is {width}x{height}; Play wants every side between "
                f"{SHOT_MIN_SIDE} and {SHOT_MAX_SIDE}px"
            )
        if long > short * SHOT_MAX_RATIO:
            die(
                f"{path.name} is {width}x{height}, a ratio of {long / short:.3f}:1; Play refuses "
                f"a phone screenshot longer than {SHOT_MAX_RATIO:g}:1"
            )
        _check_bytes(path)
        ok(f"{path.relative_to(out_dir)}  {width}x{height}  {path.stat().st_size / 1024:.0f} KiB")

    for name, want in (("featureGraphic.png", FEATURE_SIZE), ("icon.png", ICON_SIZE)):
        path = out_dir / name
        if not path.is_file():
            die(f"{path} is missing; Play requires it and refuses the listing without it")
        size = png_size(path)
        if size != want:
            die(f"{name} is {size[0]}x{size[1]}; Play requires exactly {want[0]}x{want[1]}")
        _check_bytes(path)
        ok(f"{name}  {size[0]}x{size[1]}  {path.stat().st_size / 1024:.0f} KiB")

    ok(f"{len(shots)} screenshot(s), a feature graphic and an icon, all inside Play's limits")


def _check_bytes(path):
    size = path.stat().st_size
    if size > MAX_BYTES:
        die(f"{path.name} is {size / 1024 / 1024:.1f} MB; Play's limit for one image is 8 MB")


# ---------------------------------------------------------------------------
# Driving it
# ---------------------------------------------------------------------------


def build(raw_dir, out_dir):
    layout_file = json.loads(LAYOUT_FILE.read_text())
    canvas_size = (layout_file["canvas"]["width"], layout_file["canvas"]["height"])
    defaults = layout_file["defaults"]
    shots = layout_file["shots"]

    insets_path = raw_dir / "insets.json"
    if not insets_path.is_file():
        die(
            f"{insets_path} is missing. The captures in {raw_dir} predate the insets "
            "scripts/store-shots.sh now records, and this compositor crops by them rather than "
            "by a constant — re-run scripts/store-shots.sh"
        )
    insets = json.loads(insets_path.read_text())

    print("--> fitting the gradient out of assets/icon/maskable.png")
    maskable, mono = load_artwork(ICON_SRC)
    centre, profile = fit_gradient(maskable, mono)
    check_fidelity(maskable, mono, centre, profile)

    shots_out = out_dir / "phoneScreenshots"
    shots_out.mkdir(parents=True, exist_ok=True)

    # Numbered in the order `src/shots.rs` walks them, because that order is a
    # sentence — here is your book, here is one song in it, here is the chart —
    # and `fastlane supply` uploads a directory in filename order. A dict in
    # Python 3.7 and later keeps the order it was written in, so the order of
    # `shots.json` is the order of the listing, which is also the order somebody
    # editing the file will assume it is.
    for index, (shot_id, entry) in enumerate(shots.items(), start=1):
        raw = raw_dir / f"{shot_id}.png"
        if not raw.is_file():
            die(f"{raw} is missing; run scripts/store-shots.sh first")
        layout = dict(defaults)
        layout.update({k: v for k, v in entry.items() if k != "caption"})
        layout["insets"] = insets
        picture = render_shot(
            Image.open(raw).convert("RGB"), entry["caption"], layout, canvas_size, centre, profile
        )
        path = shots_out / f"{index}_{shot_id}.png"
        picture.save(path, optimize=True)
        print(f"    {path.relative_to(out_dir)}  “{entry['caption']}”")

    feature = layout_file["feature_graphic"]
    render_feature_graphic(feature["wordmark"], feature["type_scale"], centre, profile).save(
        out_dir / "featureGraphic.png", optimize=True
    )
    render_icon(centre, profile).save(out_dir / "icon.png", optimize=True)
    print("    featureGraphic.png, icon.png")


def build_site(raw_dir, out_dir):
    """The same frames, for the landing page instead of for Play.

    Everything `render_shot` does above is for a picture that will be looked at
    as a thumbnail in a store, with no page around it: the gradient gives it a
    background, the plate gives it an edge, and the caption says what the
    screen is because nothing else on that surface will. The landing page has a
    page around it. It has its own paper under the picture, its own prose
    beside it saying what the screen is, and its own CSS to round the corners —
    so all three of those become noise, and the caption in particular becomes
    the same sentence twice.

    What is left is the crop, which is the one thing both presets want: the
    navigation bar is the emulator's furniture rather than the app's, and it
    belongs in neither picture.

    WebP, opaque, at twice the 300px slot the page shows them in. Opaque
    because the corners are rounded in CSS rather than masked here, and an
    alpha channel purchased for four corners costs about half the file again.
    """
    insets_path = raw_dir / "insets.json"
    if not insets_path.is_file():
        die(
            f"{insets_path} is missing. The captures in {raw_dir} predate the insets "
            "scripts/store-shots.sh now records, and this compositor crops by them rather than "
            "by a constant — re-run scripts/store-shots.sh"
        )
    insets = json.loads(insets_path.read_text())

    shot_ids = list(json.loads(LAYOUT_FILE.read_text())["shots"])
    out_dir.mkdir(parents=True, exist_ok=True)

    for shot_id in shot_ids:
        raw = raw_dir / f"{shot_id}.png"
        if not raw.is_file():
            die(f"{raw} is missing; run scripts/store-shots.sh first")
        plate = crop_to_content(Image.open(raw).convert("RGB"), insets)

        # The page cannot be shipped the pictures — they are built by the deploy
        # and never committed, the way the listing's are not (see .gitignore) —
        # so `site/style.css` declares the shape it is going to reserve for one
        # and the browser holds that space open until the file arrives. That
        # only works while every capture really is this shape. A phone in
        # gesture navigation, a tablet, or a landscape capture would each land
        # here with a different one, and the failure it would cause on the page
        # is every section below the fold jumping when an image loads — which
        # is exactly the kind of fault nobody sees on the machine that built it.
        if plate.size != (SITE_PLATE_ASPECT[0], SITE_PLATE_ASPECT[1]):
            die(
                f"{raw.name} crops to {plate.size[0]}x{plate.size[1]}, and site/style.css "
                f"reserves {SITE_PLATE_ASPECT[0]}x{SITE_PLATE_ASPECT[1]} for it. Either this "
                "capture came off a device the page has never been shaped for, or the shape "
                "changed and `.plate img`'s aspect-ratio has to change with it"
            )

        height = round(SITE_PLATE_WIDTH * plate.size[1] / plate.size[0])
        path = out_dir / f"{shot_id}.webp"
        plate.resize((SITE_PLATE_WIDTH, height), Image.LANCZOS).save(
            path, "WEBP", quality=82, method=6
        )
        print(f"    {path.name}  {SITE_PLATE_WIDTH}x{height}  {path.stat().st_size:,} B")

    # The masthead's mark and the tab's favicon, which are the same file. PNG
    # rather than WebP, because this one is also what a browser is handed for
    # `rel="icon"` and that is the one place on the page where the older format
    # is still the safer answer. It comes out of here rather than being
    # committed for no better reason than that everything else in site/img/
    # does — one directory, one rule about how it is filled, and nothing in it
    # that has to be remembered separately.
    icon = Image.open(ICON_SRC / "whole.png").convert("RGBA")
    icon.resize((SITE_ICON_SIZE, SITE_ICON_SIZE), Image.LANCZOS).save(
        out_dir / "icon.png", optimize=True
    )
    print(f"    icon.png  {SITE_ICON_SIZE}x{SITE_ICON_SIZE}")


def main():
    parser = argparse.ArgumentParser(
        description="Composite the Play listing's pictures out of the raw emulator frames.",
    )
    parser.add_argument("raw_dir", nargs="?", help="where scripts/store-shots.sh left its PNGs")
    parser.add_argument("out_dir", help="where the finished pictures go")
    parser.add_argument(
        "--preset",
        choices=("store", "site"),
        default="store",
        help="'store' composites Play's nine pictures; 'site' emits the landing page's "
        "caption-free plates instead (see build_site)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="only re-check an existing out_dir against Play's limits, rendering nothing",
    )
    args = parser.parse_args()

    out_dir = Path(args.out_dir)
    if args.preset == "site":
        # Play's limits are Play's. Nothing the landing page emits is uploaded
        # anywhere, and `check` would fail it on the picture count alone.
        if args.check:
            parser.error("--check is about Play's limits and has nothing to say about --preset site")
        if args.raw_dir is None:
            parser.error("a raw directory is required for --preset site")
        build_site(Path(args.raw_dir), out_dir)
        return

    if args.check and args.raw_dir is None:
        check(out_dir)
        return
    if args.raw_dir is None:
        parser.error("a raw directory is required unless --check is given on its own")

    build(Path(args.raw_dir), out_dir)
    # Always, not only under `--check`. The whole argument for these assertions
    # is that a rejection costs a signed upload, and a check somebody has to
    # remember to run is a check that is not run on the afternoon it matters.
    check(out_dir)


if __name__ == "__main__":
    main()
