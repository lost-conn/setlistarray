# PDF rendering

Card **D4**, the Phase D spike, and the evidence for **decision 3** in
[PLAN.md](PLAN.md). This document is the numbers.

> **Decided 2026-08-26: `hayro`, the pure-Rust rasteriser.** +2.49 MiB of APK,
> no binary blob, no attribution burden, one code path on both platforms, and
> PDFs work in performance mode. The escape-hatch trait that would have kept
> `pdfium-render` swappable was offered and declined — so if hayro draws
> something wrong, the pdfium measurements below are kept precisely so that
> comparison does not have to be run again. Section 8 is the reasoning as it
> stood *before* the decision, left unedited.

Nothing was implemented. No PDF code is in `src/`, `Cargo.toml` and
`build-apk.sh` are untouched, and every measurement below was taken with a
temporary edit that has been reverted. `cargo test --release` is 148/148 and
`scripts/screenshot.sh` is 5/5.

Measured 2026-08-26 on this machine: AMD Ryzen 9 9955HX, NDK r27c, rustc
1.96.0. **There is no phone and no emulator here.** Everything about Android
below is a build result, a binary inspection or a documented platform rule —
never an observation of the app running. The device-only questions are
collected at the end.

---

## The short version

| | APK (`arm64-v8a`) | Δ | Blob to ship | Licence |
| --- | ---: | ---: | --- | --- |
| **Today** | 6,799,883 B | — | — | — |
| **pdfium-render** | 10,920,555 B | **+4,120,672 B (+3.93 MiB, +61%)** | 6.4 MB third-party `libpdfium.so` | BSD-3 + 15 permissive notices |
| **hayro** (pure Rust) | 9,413,131 B | **+2,613,248 B (+2.49 MiB, +38%)** | none | MIT / Apache-2.0 |
| **Defer to system viewer** | 6,799,883 B | 0 | none | — |

Both renderers were built into this app, packaged into a real signed APK, and
the APKs measured. Both cross-compile under `cargo-ndk` for
`aarch64-linux-android` with no linker work.

**The plan's premise has expired.** Decision 3 says pure Rust has "no
rasteriser worth shipping yet". That was true when it was written. It is not
true now: `hayro` 0.7.1 rasterises this app's test documents to within
antialiasing noise of PDFium (mean absolute difference 2.0/255; 0.14 % of
pixels differ by more than half a level), at comparable speed, in one third
fewer APK bytes, with no binary blob and no attribution burden. That changes
the shape of the decision enough that it is treated as a first-class option
below rather than as the plan's dismissed third bullet.

---

## 1. Does `pdfium-render` cross-compile and link under `cargo-ndk`?

**Yes, and the question turns out to be the wrong one.** There is no link step
against PDFium at all.

```
$ cargo ndk -t arm64-v8a build --lib --release
    Finished `release` profile [optimized] target(s) in 15.53s
```

The resulting `.so`, inspected:

```
$ readelf -d libpdfspike.so | grep NEEDED
 (NEEDED)  Shared library: [libdl.so]
 (NEEDED)  Shared library: [libc.so]
$ nm -D --undefined-only libpdfspike.so | grep -c FPDF
0
$ nm -D libpdfspike.so | grep -E ' U (dlopen|dlsym)'
                 U dlopen@LIBC
                 U dlsym@LIBC
```

Zero undefined `FPDF*` symbols and no new `DT_NEEDED` entry. `pdfium-render`'s
default binding mode is `libloading` — `dlopen("libpdfium.so")` at run time,
then `dlsym` per function. Its `build.rs` links nothing unless the `static`
feature is on, and even then it only emits `cargo:rustc-link-lib=static=pdfium`
if you hand it `PDFIUM_STATIC_LIB_PATH`. Compiling for Android is therefore
free; **the entire cost is at run time and in the package.**

### Where the binary comes from, and what has to be vendored

The `pdfium-binaries` release for `chromium/8021` (published 2026-08-25) was
downloaded and unpacked. `pdfium-android-arm64.tgz` is 3,357,611 B and
contains:

