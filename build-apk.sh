#!/usr/bin/env bash
set -euo pipefail

# Build, package and (optionally) install SetListArray for Android.
#
# Adapted from rinch's examples/hello-android/build-apk.sh. The pipeline is
# cargo-ndk → javac → d8 → aapt2 → zipalign → apksigner → adb.
#
# Usage:
#   ./build-apk.sh                     # build, package, install, launch
#   ./build-apk.sh --build-only        # build and package only (no device needed)
#   ./build-apk.sh --target x86_64     # for an emulator (default: arm64-v8a)
#   ./build-apk.sh --debug             # debug profile (slow: Stylo and Parley)
#   ./build-apk.sh --software          # the tiny-skia painter instead of the GPU one
#
# Requirements:
#   ANDROID_NDK_HOME          NDK r27c        (default ~/android/android-ndk-r27c)
#   ANDROID_SDK_BUILD_TOOLS   build-tools 35  (default ~/android/sdk/build-tools/35.0.0)
#   ANDROID_SDK_PLATFORM      android.jar     (default ~/android/sdk/platforms/android-35/android.jar)
#   RINCH_DIR                 the rinch checkout supplying RinchActivity.java
#                                             (default ../rinch-fixes, matching Cargo.toml)
#   cargo-ndk on PATH, and the Android targets in rust-toolchain.toml.
#   adb only for install/launch.
#
# The APK is signed with a throwaway debug keystore. It is not a release build.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

TARGET="arm64-v8a"
BUILD_ONLY=false
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

while [[ $# -gt 0 ]]; do
    case "$1" in
        --target) TARGET="$2"; shift 2 ;;
        --debug) RELEASE=false; shift ;;
        --build-only) BUILD_ONLY=true; shift ;;
        --software) FEATURES=""; shift ;;
        # Accepted and deliberately a no-op: `--gpu` is what four cards' worth
        # of notes and commit messages say, and having it fail now would make
        # every one of them wrong.
        --gpu) FEATURES="rinch/android-gpu"; shift ;;
        -h|--help) sed -n '3,26p' "$0"; exit 0 ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

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

for path in "$ANDROID_NDK_HOME" "$BUILD_TOOLS" "$PLATFORM" "$RINCH_DIR"; do
    if [[ ! -e "$path" ]]; then
        echo "ERROR: not found: $path"
        echo "       set ANDROID_NDK_HOME / ANDROID_SDK_BUILD_TOOLS / ANDROID_SDK_PLATFORM / RINCH_DIR"
        exit 1
    fi
done

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
echo "==> Packaging APK..."
mkdir -p "$APK_DIR/lib/$ABI"
cp "$SO_PATH" "$APK_DIR/lib/$ABI/"

"$BUILD_TOOLS/aapt2" link \
    --manifest "$SCRIPT_DIR/android/AndroidManifest.xml" \
    -I "$PLATFORM" \
    --min-sdk-version 28 \
    --target-sdk-version 35 \
    -o "$APK_DIR/base.apk"

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

if [[ ! -f "$KEYSTORE" ]]; then
    echo "==> Creating a throwaway debug keystore at $KEYSTORE..."
    mkdir -p "$(dirname "$KEYSTORE")"
    keytool -genkeypair \
        -keystore "$KEYSTORE" -alias debug \
        -keyalg RSA -keysize 2048 -validity 10000 \
        -storepass android -keypass android \
        -dname "CN=Debug, O=SetListArray" 2>/dev/null
fi

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
if ! command -v adb >/dev/null; then
    echo "ERROR: adb not on PATH (try ~/android/sdk/platform-tools). Use --build-only."
    exit 1
fi
if [[ -z "$(adb devices | sed -n '2,$p' | grep -w device || true)" ]]; then
    echo "ERROR: no device or emulator attached. Use --build-only."
    exit 1
fi

echo "==> Installing..."
adb shell am force-stop "$PACKAGE" 2>/dev/null || true
adb install -r "$SCRIPT_DIR/$APK_NAME" 2>&1 | grep -E "Success|Failure"

echo "==> Launching..."
adb shell am start -n "$PACKAGE/com.rinch.RinchActivity"

echo "==> Done. 'adb logcat -s rinch' for logs."
