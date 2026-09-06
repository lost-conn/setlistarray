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
/// Read at mount. The app is portrait-locked, so these do not change under us;
/// a rotation-aware version would have to re-read them on a configuration
/// change, which the Rinch Android shell does not surface yet.
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
/// number that cannot change under a portrait-locked app is a cost with no
/// buyer. A failed read is deliberately *not* cached: it falls back for that
/// call and asks again next time, so a width asked for before the surface
/// existed does not become the answer for the rest of the process.
#[cfg(target_os = "android")]
pub fn viewport_width() -> f32 {
    use std::sync::OnceLock;
    static WIDTH: OnceLock<f32> = OnceLock::new();

    if let Some(width) = WIDTH.get() {
        return *width;
    }
    let Some((physical, _)) = rinch_android::display::viewport_size() else {
        return crate::WIDTH as f32;
    };
    // The same conversion `safe_area` above does, down to the fallback: 360dpi
    // (2.25x) is the Pixel-class default, and the guard is there because a
    // density of zero would turn a width into an infinity rather than into a
    // wrong number.
    let scale = rinch_android::display::density_dpi().unwrap_or(360) as f32 / BASELINE_DPI;
    if scale <= 0.0 {
        return crate::WIDTH as f32;
    }
    let width = physical as f32 / scale;
    // Once, on the first successful read, because the failure K31 was written
    // about was a width nobody could see. The app drew every rasterised page
    // 393 wide on a 432-wide phone for weeks, and the only symptom was a strip
    // of backdrop that looked like a margin somebody had chosen. A line in
    // logcat is what turns "the pages look a bit narrow" into a number that
    // can be checked against `wm size` and `wm density`. It costs one line per
    // process because the value is cached below and this is inside the miss.
    log::info!("viewport: {physical}px physical / {scale:.2} = {width} CSS px wide");
    let _ = WIDTH.set(width);
    width
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
}
