//! The desktop binary. Everything it does lives in the library, so that the
//! Android `cdylib` runs exactly the same app from a different door.

#[cfg(not(target_os = "android"))]
fn main() {
    setlistarray::run_desktop();
}

// Android loads the `cdylib` and calls `android_main`; this binary is not part
// of the APK. It stays a valid (empty) program so that a whole-crate
// `cargo build --target aarch64-linux-android` still resolves.
#[cfg(target_os = "android")]
fn main() {}