- `lib/libpdfium.so` — **6,396,288 B, already stripped**
- the `fpdf*.h` headers (not needed for the dynamic path)
- `LICENSE`, and a `licenses/` directory with 16 third-party licence texts
- `args.gn`, recording exactly how it was built

```
$ file lib/libpdfium.so
ELF 64-bit LSB shared object, ARM aarch64, for Android 23,
built by NDK r30-beta2, BuildID[xxHash]=afaadcb6fd9dcfe3, stripped
$ readelf -d lib/libpdfium.so | grep -E 'NEEDED|SONAME'
 (NEEDED)  libdl.so
 (NEEDED)  libm.so
 (NEEDED)  libc.so
 (SONAME)  libpdfium.so
```

Three good facts fall out of that: it needs **no `libc++_shared.so`** (libc++
is linked in statically), its `default_min_sdk_version = 23` is below this
app's `minSdkVersion="28"`, and it is already stripped so there is nothing to
save.

`args.gn` confirms the build has `pdf_enable_v8 = false` and
`pdf_enable_xfa = false` — no JavaScript engine, no XFA forms. That matters:
the V8 variant of the same library is 11,447,958 B compressed, roughly 3.4×
larger, and would drag a JS engine into an app whose entire pitch is that it
does nothing behind your back. **The non-V8 build is the only one worth
considering here.**

**There is no `libpdfium.a` in the release.** Every one of the 58 assets is a
shared object. So `pdfium-render`'s `static` feature is not reachable without
building PDFium from source, which means `depot_tools`, `gn`, `ninja` and a
Chromium-scale checkout. That is not a thing this repository is going to do.
The dynamic path is the only path.

What would have to be vendored: **one 6.4 MB binary file**, plus its 16
licence texts, plus a recorded checksum. Either committed to this repository
(6.4 MB of binary in git, forever, per PDFium version bump) or fetched by a
script at build time (which makes the build require the network, and makes
`build-apk.sh` fail differently when GitHub is down).

### Can `build-apk.sh` package a second `.so`?

**Yes, trivially — it already builds a directory and zips it.** The change is
one line next to the existing copy:

```bash
mkdir -p "$APK_DIR/lib/$ABI"
cp "$SO_PATH" "$APK_DIR/lib/$ABI/"
cp "$PDFIUM_SO" "$APK_DIR/lib/$ABI/"          # ← the whole change
```

That was done, an APK was built, and `apksigner` signed it without complaint:

```
lib/arm64-v8a/libpdfium.so        6,396,288 → 3,235,278 deflated
lib/arm64-v8a/libsetlistarray.so 21,403,664 → 6,780,130 deflated
```

The script's ABI variable already parameterises the directory, so
`--target x86_64` would work the same way given the matching tarball. Nothing
about the one-native-library assumption is load-bearing.

---

## 2. APK size

Three APKs, each built by `./build-apk.sh --build-only` for `arm64-v8a`,
release, and measured with `stat`:

| Build | APK bytes | Δ from today |
| --- | ---: | ---: |
| Today (baseline) | 6,799,883 | — |
| + `libpdfium.so` packaged, no Rust change | 10,031,723 | +3,231,840 |
| + `pdfium-render` and `image` in the crate | **10,920,555** | **+4,120,672** |
| `hayro` and `image` in the crate, no blob | **9,413,131** | **+2,613,248** |

Split by cause, for the pdfium route:

- **+3,231,840 B** — `libpdfium.so`, deflated in the APK. Irreducible; the
  file is already stripped and the non-V8 build is already the small one.
- **+889,030 B** — `libsetlistarray.so` growing 21,403,664 → 25,962,232
  uncompressed as `pdfium-render` (and `image`, for the PNG encoder) join the
  tree.

For hayro the whole cost is in one place: `libsetlistarray.so` goes
21,403,664 → 28,965,096 uncompressed, 6,780,130 → 9,396,176 deflated.

### Two size notes that are not about PDF at all

**`build-apk.sh` ships an unstripped `.so`.** `libsetlistarray.so` is 21.4 MB
uncompressed for a 6.5 MiB APK. Running the NDK's `llvm-strip` over it:

