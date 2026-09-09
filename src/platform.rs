//! The window the app got: how big it is, how much of it the app is not
//! allowed to draw in, and which way round the OS should draw the bits it puts
//! there.
//!
//! Android hands back physical pixels — the status bar, the navigation or
//! gesture bar, and the display cutout — which have to be divided by the
//! display density to become the CSS pixels the stylesheet is written in.
//! The desktop has no such thing, so it stands in the numbers a phone would
//! report, because the desktop window is a phone-shaped preview of one.
//!
//! The clock and the battery icon are Android's to draw, not ours, and it has
//! no way of knowing what colour the app painted underneath them —
//! [`set_light_system_bars`] is how it is told. The desktop window has no such
//! bars, so there it is nothing.
//!
//! Since card K8 it is also how the app learns what the *system* looks like —
//! [`night_mode`] and [`wallpaper_primary`] — rather than only what shape the
//! window is. Card K57 added the reading that supersedes the second of those,
//! [`system_accent`]: the palette Android 12+ themes *itself* with, which is a
//! better answer than the wallpaper's colour rather than merely a newer one.
//! That function's own comment has the argument.
//!
//! ## Every reading in here has a shelf life, and K53 is why that matters
//!
//! Four things this module reports were, until card K53, read once and then
//! believed forever: the insets, the viewport width, and (from K8) the night
//! mode and the wallpaper colour. Each was correct for the same single reason
//! — the app is portrait-locked and nothing was listening for a change — and
//! K53's own note says the quiet part: *"Both are correct today for the same
//! reason… Neither is correct the moment that stops being true. Why it was not
//! fixed in K31: there is nothing to listen to."*
//!
//! There is now: `rinch::set_configuration_change_handler`. So none of the
//! four is read at mount and kept any more. Three of them live in
//! [`SystemStore`](crate::store::SystemStore) as signals, re-taken whenever
//! the platform says something moved, and the fourth — the viewport width,
//! which is read from render closures far too often to be a JNI call each
//! time — keeps its cache but gains a way to be told the cache is wrong. That
//! is [`forget_cached_readings`], and `SystemStore::reread` is its one caller.
//!
//! Nothing above this module knows which platform it is on.

/// Space to keep clear on each edge, in CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SafeArea {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl SafeArea {
    /// What the desktop window pretends to be: a Pixel-class phone held
    /// upright, with a status bar above and a gesture bar below. These are the
    /// two numbers that used to be hard-coded into `app()`.
    pub const PHONE_STANDIN: SafeArea = SafeArea {
        top: 44.0,
        right: 0.0,
        bottom: 22.0,
        left: 0.0,
    };

    pub const NONE: SafeArea = SafeArea {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };
}

/// Android's density-independent-pixel baseline: 160dpi is 1 CSS pixel per
/// physical pixel.
#[cfg(target_os = "android")]
const BASELINE_DPI: f32 = 160.0;

/// The insets the OS reports, converted to CSS pixels.
///
/// **Not cached, and not read once at mount any more.** It used to say here
/// that a rotation-aware version "would have to re-read them on a
/// configuration change, which the Rinch Android shell does not surface yet".
/// It does now, so this is read into
/// [`SystemStore::safe_area`](crate::store::SystemStore) at startup and
/// re-taken from there every time the platform says its configuration moved.
/// Two JNI calls per re-read, a handful of times in a session, is not a cost
/// worth caching against — which is exactly the argument
/// [`viewport_width`] below makes in the opposite direction, and for the
/// opposite reason.
#[cfg(target_os = "android")]
pub fn safe_area() -> SafeArea {
    let insets = rinch_android::display::safe_area_insets();
    // 360dpi (2.25×) is the Pixel-class default, and the same fallback
    // `hello-android` uses when the density lookup fails.
    let scale = rinch_android::display::density_dpi().unwrap_or(360) as f32 / BASELINE_DPI;
    if scale <= 0.0 {
        return SafeArea::NONE;
    }
    SafeArea {
        top: insets.top as f32 / scale,
        right: insets.right as f32 / scale,
        bottom: insets.bottom as f32 / scale,
        left: insets.left as f32 / scale,
    }
}

