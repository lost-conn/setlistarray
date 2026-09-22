# Somebody else's artwork

One file, and it is the only thing in `site/` that this project did not make.

| File | What | Where it came from |
| --- | --- | --- |
| `google-play-badge.svg` | The English "Get it on Google Play" badge | Google's Partner Marketing Hub, [Lockups, icons & badges](https://partnermarketinghub.withgoogle.com), downloaded 2026-09-22 |

## Why it is here and not in `site/img/`

`site/img/` is gitignored, because everything in it is rebuilt by
`.github/workflows/site.yml` out of things this repository already has. This
badge is not derived from anything — it is Google's artwork, committed, and the
deploy would have no way to produce it. So it lives in a directory of its own
whose rule is the opposite one: `img/` is built, `brand/` is kept.

## Why it is served from here at all

The page could link Google's copy and save five kilobytes. It does not, for the
same reason the fonts are subset and self-hosted rather than fetched from Google
Fonts: the receipt at the foot of `site/index.html` says the page makes no
third-party requests, and a hotlinked badge would quietly make that false —
and would tell Google about every visitor into the bargain, on a page whose
whole argument is that this app tells nobody anything.
`the_site_asks_nothing_of_anybody_else` in `src/site.rs` is what stops it
drifting back.

## What Google asks, and what the page does about it

The badge is used unmodified — not recoloured, not rearranged, not rescaled in
its parts, not cropped. Beyond that there are three rules with numbers in them:

- **Minimum height of 28px.** The page sets `--badge-h: 52px`.
- **Clear space of one quarter of the badge's height on every side.** That is
  13px at this size. `site/style.css` gives the badge that much to its right
  and below; above it, the standfirst's 34px bottom margin provides it, and to
  its left the page's own 22px gutter does.
- **At least as large as any other app-store badge beside it.** There is no
  other badge on this page, and if one is ever added this is the rule to come
  back and read.

An SVG rather than a PNG, deliberately: "don't use low-resolution badges" stops
being something anybody has to check when the artwork has no resolution.

Google's guidelines also say not to use outdated badge artwork and to take the
most recent version from the Partner Marketing Hub, which is where this came
from. The hub only offers the whole 141 MB asset bundle as a single download;
this is the one file out of it that the page uses. The badge served at
`play.google.com/intl/en_us/badges/static/…` is a different, older cut of the
same artwork — close enough to look right, which is exactly why the source is
written down here.
