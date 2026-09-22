#!/usr/bin/env python3
"""Cut the app's four faces down to what the landing page needs.

    python3 -m venv .venv-fonts
    .venv-fonts/bin/pip install fonttools brotli
    .venv-fonts/bin/python scripts/site-fonts.py site/fonts

`assets/fonts/` holds the faces the app ships, at full size, because a phone
downloads them once at install and then has them forever. A web page does not:
a visitor pays for them on the way in, and the four files as they stand are
1.35 MB — thirty times the rest of the page. So the page is served subsets, and
this is what cuts them.

The licences come along. `site/fonts/licenses/` is a copy of
`assets/fonts/licenses/`, and that is not politeness: the SIL Open Font License
requires its text to travel with any modified copy of the font, and a subset is
exactly that. The colophon at the foot of `site/index.html` links to the
directory this writes.

────────────────────────────────────────────────────────────────────────────
Two slices per face, and why not one
────────────────────────────────────────────────────────────────────────────

Each face is cut twice. **base** is ASCII plus the punctuation English
typography actually uses — the curly quotes, the dashes, the ellipsis, the
non-breaking space. **ext** is the rest of Latin-1 and the rest of the general
punctuation block: the accented letters, the daggers, the things nobody plans
to use and somebody eventually does.

Both are declared in `site/style.css` with a `unicode-range`, so the browser
fetches `ext` only if a character in it appears on the page. Today none does,
and a visitor downloads 194 KB rather than 316 KB. Write an artist's name with
an acute accent in it next year and the accented file is fetched, by that
visitor, for that page, without anybody having to remember this file exists.

The single-slice alternatives are both worse, and it is worth writing down why
because each looks reasonable on its own:

*One generous subset* means every visitor pays 122 KB for glyphs the page does
not use, forever, against the chance that it one day might.

*One tight subset, cut from the characters on the page*, is the trap. The first
copy edit introducing a character the last run had not seen renders as a blank
box, in a face that otherwise looks perfectly fine, on a page nobody rebuilds
after a one-word change. That failure has already happened twice in this
project from the other direction — cards K13 and K21, a Unicode glyph in the
app's own prose coming out as tofu on a phone — and card K26's conclusion was
that a missing glyph is not a thing to notice, it is a thing to make
impossible. The split makes it impossible without charging for it.

The two `unicode-range` declarations overlap, deliberately. `ext` claims the
whole of Latin-1 and the whole of general punctuation, `base` claims its own
narrower set, and `base` is declared *after* it — where several faces match a
character, the last one declared wins, so the overlap resolves to `base` and
only `base` is fetched. That is how Google Fonts serves the same split, and the
reason the ranges here can stay two short strings instead of one long
subtraction.

────────────────────────────────────────────────────────────────────────────
The axes have to survive
────────────────────────────────────────────────────────────────────────────

Three of the four are variable fonts, and Newsreader carries two axes rather
than one: `wght` and `opsz`. Optical size is not a nicety here — the page sets
the wordmark and the display questions large, and `font-optical-sizing: auto`
in `site/style.css` is what makes those a display cut rather than body text
enlarged. `face()` in scripts/store-frame.py has the same note about the same
font for the same reason.

`pyftsubset` will instance a variable font down to a single static weight if
asked, and several of its flag combinations amount to asking. So the axes are
asserted afterwards, out of the written file's own `fvar` table, rather than
assumed from the flags that went in.

────────────────────────────────────────────────────────────────────────────
Which OpenType features are kept
────────────────────────────────────────────────────────────────────────────

`layout_features = ["*"]` is fontTools' keep-everything, and for a serif with a
full feature set it is 10% of the file in tables the page cannot reach: small
capitals, oldstyle figures, stylistic alternates, swashes. What a browser
actually applies to ordinary prose is kerning, the standard ligatures,
contextual alternates and the mark-attachment features a diacritic needs. Those
are the list below, and it is a list rather than a wildcard so that adding one
is a decision somebody writes down.
"""

import argparse
import shutil
import sys
from pathlib import Path

try:
    from fontTools.subset import Options, Subsetter
    from fontTools.ttLib import TTFont
except ImportError:  # pragma: no cover - the message is the whole point
    sys.exit(
        "\033[31m✗ fontTools is not importable. Most distributions now mark the system "
        "Python as externally managed (PEP 668) and refuse `pip install --user`; the "
        "venv lines at the top of this file are the way in.\033[0m"
    )

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets" / "fonts"
LICENSES = SRC / "licenses"

RED = "\033[31m"
GREEN = "\033[32m"
RESET = "\033[0m"

