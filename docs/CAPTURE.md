# Offline webpage capture

Card **E1**, the Phase E spike: fetch a chord page, make it safe, make it
local, and work out whether the rest of Phase E is worth building.

The engine is `src/capture/`. It is a real part of the app with 30 tests
behind it, not a throwaway. The UI on top of it is E2–E6 and does not exist
yet.

---

## The Android permission: decided, 2026-08-26

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

That line was precisely what this app promised never to have. It said so in a
comment at the top of `android/AndroidManifest.xml`, in a README section
heading, and — as of card E1 — in a test:
`the_android_manifest_asks_for_no_permissions` in `src/lib.rs`. (The test did
not previously exist. The README had claimed it did.)

E1 stopped there and put the choice to the owner rather than making it. **The
choice has been made: the permission goes in, and the promise is rewritten.**

### What changed

* `android/AndroidManifest.xml` declares
  `<uses-permission android:name="android.permission.INTERNET" />`, and its
  header comment now says what it is for instead of saying there is none.
* The test is now `the_android_manifest_asks_only_for_internet`. It is an
  allowlist of exactly one: two permissions fail, and one that is not INTERNET
  fails. It was rewritten rather than deleted, because the promise narrowed
  rather than disappeared.
* A second test, `the_http_client_is_named_in_exactly_one_file`, asserts that
  `src/capture/fetch.rs` is the only file under `src/` that names the HTTP
  client. It is a floor under the "one call site" half of the claim, not card
  X2.
* `android:allowBackup="false"` is untouched. The permission lets the app reach
  out; cloud backup would let Google reach in. Only one of those is a feature
  anyone asked for.

The promise the app now makes, in the README and in the manifest, is: **one
permission, one call site, nothing else reaches the network.**

### Why, and what was not chosen

The reasoning is that "no permissions" was never the point — *no reach into the
user's device or data* was. INTERNET is the one permission that grants none of
that: it does not read files, contacts, location, or any identifier, it is
invisible to the user, and it cannot be revoked because there is nothing to
revoke. Meanwhile the feature it unlocks is the app's distinguishing one, and
Android is the platform the design handoff targets. Trading the headline
feature on the primary platform for the absolutist version of a sentence in a
README is a bad trade, and the honest narrower sentence is more convincing than
the absolutist one anyway, because it can be checked.

Two options were considered and **not** taken:

1. ~~**Keep the promise, and Phase E is desktop-only.**~~ Rejected. The capture
   engine works and is tested; offering it only on the desktop leaves the app's
   headline feature missing from the platform the design targets, in exchange
   for a claim no user is checking.
2. ~~**Capture through the system share sheet instead.**~~ Not taken *now*, and
   not because it is a bad idea. Android's `ACTION_SEND` can hand a page's HTML
   to the app without the app ever opening a socket, which would keep the
   literal promise intact and move the fetch to a component that already has
   the permission. It needs a `rinch-android` intent-filter API that does not
   exist, it is a bigger piece of work, and it changes E2 from "paste a URL"
   to "share to SetListArray". It also does not replace the URL field on the
   desktop, so it is an addition rather than a substitute. Worth revisiting as
   its own card once `rinch-android` can receive an intent.

### What this does not claim

The APK builds with the permission declared and `aapt2` is happy with it. That
is the whole of what has been observed. **The app has never run on a phone or
an emulator**, so no capture has ever completed on Android, and "the socket now
opens" is a reading of the documentation, not a measurement. Everything in the
site table below was measured on the desktop.

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
Release build of the crate is unchanged in wall-clock terms. The APK is 6.5
MiB with the capture engine in it, against 5.8 MiB before — about 0.7 MiB of
`libsetlistarray.so`, nearly all of it html5ever and its parser tables.

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

**Phase E total: roughly 6–9 days**, against the 5 M's the plan implied. The
Android question above is settled, so none of it is contingent any more —
though E2's UI now has to exist on both platforms rather than one. E5 is the risk; E2 is the
work; E3, E4 and E6 are largely done in the engine.