#[cfg(not(target_os = "android"))]
pub fn safe_area() -> SafeArea {
    SafeArea::PHONE_STANDIN
}

/// The width of the window the app is really drawing into, in CSS pixels.
///
/// Card K31 is the whole of why this exists. `crate::WIDTH` is 393 because 393
/// is the canvas the design handoff was drawn on, and everything that
/// rasterises a page to a pixel width — the chart column, the viewer, the two
/// page widths in `song_detail`, the capture preview — worked that number out
/// from it. On Android that is not the window. Android ignores the size an app
/// asks for and lays out against whatever the surface turned out to be, and on
/// the moto g stylus 5G the surface is 1080 physical pixels at density 400,
/// which is 432 logical. So a page that should have been full-bleed was drawn
/// 393 wide with about 20 px of backdrop down each side — nothing broken, just
/// not the design, and wrong by a different amount on every handset.
///
/// It was left unfixed when it was found because there was no viewport width
/// to ask for: `rinch_android::display` knew the insets, the density and the
/// refresh rate, and not the size. There is now
/// (`rinch_android::display::viewport_size`), and this is the shim over it.
///
/// **Cached, unlike [`safe_area`], and for the opposite reason.** The safe area
/// is read once at mount by each screen that wants it, so a JNI call per read
/// costs nothing. This one is read from `song_detail`'s render closures, which
/// re-run on every redraw of the screen — two JNI calls per frame to re-learn a
/// number that does not change from one frame to the next is a cost with no
/// buyer. A failed read is deliberately *not* cached: it falls back for that
/// call and asks again next time, so a width asked for before the surface
/// existed does not become the answer for the rest of the process.
///
/// **Cached is not the same as frozen, and it used to be — card K53.** This
/// was an `OnceLock`, which is to say the first successful reading was the
/// answer for the life of the *process*. That was defensible for exactly as
/// long as the sentence "the app is portrait-locked, so the window size cannot
/// change underneath it" was the end of the argument, and it stopped being the
/// end of it the moment there was a configuration change to listen to. So the
/// cache is now a [`CachedWidth`], which is an `OnceLock` that can be told to
/// forget — see [`forget_cached_readings`], whose one caller is
/// `SystemStore::reread`.
#[cfg(target_os = "android")]
pub fn viewport_width() -> f32 {
    VIEWPORT_WIDTH
        .get_or_read(|| {
            let (physical, _) = rinch_android::display::viewport_size()?;
            // The same conversion `safe_area` above does, down to the fallback:
            // 360dpi (2.25x) is the Pixel-class default, and the guard is there
            // because a density of zero would turn a width into an infinity
            // rather than into a wrong number.
            let scale = rinch_android::display::density_dpi().unwrap_or(360) as f32 / BASELINE_DPI;
            if scale <= 0.0 {
                return None;
            }
            let width = physical as f32 / scale;
            // Once per fill of the cache, because the failure K31 was written
            // about was a width nobody could see. The app drew every rasterised
            // page 393 wide on a 432-wide phone for weeks, and the only symptom
            // was a strip of backdrop that looked like a margin somebody had
            // chosen. A line in logcat is what turns "the pages look a bit
            // narrow" into a number that can be checked against `wm size` and
            // `wm density`. It costs one line per process — or, since K53, one
            // more per configuration change, which is exactly when you want to
            // see it.
            log::info!("viewport: {physical}px physical / {scale:.2} = {width} CSS px wide");
            Some(width)
        })
        .unwrap_or(crate::WIDTH as f32)
}

/// On the desktop the answer is `crate::WIDTH`, and that is not a stand-in the
/// way [`SafeArea::PHONE_STANDIN`] is one — it is the truth.
///
/// `run_desktop` hands that number to the shell as the window to create
/// (src/lib.rs) and the desktop shell honours it, so the window really is 393
/// CSS pixels wide. Reading it back here is a report, not a guess, which is
/// also why it is not circular: the one place that must never ask this
/// function is `run_desktop` itself, and it does not.
///
/// It matters that this stays exactly 393: `scripts/screenshot.sh` measures a
/// desktop window against `scripts/screenshot-baseline.json`, and several of
/// those checks count absolute pixels that were recorded at 393.
#[cfg(not(target_os = "android"))]
pub fn viewport_width() -> f32 {
    crate::WIDTH as f32
}

