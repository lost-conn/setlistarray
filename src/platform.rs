//! The system bars: how much of the screen the app is not allowed to draw in,
//! and which way round the OS should draw the bits it puts there.
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

    #[test]
    fn the_desktop_reserves_the_strip_the_hard_coded_one_used_to() {
        let safe = safe_area();
        assert_eq!(safe.top, 44.0, "the status-bar strip");
        assert_eq!(safe.bottom, 22.0, "the bottom nav's padding");
        assert_eq!((safe.left, safe.right), (0.0, 0.0));
    }
}