Two things Phase E should probably grow, neither of them in E2–E6 as written:

- **A card for the Ultimate Guitar JSON extractor.** Size S–M. It brings the
  largest chord site on the internet from "impossible" to "works", and the
  spike has already located the payload.
- **A line in E2's copy that sets the expectation.** "Works with pages that are
  pages" is the honest framing, and it costs nothing to say before someone
  pastes a Cloudflare-protected URL.

---

## Card E2 — the capture screen, and the state machine E4 builds on

Wireframe `1l`, built 2026-08-28. The screen is `src/screens/capture.rs`; the
route is `Route::CaptureWebpage { song }`; the way in is the third row of song
detail's **+ Add attachment** chooser. `Save as: Reader text ▾` is not on it —
that row is E3, and a control that looked live and did nothing would be worse
than the gap (the rule `song_form` set for the same section).

### The states

One enum, so that "running with a result in hand", "attaching a capture that
failed" and "a cancelled capture still ticking a bar" cannot be constructed.

| State | What it means | Attach? |
| --- | --- | :-: |
| `Waiting` | Nothing asked for. The URL field is the screen. | no |
| `Running { run, url, step, stop }` | A worker is out. `step` is the last thing it reported. | no |
| `Settled { run, url, outcome }` | The worker answered. All five outcomes live in `outcome`. | if there is a page |
| `Attached { attachment }` | It is a chart on the song. The screen leaves. | no |
| `NoWorker { url }` | The device would not give the app a thread. | no |

