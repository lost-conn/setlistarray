# Building it

How to get SetListArray running, on the desktop and on a phone. Moved out of
the README when that became a front page: the README now names only the three
things that stop a fresh clone from building, and this is the long version of
all of them.

---

## Running it

```bash
cargo run --release          # always --release; debug Stylo/Parley is slow
cargo run --release -- --seed   # ...with the demo library, if yours is empty
```

The typefaces are in the binary (`crate::FONTS`), so nothing has to be
installed first. `scripts/install-fonts.sh` still puts Newsreader and Karla on
the system for other tools that want them; the app no longer needs it.

The library lives in `$XDG_DATA_HOME/setlistarray` (`~/.local/share/setlistarray`):
a rhypedb database in `db/`, and one directory per attachment under
`attachments/`. A first run opens an empty book; `--seed` fills it with the demo
content, and only ever into a library that has nothing in it.

Requires stable Rust (`rust-toolchain.toml` pins it, along with the two Android
targets). Rinch's docs ask for nightly; current main does not need it.

Also requires `mold` and `clang` on PATH: `.cargo/config.toml` links host builds
with mold, which turns a relink of this dependency tree into well under a second.
Package names are `mold` and `clang` on Debian/Ubuntu, Fedora and Arch alike. To
build without them, delete `.cargo/config.toml` — it only overrides the host
triple, so the Android build goes through the NDK's linker either way.

**This will not build from a fresh clone on its own.** Both frameworks are
local path dependencies, so `Cargo.toml` expects three checkouts side by side:

```
projects/personal/
├── setlistarray/     ← this
├── rinch-fixes/      ← github.com/joeleaver/rinch, branch carrying #245, #246, #266, #267, #270, #274, #281, #286, #292, #298, #317, #342, #344, #353, #402, #417
└── rhypedb-main/     ← github.com/joeleaver/rhypedb, main
```

Both pins are deliberate and temporary — see "The Rinch contributions" in
[RINCH.md](RINCH.md), and card A1. When those PRs land, `rinch` goes back to a
git revision; rhypedb stays a path dep while this app is its early in-process
consumer.

**The pin is deliberately not on `main`.** See "The viewport scale fault" in
[RINCH.md](RINCH.md).

The handoff targets Android, and Rinch has an Android backend, so the app runs
on both — it has been on a phone since. On the desktop it runs in a 393×852
phone-shaped window;
nothing in the UI code assumes either platform. See [ANDROID.md](ANDROID.md).

## Android

Everything the APK itself needs. What the app *does* once it is on a phone —
what the hardware settled, what is still open, and the one permission — is in
[ANDROID.md](ANDROID.md).

### Prerequisites

| What | Default the script looks in | Override |
| --- | --- | --- |
| NDK r27c | `~/android/android-ndk-r27c` | `ANDROID_NDK_HOME` |
| SDK build-tools 35 | `~/android/sdk/build-tools/35.0.0` | `ANDROID_SDK_BUILD_TOOLS` |
| `android.jar` (android-35) | `~/android/sdk/platforms/android-35/android.jar` | `ANDROID_SDK_PLATFORM` |
| The Rinch checkout (for `RinchActivity.java`) | `../rinch-fixes` | `RINCH_DIR` |
| Signing key | `target/debug.keystore`, generated on first run | `ANDROID_DEBUG_KEYSTORE` |
| bundletool, `--bundle` only | `~/android/bundletool-all-1.18.3.jar` — the `-all` jar from [its releases](https://github.com/google/bundletool/releases), not the library jar in `~/.gradle/caches` | `BUNDLETOOL_JAR` |
| Upload key, `--bundle` only | none: the bundle is left unsigned, and Play refuses it | `ANDROID_UPLOAD_KEYSTORE`, `ANDROID_UPLOAD_KEY_ALIAS` (default `upload`), `ANDROID_UPLOAD_STORE_PASS`, `ANDROID_UPLOAD_KEY_PASS` |

Also `cargo-ndk` on `PATH` (`cargo install cargo-ndk`), a JDK for `javac`, and
the Android targets — which `rust-toolchain.toml` already declares. `adb` is
needed only to install.

### Building

```bash
export ANDROID_NDK_HOME=$HOME/android/android-ndk-r27c
./build-apk.sh --build-only          # signed APK at ./setlistarray.apk
./build-apk.sh --target x86_64       # for an emulator or Waydroid, not a phone
./build-apk.sh                       # the above, then adb install and launch
./build-apk.sh --bundle --build-only # App Bundle for Play at ./setlistarray.aab
./build-apk.sh --bundle              # the above, then installed through bundletool as Play would
```

**For Play**, `--bundle` is the one to upload. Both endings take their
`versionCode` from `git rev-list --count HEAD` and their `versionName` from
`Cargo.toml`, injected at link time; the manifest deliberately says neither,
because Play refuses a versionCode it has already seen and aapt2 will not
override one the manifest sets. So a bundle for upload wants a committed tree:
two uploads from the same commit are the same versionCode, and the second is
refused. The app targets SDK 36, which Play has required of new apps and
updates since 2026-08-31, and opts out of predictive back to keep the Back key
arriving on Android 16 — the manifest has the why.

Defaults to `arm64-v8a` and the release profile. It builds `--lib` only: the
desktop binary and the probe are not part of the APK. Roughly 6.6 MiB on either
target — 6,935,051 bytes for arm64-v8a, 6,976,008 for x86_64 — almost all of it
`libsetlistarray.so`, a stripped 21 MiB shared object that the zip squeezes to
about a third. It was 5.8 MiB before the capture engine brought html5ever and
its friends in.

### The launcher icon

`android/res/` is generated, not exported. The designed artwork is the three
files in `assets/icon/` — the full-bleed `maskable.png`, the glyph alone on
transparent in `mono.png`, and the rounded-square lockup in `whole.png` — and
`scripts/make-icons.py` turns them into the adaptive icon Android actually
wants:

```bash
python3 scripts/make-icons.py        # needs Pillow and NumPy; nothing else here does
```

It is committed output, so a checkout builds without running it. Re-run it when
the artwork changes.

Two things it does that an export cannot. It **splits the layers**: the design
has the glyph sitting on a radial gradient, and an `<adaptive-icon>` needs those
as separate drawables, so the script fits the gradient against the pixels
`mono.png` says the glyph never touches and paints it back clean — measured
0.28/255 RMS against the artwork, and it refuses to write anything if that ever
drifts past 6. And it **rescales for Android's mask**: the artwork was drawn to
the web maskable safe zone (a circle over 80% of the canvas) where Android
guarantees only 61%, so at its drawn size a circular launcher mask clipped the
corners off both brackets. The whole design is scaled by 72/108, which makes
`maskable.png` exactly the visible 72dp and the rest gradient bleed.

The third layer is `<monochrome>`, for Android 13's themed icons, and it is not
the foreground reused: the system tints that drawable's *alpha*, and the
foreground's alpha carries the design's drop shadow, which tinted stops being a
shadow and becomes an accent-coloured halo. So the monochrome coverage is
unmixed out of the artwork against the fitted gradient, where the shadow — being
darker than the gradient rather than lighter — falls out on its own.

`the_launcher_icon_is_adaptive_and_keeps_its_monochrome_layer` in `src/lib.rs`
holds the four files that only work together: the manifest's two attributes, the
`<adaptive-icon>` and its three layers, every generated PNG at its right pixel
size, and `build-apk.sh`'s `aapt2 compile`. Only the last of those fails loudly
on its own. The rest build a perfectly good APK with the wrong picture on it.
The whole set costs 222 KiB of the APK.