/// How much of the bottom edge the soft keyboard is covering right now, in
/// CSS pixels. Zero when it is down.
///
/// Card K32: the capture screen's footer (Cancel · Attach,
/// `screens/capture.rs`) is the last `flex-shrink: 0` child of a full-height
/// column, so it sits at the very bottom of the window — which is exactly
/// where the keyboard also lands once it opens to type a URL. Measured on the
/// moto g stylus 5G (SDK 33, scale 2.50), the keyboard does not merely cover
/// the footer, it puts it entirely off-screen: no part of either button
/// stays reachable, and the only way back to it is dismissing the keyboard
/// first, which nothing tells the user to do and no other app requires.
///
/// Unlike [`safe_area`] and [`viewport_width`] this cannot be read once at
/// mount, because the number it reports changes *while the screen stays
/// open* — the keyboard slides in and out from under the same field. So this
/// hands back a `Signal<f32>` instead of a plain `f32`, driven by
/// `rinch::reactive::poll_signal`, which the Android shell drains once per
/// painted frame (`../rinch-fixes/crates/rinch/src/shell/android_frame.rs`).
/// A screen that wants its layout to track the keyboard reads the signal from
/// its own render closures exactly the way it would read any other one.
///
/// **`PollRate::Hz(30)`, not `EveryFrame`.** `EveryFrame` would mean a JNI
/// call into `getImeInset` on every single frame this screen is mounted —
/// not just while the keyboard is animating, but for as long as the user sits
/// looking at a filled-in field — and card K45 already has this device's GPU
/// running 1.4x over its frame budget with nothing extra asked of it. The
/// inset only actually moves during the roughly 250ms the keyboard takes to
/// open or close; at 30Hz that is at most seven or eight stale reads spread
/// across an animation, which lands the footer a frame or two late in a way
/// nobody watching a keyboard slide will ever notice, for a third of the JNI
/// cost of asking every frame forever.
///
/// **Called from inside the screen that wants it, not from `app()`.**
/// `poll_signal` ties the poll's lifetime to the signal it returns — the
/// shell keeps sampling for exactly as long as something holds that signal,
/// and drops the entry on the first drain after it is freed. `CaptureScreen`
/// calls this itself (rather than `app()` reading it once up front the way it
/// reads [`safe_area`]) so the JNI call exists only while that screen is
/// mounted and costs nothing on the eleven other screens that never open a
/// keyboard footer question in the first place.
#[cfg(target_os = "android")]
pub fn keyboard_inset() -> rinch::Signal<f32> {
    rinch::reactive::poll_signal(
        || {
            // Same conversion `safe_area` and `viewport_width` both use: 360dpi
            // (2.25x) is the Pixel-class fallback, and the guard is there
            // because a density of zero would turn an inset into an infinity
            // rather than into a wrong number.
            let scale = rinch_android::display::density_dpi().unwrap_or(360) as f32 / BASELINE_DPI;
            if scale <= 0.0 {
                return 0.0;
            }
            rinch_android::display::ime_inset().unwrap_or(0) as f32 / scale
        },
        rinch::reactive::PollRate::Hz(30),
    )
}

/// The desktop window has no soft keyboard to cover anything, so this is not
/// a stand-in the way [`SafeArea::PHONE_STANDIN`] is one — a desktop build
/// that ever produced a non-zero reading here would be reporting a keyboard
/// that does not exist.
#[cfg(not(target_os = "android"))]
pub fn keyboard_inset() -> rinch::Signal<f32> {
    rinch::Signal::new(0.0)
}

