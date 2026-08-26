# Offline webpage capture

Card **E1**, the Phase E spike: fetch a chord page, make it safe, make it
local, and work out whether the rest of Phase E is worth building.

The engine is `src/capture/`. It is a real part of the app with 30 tests
behind it, not a throwaway. The UI on top of it is E2–E6 and does not exist
yet.

---

## Stop here first: this cannot ship on Android

**`android.permission.INTERNET` is required to open a socket on Android.** It
is not optional, it is not a runtime prompt, and there is no way around it from
app code. Without it the kernel refuses the socket and Java throws
`SocketException: socket failed: EACCES (Permission denied)` — the `inet`
group (`AID_INET`, gid 3003) is granted to a package by the installer only when
the manifest declares the permission.

The Android documentation is explicit: *"To perform network operations in your
application, your manifest must include the following permissions:
`android.permission.INTERNET`"*. It is a **normal** permission — granted at
install, never prompted for, not shown in the app's permission screen, and not
revocable — but it is still a `<uses-permission>` line in the manifest.

That line is precisely what this app promises never to have. `android/
AndroidManifest.xml` says so in a comment, the README says so in a section
heading, and — as of this card — a test says so too:
`the_android_manifest_asks_for_no_permissions` in `src/lib.rs`. (It did not
previously exist. The README claimed it did.)

So there is a decision to make before E2, and it is not a technical one:

1. **Keep the promise, and Phase E is desktop-only.** The capture engine works
   and is tested; on Android the URL field is not offered. The app's headline
   feature is missing from the platform the design targets.
2. **Add `INTERNET` and rewrite the promise.** "No permissions" becomes "no
   permissions that can reach your data" — INTERNET is normal-level and
   invisible to the user, and everything else stays true. The manifest test
   becomes an allowlist of exactly one.
3. **Capture through the system browser instead.** Android's share sheet and
   `ACTION_SEND` can hand a page's HTML to the app without the app ever
   opening a socket. This keeps the promise literally intact and moves the
   fetch to a component that already has the permission. It is a bigger piece
   of work, it needs a `rinch-android` intent-filter API that does not exist,
   and it changes E2 from "paste a URL" to "share to SetListArray".

I have not chosen. Nothing in the manifest has been touched.

Everything below is measured on the desktop, where it all works.

---

## The site table

Eight real pages, full-page mode, the honest User-Agent, one run on
2026-08-26. `on disk` is `page.html` plus downloaded images. `chart?` is
whether a chord chart actually survived into the saved file, checked by
counting chord-marked elements and preformatted characters in the output —
never by reproducing what came down.

| Site | fetched | on disk | images | chord lines | `<pre>` chars | scripts cut | chart? | outcome |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | :-: | --- |
| cifraclub.com | 554,077 | 177,802 | 0/0 | 57 | 3,934 | 90 | **yes** | `Captured` |
| chordie.com (guitaretab) | 52,149 | 44,650 | 7/7 | 2 | 746 | 26 | **yes** | `Captured` |
| hymnal.net | 86,725 | 74,727 | 1/1 | 0 | 0 | 15 | **yes** | `Captured` |
| azlyrics.com | 83,709 | 89,711 | 7/7 | 0 | 0 | 24 | no | `Blocked(NoChart)` |
| tabs.ultimate-guitar.com | 153,492 | 139,783 | 0/0 | 0 | 0 | 14 | no | `Blocked(ScriptShell)` |
| songselect.ccli.com | 151,687 | 145,039 | 0/0 | 0 | 0 | 10 | no | `Blocked(ScriptShell)` |
| musicnotes.com | 292,390 | 464,755 | 24/38 | 0 | 0 | 71 | no | `Blocked(NoChart)` |
| e-chords.com | 5,795 | 1,957 | 0/0 | 0 | 0 | 1 | no | `Blocked(Challenged)` |

Also measured and not tabulated: **chordify.net** answers the same Cloudflare
interstitial as e-chords (5,809 bytes, `Blocked(Challenged)`), and
**911tabs.com** captures cleanly but is an index of links rather than a chart —
it was what caught the `chord_markup` false positive described below.

Reproduce any row with:

```bash
cargo run --release --bin capture_probe -- [--reader] [--browser-ua] URL...
```

It prints counts and byte totals only. What comes down is somebody's
copyrighted chart and it belongs in the user's library, not in this
repository.

### Reader mode against full page

Same four capturable pages, both modes, comparing `page.html` alone:

