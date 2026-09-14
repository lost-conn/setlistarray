#!/usr/bin/env bash
set -euo pipefail

# Build, package and (optionally) install SetListArray for Android.
#
# Adapted from rinch's examples/hello-android/build-apk.sh. The pipeline is
# cargo-ndk → javac → d8 → aapt2 → zipalign → apksigner → adb, or with
# --bundle, cargo-ndk → javac → d8 → aapt2 → bundletool → jarsigner.
#
# Usage:
#   ./build-apk.sh                     # build, package, install, launch
#   ./build-apk.sh --build-only        # build and package only (no device needed)
#   ./build-apk.sh --bundle            # an Android App Bundle for Play, not an APK
#   ./build-apk.sh --target x86_64     # for an emulator (default: arm64-v8a)
#   ./build-apk.sh --debug             # debug profile (slow: Stylo and Parley)
#   ./build-apk.sh --software          # the tiny-skia painter instead of the GPU one
#   ./build-apk.sh --shots             # the store-listing tour, not the app (card S1)
#
# Requirements:
#   ANDROID_NDK_HOME          NDK r27c        (default ~/android/android-ndk-r27c)
#   ANDROID_SDK_BUILD_TOOLS   build-tools 35  (default ~/android/sdk/build-tools/35.0.0)
#   ANDROID_SDK_PLATFORM      android.jar     (default ~/android/sdk/platforms/android-35/android.jar)
#   RINCH_DIR                 the rinch checkout supplying RinchActivity.java
#                                             (default ../rinch-fixes, matching Cargo.toml)
#   BUNDLETOOL_JAR            bundletool-all, for --bundle only
#                                             (default ~/android/bundletool-all-1.18.3.jar)
#   cargo-ndk on PATH, and the Android targets in rust-toolchain.toml.
#   adb only for install/launch.
#
# The APK is signed with a throwaway debug keystore. It is not a release build.
# The bundle is signed with the upload key in ANDROID_UPLOAD_KEYSTORE (alias
# ANDROID_UPLOAD_KEY_ALIAS, default "upload"; passwords from
# ANDROID_UPLOAD_STORE_PASS and ANDROID_UPLOAD_KEY_PASS, or prompted for), and
# is left unsigned — which Play will refuse — when that is not set.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

TARGET="arm64-v8a"
BUILD_ONLY=false
BUNDLE=false
RELEASE=true
# The GPU shell is a rinch feature, not one of ours, so there is nothing in
# Cargo.toml to name it — `--features rinch/android-gpu` reaches through the
# dependency. It is **on by default**, and it took four cards to earn that.
#
# Card K27 measured it 2-10x *slower* than the software painter, because every
# frame was rasterised on the GPU and then read back to the CPU to be handed to
# a softbuffer surface. K35 removed the readback and presented the wgpu
# swapchain directly. K42 found the 9.57ms it then spent in `vkQueuePresentKHR`
# was not a swapchain misconfiguration but the queue declining a frame from a
# pipeline 2.1x over budget, and K43 cut the rasterisation itself by a third.
#
# What finally allowed the flip was not speed, though. K36 fixed a silent
# correctness difference — the GPU painter clipped an `opacity` layer to the
# element's border box and the software painter did not — and shipping the
# faster path while it could still throw a shadow away was not a trade worth
# making. Measured on the moto g stylus 5G with `scripts/frame-probe.sh`, two
# runs each, against stock Settings holding 8.33ms p50 and 120.0fps:
#
#   ours, --gpu       p50 8.37ms  p95 16.69ms  84.8-85.7fps  38.7-39.9% missed
#   ours, --software  p50 8.35ms  p95 25.18ms  70.6-70.9fps  43.9-45.6% missed
#
# The medians are the same; the difference is in the frames that miss, which
# take two refreshes on the GPU path and three on the software one. It costs
# 4.2 MB of APK (22.25 -> 26.44 MB), which is not nothing so soon after K33.
#
# `--software` is kept, and not only for the next comparison: the GPU path is
# proven on exactly one driver, this handset's Adreno 619, and the painter with
# years behind it should stay one flag away.
FEATURES="rinch/android-gpu"