| | uncompressed | deflated |
| --- | ---: | ---: |
| baseline, as shipped | 21,403,664 | 6,780,130 |
| baseline, stripped | 14,964,736 | 5,593,181 |
| hayro build, as shipped | 28,965,096 | 9,396,176 |
| hayro build, stripped | 20,262,680 | 7,847,844 |

**Stripping recovers 1.13–1.48 MiB** — between a third and a half of hayro's
entire cost, and more than a quarter of pdfium's. It is a one-line change to
`build-apk.sh` and it is worth doing whatever is decided here. (It is not
proposed as part of this card; it is noted because it changes what the size
numbers above mean.)

**The APK deflates its native libraries**, so they are extracted at install.
The manifest sets no `android:extractNativeLibs`, and there is no AGP here to
inject `false`. Installed footprint of native code is therefore the
*uncompressed* column: 21.4 MB today, 32.4 MB with PDFium (25.96 + 6.40),
29.0 MB with hayro. If on-device footprint matters more than download size,
that reverses part of the comparison — and `extractNativeLibs="false"` plus
`zipalign -p 4` is the fix, again independent of this decision.

---

## 3. The desktop story

Easier, but not free, and it has a trap in it.

`libpdfium.so` for `linux-x64` is 7,684,576 B as shipped and **not** stripped;
`strip -s` takes it to **6,548,192 B** and it still loads and renders
correctly (verified). There is no distribution package — `ldconfig -p` finds
nothing and neither Debian nor Ubuntu ship one — so "the user already has it"
is not available. The app would ship it.

**The trap:** `Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))`,
which is the recipe in `pdfium-render`'s own README, resolves relative to the
**current working directory**, not to the executable. Measured:

```
=== from /tmp (no libpdfium) ===
bind_to_system_library : Err(DlOpen { "libpdfium.so: cannot open shared object file" })
bind ./ (cwd-relative) : Err(DlOpen { "./libpdfium.so: cannot open shared object file" })
bind next to exe       : Err(DlOpen { ".../target/release/libpdfium.so: ..." })

=== from a cwd containing libpdfium.so ===
bind ./ (cwd-relative) : Ok
bind next to exe       : Err(...)
```

`cargo run` from the repository root would work, and the installed app launched
from a desktop entry would not. Any real use has to resolve against
`std::env::current_exe()` and then fall back to `bind_to_system_library()`, and
the packaging story becomes "install a 6.5 MB `.so` next to the binary, or into
a directory on the loader path". For a project that currently ships as
`cargo run --release`, that is a new kind of thing to have to say.

hayro has none of this. It is a crate; it links; there is nothing to find.

---

## 4. Licence

**PDFium is BSD-3-Clause** (`licenses/pdfium.txt` in the release tarball, and
the upstream `LICENSE` at pdfium.googlesource.com). Permissive, no copyleft,
no source-disclosure obligation.

**The prebuilt binary carries 16 licence texts**, all of them permissive:
abseil (Apache-2.0), agg23, catapult, cpu_features, fast_float, freetype (FTL),
icu, lcms, libjpeg_turbo (IJG + BSD), libopenjpeg, libpng, libunwind,
llvm-libc, pdfium, simdutf, zlib. **Nothing copyleft is in the shipped
binary**, and because the build has V8 and XFA off, none of that surface is
present either.

**`pdfium-binaries` itself is MIT** (Benoît Blanchon), covering the build
scripts rather than the output. **`pdfium-render` is MIT OR Apache-2.0.**

**Neither infects this app.** What they do impose is **attribution**:
BSD-3-Clause requires the copyright notice and licence text to be reproduced
with the binary. That is a real, new obligation — this app has no about screen
and no notices file today, so choosing PDFium means Settings (card H1) grows a
licences page and the repository grows sixteen licence texts. It is small work
and it is not optional.

For comparison: **MuPDF is AGPL-3.0** and so is `mupdf-rs` (both the `mupdf`
and `mupdf-sys` crates). The plan's read of it is correct and unchanged.
**hayro is Apache-2.0 OR MIT**, which is the same obligation this app already
has for every other crate in its tree — a line in a notices file that does not
exist yet either, but no new *kind* of obligation.

### The trust question