| Site | full page | reader | saved | chord elements kept | `<pre>` kept |
| --- | ---: | ---: | ---: | :-: | :-: |
| cifraclub.com | 177,802 | 136,165 | 23% | — | 1/1 |
| hymnal.net | 65,897 | 51,427 | 22% | 38/38 | — |
| chordie.com | 34,975 | 19,888 | 43% | 48/57 | 3/3 |
| azlyrics.com | 18,757 | 12,752 | 32% | — | — |

Reader mode keeps the chart on every page that has one, and saves a fifth to
two fifths of the bytes. The saving is real but not dramatic, because on a
chord site the chart container *is* most of the page.

---

## The verdict, honestly

**This captures server-rendered chord sites, and nothing else.** Three of the
eight pages produced a usable offline chart. That is not a bug in the engine —
it correctly identified all five failures and said which kind each was — but it
is a much narrower feature than "paste any chord site".

Broken down:

- **Works, reliably.** CifraClub, hymnal.net, chordie/guitaretab. These are
  ordinary HTML documents. The chart survives, the images come down, the
  captured file opens with the network off. CifraClub in particular is one of
  the largest chord sites in the world and captures perfectly.
- **Bot walls: e-chords, chordify.** Cloudflare's "Just a moment…"
  interstitial, 403, ~5.8 KB. Nothing gets through this from a plain HTTP
  client, with any User-Agent — I checked, and `--browser-ua` does not help,
  because the challenge is a JavaScript proof-of-work, not a header test. These
  sites are permanently out of reach without an embedded browser engine.
- **Single-page apps: Ultimate Guitar, SongSelect.** 150 KB of bundle and 57
  characters of text. **But** — and this is the most interesting thing the
  spike found — Ultimate Guitar's chart is *in the bytes we already fetched*.
  It sits in a 134 KB JSON `data-content` attribute on a `div.js-store`, with
  154 `[ch]…[/ch]` markers delimiting the chords. A UG-specific extractor
  (parse the attribute, convert the markers to marked-up spans) is perfectly
  feasible and would bring the single most-used chord site on the internet into
  range. It is also site-specific, undocumented and will break without notice.
  That is a card of its own, not part of E2–E6.
- **Paywalls: musicnotes, SongSelect.** musicnotes is sheet music delivered as
  images behind a purchase; we capture 24 preview images and 8.5 KB of product
  copy, and correctly report no chart. SongSelect needs a CCLI licence. Neither
  is capturable and neither should be.
- **Lyrics-only sites: azlyrics.** Captures fine, has no chords in it, and the
  engine says so rather than saving it as a chart. This is the failure mode
  most likely to confuse a user, because the capture *worked*.

The feature is still worth building. Someone whose sites are CifraClub and
hymnal.net gets exactly what the handoff promises. Someone whose site is
Ultimate Guitar gets a clear "this page builds itself with JavaScript" and a
suggestion to paste the chart in as text — which is honest, and better than a
blank attachment. What has to change is the *expectation* set in E2's UI: this
is not "capture any page", it is "capture pages that are pages".

---

## Design

```
  fetch ──▶ parse ──▶ sanitise ──▶ judge ──▶ [narrow] ──▶ images ──▶ write
    │                     │           │                     │
 Failure              Stripped     Outcome            Partial/Missed
```

| File | What it does |
| --- | --- |
| `mod.rs` | `capture()`, `write_into()`, `Outcome`, `Limits`, the disk layout |
| `fetch.rs` | The only socket in the app, over `rinch-http`; the `Fetcher` seam and a canned network for tests |
| `dom.rs` | Parse, serialise, walk, detach, text extraction. The only file that names `NodeData` |
| `sanitise.rs` | Strip everything that can execute or reach the network |
| `reader.rs` | Readability-style extraction, weighted for chord charts |
| `detect.rs` | Chord-content, paywall, bot-wall and JS-shell heuristics |
| `../bin/capture_probe.rs` | The harness that produced the table above |

### The typed outcome

E4 has to draw three distinct failure screens, so they are three distinct
variants rather than a `Result<String>`:

| Variant | Meaning |
| --- | --- |
| `Captured(page)` | Everything came down |
| `Partial(page)` | Page came down, some images did not; `page.missed` says which and why |
| `Blocked { reason, page }` | Page came down and is not a chart |
| `Failed(failure)` | Nothing came down |

`BlockReason` has four values, and the fourth was added because of a real
misclassification during the spike: `Challenged` (a Cloudflare-style bot wall),
`Paywalled`, `ScriptShell`, `NoChart`. Cloudflare answers **403**, which the
paywall rule claimed — and "buy a subscription" is exactly the wrong advice for
someone whose only problem is that they are not a browser. Each reason carries
its own `explain()` string, written next to the rule that fires it so the two
cannot drift apart.