# `--shots` (card S1) builds an APK that is not the app: it seeds the demo
# library, pins the theme and the accent away from the emulator's wallpaper,
# and walks itself through the screens that sell SetListArray, announcing each
# one to logcat for `scripts/store-shots.sh` to photograph. See `src/shots.rs`.
#
# It is composed with FEATURES after the loop rather than appended inside it,
# and that is not tidiness. `--software` sets FEATURES to the empty string, so
# `--shots --software` in that order would have thrown the shots feature away
# and `--software --shots` would not — two spellings of one intention giving
# two different APKs, and the wrong one of them is an APK that looks correct,
# installs, launches, and then sits on the library screen forever while the
# capture script times out waiting for a line it will never print.
SHOTS=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --target) TARGET="$2"; shift 2 ;;
        --debug) RELEASE=false; shift ;;
        --build-only) BUILD_ONLY=true; shift ;;
        --bundle) BUNDLE=true; shift ;;
        --software) FEATURES=""; shift ;;
        --shots) SHOTS=true; shift ;;
        # Accepted and deliberately a no-op: `--gpu` is what four cards' worth
        # of notes and commit messages say, and having it fail now would make
        # every one of them wrong.
        --gpu) FEATURES="rinch/android-gpu"; shift ;;
        -h|--help) sed -n '3,34p' "$0"; exit 0 ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

if [[ "$SHOTS" == true ]]; then
    FEATURES="${FEATURES:+$FEATURES,}shots"
fi

case "$TARGET" in
    arm64-v8a)   ABI="arm64-v8a";   TRIPLE="aarch64-linux-android" ;;
    x86_64)      ABI="x86_64";      TRIPLE="x86_64-linux-android" ;;
    armeabi-v7a) ABI="armeabi-v7a"; TRIPLE="armv7-linux-androideabi" ;;
    x86)         ABI="x86";         TRIPLE="i686-linux-android" ;;
    *) echo "Unknown target: $TARGET"; exit 1 ;;
esac

ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$HOME/android/android-ndk-r27c}"
BUILD_TOOLS="${ANDROID_SDK_BUILD_TOOLS:-$HOME/android/sdk/build-tools/35.0.0}"
PLATFORM="${ANDROID_SDK_PLATFORM:-$HOME/android/sdk/platforms/android-35/android.jar}"
RINCH_DIR="${RINCH_DIR:-$SCRIPT_DIR/../rinch-fixes}"
KEYSTORE="${ANDROID_DEBUG_KEYSTORE:-$SCRIPT_DIR/target/debug.keystore}"

PACKAGE="dev.lostconnection.setlistarray"
LIB_NAME="libsetlistarray.so"
APK_NAME="setlistarray.apk"
AAB_NAME="setlistarray.aab"

# The `-all` jar from bundletool's GitHub releases, not the one Gradle keeps in
# `~/.gradle/caches`: that one is the library half, with no `Main-Class` and
# none of its dependencies, and `java -jar` on it says only "no main manifest
# attribute". 1.18.3 was the latest release on 2026-09-10, when this was
# written.
BUNDLETOOL="${BUNDLETOOL_JAR:-$HOME/android/bundletool-all-1.18.3.jar}"

REQUIRED=("$ANDROID_NDK_HOME" "$BUILD_TOOLS" "$PLATFORM" "$RINCH_DIR")
if [[ "$BUNDLE" == true ]]; then
    REQUIRED+=("$BUNDLETOOL")
fi
for path in "${REQUIRED[@]}"; do
    if [[ ! -e "$path" ]]; then
        echo "ERROR: not found: $path"
        echo "       set ANDROID_NDK_HOME / ANDROID_SDK_BUILD_TOOLS / ANDROID_SDK_PLATFORM / RINCH_DIR / BUNDLETOOL_JAR"
        exit 1
    fi
done

# Shared by the two endings below. The APK goes to the phone with `adb
# install` and the bundle through bundletool, and everything either side of
# that step is the same.
require_device() {
    if ! command -v adb >/dev/null; then
        echo "ERROR: adb not on PATH (try ~/android/sdk/platform-tools). Use --build-only."
        exit 1
    fi
    if [[ -z "$(adb devices | sed -n '2,$p' | grep -w device || true)" ]]; then
        echo "ERROR: no device or emulator attached. Use --build-only."
        exit 1
    fi
}

launch() {
    echo "==> Launching..."
    adb shell am start -n "$PACKAGE/com.rinch.RinchActivity"
    echo "==> Done. 'adb logcat -s rinch' for logs."
}