Choosing PDFium means shipping, inside this app's APK, **6.4 MB of compiled
C++ that nobody in this project built, read or can practically audit**, from a
GitHub account that is not Google's. That sits awkwardly next to an app whose
README argues from a manifest with one permission in it and a test that counts
the permissions.

It is not as bad as it sounds, and the mitigations are real:

- The release ships **`pdfium-attestation.json`**, a GitHub build-provenance
  attestation. `slsa-verifier` can tie the binary back to the workflow run and
  the source commit that produced it. That is meaningfully better than a
  checksum on a download page.
- The build is reproducible in principle: `args.gn` ships alongside the binary
  and records the exact configuration.
- The code inside it is Google's, is the PDF engine in Chrome, and is one of
  the most-fuzzed C++ codebases in existence.

But three things stay true. The attestation proves *provenance*, not *intent*
— it says this binary came from that workflow and that source, not that the
source is benign. The binary parses **hostile input** (a PDF a user was handed)
in C++ **in this app's process**, with no sandbox; Android's own docs recommend
isolating `PdfRenderer` into a separate process for exactly this reason, and an
app cannot do that from a `NativeActivity`. And verification has to actually be
wired into the build, or it is a paragraph in a document rather than a control.

hayro, by contrast, is `#![forbid(unsafe_code)]` Rust parsing the same hostile
input. A parser bug there is a panic, not a heap primitive. For an offline app
whose whole argument is "you can check what this does", that is not a small
difference.

---

## 5. Rasterise-on-import, checked against reality

The plan proposes: rasterise each page to a cached PNG at import, show it
through Rinch's `Image`. **The shape is right.** The numbers are better than
one might fear and the encoding choice matters more than the renderer choice.

Four documents were used. Two are real files from this machine; two were built
for the test because a chord chart is not a technical manual:

- **`chartbook30.pdf`** — 30 pages, generated: chords over lyrics in Courier,
  section headers, metadata line, base-14 fonts **not embedded**. This is what
  a chart exported from a text tool looks like, and the non-embedded fonts are
  deliberate — they are the case that stresses font substitution.
- **`scanbook30.pdf`** — the same 30 pages as 200 dpi greyscale JPEGs wrapped
  in a PDF, 6.3 MB. The photocopied-Real-Book case.
- `/usr/share/doc/fig2dev/manual.pdf` — 22 pages, embedded Type 1 fonts,
  vector figures.
- `/usr/share/doc/shared-mime-info/shared-mime-info-spec.pdf` — 19 pages.

Pipeline: render → `into_luma8()` → PNG via the `image` crate at default
settings, written to disk. Widths chosen to match the app: **320 px** is about
the attachment card, **1080 px** is a full-bleed page on a 393 pt-wide phone at
2.75×, **1600 px** gives headroom to pinch-zoom.

### PDFium, 1080 px wide

| Document | ms/page (wall) | of which render | cache/page | 30-page book |
| --- | ---: | ---: | ---: | ---: |
| chart book (text) | 6.0 | 5.1 | 104 KiB | **3.05 MiB** |
| fig2dev manual | 6.7 | 5.6 | 127 KiB | 3.7 MiB equiv. |
| mime-info spec | 6.5 | 5.6 | 131 KiB | 3.8 MiB equiv. |
| scanned book | 25.6 | 24.2 | 166 KiB | **4.85 MiB** |

At 320 px: 0.9–1.5 ms/page and 18–21 KiB/page. At 1600 px: 14.1 ms/page and
182 KiB/page for the chart book.

Fixed costs are negligible: `dlopen` + bind is **0.3–0.6 ms**, and opening a
document is 60 µs to 1.8 ms.

### hayro, 1080 px wide, same pipeline

| Document | render ms, 30/22/19 pages | vs PDFium |
| --- | ---: | ---: |
| chart book | 162.6 ms | PDFium 154.0 ms |
| fig2dev manual | 111.8 ms | PDFium 122.2 ms |
| mime-info spec | 106.6 ms | PDFium 106.1 ms |
| scanned book | 365.7 ms | PDFium 726.0 ms |