/// Tell the OS which way to draw the status and navigation bars' own contents.
///
/// `true` means the app has painted something light under them, so the clock,
/// the battery, the signal icons and the gesture pill should all be dark. The
/// system's default is the opposite — white glyphs, for a dark app — which on
/// this app's cream `--sla-paper` is barely there at all.
///
/// Both bars take the same answer here because the app is one shade end to end:
/// the theme paints the whole page, and the strip behind each bar is that page.
/// Rinch keeps them apart so an app with dark bottom chrome can differ.
///
/// Unlike [`safe_area`] this is not read once at mount — it is written whenever
/// the theme changes, so the bars follow a runtime flip of dark mode rather
/// than only the mode the app started in.
#[cfg(target_os = "android")]
pub fn set_light_system_bars(light: bool) {
    rinch_android::display::set_light_status_bars(light);
    rinch_android::display::set_light_navigation_bars(light);
}

#[cfg(not(target_os = "android"))]
pub fn set_light_system_bars(_light: bool) {}

/// A reading that is expensive enough to be worth keeping and short-lived
/// enough that keeping it forever is a bug.
///
/// This is the shape card K53 asked for and the reason it is a named type
/// rather than three lines inside [`viewport_width`]: *"structure the code so
/// this is testable without a window"*. An `OnceLock<f32>` cannot be tested at
/// all — there is one per process and no way to put it back — whereas this is
/// an ordinary value with a `forget`, so the property that actually matters
/// ("after a configuration change, the next read asks the platform again
/// rather than handing back what it had") is a `cargo test` on a laptop rather
/// than a thing somebody claims after flipping a switch on a phone.
///
/// An `AtomicU32` holding the f32's bits, rather than a `Mutex<Option<f32>>`,
/// because this is read from render closures on every redraw and a lock there
/// would be a lock in the frame path for a number. [`UNSET`] is the empty
/// state: it is a NaN bit pattern, and a viewport width is never NaN, so the
/// sentinel cannot collide with a real reading.
///
/// **A failed read is not cached.** `get_or_read` stores only a `Some`, which
/// keeps K31's original rule — a width asked for before the surface existed
/// must not become the answer for the rest of the process — intact through the
/// rewrite.
struct CachedWidth {
    bits: std::sync::atomic::AtomicU32,
}

/// The "nothing cached" bit pattern — a quiet NaN, which no width ever is.
const UNSET: u32 = u32::MAX;

impl CachedWidth {
    const fn new() -> Self {
        Self { bits: std::sync::atomic::AtomicU32::new(UNSET) }
    }

    fn get_or_read(&self, read: impl FnOnce() -> Option<f32>) -> Option<f32> {
        use std::sync::atomic::Ordering;
        let held = self.bits.load(Ordering::Relaxed);
        if held != UNSET {
            return Some(f32::from_bits(held));
        }
        let fresh = read()?;
        self.bits.store(fresh.to_bits(), Ordering::Relaxed);
        Some(fresh)
    }

