//! The Android entry point.
//!
//! The platform loads `libsetlistarray.so` and calls `android_main`; from there
//! it is the same `app()` the desktop binary runs. Two things are decided here
//! and nowhere else: where the library is written, and which shell starts it.

use rinch::prelude::*;

use crate::db::DataDir;

#[unsafe(no_mangle)]
fn android_main(android_app: AndroidApp) {
    // App-private storage: `/data/data/<package>/files`. Nothing else on the
    // device can read it, it is backed up and removed with the app, and it
    // needs no permission — which is why this app asks for none.
    let root = android_app.internal_data_path().unwrap_or_else(|| {
        // Should not happen: NativeActivity always supplies one. Losing the
        // library on the next launch beats refusing to start.
        log::error!("no internal data path; falling back to a temporary directory");
        std::env::temp_dir().join("setlistarray")
    });
    // Installs the directory and records that there is no `--seed` here: a
    // phone has no command line, and demo content on a real device would be
    // someone else's songs in your book.
    //
    // Except in the one build where it is the entire point. `--features shots`
    // (card S1, `crate::shots`) produces an APK whose only job is to be
    // installed on a throwaway emulator, walked through the seven screens that
    // sell the app, and photographed for the Play listing — so it seeds, and
    // `crate::shots::run`, armed from `crate::app` once the stores exist, pins
    // the theme and the accent so that the emulator's wallpaper does not get a
    // vote in what colour a public store page is.
    //
    // Both arms are spelled out rather than one arm and a `cfg!` inside
    // `start_android`, because this is the seam: **this line is where a build
    // decides whether it is the app or the photograph of it**, and a reader
    // who greps for `shots` on Android should land here. Nothing else in this
    // file changes, and without the feature `android_main` is byte for byte
    // the function it has always been.
    #[cfg(not(feature = "shots"))]
    crate::start_android(DataDir::new(root));
    #[cfg(feature = "shots")]
    crate::start_android_for_shots(DataDir::new(root));

    // `crate::FONTS` and not the device's font list: a phone has Roboto and
    // Noto and whatever else its OEM chose, and none of it is this app's.
    // They go in through the entry point rather than being registered
    // beforehand so that they are in place before the first layout pass —
    // otherwise the first frame measures against a fallback and reflows.
    run_android_with_fonts(
        android_app,
        "SetListArray",
        crate::WIDTH,
        crate::HEIGHT,
        crate::app,
        Some(crate::theme_props()),
        crate::FONTS,
    );
}