hayro's README says "no effort has been put into performance optimizations".
On these documents it does not need any: it is within 6 % of PDFium on text
and **twice as fast** on the scanned book. (The encode column in the raw runs
is slower for hayro only because the harness copies its `Pixmap` through an
extra RGBA `Vec`; that is the harness, not the crate.)

### Fidelity: hayro vs PDFium, measured

Every page of every document, rendered by both at 1080 px, greyscale,
per-pixel absolute difference:

| Document | pages | mean \|diff\| | px differing >32/255 | px differing >128/255 |
| --- | ---: | ---: | ---: | ---: |
| chart book | 30 | 2.01 / 255 | 2.41 % | 0.14 % |
| fig2dev manual | 22 | 2.04 / 255 | 2.33 % | 0.12 % |
| mime-info spec | 19 | 2.22 / 255 | 2.66 % | 0.08 % |
| scanned book | 30 | 0.37 / 255 | 0.00 % | 0.00 % |

That is glyph-edge antialiasing and nothing else. Both renderers were also
inspected by eye at 1080 px: the chart book's chords, lyrics and section
headers are correct in both, **including the non-embedded base-14 fonts**, and
the fig2dev manual's embedded Computer Modern Type 1 text renders correctly in
both.

### The encoding choice is worth more than the renderer choice

The obvious implementation — `bitmap.as_image().into_rgb8()`, PNG — produces:

| Encoding of the same 30-page chart book at 1080 px | total | per page |
| --- | ---: | ---: |
| RGB8 PNG (the naive path) | 8.35 MiB | 285 KiB |
| **Luma8 PNG, `image` crate defaults** | **3.05 MiB** | **104 KiB** |
| greyscale PNG, maximum compression | 2.27 MiB | 77 KiB |
| greyscale WebP q80 | 2.05 MiB | 70 KiB |
| **16-colour palette PNG** | **1.30 MiB** | **44 KiB** |
| greyscale JPEG q80 | 3.46 MiB | 118 KiB |

**A chart is black ink on white paper.** Storing it as 24-bit colour costs 6.4×
what a 16-colour palette costs for output that is visually identical. A
30-page chart book is **1.3 MiB, not 8.4 MiB**, if the cache is encoded for
what it actually contains. Against a stated budget of 300 songs, that is the
difference between a plausible cache and an unusable one.

### A gotcha that cost an hour, recorded so it does not cost another

**`PdfRenderConfig::set_format(PdfBitmapFormat::Gray)` renders a blank page.**
Not an error — a correctly-sized, entirely white bitmap:

```
BGRA (default)             fmt=BGRA 1080x1398 raw_len=6039360 non-0xff=215277
Gray fmt only              fmt=Gray 1080x1398 raw_len=1509840 non-0xff=0
Gray fmt + grayscale flag  fmt=Gray 1080x1398 raw_len=1509840 non-0xff=0
BGRA + grayscale flag      fmt=BGRA 1080x1398 raw_len=6039360 non-0xff=215277
```

The buffer length is right (`w*h`), so this is PDFium refusing to draw into an
8-bpp destination, not a stride bug in the wrapper. `pdfium-render`'s
`as_image()` will happily hand back the blank `GrayImage`. The working recipe
is **render BGRA, convert to `Luma8` in Rust** — which costs a 4-byte-per-pixel
intermediate but is what produced every PDFium number above. `FPDF_GRAYSCALE`
(`use_grayscale_rendering(true)`) works fine, but only into a BGRA buffer.

### What a 30-page chart book actually costs

On this desktop, at 1080 px, Luma8 PNG:

- **import time: 180 ms** for a text chart book, **767 ms** for a scanned one
  (PDFium); 409 ms / 615 ms for hayro.
- **disk: 3.05 MiB**, or **1.3 MiB** with a palette encoder.

A phone is slower. **This was not measured and cannot be** — the honest bound
is that a mid-range phone's single-core throughput is commonly 3–8× below a
9955HX, which would put a 30-page import somewhere between half a second and
six seconds. That is a progress-bar-shaped number, not a freeze-the-UI-shaped
one, but it is an extrapolation and should be treated as one. Peak memory is
bounded and small: one page bitmap at 1600 px is 1600×2071×4 ≈ 13 MB.

