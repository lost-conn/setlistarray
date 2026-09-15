# SetListArray

An offline-first book of the songs you know how to play, with the chart you
read from attached to each one. No account, no sign-in, no sync, and nothing
about your book leaves the device.

## The questions it answers

**"Play something."** Someone hands you a guitar and the question gets hard
at exactly the moment it is asked. The library is that question answered in
advance: every song can be marked solid, rusty or still learning, and the
library opens already sorted that way — the ones you could start this second,
then the ones that want a run-through first. Mark them as you go, and sort by
what you have not played in months when you want to bring something back.

**"There's no signal here."** The campsite up a fire road, the basement, the
green room with one bar. Charts live on the device rather than on a page that
has to load: PDFs drawn a page at a time with zoom and rotate, webpages you
pasted in once while you had signal and the app has kept ever since, and
lyrics and chords you typed yourself in a monospaced field, so the chords stay
over the syllables you put them over. A song can hold more than one chart —
the fingerstyle arrangement and the three-chord version of the same tune — and
opens on whichever you made primary.

**"I remember the second verse. I don't remember the title."** Search reads
the words *inside* your charts, typed and captured alike, as well as titles,
artists and setlists. Half a line is enough.

**"How long does the set actually run?"** Build it, put it in the order you
want, and watch the running time add up beside it — so you find out at home
that you are nine minutes short, not on stage. A song with no chart attached
is flagged while you can still do something about it. Then one tap starts the
set: the chart fills the screen at a size you can read from a music stand, the
top bar says where you are in the set, the bottom says what is coming next,
and the screen stays awake until the set is over.

## What it does

- **A song** needs a title and an artist. Key, capo, tuning, tempo, length,
  tags, notes, when you last played it and how solid you are on it are all
  optional, and the form keeps them folded away until you want them.
- **Charts**, three kinds, any number per song: PDFs, webpages captured for
  offline reading, and typed lyrics and chords. One is the primary chart and
  is what the song opens on.
- **Capture** takes a URL once and never asks the network about it again. You
  choose reader text or the whole page *before* a byte is saved. What survives
  the trip is narrower than "any chord site" — [docs/CAPTURE.md](docs/CAPTURE.md)
  has the table of what does.
- **Search** across titles, artists, setlists and the text inside charts, plus
  filters on confidence, tag, tuning and whether a song has a chart at all.
- **Setlists** with positions, a cumulative start-time column, a warning on
  songs with nothing to read from, and a derived "Before you start" panel.
- **Performance mode**: the chart full-screen, set position top and bottom, a
  darker theme of its own for a dim stage, and keep-screen-awake.
- **Backup** is one `.zip` of the database and every attachment, exported
  wherever you keep things that matter and imported back onto the same phone
  or a new one.
- **It can look like the rest of your phone.** The accent colour follows your
  Material You wallpaper palette or is picked by hand; dark mode follows the
  system or is set outright.

## Privacy

SetListArray asks Android for one permission, `INTERNET`, and uses it for
exactly one thing: fetching a webpage you pasted in yourself, so that it still
opens when the venue has no signal. That is one call site, and a test holds it
to being the only file under `src/` that names an HTTP client at all. Another
test fails the build if a second `<uses-permission>` ever appears in the
manifest.

There is no analytics, no crash reporting, no advertising and no sign-in. Your
songs live in the app's own private storage on the device, cloud backup is
switched off, and the book is backed up when you export it and not before. The
reasoning, and the two options not taken, are under "One permission, on
purpose" in [docs/ANDROID.md](docs/ANDROID.md).

## Building it

```bash
cargo run --release          # always --release; debug Stylo/Parley is slow
cargo run --release -- --seed   # ...with the demo library, if yours is empty
./build-apk.sh               # the APK, installed and launched on a device
```

Three things are worth knowing before you try:

- **Stable Rust**, pinned by `rust-toolchain.toml` along with the two Android
  targets. Rinch's own docs ask for nightly; current main does not need it.