PROFILE="release"
PROFILE_FLAG="--release"
if [[ "$RELEASE" == false ]]; then
    PROFILE="debug"
    PROFILE_FLAG=""
fi

# ── Rust ─────────────────────────────────────────────────────────────────────
# Link-time optimisation lives here rather than in `[profile.release]`, and the
# reason is a measurement rather than a preference.
#
# Card K33 wanted `lto = "fat"` and `codegen-units = 1` for what they take off
# the phone, and they do: `libsetlistarray.so` 25,509,032 → 22,223,448 B, a
# 3.29 MB saving with no change in frame timing on the device (median 8.33ms
# against 8.34ms over ten flings, both pinned to the 120Hz vsync). Thin LTO was
# measured too and is a wash — 22,232,432 B, 87s against fat's 85s — so the
# cost is `codegen-units = 1`, not the LTO mode, and fat is very slightly
# smaller for the same money.
#
# Put those two lines in `Cargo.toml` and they apply to the desktop build as
# well, where they buy nothing and cost everything: an incremental rebuild
# after touching `src/lib.rs` went **7.28s → 124.74s**, seventeen times slower,
# and that is the loop `scripts/screenshot.sh` runs on. A full APK build goes
# 46.76s → 84.81s, which is the price of the 3.29 MB and is paid by a build
# that installs to a phone anyway.
#
# `:=` rather than `=`, so a card that needs a fast on-device loop can say
# `CARGO_PROFILE_RELEASE_LTO=false ./build-apk.sh` and get the 47-second build
# back without editing anything.
: "${CARGO_PROFILE_RELEASE_LTO:=fat}"
: "${CARGO_PROFILE_RELEASE_CODEGEN_UNITS:=1}"
export CARGO_PROFILE_RELEASE_LTO CARGO_PROFILE_RELEASE_CODEGEN_UNITS

# `--lib` only: the desktop binary and the probe are not part of the APK.
echo "==> Building $TARGET ($PROFILE)..."
export ANDROID_NDK_HOME
(cd "$SCRIPT_DIR" && cargo ndk -t "$TARGET" build --lib $PROFILE_FLAG ${FEATURES:+--features "$FEATURES"})

SO_PATH="$SCRIPT_DIR/target/$TRIPLE/$PROFILE/$LIB_NAME"
if [[ ! -f "$SO_PATH" ]]; then
    echo "ERROR: .so not found at $SO_PATH"
    exit 1
fi

# ── Java companion classes ───────────────────────────────────────────────────
# RinchActivity is a NativeActivity subclass; the two input classes are the IME
# bridge. They come from rinch, not from this repo.
JAVA_SRC="$RINCH_DIR/crates/rinch-android/java"

APK_DIR="$(mktemp -d)"
trap 'rm -rf "$APK_DIR"' EXIT

echo "==> Compiling Java..."
mkdir -p "$APK_DIR/classes"
javac -source 8 -target 8 \
    -classpath "$PLATFORM" \
    -d "$APK_DIR/classes" \
    "$JAVA_SRC/com/rinch/RinchActivity.java" \
    "$JAVA_SRC/com/rinch/RinchInputConnection.java" \
    "$JAVA_SRC/com/rinch/RinchInputView.java" 2>&1 | grep -v "^Note:" || true

echo "==> Converting to DEX..."
# `--lib` gives d8 the platform classes it needs to desugar the lambdas in
# RinchActivity; without it every one of them is a warning.
"$BUILD_TOOLS/d8" --min-api 28 --lib "$PLATFORM" --output "$APK_DIR/" \
    $(find "$APK_DIR/classes" -name '*.class')

# ── Package ──────────────────────────────────────────────────────────────────
echo "==> Packaging..."
mkdir -p "$APK_DIR/lib/$ABI"
cp "$SO_PATH" "$APK_DIR/lib/$ABI/"

# The launcher icon is the only compiled resource this app has, and it arrived
# on 2026-09-09 — before that `android/res/` did not exist and this was a bare
# `aapt2 link` over a manifest with nothing to resolve.
#
# `aapt2` will not take a directory of PNGs and XML at link time. Everything has
# to go through `compile` first, which is what turns `mipmap-xxxhdpi/
# ic_launcher.png` into a configuration-tagged entry and `mipmap-anydpi-v26/
# ic_launcher.xml` into a binary `<adaptive-icon>`; the result is a flat archive
# that `link` takes as a **positional argument**. Not `-R`: that flag exists for
# overlaying somebody else's resources on top of yours, it is accepted here
# without complaint, and it produces an APK whose `resources.arsc` the manifest's
# `@mipmap/ic_launcher` cannot resolve to.
#
# The failure when either half is missing is at least a loud one — `link` stops
# with "resource mipmap/ic_launcher not found" rather than building an APK with
# a green robot on it.
echo "==> Compiling resources..."
"$BUILD_TOOLS/aapt2" compile --dir "$SCRIPT_DIR/android/res" -o "$APK_DIR/res.zip"