**Rasterise-on-import is the right shape either way.** Both renderers are fast
enough to rasterise lazily instead — page one at import for the card, the rest
on first open — and that is probably the better design, because it makes
import instant and spreads the cost over pages the user actually looks at.

---

## 6. The alternatives, fairly

### Deferring to the system viewer

**On Android this is not free, and it is not two lines.**

The app stores attachments in app-private internal storage
(`/data/data/dev.lostconnection.setlistarray/files/attachments/…`). Handing
that path to another app with `Intent.ACTION_VIEW` and a `file://` URI throws
**`FileUriExposedException`** — enforced unconditionally for `targetSdk ≥ 24`
by StrictMode, and this app targets 35. So:

1. **A `FileProvider` is required.** That is a `<provider>` element in
   `AndroidManifest.xml`, an XML paths resource, `grantUriPermissions="true"`,
   and `FLAG_GRANT_READ_URI_PERMISSION` on the intent. It needs **no new
   `<uses-permission>`** — the one-permission promise survives intact, which is
   worth saying plainly. But it *is* a new manifest element, and
   `the_android_manifest_asks_only_for_internet` only counts permissions, so
   the manifest would grow something the tests do not currently watch.
2. **The androidx `FileProvider` class is not reachable here.** `build-apk.sh`
   runs `javac` over three `.java` files from the Rinch checkout and `d8` over
   the result. There is no Gradle, no dependency resolution, no AAR handling.
   Using androidx would mean adding one; the realistic alternative is a
   hand-written `ContentProvider` subclass of ~60 lines living in this
   repository, which is the first Java this project would own.
3. **There is no way to fire an intent.** `rinch-android` exposes clipboard,
   IME, file picker, share, camera, location, sensors, notifications,
   permissions and display — **not `startActivity`**. `pick_file` and `share`
   exist; a general `ACTION_VIEW` does not. That is an upstream contribution
   or a JNI call written here.
4. **It may resolve to nothing.** Not every Android device ships a PDF viewer.
   `ACTION_VIEW` with no handler throws `ActivityNotFoundException`, which is
   an error screen this app would have to design.

So "defer" on Android is: a `<provider>`, a Java class, a JNI bridge, and a
failure state. Call it S–M, not the zero it looks like.

**On the desktop it genuinely is nearly free.** `xdg-open` is present, and
`xdg-mime query default application/pdf` resolves (Okular, here). It is a
`Command::new("xdg-open").arg(path).spawn()`. Worth noting only that spawning
a process is the second thing after the HTTP client that reaches outside this
app, and the "one call site" framing in the README is about the network
specifically — but a reviewer reading that section will want the distinction
written down rather than assumed.

**What it costs the user**, which is the part that matters:

- The chart opens **in another app, in another window**. On the desktop that is
  a context switch mid-song; on a phone it is leaving SetListArray entirely and
  having to navigate back.
- **Card D5's attachment viewer would not handle PDFs.** Its whole design —
  full-screen dark chrome, page prev/next, zoom, rotate, auto-hiding chrome —
  applies to typed text and captured pages only. The most common chart format
  is the one it cannot show.
- **Phase F is worse.** Performance mode is `2 / 5`, a full-bleed chart, swipe
  between songs, keep-awake. A PDF chart cannot be in that flow at all: it is
  a song whose chart lives in another app. Swiping to it would show a card that
  says "open in viewer". That is the real cost of deferring — not a missing
  feature in Phase D, but a hole in the middle of the feature the app exists
  for.
- The attachment card shows **no thumbnail**, because there is nothing to draw.
  The library's row thumbs (which `scripts/screenshot.sh` samples) would be
  blank for every PDF-only song.

### An option decision 3 does not list: Android's own PDF renderer

`android.graphics.pdf.PdfRenderer` has existed since **API 21**, and is present
in the `android-35` platform jar this build already links against (verified
with `javap`). It renders a page into a `Bitmap` from a seekable
`ParcelFileDescriptor`. For a file in the app's own internal storage it needs
**no permission at all**.

It is, underneath, PDFium — the same engine, shipped by the platform, at **zero
APK bytes**.