`Settled` is where **E4's three screens go**: `outcome` is
`Captured | Partial | Blocked | Failed | Cancelled`, each already carrying its
own user-facing sentence (`Failure`'s `Display`, `BlockReason::explain`), so
E4 is a `match` on one field and does not have to touch the machine.

**Pruned, deliberately:**

- **`Attaching`.** `write_into` is called straight from the Attach handler on
  the UI thread and no frame is drawn while it runs, so the state would be
  unobservable. Bounded by `max_page_bytes + max_total_asset_bytes` = 12 MiB,
  less than `pdf::import` already writes synchronously. If that write ever
  moves to a worker, this state comes back.
- **`Cancelling`.** The screen does not wait for a worker to acknowledge. It
  returns to `Waiting` at once and drops the late answer by run id.
- **`Restoring`.** Nothing is persisted before Attach, so an app killed
  mid-capture comes back with no capture and nothing on disk to reconcile.
  That is also what makes Cancel free.

### The transitions

| From | Event | Guard | To | Side effect |
| --- | --- | --- | --- | --- |
| `Waiting` | Capture | URL not blank | `Running{run+1}` | spawn a worker |
| `Waiting` | Capture | thread refused | `NoWorker` | — |
| `Running` | `Step(run)` | run matches | `Running{step}` | — |
| `Running` | `Done(run)` | run matches | `Settled` | — |
| `Running` | Cancel | — | `Waiting`, URL kept | flag set by `Drop` |
| `Running` | ← | — | leaves | flag set by scope disposal |
| `Settled` | Attach | a page is present | `Attached` | row + `write_into` |
| `Settled` | Attach | write failed | `Settled` (restored) | row detached again |
| `Settled` | Capture ("Try again") | — | `Running{run+1}` | spawn a worker |
| `Settled` | Cancel | — | leaves | — |
| `NoWorker` | Capture | — | `Running` / `NoWorker` | spawn a worker |

**Rejected edges**, each of them a test: a `Step` or `Done` for a superseded
run; anything at all arriving after Cancel; a second `Done` from one worker;
anything reaching `Attached`; Attach pressed twice; Attach over a state with no
page.

### External reality

| What happens | What the machine does |
| --- | --- |
| App backgrounded mid-fetch | The worker keeps going; its `update_send` closures queue on the main-thread dispatcher and run when the loop pumps again. |
| Process killed mid-fetch | Nothing was written. The screen comes back at the library, the capture is gone, and there is no half-state on disk. |
| Network drops between the HTML and the images | Every image fetch fails, each becomes a `Missed`, the outcome is `Partial`, and Attach stays available — a chart with a missing photo is still the chart. |
| Cancel while a request is in flight | The flag is honoured at the next checkpoint (between requests). The open socket is **not** aborted — `rinch-http` exposes no way — so one request runs to its timeout on a thread nobody is listening to. Nothing is written either way. |
| Attach pressed twice | The first press takes the capture out of the state; the second finds `attachable() == None`. On a single-threaded UI the second tap cannot land *during* the first write. |
| A redirect | ureq follows it and does not say where it went. `source_url` records what the user pasted, which is what E6 should re-fetch anyway. The **preview is the mitigation**: the spike's songsterr URL that redirected to a different song is visible in the preview before Attach. |
| A 30 MB image | Downloaded in full (no mid-download abort), refused by `max_asset_bytes`, recorded as `Missed` → `Partial`. Costs transient heap, not disk. |
| The screen is gone when the worker answers | A write to a disposed signal is a warn-once no-op. Usually the worker never gets that far, because the flag was set on the way out. |

### Where the fetch runs

**A thread per capture**, `sla-capture`, 4 MB of stack — four times D5's
rasteriser thread, because `dom::walk` and `dom::collect_text` recurse once per
level of nesting over markup from a stranger's server.

* **UI → worker** is one `AtomicBool`, read at every checkpoint the engine
  offers and answered as `Wanted::No`.
* **worker → UI** is `Signal::update_send`, which hops a closure onto the main
  thread and runs it against the live state. The *reduction happens on the main
  thread*, inside `Flow::deliver`, which is what makes comparing run ids there
  safe. A mailbox signal the worker `set` instead would lose a message whenever
  two arrived between frames, and the lost one could be the final outcome.

`StopOnDrop` lives *inside* `Flow::Running` and sets the flag in its `Drop`, so
one mechanism covers Cancel, a second Capture, the worker finishing, and the
screen being unmounted — every one of them is a way that value stops being
reachable. Progress ticks mutate through `&mut Flow` so a running capture does
not cancel itself with its own report. **Correctness never depends on the flag
arriving**, which is D5's lesson kept: the run id is what keeps the screen
right, the flag only stops work.

### What the engine had to grow

* `Progress::Stripping { bytes }` and `Progress::Images { done, total, bytes }`
  — the counter under the bar has to be measured, and `fetched_bytes` was only
  available after the capture finished.
* `Wanted`, returned from the progress callback. It is the only place a
  blocking capture hands control back, so it is the only place a Cancel can be
  heard. The answer is latched, so a stale closure cannot un-cancel a run.
* `Outcome::Cancelled`, carrying no page. Handing back half a capture as though
  it were a result is the lie the variant exists to prevent.

### Two faults found by running it

**`RefCell already borrowed`, on Attach, on the real app, with every test
green.** `Signal::with` holds the signal store borrowed for as long as its
closure runs, and `attach_captured` writes to `SongsStore` — another signal,
wanting `borrow_mut`. `flow.with(|state| attach_captured(…state.attachable()))`
is a nested borrow of one `RefCell` and it panicked on the main thread of the
first end-to-end run. It is the same fault `AttachmentsStore::update` records
from card D2, met from the other side. The fix is structural rather than a
comment: `Flow::take_settled` moves the capture *out* of the signal, and the
store is written afterwards — which also saves copying twelve megabytes and
*is* the double-tap guard, because after the take there is nothing to attach.

**← and Cancel were the same handler**, which made "the user leaves the screen
mid-capture" unreachable — a row of the cancel table with no way to get to it.
They are two intentions and now have two controls.

### Verified

* `cargo test`: 366 (334 before), the state machine tested without a network,
  a window or a thread.
* `scripts/screenshot.sh`: 9/9.
* **Desktop, on `:99`**: `hymnal.net/en/hymn/h/1` captured, cancelled
  mid-flight and re-captured, and attached. `78 KB so far` while running,
  `66 KB on this device` when settled, `Downloaded 1 image`. On disk:
  `attachments/25/page.html` (56,248 B) + `assets/000.png` (8,830 B), zero
  `<script>`, `src="assets/000.png"`, and no remote `img`/`link`/`script`
  reference left but an inert `<link rel=canonical>`.
* **On the phone (moto g stylus 5G, ZY22FD66GZ)**: the same page captured and
  attached. **This is the first capture that has ever completed on Android** —
  the permission section above says the socket opening was "a reading of the
  documentation, not a measurement", and it is now a measurement.

### Found and not fixed

* **Full-page mode previews the site's navigation**, not the chart: hymnal.net
  gives `Login · Sign up · Follow us:` before the hymn. Reader mode is what
  fixes it and reader mode is E3.
* **The soft keyboard covers the footer** on Android, so Capture cannot be
  reached without dismissing it first. The same Rinch gap `chart_editor`
  documents (no IME inset is exposed); this screen does not even have that
  screen's scrollable padding, because its footer is pinned.
* **A crash between `SongsStore::attach` and `write_into` leaves an empty
  attachment directory** with no row. The panic above produced one. Nothing in
  this card introduced it — it is the shape `pdf::import` has had since D3 —
  but a sweep for directories with no row is worth a card.

---

## Card E5 — rendering a captured page, and what "render" turned out to mean

Built 2026-08-28. The engine is `src/capture/render.rs`; the one component both
screens mount is `src/screens/captured_page.rs`. The card body was a title and
two dependencies, so the first half of the work was deciding what it meant.

### Rinch can be handed markup. It cannot be handed the saved file.

`sanitise.rs`'s header had already assumed the highest-fidelity reading — *"the
captured page is opened later by E5 in the app's own renderer"* — and Rinch is a
browser-grade stack, so the first question was whether `page.html` could simply
be handed over. The mechanism exists: `NodeHandle::set_inner_html`
(`rinch-core/src/dom/mod.rs`) parses a string into real DOM nodes under any
element the app owns, and those nodes get real Stylo styling and real Taffy
layout, in a subtree of an ordinary `rsx!` tree. Four measurements say the input
cannot be the file:

1. **Rinch's HTML parser is a hand-rolled scanner** — `rinch-dom/src/html_parser.rs`,
   not html5ever. Run over the three sites the table above says the engine
   actually captures, the first text node it produces on **every one of them** is
   the string `!doctype html>`: a `<!…>` declaration is not a tag it knows and
   the remainder falls through to the text branch. It decodes six named entities
   and no numeric ones, and it has no implicit end tags, so the unclosed `<p>`
   and `<li>` that html5ever exists to recover from nest instead of closing.
2. **A `<style>` block in injected markup is loaded into the document's
   stylist.** `append_child` calls `maybe_load_style_css`
   (`rinch-dom/src/style_resolution/mod.rs`), which hands the CSS to
   `load_stylo_css` and re-resolves every node in the tree. One stylist per
   document, one document per window: a stranger's `p { margin: 0 }` would
   restyle SetListArray, not just the attachment.
3. **Most of the page's CSS is gone anyway, deliberately.** E1 drops every
   `<link rel=stylesheet>`. Measured on the capture this was built against:
   hymnal.net's saved page contains **zero** `<style>` blocks. Its chord display
   is `<div class="chord-text">` boxes that the site's external sheet makes
   `inline-block`, with the chord `block` above the syllable, and the class that
   hides the whole scaffold is `hidden`. There is no fidelity left to preserve:
   the "high-fidelity" render of that page is a column of one-syllable lines
   that was never meant to be visible at all.
4. **Rinch's UA stylesheet is small on purpose** (`rinch-dom/src/dom_impl/mod.rs`):
   `display` for the usual tags, bold for `<strong>`, italic for `<em>`, list
   indentation, and nothing else. No `<pre>` monospace, no `white-space: pre`,
   no heading sizes. A chord chart handed over raw comes back proportional and
   word-wrapped, which is the one thing a chart cannot survive.

### So the app rebuilds the page and Rinch lays it out

`page.html` is parsed with html5ever — already in the tree, already this
module's parser, spec-correct on what chord sites really serve — walked, and
re-emitted as a small well-formed fragment with the app's own typography inlined
on it. That fragment goes to `set_inner_html`, and from there it is Stylo,
Parley and Taffy: real inline flow, real `<pre>`, real tables, real images, real
line breaking. Rinch's parser is then only ever fed markup this app generated,
which is the one input it is reliable on.

| Input | Output |
| --- | --- |
| `head`, `style`, `script`, `title`, `iframe`, `canvas`, `svg`, form controls | dropped, subtree and all |
| a tag in `render::STYLED` | itself, with the app's inline style |
| any other element | unwrapped — its children take its place |
| an element with nothing visible under it | dropped |
| `<img src="assets/000.png">` | an absolute path, both axes in pixels |
| an image whose file is gone | its `alt`, in muted italic |
| the site's own `style=` | dropped, except `display: none` |

Two rules earned their place by being got wrong first:

* **Empty containers are pruned.** Emitting every styled element produced 809
  elements on the CifraClub capture, most of them empty `<div>`s — the
  scaffolding for the sheet E1 threw away. Each is a Taffy node and a Stylo
  resolution laying out nothing, and on the card the whole element budget was
  spent on empty boxes before a word of the chart was reached.
* **Inside a `<pre>`, block elements are unwrapped.** CifraClub writes one
  `<div>` per line inside the `<pre>`, each ending in a newline — a block box
  *and* a line break — so the first render came out double-spaced. Inside
  preformatted text the line breaks are in the characters; only inline elements
  (the `<b>` around each chord symbol) are kept.

Every length in the fragment is in `em` except an image's two axes, which is
what lets **one fragment serve both places**: the card renders at 12.5 px and
the viewer at 14.5 px, and the viewer's `A−`/`A+` are a `font-size` on the host,
so the whole document scales rather than only its prose. Images state both axes
in pixels because of **K28**, exactly as `song_detail::page_image` does for a
rasterised PDF page; their sizes come from `render::image_pixels`, which reads
PNG, GIF, WebP and JPEG headers without decoding — the same argument
`pdf::pages::page_pixels` makes for a PNG, applied to the four formats
`assets::extension` can name.

### The card and the viewer are one component

A PDF gets that agreement for free: `pdf::pages` rasterises one PNG and both
screens draw the same file. A captured page has no such artefact, so the
agreement is `screens::captured_page::CapturedPageView` — `song_detail` mounts
it in the card clipped to the height of the eight `T_CHART` lines it replaces,
`attachment_viewer` mounts it full-screen, and the only thing they pass
differently is how big it is. `song_detail::preview` and
`attachment_viewer::empty_state` both return early for a captured page, so
neither screen can say a second thing about one.

### Failure

| State | Shown |
| --- | --- |
| `page.html` renders | the page |
| no `page.html`, extracted text in the row | that text, through the same renderer as a `<pre>`, under *"The saved page is no longer on this device."* |
| the same in an in-memory library (`--seed`) | that text, and no complaint — there was never a file |
| `page.html` there, nothing renderable in it | *"This saved page could not be read."* |
| neither | whichever of the two sentences fits |
| the element budget ran out | the page, then *"This page was too long to draw in full."* |

The text fallback is the point rather than a consolation: `Attachment::body`
lives in the database and `page.html` lives in a directory on a phone, so the
two go missing independently, and a user whose library folder was deleted still
has the words. The explaining note goes **above** the content and the truncation
note **below** it, which was a correction: on `:99` the first one sat at the
bottom of a two-thousand-pixel column of text, a footnote nobody reaches to a
question they had at the top.

`page.html` is never *unparseable* — html5ever recovers from anything — so
"could not be read" means "nothing in it was a tag this app draws", which is a
real state a page of pure `<head>` produces.

### The thumbnail

**Unchanged: the `WEB` badge.** The card asked "what a captured page's thumbnail
is in the library row, where a PDF now shows page one", and a PDF does not show
page one there — `ui::AttachmentThumb` draws a 38 px badge for all three kinds,
and `library::primary_kind`'s own comment says why: *"it reads the kind and
nothing else: the body stays on disk, which is what lets three hundred of these
rows be built."* Rendering a page per row would be three hundred html5ever
parses to draw a list. The place a PDF shows page one is the **card** on song
detail, and there a captured page now shows the top of the page.

### A Rinch bug this found

**An `<img>` inside an `<a>` disappears.** Right `src`, a computed width and
height from its own style, and a 0x0 layout box — while the same `<img>` as a
direct child of the block, or beside text in a `<p>`, lays out correctly. Every
site's logo and half its chord diagrams are wrapped in an anchor, so on the
first end-to-end run hymnal.net's masthead was simply absent.

`ifc.rs::mark_inline_descendants` marks an inline child with its `ifc_root` and
does not recurse into it. That was enough for text, because
`walk_inline_children` recurses either way — but it left any inline-block
*descendant* with `ifc_root == None`, so `compute_inline_block_layouts` never
measured it, and the Parley `InlineBox` pushed for it read a `layout` that was
still zero. The fix is to recurse, exactly as the `display: contents` branch
already does. Fixed in `../rinch-fixes`; see the README's PR list.

### Verified

* `cargo test`: **394** (366 before) — 21 in `capture::render`, 7 in
  `screens::captured_page`, and two rewritten on the screens whose captured-page
  branch moved.
* `scripts/screenshot.sh`: 9/9.
* **Desktop, on `:99`**, driven over Rinch's debug IPC (card C6's mechanism):
  `hymnal.net/en/hymn/h/1` and `cifraclub.com/pink-floyd/wish-you-were-here/`
  captured through the real capture screen and attached. The CifraClub chart
  renders as one `<pre>` — computed `white_space: Pre`, `font-family:
  DejaVu Sans Mono…`, 13.34 px, 5,703 px tall, chord symbols bold above their
  syllables, single-spaced. `A+` twice took an `<h1>` from 27 px to 105 px and
  rebuilt the node each time. hymnal.net's masthead PNG lays out at 276x80 from
  `assets/000.png`. Deleting `page.html` and `assets/` underneath a live library
  put *"The saved page is no longer on this device."* above the extracted text
  in both the card and the viewer.