# The two slices, in CSS `unicode-range` syntax. These strings are what
# `site/style.css` declares, character for character, and
# `the_site_serves_the_slices_the_subsetter_cuts` in src/site.rs is what fails
# the build when only one of the two files is edited. `ext` is listed first
# here because that is the order the stylesheet has to declare them in — see
# the overlap note in the docstring.
EXT_RANGE = "U+A0-FF,U+2000-206F,U+2212"
BASE_RANGE = "U+0-7F,U+A0,U+2013-2014,U+2018-2019,U+201C-201D,U+2022,U+2026"
SLICES = (("ext", EXT_RANGE), ("base", BASE_RANGE))

# Kerning, the standard and contextual ligatures, glyph composition, the
# language-specific forms a locale may ask for, and mark attachment. See the
# last section of the docstring for what is deliberately not here.
FEATURES = ["kern", "liga", "clig", "calt", "ccmp", "locl", "mark", "mkmk", "rlig"]

# Source file, the stem its slices are written under, and the variation axes
# the result must still have. An empty set means the face is not variable and
# must not acquire an `fvar` table on the way through, which would mean the
# wrong file was read.
FACES = (
    ("Newsreader[opsz,wght].ttf", "newsreader", frozenset({"opsz", "wght"})),
    ("Newsreader-Italic[opsz,wght].ttf", "newsreader-italic", frozenset({"opsz", "wght"})),
    ("Karla[wght].ttf", "karla", frozenset({"wght"})),
    ("DejaVuSansMono.ttf", "dejavu-mono", frozenset()),
)


def die(message):
    sys.exit(f"{RED}✗ {message}{RESET}")


def codepoints(spec):
    """A CSS `unicode-range` string as a set of integers."""
    out = set()
    for part in spec.split(","):
        part = part.strip().removeprefix("U+")
        if "-" in part:
            lo, hi = part.split("-")
            out.update(range(int(lo, 16), int(hi, 16) + 1))
        else:
            out.add(int(part, 16))
    return out


def axes_of(path):
    """The variation axis tags in a font file, as a set.

    A set rather than a tuple because the declared order is not the filename's
    order and is not worth asserting either way: `Newsreader[opsz,wght].ttf`
    declares `wght` first, which `face()` in scripts/store-frame.py ran into
    from the other direction — Pillow takes axis values in declaration order,
    so that file has to know it. All that matters here is that both are present.
    """
    with TTFont(path) as font:
        if "fvar" not in font:
            return frozenset()
        return frozenset(axis.axisTag for axis in font["fvar"].axes)


def cut(src, dest, wanted, expected_axes):
    options = Options()
    options.flavor = "woff2"
    options.layout_features = FEATURES

    font = TTFont(src)
    subsetter = Subsetter(options=options)
    subsetter.populate(unicodes=wanted)
    subsetter.subset(font)
    font.flavor = "woff2"
    font.save(dest)
    font.close()

    got = axes_of(dest)
    if got != expected_axes:
        die(
            f"{dest.name} came out with variation axes {sorted(got) or '(none)'} and the page "
            f"needs {sorted(expected_axes) or '(none)'}. A subset that has dropped an axis "
            "renders as one static weight, which looks very nearly right — which is why this "
            "is asserted rather than trusted. See 'The axes have to survive' above"
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("out_dir", help="where the subset faces go, normally site/fonts")
    args = parser.parse_args()

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    # `ext` is only ever fetched by a visitor whose page needs a character in
    # it, so the two totals are reported apart rather than added up. The first
    # is what the page costs today.
    totals = {name: 0 for name, _ in SLICES}
    source_bytes = 0

    for filename, stem, expected_axes in FACES:
        src = SRC / filename
        if not src.is_file():
            die(f"{src} is missing; this script cuts the faces the app itself ships")
        source_bytes += src.stat().st_size

        sizes = []
        for slice_name, spec in SLICES:
            dest = out_dir / f"{stem}-{slice_name}.woff2"
            cut(src, dest, codepoints(spec), expected_axes)
            totals[slice_name] += dest.stat().st_size
            sizes.append(f"{slice_name} {dest.stat().st_size:>7,} B")
        print(f"    {stem:20} {src.stat().st_size:>9,} B -> " + "   ".join(sizes))

    shutil.copytree(LICENSES, out_dir / "licenses", dirs_exist_ok=True)
    print(f"    {'licenses/':20} {len(list(LICENSES.iterdir()))} files, as the OFL asks")
    print(
        f"{GREEN}✓{RESET} {source_bytes:,} B of source faces -> {totals['base']:,} B "
        f"fetched by every visitor, {totals['ext']:,} B fetched only if the page ever "
        "uses a character in it"
    )


if __name__ == "__main__":
    main()