The catch is that it is Java, so it needs a JNI bridge and a Java shim, neither
of which `rinch-android` has; the desktop still needs a renderer of its own, so
this is an addition to one of the other options rather than a replacement for
them; and it is constrained (not thread-safe, one page open at a time, and the
richer API — `searchText`, `getTextContents`, which would have been interesting
for card G2 — arrived at API 35, well above this app's `minSdk` of 28).

It is recorded here because "PDFium on Android, for free, with no blob" is a
real thing that exists and the decision should know about it. A hybrid —
`PdfRenderer` on Android, hayro or `pdfium-render` on the desktop — pays the
size cost on the platform where it matters least. It also means two renderers
and two sets of output to keep consistent, which is its own kind of expensive.

### Has the pure-Rust picture changed? Yes, decisively

Checked against crates.io on 2026-08-26 rather than repeated from the plan:

| Crate | Version | Updated | Downloads | Rasterises? | Licence |
| --- | --- | --- | ---: | --- | --- |
| **`hayro`** | **0.7.1** | 2026-06-05 | 1,677,740 | **yes** | Apache-2.0 OR MIT |
| `hayro-interpret` | 0.7.0 | 2026-05-15 | 2,011,032 | (engine) | Apache-2.0 OR MIT |
| `hayro-syntax` | 0.7.2 | 2026-05-28 | 2,031,954 | (parser) | Apache-2.0 OR MIT |
| `pdf-render` | 1.0.0-beta.18 | 2026-08-14 | 1,092 | claims to | — |
| `lopdf` | 0.44.0 | 2026-07-10 | 16,813,698 | no — object model only | MIT |
| `oxidize-pdf` | 4.7.0 | 2026-08-25 | 71,067 | no — extraction for RAG | — |
| `mupdf` | 0.8.0 | 2026-06-22 | 1,590,845 | yes | **AGPL-3.0** |

`hayro` is by Laurenz Stampfl (also `krilla`), builds on `vello_cpu`, and is
`#![forbid(unsafe_code)]`. Its regression suite is over 1000 PDFs scraped from
the `pdf.js` and PDFBox suites. It was built into this app, cross-compiled for
`aarch64-linux-android`, packaged into a signed APK, and benchmarked against
PDFium page by page — all of that is in sections 2 and 5 above.

Two claims about it need correcting before they get repeated:

- **Its README understates it.** The README says "lack of support for
  encrypted/password-protected PDF files". That is out of date for 0.7:
  `hayro-syntax` ships `crypto/{rc4,aes,md5,sha256,sha384,sha512}.rs`,
  implements the standard security handler for revisions 2–6 (RC4-40/128,
  AESV2, AESV3), and exposes `Pdf::new_with_password`. Owner-password-encrypted
  charts — the print-restricted PDFs chord sites hand out — are handled.
- **It is not a toy.** 1.68 M downloads on the top-level crate and 2 M on the
  parser, released steadily through 2026.

What is still true: it is 0.x, it says so, and it names its gaps (blending and
isolation, knockout groups, colour-key masking, non-embedded CID fonts). None
of those appeared in the four documents tested, and none of them is
characteristic of a chord chart. But a 0.x renderer will meet a PDF it draws
wrong, and the mitigation is that a PDF it draws wrong is a *visual* bug in a
memory-safe crate, not a parser exploit in C++.

`mupdf-rs` is AGPL on both crates and the plan's assessment stands unchanged.

---

## 7. What only a device can settle

Nothing below was observed. Each is a real risk and each is cheap to check
once hardware exists.

**Both renderers:**

1. **Import time on a real phone.** Every timing here is a 9955HX. The
   extrapolation to 0.5–6 s for a 30-page book is arithmetic, not a
   measurement.
2. **Whether Rinch's `Image` will display a cached PNG from app-private
   storage at all.** `docs/CAPTURE.md` already flags that "loading local
   `assets/` files as image sources is untried" for card E5. It is untried here
   too, and it is on the critical path for either renderer — a PDF that
   rasterises perfectly into a file the UI cannot show is not a feature. This
   is arguably the largest unknown in the whole card and it is *not* about PDF.
3. **Rasterising off the UI thread.** Rinch's main-thread callback parking is
   untried in this app (same note in CAPTURE.md). A 6-second import on the UI
   thread is an ANR.

**PDFium specifically:**

4. **Whether `dlopen("libpdfium.so")` finds the APK-bundled library.** The
   reasoning is sound — Android's linker namespace for an app includes its own
   native library directory, which is exactly how `System.loadLibrary` works —
   and `Pdfium::bind_to_system_library()` does a bare `dlopen` by soname, which
   should resolve there. **It has not been observed.** If it does not, the
   fallback is to pass the absolute path from
   `ApplicationInfo.nativeLibraryDir` over JNI, which `rinch-android` does not
   expose.
5. **Font substitution for non-embedded fonts.** The Android build contains the
   "Chrome Sans MM" multiple-master substitution font and scans `/system/fonts`
   (both confirmed by `strings`), so base-14 charts should substitute the way
   they did on the desktop. Should.
6. **The version skew.** `pdfium-render` 0.9.3's `pdfium_latest` feature means
   `pdfium_7881`; the binaries measured are `chromium/8021`. The `FPDF_*` C API
   is stable across these and everything worked on the desktop, but the
   bindings were generated against different headers and that is a mismatch
   somebody should be aware of before it produces a confusing crash.

**Deferring specifically:**

7. **Whether the target device has a PDF viewer at all.**

---

## 8. The trade-off, as I read it

Written before the decision and left as it was. The owner chose hayro.

**PDFium's case** is maturity. It is the PDF engine in Chrome, it is the one
that renders the PDF that nothing else renders, and choosing it means never
wondering whether a chart looks wrong because of the renderer. Its cost is
3.93 MiB of APK, a 6.4 MB unauditable C++ blob parsing hostile input in this
app's process, a `dlopen` recipe unverified on device, a desktop packaging
problem, and sixteen licence texts.

**hayro's case** is that it made PDFium's case weaker than the plan assumed.
It is 1.5 MiB cheaper in the APK, comparable in speed, indistinguishable in
output on everything tested, has no blob, no `dlopen`, no cwd trap, no
packaging story on either platform, and forbids `unsafe`. Its cost is that it
is 0.x software with a list of known gaps, and it will eventually draw
something wrong. Its escape hatch is good, though: the two renderers have the
same shape — bytes in, bitmap out — so a module that hides one behind a
function can swap to the other later, and the cached PNGs are just files that
can be re-rasterised.

**Deferring's case** is that it costs nothing today. It is weaker than it
looks, because on Android it is not actually free (a `<provider>`, a Java
class, a JNI bridge, a failure state), and because the thing it defers is not a
Phase D feature but the middle of Phase F. A performance mode that cannot show
the most common chart format is not a performance mode.

If a fourth option is wanted: **rasterise nothing yet, but decide the cache
format now.** The single most consequential number in this document is that the
same 30 pages are 8.35 MiB or 1.30 MiB depending on how they are encoded, and
that choice is independent of which renderer makes them.

---

## Appendix: what was run

Everything lives in the scratchpad, not the repository.

- `pdfium-binaries` `chromium/8021`, `pdfium-android-arm64.tgz` and
  `pdfium-linux-x64.tgz`, downloaded and unpacked; `.so` inspected with
  `file`, `readelf -d`, `readelf -S`, `nm -D`, `strings`.
- A throwaway crate with `pdfium-render = "0.9.3"` and `hayro = "0.7"`:
  `cargo build --release` on the host, `cargo ndk -t arm64-v8a build --release`
  for Android. Benchmarks over four documents at three widths, output PNGs
  written and inspected, and a per-pixel diff between the two renderers.
- `Cargo.toml`, `src/lib.rs` and `build-apk.sh` in this repository temporarily
  edited three times to build three real APKs, then restored from backups.
- Verified afterwards: `git status` empty, `git diff` empty, still on `master`,
  `setlistarray.apk` restored byte-for-byte to the 6,799,883 B baseline,
  `cargo build --release` clean, `cargo test --release` 148/148,
  `scripts/screenshot.sh` 5/5.
