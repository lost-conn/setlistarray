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
    crate::start_android(DataDir::new(root));

    run_android_with_theme(
        android_app,
        "SetListArray",
        crate::WIDTH,
        crate::HEIGHT,
        crate::app,
        crate::theme_props(),
    );
}