* **On the phone (ZY22FD66GZ), in airplane mode**, cold-started: the page E2
  captured renders in the card and full-screen with no network at all. That is
  the whole promise of the feature and it is now a measurement rather than a
  claim.

### Found and not fixed

* **A chart whose layout lived in an external stylesheet cannot be rebuilt.**
  hymnal.net renders as clean lyrics followed by the chord scaffolding one
  syllable per line, because the sheet that made `.chord-text` `inline-block`
  was dropped at capture time and so was the `.hidden` that put the scaffold
  away. Two cards' worth of possible answers, neither of them E5's: keep a
  same-origin `<style>` and rewrite its selectors to a scoping prefix so it can
  be loaded without escaping into the app, or extend E3's reader mode to
  recognise the chord-span pattern and rebuild it. Sites whose chart is a
  `<pre>` — CifraClub, guitaretab — are unaffected and come out right.
* **The card still opens on the site's navigation.** Same note E2 left; the fix
  is E3's, by capturing less rather than by drawing less.
* **Rinch's `decode_html_entities` replaces `&amp;` first**, so text containing
  the literal characters `&lt;` arrives on screen as `<`. Harmless here — it is
  a text node by then, and there is no JS engine — but it is the classic
  wrong-order double-decode and worth an upstream line.
* **SVG, BMP and AVIF images fall back to their `alt`.** `image_pixels` reads
  four formats; SVG has no pixel size to read and Rinch has no decoder for any
  of the three. Rare on a chord site, and a caption is an honest answer.