`Blocked` still hands back the page where there is one. A paywalled teaser may
be worth keeping; that is the user's call, not the engine's.

### Judging happens before narrowing

The question E4 answers is "is there a chart behind this URL", which is about
the page the site served — not about the fragment reader mode chose to keep. A
paywall notice lives in a banner that reader extraction throws away. There is a
test for this (`the_verdict_survives_reader_mode_throwing_the_banner_away`).

### Sanitising

Two rules. **Nothing may execute**: `<script>`, `on*` attributes,
`javascript:`/`vbscript:`/`data:text/html` URLs, the frame family, `<object>`,
`<embed>`, `<canvas>`, form controls, `<meta http-equiv=refresh>`. "Rinch has
no JS engine" is a property of this month's Rinch, not a security boundary, and
a file written to disk outlives the assumption.

**Nothing may reach the network**, which is the stricter rule and is what
offline-first actually means:

- **`<link rel=stylesheet>` is dropped whichever host it names** — and so are
  `preload`, `prefetch`, `preconnect`, `dns-prefetch` and `modulepreload`,
  which are the same request under other names. A same-host stylesheet is no
  better than a third-party one: both are a request that will fail on a train.
  This is the decision the card asked to be made and documented.
- **`<style>` blocks are kept**, with `@import` and absolute `url()` references
  neutralised. Those are the only two ways inline CSS reaches the network.
  Keeping them matters more than it sounds: chord sites position the chord row
  over the lyric row in CSS, and a capture that dropped the stylesheet would
  turn a chart into a wall of words. Relative `url()` is left alone — it
  resolves inside the attachment directory and 404s locally, which costs
  nothing.
- **`<a href>` is rewritten to absolute** and kept. It cannot be followed
  offline, but it records where the link went, and leaving it relative would
  silently resolve against the attachment directory.
- Tracking pixels (1×1, or a known analytics host) and ad containers go. The ad
  keyword list is deliberately conservative — long substrings like `advert` and
  `adsbygoogle`, plus whole class *tokens* like `ads`. There is no bare `ad` or
  `banner` on it, because those match `download`, `headband` and half the class
  names on the web. There is a test for exactly that.
- **`<noscript>` contents survive.** On a JS-only page it is occasionally the
  only real text on offer, which is why the serialiser runs with
  `scripting_enabled: false`.

Sanitising runs **before** asset rewriting so that adverts and tracking pixels
are gone before the downloader picks its images — the app never fetches an
advert. The price is that `srcset` and the `data-src` family have to survive
that pass, and `assets::rewrite` is what finally removes them. Neither pass is
optional.

### Assets

Every `<img>`, resolved against `<base href>` or the requested URL, downloaded,
written to `assets/NNN.ext`, and the `src` rewritten to that relative path. The
extension comes from the `Content-Type` first and the URL second, because a
chord site serving a PNG from a path ending `.php` is not unusual.

`src` is deliberately **not** the first attribute consulted: on a lazy-loading
site it holds a grey placeholder and the real image is in `data-src`. Taking
`src` would capture the placeholder and call it a success.

An image that does not arrive keeps its `alt` and loses its `src`, so the saved
page shows a captioned broken-image box rather than sitting there trying to
reach a network that is not there. Each miss produces `Outcome::Partial`, never
a failure — a page with nine images of which one 404s is still the chart the
user wanted.

Each downloaded image also gets `data-captured-from` with its original URL.
Inert, cheap, and it is what E6's re-check would read.

### Reader extraction — in, and it needed real work

**Feasible in pure Rust and implemented**, as `CaptureMode::Reader`. It is a
readability-style heuristic over the DOM we have already parsed: score blocks
by text length and comma count, roll the scores up into their containers, apply
a class/id bonus and penalty, divide by link density, take the best.

I did not use `dom_smoothie` or another readability port, and the reason is not
"not invented here". **Generic readability is actively wrong for a chord
chart.** Every implementation descends from arc90's, and arc90's scoring
rewards long sentences and commas and punishes short lines — which is a precise
description of what it does to a chart. Four bars of chord symbols over a lyric
line score at the bottom of every candidate list, and a `<pre>`-only page can
score zero. Two additions make the difference:

- **`<pre>` is content, heavily weighted.** A monospaced block is the strongest
  single signal a page carries that it holds a chart.
- **A hard chart anchor overrides the scores.** Whatever the scoring says, the
  node kept must contain most of the page's chart, where "chart" means
  preformatted text *plus* elements whose class or id says `chord`, `lyric` or
  `tablature`. The second half is essential: many sites wrap every chord in its
  own two-character `<span class="chord">` and position it in CSS, which is
  invisible to any measurement of text length.

