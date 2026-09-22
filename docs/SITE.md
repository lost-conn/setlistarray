# The landing page

`site/` is the page at **setlistarray.lostconnection.dev**: one hand-written
HTML file, one stylesheet, `brand/` — the one piece of artwork here that this
project did not make — and two directories that are built rather than
committed. `.github/workflows/site.yml` puts it on GitHub Pages.

This document is the argument behind it — why it is not what the card asked
for, what the page is made of, and which tests hold it to the app.

---

## Why this is not built in rinch

The card that asked for a website asked for something larger:

> The app needs a landing page. Might as well build this in rinch and make the
> whole app have web/PWA support.

Those are two jobs and only one of them is possible today.

**The landing page is static HTML because that is what a landing page is.**
`rinch-web` is real — a browser-native DOM backend, not a canvas shim, and it
would render this page. What it would also do is hand a stranger a blank screen
until a multi-megabyte WebAssembly bundle arrived, and hand a search engine
nothing at all. The one page whose entire job is to load instantly for somebody
who has never heard of this app is the last page in the world to put a GUI
framework underneath. It is 30 KB of markup and CSS and it should stay that
way.

**The whole app on the web is blocked below the framework, in the database.**
This was checked rather than assumed, and it is worth writing down so that the
next person to have the idea gets the measurement instead of repeating it:

- `rhypedb-storage` is a log-structured merge tree built on `memmap2`,
  `std::fs` and a `cfg(unix)` dependency on `libc`. There is no `wasm` anywhere
  in that workspace's `Cargo.toml` files. It does not compile to
  `wasm32-unknown-unknown`, and making it would be a storage-backend rewrite
  onto IndexedDB or OPFS — in another repository, not this one.
- **CORS ends offline capture regardless.** `rinch-http` already has a `fetch`
  path on wasm, so the transport is not the problem; the problem is that a
  browser will not let this origin fetch a chord site, and no amount of work
  here changes that. The headline feature of the app is the one that does not
  survive the port.
- Everything else would be fine. `hayro`, `vello_cpu`, `png` and `zip` are pure
  Rust. The file picker would want a web implementation over the File System
  Access API. That is the easy half, and it is the half that looks like the
  whole job until somebody checks the other one.

So the page ships and the port does not. That is a separate card with the
findings above attached to it.

---

## What the page is made of

**The colours and the faces are the app's, not the page's.** Every hex in
`site/style.css` is copied out of `src/theme.rs` — `LIGHT_NEUTRALS`,
`DARK_NEUTRALS` and the `RUST` accent — under the app's own `--sla-*` custom
property names, so that the two share a vocabulary rather than a resemblance. A
screenshot dropped onto this page sits on the paper it was photographed on. The
page follows `prefers-color-scheme` because the app follows the system.

Only Rust is checked and only Rust is used. The app lets a phone pick its
accent out of the Material You palette; a web page has nobody to ask, and the
store pictures are pinned to Rust for the same reason.

**The headline is a chord chart.** The most characteristic object this app
holds is chord symbols in a monospaced field sitting over the syllables
somebody put them over, so the hero is one rather than a sentence describing
one. Scroll to the song-detail plate and the app is rendering the same thing.

The alignment is real — the chords are positioned by counting characters — and
that only holds while neither line wraps, which is what the `clamp()` on
`.chart` is tuned for: 31 characters, unwrapped, from a 360px phone up to the
760px hero.

**Monospace means one thing, and it is used twice.** The chart at the top and
the receipt at the bottom, both being things that are simply written down. It
is nowhere else on the page, so it never decays into a font for small labels.

**The four questions are set in Newsreader italic because they are spoken.**
`"Play something."` is something a person says to you; `What a song holds` is
the page talking, and stays roman. The spine is the one the README and
`store/metadata/en-US/full_description.txt` are both organised around, for the
reason `store/shots.json` gives about captions — nobody arriving here wants a
feature list, they want a description of something that has happened to them.

**The page makes no third-party requests.** No CDN, no Google Fonts, no
analytics, no hotlinked badge. That is the web version of the app's
one-permission promise, it is the last line of the receipt, and it is checkable
in a network panel in about four seconds. It is also a test; see below.

**The call to action is Google's badge, not a button of ours.** The brand
guidelines are explicit that a badge, rather than a lockup or a home-made
button, is what drives an install — so the rust button that used to sit there
is gone, and the accent stays on the chords and the links. The artwork is
served from `site/brand/`, unmodified, as an SVG so that "don't use
low-resolution badges" stops being anybody's job to check.
`site/brand/README.md` has the provenance, the three rules with numbers in
them, and the reason the file is committed while everything in `site/img/` is
built.

---

## The two halves that are built

Neither is committed. `.gitignore` says why, and it is the same bargain the
listing images are under: keep what is worth reviewing in a diff, rebuild what
is a pure function of it.

### Fonts — `scripts/site-fonts.py`