# The debug keystore is made here rather than just before `apksigner`, where it
# lived until the bundle arrived, because both endings sign with it now: the
# APK directly, and the bundle's generated APKs through bundletool — for the
# alignment check below and for installing on a phone.
if [[ ! -f "$KEYSTORE" ]]; then
    echo "==> Creating a throwaway debug keystore at $KEYSTORE..."
    mkdir -p "$(dirname "$KEYSTORE")"
    keytool -genkeypair \
        -keystore "$KEYSTORE" -alias debug \
        -keyalg RSA -keysize 2048 -validity 10000 \
        -storepass android -keypass android \
        -dname "CN=Debug, O=SetListArray" 2>/dev/null
fi

# The version is the build's to say and not the manifest's, and the note at the
# top of AndroidManifest.xml has the trap in it: `--version-code` only fills a
# gap, so the manifest has to leave one.
#
# The code is the commit count of HEAD — 118 on the day this was written. Play
# needs a number that only ever goes up and refuses an upload that reuses one,
# and a commit count is such a number without anybody having to remember to
# bump it. It stops being one if master's history is ever rewritten shorter,
# which is worth knowing rather than likely; `ANDROID_VERSION_CODE` overrides it
# for that day. The name is `Cargo.toml`'s `version`, which is what the crate
# already calls itself.
VERSION_CODE="${ANDROID_VERSION_CODE:-$(git -C "$SCRIPT_DIR" rev-list --count HEAD)}"
VERSION_NAME="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$SCRIPT_DIR/Cargo.toml" | head -n 1)"

LINK_ARGS=(
    --manifest "$SCRIPT_DIR/android/AndroidManifest.xml"
    -I "$PLATFORM"
    --version-code "$VERSION_CODE"
    --version-name "$VERSION_NAME"
)