Measured: **before the anchor was chart-aware, reader mode dropped the chart on
three of the four capturable sites.** After, it keeps it on all four. If the
chart turns out to be spread directly across `<body>` with no container holding
it, `extract` returns `None`, `reader_fell_back` is set, and the full page is
kept — E3 should say so rather than quietly obliging.

---

## What bit, and what to watch

### `markup5ever_rcdom` empties nodes you are still holding

The nastiest bug of the spike, and worth knowing about before anyone else
touches `dom.rs`. `markup5ever_rcdom::Node` has a hand-written `Drop` that
avoids blowing the stack on a deep tree by walking its descendants iteratively
and `mem::take`-ing the `children` vector out of **every node it reaches** —
regardless of whether something else still holds a strong `Rc` to one.

So detaching an ancestor of a node you are keeping, and letting it fall out of
scope, silently empties the node you are keeping. It stays alive, keeps its tag
and its attributes, and its whole subtree is gone. Reader mode wrote out
`<div class="row main-content"></div>` and a 3.7 KB file on a page whose
extraction had picked exactly the right node.

The fix is one line of ordering — move the keeper under `<body>` *first*, then
sweep — and there is a regression test (`narrowing_does_not_gut_the_node_it_is_
keeping`). It only shows up when the chart is nested, which is why the original
test, with the chart as a direct child of `<body>`, passed throughout.

### `rinch-http` gaps

Three limits are inherited rather than chosen. None is a blocker; all three are
small upstream fixes and worth reporting.

| Gap | Effect here |
| --- | --- |
| **Timeouts are fixed** at 30s connect / 60s overall, built inside the crate with nothing exposed to override them | Fine for one page. But the timeout is *per request*, so a page with 24 images gets 24 independent 60s budgets. `Limits::deadline` (90s) is ours, checked between image fetches, and turns a slow CDN into a partial capture instead of a twenty-minute hang. Measured: one sheet-music page spent 21–29s on images alone. |
| **10 MiB hard body ceiling**, from ureq's `read_to_vec` | `Limits::max_page_bytes` (4 MiB) can only tighten it, and only *after* the bytes are in memory. There is no way to abort a download part-way through this API. |
| **The final URL after redirects is not reported.** `Response` has status, headers and body; ureq followed the redirects and did not say where it ended up | Relative asset URLs resolve against `<base href>` if present and the *requested* URL otherwise, which is wrong for a page that redirects across hosts. It bit during the spike in a more embarrassing way than missing images: a songsterr URL redirected to a completely different song, and nothing in the response said so. A `Response::url` field would fix it. |

`HttpError::Body` also covers two unrelated things — the size ceiling and a
read that ran past the global timeout — and the only way to tell them apart is
to match on the message string. `capture` does exactly that, because "too big
to keep" and "could not reach the site" are different sentences on E4's screen.

### Heuristics that had to be tuned against real pages

- **`tab-` is not tablature.** Bootstrap names its tab component `tab-content`
  and `tab-pane`. Matching it scored 911tabs — an index of links with no chart
  on it anywhere — as a chart. Only `chord`, `lyric` and `tablature` count now.
- **A note name is not a chord.** The first chord matcher allowed a set of
  letters in the quality, which accepts "Bring", "Cars", "Dogs" and "Ban" as
  chords and turns any line of lyrics starting with a capital note name into a
  false chord row. The quality is now matched by consuming known fragments
  (`maj`, `min`, `dim`, `aug`, `sus`, `add`, `m`, digits, accidentals), and
  there is a test listing the words that used to slip through.
- **Bar lines are punctuation.** `C/G  F/A  |  Dm7` failed a strict
  three-quarters-are-chords rule until tokens with no alphanumerics stopped
  counting in the denominator.
- **Gate phrases only count on a thin page.** "Subscribers only" appears in the
  footer of every site on earth. It is only evidence on a page too short to be
  the article.

Every heuristic here is tuned to prefer a false negative. Telling someone their
perfectly good chart is a paywall is worse than saving one page too many.

---

## Dependencies

Four added, all pure Rust, all permissive, all cross-compiling to
`aarch64-linux-android` (verified: `cargo ndk -t arm64-v8a build --release
--lib` is clean).