- **`mold` and `clang` on `PATH`.** `.cargo/config.toml` links host builds with
  mold, which turns a relink of this dependency tree into well under a second.
  Delete that file to build without them; it only overrides the host triple, so
  the Android build goes through the NDK's linker either way.
- **It will not build from a fresh clone on its own.** Both frameworks are
  local path dependencies, so `Cargo.toml` expects three checkouts side by
  side — this one, `rinch-fixes` and `rhypedb-main`. The pins are deliberate
  and temporary, and [docs/BUILDING.md](docs/BUILDING.md) says what they are
  and why.

[docs/BUILDING.md](docs/BUILDING.md) is the long version of all of that, the
Android prerequisites, and the generated launcher icon.

## Where everything else is

| Document | What is in it |
| --- | --- |
| [docs/BUILDING.md](docs/BUILDING.md) | Running it on the desktop, the sibling checkouts, the Android NDK prerequisites, `build-apk.sh`, the launcher icon. |
| [docs/TESTING.md](docs/TESTING.md) | The pixel-level visual regression net, why its sampled regions are anchored to an edge rather than pinned to a coordinate, and a map of the source tree. |
| [docs/ANDROID.md](docs/ANDROID.md) | What running on real hardware settled, what is still broken there, the one permission, and why every affordance in this app is a tap. |
| [docs/RINCH.md](docs/RINCH.md) | Every fault found in the GUI framework underneath this app, and the pull request carrying each fix. |
| [docs/NOTES.md](docs/NOTES.md) | The things this codebase knows that are not obvious from reading it, including three about rhypedb, and where the design handoff went. |
| [docs/RELEASING.md](docs/RELEASING.md) | How a tag becomes a build on Play, and how the store listing is built out of this repository. |
| [docs/CAPTURE.md](docs/CAPTURE.md) | The offline webpage capture engine: what it fetches, what it throws away, and which sites survive it. |
| [docs/PDF.md](docs/PDF.md) | Choosing a PDF rasteriser, with the measurements. |
| [docs/PLAN.md](docs/PLAN.md) | The phased build plan, and what is left of it. |

Built on [Rinch](https://github.com/joeleaver/rinch), a Rust GUI framework, and
[rhypedb](https://github.com/joeleaver/rhypedb), an embedded graph database.
One crate builds both the desktop binary and the Android `cdylib`; there is no
`#[cfg]` in any screen.

## License

SetListArray is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version. It is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See [LICENSE](LICENSE) for the full text.

GPLv3 rather than the MIT/Apache pair most of the Rust tree carries, and for one
reason: a modified SetListArray shipped to anyone has to ship its source with
it. Every dependency is permissive or MPL-2.0, and all of them fold into a
GPLv3 program.

The license covers the code, not the name. "SetListArray" and the icon in
`assets/icon/` say which app this is, and a fork is welcome under a name and an
icon of its own. A copy on Play wearing this one's is exactly what the GPL does
not prevent — anyone may redistribute the app, even for money, so long as the
source goes with it — and what the store's impersonation policy does.

The bundled fonts keep their own licenses, in `assets/fonts/licenses/`: the SIL
Open Font License 1.1 for Newsreader and Karla, and the Bitstream Vera terms for
DejaVu Sans Mono.

The app shows all of this itself, under Settings → About → Licenses: its own
notice and the GPL's text, then the notice of every crate it is built from,
which MIT, BSD and Apache all require a binary to carry. Those notices are
generated, not written:

```bash
cargo install cargo-about --features cli --locked   # once
python3 scripts/make-notices.py                     # after anything that changes Cargo.lock
```

It writes `src/licenses/third_party.rs` from each crate's own license files, and
refuses to if any crate a build actually compiles came back without one. A test
fails whenever `Cargo.lock` has moved since the file was generated.
`about.toml`'s `accepted` list is every license a dependency may carry — each of
them compatible with GPLv3 — so a crate under anything else stops the generator
rather than joining the APK.