```bash
python3 -m venv .venv-fonts
.venv-fonts/bin/pip install fonttools brotli
.venv-fonts/bin/python scripts/site-fonts.py site/fonts
```

1.35 MB of source faces become 194 KB that every visitor fetches and 129 KB
that almost nobody does. Each face is cut twice — `base` is ASCII and the
punctuation English prose actually uses, `ext` is the accented Latin-1 letters
and the rest of the punctuation block — and `site/style.css` declares both
against a `unicode-range`, so the browser fetches `ext` only if a character in
it appears on the page. Today none does.

The script's own docstring has the argument for that split over the two obvious
single-subset alternatives, and the short version is that the tight one is a
trap: a subset cut from the words currently on the page turns the next copy
edit into a blank box. Cards K13, K21 and K26 are this project's history with
exactly that failure from the other direction.

Two things it asserts rather than assumes: that the variable axes survived
(Newsreader has `opsz` as well as `wght`, and `font-optical-sizing: auto` on
the page depends on it), and that the OFL and Bitstream Vera texts are copied
alongside, which both licences require of a modified copy — which a subset is.

### Screenshots — `scripts/store-frame.py --preset site`

```bash
scripts/store-frame.py .shots site/img --preset site
```

The same raw frames the Play listing is composited from, with the same crop and
none of the rest. The listing's pictures carry a gradient, a device plate and a
caption because they are looked at as thumbnails in a store with nothing around
them; this page has its own paper, its own prose beside each picture saying what
the screen is, and its own CSS to round the corners, so all three become noise
and the caption becomes the same sentence twice.

What both want is the crop. The navigation bar is the emulator's furniture
rather than the app's, and how far to crop is measured in `insets.json` rather
than written down — `store-frame.py`'s header has that argument, and it is the
reason the site preset lives in that file instead of in a script of its own
that would sooner or later measure the bar its own way.

The plates come out 600×997, twice the 300px slot the page shows them in.
`site/style.css` declares that shape as an `aspect-ratio` and the compositor
refuses a capture that is not it, because the page is never shipped the
pictures and has to reserve the space before one arrives.

---

## What the tests hold

`src/site.rs`, tests only, `include_str!` throughout so they read the
repository rather than a working directory.

| Test | What breaks without it |
| --- | --- |
| `the_site_css_repeats_the_theme_tokens_exactly` | A hex moves in `theme.rs` and the page keeps painting the old one. Also fails on an `--sla-*` token the page invents and the app does not have — one that reads as shared and is not. |
| `the_site_asks_nothing_of_anybody_else` | Any subresource — `src`, `<link href>`, `url()` — acquires a scheme, and the receipt's last line quietly becomes false. Links are not requests and are not checked. |
| `the_site_reserves_the_shape_the_compositor_emits` | `SITE_PLATE_ASPECT` and the `aspect-ratio` on `.plate img` part company, and every section below the fold jumps when an image loads — invisible on the machine that built the page, where it is already cached. |
| `the_site_serves_the_slices_the_subsetter_cuts` | A `unicode-range` drifts from the slice it selects, or the subsetter writes a file no `@font-face` asks for. |
| `every_picture_the_page_shows_is_a_shot_the_tour_photographs` | A screen is renamed in `src/shots.rs` and the page keeps a hole where a screenshot was. |

Each was checked by breaking the thing it guards and watching it fail, which is
the only way to know a test of this kind is not passing vacuously.

---

## Deploying it

`.github/workflows/site.yml`, on a push to `master` touching `site/**` or
either script, and on demand. It cuts the fonts, crops the plates, writes the
`CNAME`, checks that every file the page references is actually in the
directory, and hands `site/` to Pages. There is no build step for the page
itself.

**The screenshots come from the most recent successful `Listing` run**, not
from an emulator of its own. Photographing the app is twenty minutes and a
headless emulator that is currently unreliable enough to need a software
painter; making a one-word copy edit pay that, and making this page's
availability depend on it, is the wrong trade. The workflow's header states the
cost honestly — inputs from another run can be inputs from another commit — and
`shots_run` is the way to pin a specific one. With no such artifact anywhere,
the run fails rather than deploying a page with seven holes in it.

Three things live outside the repository and have to be done once:

1. **Pages source** set to GitHub Actions, in Settings → Pages.
2. **Custom domain** `setlistarray.lostconnection.dev`. The workflow writes the
   `CNAME` file on every deploy, deliberately: an artifact without one reverts
   the site to the default `github.io` address and breaks every saved link.
3. **DNS**, a `CNAME` record from `setlistarray` to `<user>.github.io`. The
   certificate is issued after the record resolves and can take a few hours.

---

## Still to do

**The copy is hand-written and can drift.** The page's prose is a trimmed
cousin of `store/metadata/en-US/full_description.txt`, not generated from it:
the registers genuinely differ, Play forbids formatting this page wants, and a
generator emitting marketing prose would cost more than the drift it saved.
`docs/RELEASING.md` carries the reminder to look at both together.