    fn forget(&self) {
        self.bits.store(UNSET, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The one cache in this module. Declared on both platforms even though only
/// the Android [`viewport_width`] consults it, so that
/// [`forget_cached_readings`] is a real function with a real effect on the
/// laptop the tests run on — a `#[cfg]`-stubbed no-op would be a thing nobody
/// could check until it was on a phone.
static VIEWPORT_WIDTH: CachedWidth = CachedWidth::new();

/// Throw away every reading this module is holding on to, so that the next
/// caller gets a fresh one.
///
/// Called from `SystemStore::reread` and nowhere else — the platform has said
/// its configuration moved, and this is the half of that news which cannot be
/// delivered as a signal because its readers are plain functions called from
/// render closures rather than components.
///
/// It does **not** repaint anything, and that is deliberate rather than
/// missing. See `SystemStore::reread` for what a width change does and does
/// not reach, and `crate::store::system`'s header for the argument about
/// pages that were already rasterised.
pub fn forget_cached_readings() {
    VIEWPORT_WIDTH.forget();
}

/// Whether the system is in dark mode: `Some(true)` for night, `Some(false)`
/// for day, `None` when the platform declines to say.
///
/// `Configuration.uiMode & UI_MODE_NIGHT_MASK`, read through the activity's
/// own resources — the same answer `isSystemInDarkTheme()` is built on. The
/// shim is one line; what is worth writing down is the third state, because
/// the app has to have an answer for it.
///
/// **`None` is not "light".** `UI_MODE_NIGHT_UNDEFINED` is a real value that
/// some OEM skins and every non-phone UI mode leave in place for the life of
/// the process, and a JNI failure produces the same `None`. `ThemeChoice`'s
/// `FollowSystem` therefore falls back to *light* when this is `None`, but it
/// does so as a stated default of this app's own rather than by pretending
/// the platform answered — see `crate::derive::dark_active`, which is the one
/// place that decision is made.
///
/// The desktop has no such setting to read. It answers `None` for the same
/// reason [`keyboard_inset`] answers zero: not a stand-in, a fact.
#[cfg(target_os = "android")]
pub fn night_mode() -> Option<bool> {
    rinch_android::display::night_mode()
}

#[cfg(not(target_os = "android"))]
pub fn night_mode() -> Option<bool> {
    None
}

/// The dominant colour of the user's wallpaper — the seed Material You builds
/// a device's palette from, and what `AccentChoice::FromSystem` resolves
/// through since card K8.
///
/// `WallpaperManager.getWallpaperColors(FLAG_SYSTEM).getPrimaryColor()`,
/// unmodified. Every question about whether that colour is *legible* is
/// answered above this seam, in `theme::WallpaperAccent`, because a contrast
/// ratio is a fact about a pair of colours and this end of the call knows only
/// one of them.
///
/// **`None` is ordinary and is not a fault.** No wallpaper set, a live
/// wallpaper whose service publishes no colours (most do not), a lock-screen-
/// only image, or an OEM that replaced the wallpaper stack — all of them
/// answer `None` and none of them will fix themselves. The development
/// device this card was verified on is in exactly that state: the owner runs
/// a live wallpaper, so `FromSystem` on it resolves down the fallback arm to
/// Rust, every time, which is why that arm has as many tests as the arm that
/// finds a colour.
#[cfg(target_os = "android")]
pub fn wallpaper_primary() -> Option<crate::theme::Rgb> {
    let (r, g, b) = rinch_android::display::wallpaper_primary()?;
    Some(crate::theme::Rgb::new(r, g, b))
}

#[cfg(not(target_os = "android"))]
pub fn wallpaper_primary() -> Option<crate::theme::Rgb> {
    None
}

/// The tones `system_accent1_*` is published at, and the only values
/// [`system_accent`] will ask the platform about.
///
/// Verified rather than read off a document: `adb pull
/// /system/framework/framework-res.apk` from the development handset (a moto g
/// stylus 5G, SDK 33) and `aapt2 dump resources` lists exactly sixty-five
/// colour entries in this family — thirteen tones each for `system_accent1`,
/// `system_accent2`, `system_accent3` and the two neutral ramps — and nothing
/// between them. There is no `system_accent1_550`.
///
/// **This is a mirror of the list in `rinch_android::display::system_accent`,
/// and the duplication is the point.** The framework's copy is the one that
/// gates the JNI call; this one exists so that the app's *choice* of tone is a
/// fact a laptop can check. Card K15's rule is that anything found on hardware
/// should become a `cargo test` that fails without a device, and "we asked for
/// a resource that has never existed" is exactly the kind of thing that would
/// otherwise be discovered as a silent `None` on a phone, indistinguishable
/// from an honest API-30 device with no palette at all.
pub const PALETTE_TONES: [u16; 13] =
    [0, 10, 50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];

/// The one tone this app reads — `system_accent1_500`.
///
/// The middle of the ramp, and the seed rather than a finished colour. Which
/// tone to take is only a question if you are picking one *per mode*, and
/// `theme::DerivedAccent` explains at length why this app does not: the ramp's
/// tones are tuned against Material's surfaces, and this app paints on a warm
/// cream and a brown-cast near-black. 500 is the tone Material itself treats
/// as the reference chroma of the ramp, and `theme::derive_family` will walk it
/// to wherever it has to go for each of our two papers.
pub const PALETTE_TONE: u16 = 500;

/// One tone off the Material You palette **the system is itself themed with** —
/// `android.R.color.system_accent1_<tone>`, published as a framework colour
/// resource since Android 12 (API 31).
///
/// **Why this is asked before [`wallpaper_primary`] and not instead of it.**
/// The two answer different questions, and the difference only shows up on a
/// configuration that is easy to have and impossible to detect from the
/// wallpaper's side. `settings get secure theme_customization_overlay_packages`
/// on the development phone reads
/// `"android.theme.customization.color_source":"home_wallpaper"` today, which
/// is the case where the two agree. Pick one of the basic colours in Wallpaper
/// & style instead and that key reads `"preset"` — the system repaints itself
/// in the chosen colour, the wallpaper keeps its own quite different primary,
/// and `getWallpaperColors` goes on returning it with total confidence. That
/// is not a missing answer, it is a **confidently wrong** one, and an app that
/// only ever asked the wallpaper would follow the colour the user had gone
/// into the settings app specifically to override.
///
/// So the palette is asked first because it is the better answer, not merely
/// the newer one. The wallpaper stays as the fallback for API 28-30 — this
/// app's `minSdk` is 28 — where there is a wallpaper to read and no palette to
/// read it from. See `AccentChoice::resolve`, which is where the order is
/// actually written down and tested.
///
/// `None` for a tone that was never published, for every device below API 31,
/// and for a JNI failure. The first two are ordinary and permanent; the third
/// logs a warning on the framework side. The desktop has no palette at all and
/// says so, the same way [`night_mode`] and [`wallpaper_primary`] do.
#[cfg(target_os = "android")]
pub fn system_accent(tone: u16) -> Option<crate::theme::Rgb> {
    let (r, g, b) = rinch_android::display::system_accent(tone)?;
    Some(crate::theme::Rgb::new(r, g, b))
}

#[cfg(not(target_os = "android"))]
pub fn system_accent(_tone: u16) -> Option<crate::theme::Rgb> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 393 the screenshot net is measured against.
    ///
    /// `scripts/screenshot.sh` captures a real desktop window and checks
    /// regions from `scripts/screenshot-baseline.json` that count absolute
    /// pixels, all of them recorded at a 393-wide window. K31 replaced the
    /// constant every rasterised column was derived from with a function, and
    /// the one thing that must not change on this platform is the number that
    /// function returns. A red net for that reason would be a real failure
    /// about a fake problem, and it would be found by a human squinting at a
    /// screenshot rather than here.
    #[test]
    fn the_desktop_viewport_is_the_width_the_baseline_was_measured_at() {
        assert_eq!(viewport_width(), 393.0);
        assert_eq!(viewport_width(), crate::WIDTH as f32, "and it is the window we asked for");
    }

    #[test]
    fn the_desktop_reserves_the_strip_the_hard_coded_one_used_to() {
        let safe = safe_area();
        assert_eq!(safe.top, 44.0, "the status-bar strip");
        assert_eq!(safe.bottom, 22.0, "the bottom nav's padding");
        assert_eq!((safe.left, safe.right), (0.0, 0.0));
    }

    /// A desktop window has no soft keyboard, so the honest answer is always
    /// zero, not a phone-shaped guess the way [`SafeArea::PHONE_STANDIN`] is
    /// one for the other two readings.
    #[test]
    fn the_desktop_has_no_keyboard_to_cover_anything() {
        assert_eq!(keyboard_inset().get(), 0.0);
    }
    /// Card K53's own requirement, in the only form a laptop can check it:
    /// *"that the configuration-change handler actually re-reads rather than
    /// returning a cached value"*. The closure counts, so this is a statement
    /// about how many times the platform was asked, not about what came back.
    #[test]
    fn a_cached_reading_is_taken_once_and_taken_again_after_it_is_forgotten() {
        let cache = CachedWidth::new();
        let asks = std::cell::Cell::new(0);
        let mut answer = 432.0_f32;
        let read = |cache: &CachedWidth, answer: f32| {
            cache.get_or_read(|| {
                asks.set(asks.get() + 1);
                Some(answer)
            })
        };

        assert_eq!(read(&cache, answer), Some(432.0));
        assert_eq!(read(&cache, answer), Some(432.0));
        assert_eq!(asks.get(), 1, "the second read went to the cache, as it should");

        // The window moved underneath the app, and something told us so.
        cache.forget();
        answer = 673.0;
        assert_eq!(
            read(&cache, answer),
            Some(673.0),
            "after a configuration change the cache handed back the old width"
        );
        assert_eq!(asks.get(), 2, "and it asked the platform exactly once more");
    }

    /// K31's rule, carried through K53's rewrite: a width asked for before
    /// there was a surface to measure must not become the answer for the rest
    /// of the process.
    #[test]
    fn a_failed_reading_is_not_cached_and_is_asked_for_again() {
        let cache = CachedWidth::new();
        assert_eq!(cache.get_or_read(|| None), None);
        assert_eq!(cache.get_or_read(|| Some(432.0)), Some(432.0));
        assert_eq!(cache.get_or_read(|| None), Some(432.0), "and now it is held");
    }

    /// The sentinel has to be a value no real reading can produce, or a
    /// perfectly good width would read back as an empty cache forever.
    #[test]
    fn the_empty_marker_is_not_a_width_anything_could_report() {
        assert!(f32::from_bits(UNSET).is_nan());
        for width in [1.0_f32, 393.0, 432.0, 673.0, f32::MAX] {
            assert_ne!(width.to_bits(), UNSET, "{width} would read back as an empty cache");
        }
    }

    /// Calling it is the whole of the contract on the desktop: there is a
    /// cache to clear, it is not consulted here, and clearing it changes
    /// nothing about the number this platform reports.
    #[test]
    fn forgetting_the_cached_readings_leaves_the_desktop_width_where_it_was() {
        forget_cached_readings();
        assert_eq!(viewport_width(), crate::WIDTH as f32);
    }

    /// The desktop has no system theme to follow, no wallpaper to read and no
    /// Material You palette to be handed, and all three say so rather than
    /// guessing. This is the reading that makes `ThemeChoice::FollowSystem`
    /// fall back to light and `AccentChoice::FromSystem` fall back to Rust on
    /// a laptop.
    #[test]
    fn the_desktop_declines_to_answer_about_the_system_theme_the_palette_and_the_wallpaper() {
        assert_eq!(night_mode(), None);
        assert_eq!(wallpaper_primary(), None);
        assert_eq!(system_accent(PALETTE_TONE), None);
    }

    /// The tone this app actually asks for is a tone that exists.
    ///
    /// This is the whole of what K57's tone validation is worth on a laptop,
    /// and it is worth having: [`system_accent`]'s framework half rejects an
    /// unpublished tone by returning `None` before it makes a JNI call, which
    /// is exactly the same `None` an honest API-30 device returns — so a typo
    /// in [`PALETTE_TONE`] would not fail, it would silently move every
    /// `FromSystem` user onto the wallpaper fallback, on every device, forever.
    /// Nobody would see it; the app would just quietly be the old app.
    #[test]
    fn the_tone_the_app_reads_is_one_the_platform_publishes() {
        assert!(
            PALETTE_TONES.contains(&PALETTE_TONE),
            "system_accent1_{PALETTE_TONE} is not a resource any device has"
        );
    }

    /// And the mirror of the framework's list is the list the device really
    /// has — thirteen tones, the ends of the ramp at 0 and 1000, the two extra
    /// near-white steps Material puts at 10 and 50, and hundreds in between.
    /// Pulled from `/system/framework/framework-res.apk` on the development
    /// handset; see [`PALETTE_TONES`].
    ///
    /// The near-misses are named rather than left implied, because "550 looks
    /// like a tone" is precisely the mistake this list exists to catch.
    #[test]
    fn the_published_tones_are_the_thirteen_the_framework_ships_and_no_others() {
        assert_eq!(PALETTE_TONES.len(), 13);
        assert_eq!(PALETTE_TONES.first(), Some(&0));
        assert_eq!(PALETTE_TONES.last(), Some(&1000));
        for plausible_but_absent in [5, 20, 150, 450, 550, 950, 1100] {
            assert!(
                !PALETTE_TONES.contains(&plausible_but_absent),
                "system_accent1_{plausible_but_absent} does not exist and must not be asked for"
            );
        }
        // Ascending, because the constant reads as a ramp and a reader is
        // entitled to assume the order means something.
        assert!(
            PALETTE_TONES.windows(2).all(|pair| pair[0] < pair[1]),
            "{PALETTE_TONES:?} is not in ramp order"
        );
    }

}