# ── Bundle ───────────────────────────────────────────────────────────────────
# What Play takes, and the reason this script grew a second ending. An App
# Bundle is not an installable thing: it is the *inputs* to an APK, laid out by
# what each file is rather than where it goes, and Play builds and signs the
# APKs each device downloads from it. Everything above — the library, the dex,
# the compiled icon — is shared with the APK path. What differs is that aapt2
# links resources to protobuf rather than to a binary `resources.arsc`
# (`--proto-format`, the form bundletool reads), and that the last steps are
# bundletool's rather than ours.
#
# That last part is the catch, and it is card K33's again. `zip -0` and
# `zipalign -P 16` further down are how the APK path keeps the library stored
# and 16 KB-aligned, and neither survives into a bundle: Play repacks the
# library itself. So the same two facts are asked of bundletool instead, in
# `BundleConfig.json` — `uncompressNativeLibraries`, so the library is stored
# and `extractNativeLibs="false"` means something, and `PAGE_ALIGNMENT_16K`,
# because bundletool's default is 4 KB. Leave the alignment out and the bundle
# builds without complaint and is then refused by Play's 16 KB check.
#
# bundletool's own `config.proto` marks that alignment field experimental —
# "might be changed or completely removed" — so it is not taken on trust. Before
# the bundle is signed, the APKs an Android 16 phone would be given are cut from
# it and the one carrying the library is checked the way the APK path's own
# output would be: stored, and `zipalign -c -P 16`. A bundletool upgrade that
# drops the field fails here, on a laptop, rather than at upload.
if [[ "$BUNDLE" == true ]]; then
    if [[ -n "$(git -C "$SCRIPT_DIR" status --porcelain)" ]]; then
        echo "==> NOTE: the tree has uncommitted changes. This bundle is versionCode $VERSION_CODE,"
        echo "    the same as a build of HEAD, and Play takes exactly one upload per code."
    fi

    echo "==> Linking resources for the bundle..."
    "$BUILD_TOOLS/aapt2" link --proto-format "${LINK_ARGS[@]}" \
        -o "$APK_DIR/proto.apk" \
        "$APK_DIR/res.zip"

    # A module is a directory per kind of thing: the manifest under
    # `manifest/`, the dex under `dex/`, and `res/`, `resources.pb` and `lib/`
    # where an APK would have them.
    MODULE="$APK_DIR/module"
    mkdir -p "$MODULE/manifest" "$MODULE/dex"
    (cd "$MODULE" && unzip -q "$APK_DIR/proto.apk")
    mv "$MODULE/AndroidManifest.xml" "$MODULE/manifest/"
    cp "$APK_DIR/classes.dex" "$MODULE/dex/"
    cp -r "$APK_DIR/lib" "$MODULE/"
    (cd "$MODULE" && zip -qr "$APK_DIR/base.zip" .)

    cat > "$APK_DIR/BundleConfig.json" <<'EOF'
{
  "optimizations": {
    "uncompressNativeLibraries": { "enabled": true, "alignment": "PAGE_ALIGNMENT_16K" }
  }
}
EOF

    echo "==> Building bundle..."
    java -jar "$BUNDLETOOL" build-bundle \
        --modules="$APK_DIR/base.zip" \
        --config="$APK_DIR/BundleConfig.json" \
        --output="$APK_DIR/$AAB_NAME"

    echo "==> Checking what Play would cut from it for an Android 16 phone..."
    cat > "$APK_DIR/device.json" <<EOF
{ "supportedAbis": ["$ABI"], "supportedLocales": ["en-US"], "screenDensity": 480, "sdkVersion": 36 }
EOF
    java -jar "$BUNDLETOOL" build-apks \
        --bundle="$APK_DIR/$AAB_NAME" \
        --output="$APK_DIR/check.apks" \
        --device-spec="$APK_DIR/device.json" \
        --ks="$KEYSTORE" --ks-key-alias=debug \
        --ks-pass=pass:android --key-pass=pass:android
    unzip -q "$APK_DIR/check.apks" -d "$APK_DIR/check"
    SPLIT=""
    for apk in "$APK_DIR"/check/splits/*.apk; do
        if unzip -l "$apk" | grep "lib/$ABI/$LIB_NAME" >/dev/null; then
            SPLIT="$apk"
        fi
    done
    if [[ -z "$SPLIT" ]]; then
        echo "ERROR: no APK bundletool generated carries lib/$ABI/$LIB_NAME"
        exit 1
    fi
    if ! unzip -v "$SPLIT" | grep "lib/$ABI/$LIB_NAME" | grep " Stored " >/dev/null; then
        echo "ERROR: bundletool deflated $LIB_NAME in $(basename "$SPLIT"); the manifest's"
        echo "       extractNativeLibs=\"false\" cannot map a deflated library (card K33)"
        exit 1
    fi
    if ! "$BUILD_TOOLS/zipalign" -c -P 16 4 "$SPLIT" >/dev/null; then
        echo "ERROR: $LIB_NAME in $(basename "$SPLIT") is not 16 KB-aligned; Play will refuse"
        echo "       the bundle, and a 16 KB-page device could not map the library (card K33)"
        exit 1
    fi
    echo "    $(basename "$SPLIT"): $LIB_NAME stored, 16 KB-aligned"

    # A bundle is signed as a JAR, which is why this is `jarsigner` and not the
    # `apksigner` the APK uses — v2 and v3 signatures are APK formats, and
    # Play's own documentation signs bundles this way. The key is the *upload*
    # key: under Play App Signing, Google re-signs what devices receive with an
    # app signing key it holds, and the upload key only proves to Play that
    # the upload came from here. `-storepass:env` names the variable rather
    # than its value, so the password is in neither `ps` nor shell history;
    # left unset, jarsigner asks for it.
    if [[ -n "${ANDROID_UPLOAD_KEYSTORE:-}" ]]; then
        echo "==> Signing with the upload key..."
        jarsigner -sigalg SHA256withRSA -digestalg SHA-256 \
            -keystore "$ANDROID_UPLOAD_KEYSTORE" \
            ${ANDROID_UPLOAD_STORE_PASS:+-storepass:env ANDROID_UPLOAD_STORE_PASS} \
            ${ANDROID_UPLOAD_KEY_PASS:+-keypass:env ANDROID_UPLOAD_KEY_PASS} \
            "$APK_DIR/$AAB_NAME" "${ANDROID_UPLOAD_KEY_ALIAS:-upload}"
    else
        echo "==> NOT SIGNED: ANDROID_UPLOAD_KEYSTORE is not set. Play will refuse this"
        echo "    bundle; installing it below still works, because bundletool signs what"
        echo "    it generates."
    fi

    cp "$APK_DIR/$AAB_NAME" "$SCRIPT_DIR/$AAB_NAME"
    echo "==> Bundle ready: $SCRIPT_DIR/$AAB_NAME ($(du -h "$SCRIPT_DIR/$AAB_NAME" | cut -f1)), versionCode $VERSION_CODE, versionName $VERSION_NAME"

    if [[ "$BUILD_ONLY" == true ]]; then
        exit 0
    fi

    # A bundle cannot be installed as it is; bundletool does for one phone
    # what Play does for all of them. The generated APKs are signed with this
    # script's debug key rather than the upload key, and on purpose: the copy
    # already on the phone came from the APK path with that signature, Android
    # refuses an update signed by any other key, and the only way past that
    # refusal is uninstalling — which deletes the library in app-private
    # storage along with the app.
    require_device
    echo "==> Installing, as Play would..."
    java -jar "$BUNDLETOOL" build-apks \
        --bundle="$APK_DIR/$AAB_NAME" \
        --output="$APK_DIR/device.apks" \
        --connected-device --adb="$(command -v adb)" \
        --ks="$KEYSTORE" --ks-key-alias=debug \
        --ks-pass=pass:android --key-pass=pass:android
    adb shell am force-stop "$PACKAGE" 2>/dev/null || true
    java -jar "$BUNDLETOOL" install-apks --apks="$APK_DIR/device.apks" --adb="$(command -v adb)"
    launch
    exit 0
fi

"$BUILD_TOOLS/aapt2" link "${LINK_ARGS[@]}" \
    -o "$APK_DIR/base.apk" \
    "$APK_DIR/res.zip"

# `classes.dex` is deflated like any other entry; the library is **stored**.
#
# That -0 is half of card K33's second fix, and it only makes sense with the
# other half — `android:extractNativeLibs="false"` in AndroidManifest.xml. Ask
# the loader to map the library straight out of the APK and it can only do
# that if the bytes in the APK are the bytes of the ELF, uncompressed and
# aligned to a page boundary. Compress them and the installer has to extract a
# second copy to `/data/app/…/lib/arm64/`, which is exactly the 35 MB
# duplicate K33 measured on the phone. The two lines below and the manifest
# attribute are one change wearing three hats, and
# `the_apk_stores_its_native_library_uncompressed` in src/lib.rs fails the
# build if any of them goes missing on its own.
(cd "$APK_DIR" && zip -qr base.apk classes.dex && zip -qr -0 base.apk lib/)

# `-P 16`, not `-p`. Both page-align the stored `.so`; `-p` means 4 KB and
# `-P 16` means 16 KB, and 16 KB is what this library actually wants — NDK
# r27c links it with `p_align 0x4000` on every LOAD segment (checked with
# `llvm-readelf -lW`), and Android 15 requires 16 KB-page support of anything
# targeting SDK 35, which this manifest does. Aligning to 4 KB would still
# install and still run on the 4 KB-page devices of today, and would fail to
# map on a 16 KB-page one.
"$BUILD_TOOLS/zipalign" -f -P 16 4 "$APK_DIR/base.apk" "$APK_DIR/aligned.apk"

"$BUILD_TOOLS/apksigner" sign \
    --ks "$KEYSTORE" --ks-key-alias debug \
    --ks-pass pass:android --key-pass pass:android \
    --out "$APK_DIR/$APK_NAME" \
    "$APK_DIR/aligned.apk" 2>/dev/null

cp "$APK_DIR/$APK_NAME" "$SCRIPT_DIR/$APK_NAME"
echo "==> APK ready: $SCRIPT_DIR/$APK_NAME ($(du -h "$SCRIPT_DIR/$APK_NAME" | cut -f1))"

if [[ "$BUILD_ONLY" == true ]]; then
    exit 0
fi

# ── Install ──────────────────────────────────────────────────────────────────
require_device

echo "==> Installing..."
adb shell am force-stop "$PACKAGE" 2>/dev/null || true
adb install -r "$SCRIPT_DIR/$APK_NAME" 2>&1 | grep -E "Success|Failure"

launch