| Crate | Version | Licence | Why |
| --- | --- | --- | --- |
| `rinch-http` | path | MIT/Apache-2.0 | One HTTP API over `ureq` natively and `fetch` on wasm. The plan's call, and it keeps `ureq` out of this crate's `Cargo.toml`. |
| `html5ever` | 0.39 | MIT/Apache-2.0 | Servo's HTML parser. Spec-correct error recovery on the malformed markup chord sites actually serve. |
| `markup5ever_rcdom` | 0.39 | MIT/Apache-2.0 | The html5ever project's own minimal DOM, versioned in lockstep. |
| `url` | 2 | MIT/Apache-2.0 | Resolving relative asset URLs. Already in the tree via Rinch. |

**Cost: eight new packages** (`html5ever`, `markup5ever`, `markup5ever_rcdom`,
`tendril`, `xml5ever`, `cookie`, `cookie_store`, `rinch-http`). Cheaper than it
looks, because Stylo already brings `string_cache`, `phf` and
`precomputed-hash`, and `rinch-tabler-icons` already brings `ureq` and its TLS
stack. `xml5ever` is dead weight pulled in by `markup5ever_rcdom`'s serialiser.
Release build of the crate is unchanged in wall-clock terms; the APK was not
re-measured because capture cannot ship on Android yet.

**Considered and rejected:**

- **`lol_html`** (Cloudflare, BSD-3). A streaming CSS-selector rewriter, and a
  very good fit for sanitising. Rejected because it builds no tree, and reader
  extraction needs to score nodes and walk to a common ancestor. Using both
  would mean parsing twice.
- **`scraper`.** html5ever plus a CSS selector engine, read-oriented. A
  selector engine over hostile input is more attack surface, not less, and a
  sanitiser only ever needs to match tag names and attributes.
- **`dom_smoothie`.** A maintained readability port. Rejected for the reason
  above: generic readability deletes chord charts.

**Risk worth recording.** `markup5ever_rcdom`'s crates.io blurb calls it
"Basic, unsupported DOM structure for use by tests in html5ever/xml5ever", and
its version carries a literal `+unofficial`. The `Drop` behaviour above is a
symptom of exactly that. The mitigation is that `dom.rs` is the only file that
names it — every other file in `src/capture/` goes through six functions — so
replacing it with `dom_query` or a hand-rolled arena is a change to one file.

---

## Revised sizing for E2–E6

The spike was sized `?`. It came in at about a day and a half, most of which
went on the two heuristics and the `RcDom` bug rather than on the plumbing.

The engine landing early moves work *out* of the later cards: E4's three
screens are now a `match` on an enum whose variants already exist and already
carry their own explanatory text, and E3's mode switch is a two-value enum with
both branches implemented and tested.

| # | Card | Was | Now | Why |
| --- | --- | :-: | :-: | --- |
| E2 | Capture UI: URL field, progress checklist, progress bar, running byte count | M | **M** | Unchanged. `Progress` gives the checklist its stages and `fetched_bytes` gives it the counter, but the work is a screen, a worker thread and hopping progress back to the UI thread — and Rinch's main-thread callback parking is untried in this app. |
| E3 | `Save as: Reader text ▾` with a preview of what gets kept | M | **S** | Both modes are implemented and proven. What is left is a segmented control, a preview pane rendering `page.text`, and surfacing `reader_fell_back`. |
| E4 | Failure states: fetch failed, partial, paywalled/JS-only | M | **S–M** | The engine already distinguishes *five* states (the fourth and fifth being bot-wall and no-chart-found) and each carries its own user-facing sentence. This is three or four screens and their copy, not any logic. |
| E5 | Render a captured page in the attachment card and the viewer | M | **L** | **This is the one that got bigger.** A captured page is arbitrary third-party HTML and CSS, and Rinch's renderer is Stylo and Parley — capable, but nothing in this app has yet asked it to lay out a stranger's markup. Loading local `assets/` files as image sources is also untried. Expect a spike inside this card. |
| E6 | Settings → "Re-check saved pages", off by default | S | **S** | Unchanged and now cheaper: `capture_probe` is most of it, and `data-captured-from` records what to re-fetch. |

**Phase E total: roughly 6–9 days**, against the 5 M's the plan implied — and
that is contingent on the Android decision above. E5 is the risk; E2 is the
work; E3, E4 and E6 are largely done in the engine.

Two things Phase E should probably grow, neither of them in E2–E6 as written:

- **A card for the Ultimate Guitar JSON extractor.** Size S–M. It brings the
  largest chord site on the internet from "impossible" to "works", and the
  spike has already located the payload.
- **A line in E2's copy that sets the expectation.** "Works with pages that are
  pages" is the honest framing, and it costs nothing to say before someone
  pastes a Cloudflare-protected URL.
