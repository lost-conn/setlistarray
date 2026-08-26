//! SetListArray — an offline-first book of the songs you know how to play.
//!
//! Nothing here talks to a network. There is no account and no sync.
//!
//! One crate, two entry points. The desktop binary (`src/main.rs`) calls
//! [`run_desktop`]; on Android the crate is built as a `cdylib` and the
//! platform calls `android_main` (`src/android.rs`). Everything between those
//! two doors is shared — the screens contain no `#[cfg]` at all. The two
//! places the platforms genuinely differ have their own seams:
//! [`platform::safe_area`] for the status bar and gesture bar, and
//! [`db::DataDir`] for where the library is written.

// The rsx! macro re-emits `let` bindings from control-flow bodies inside the
// closures it generates, but rustc lints the original spans — so bindings that
// demonstrably render come back as "unused". Silenced here rather than
// renaming half the screen code to `_x`.
#![allow(unused_variables)]
// The store API is complete ahead of the wireframe screens that will call it.
#![allow(dead_code)]

#[cfg(target_os = "android")]
mod android;
pub mod db;
mod derive;
mod model;
pub mod platform;
mod screens;
mod seed;
mod store;
mod theme;
mod ui;

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use db::DataDir;
use platform::SafeArea;
use screens::{Library, SetlistDetail, Setlists, SongDetail, Stub};
use store::{
    AttachmentsStore, LibraryViewStore, NavStore, PlaybackStore, Route, SettingsStore,
    SetlistsStore, SongsStore, Tab,
};
use theme::{T_NAV_LABEL, tokens};
use ui::icon;

/// A phone in the hand: 393×852 is the Pixel-class viewport the designs assume.
/// Ignored on Android, where the window is whatever the device is.
pub const WIDTH: u32 = 393;
pub const HEIGHT: u32 = 852;

/// The bottom nav's own breathing room, below which the gesture-bar inset is
/// not allowed to shrink it. A device with no gesture bar reports 0.
const NAV_MIN_GAP: f32 = 10.0;

#[component]
pub fn app() -> NodeHandle {
    // Where the library is written. The entry point installed it; publishing it
    // as a context is what lets a repository reach it without knowing which
    // platform it is on.
    create_context(DataDir::current());

    // Read once, at mount: on Android these are JNI calls, and the app is
    // portrait-locked, so the insets do not change under us.
    let safe: SafeArea = platform::safe_area();

    let settings = create_store(SettingsStore::new());
    create_store(SongsStore::new(seed::songs()));
    create_store(SetlistsStore::new(seed::setlists()));
    create_store(AttachmentsStore::new(seed::attachments()));
    create_store(LibraryViewStore::new());
    create_store(PlaybackStore::new());
    let nav = create_store(NavStore::new());

    rsx! {
        div {
            // Every token lives here; nothing downstream hard-codes a hex.
            // The horizontal insets sit on the root so every screen inherits
            // them; a portrait phone reports 0 for both, a cutout in landscape
            // does not.
            style: {move || format!(
                "{} height: 100vh; display: flex; flex-direction: column; \
                 padding-left: {left}px; padding-right: {right}px;",
                tokens(settings.dark_mode.get(), settings.accent_resolved()),
                left = safe.left,
                right = safe.right,
            )},

            // The OS draws the real status bar (and the notch); this is the
            // space it occupies.
            div { style: {format!("height: {}px; flex-shrink: 0;", safe.top)} }

            match nav.route.get() {
                Route::Library => Library {},
                Route::Setlists => Setlists {},
                Route::SongDetail(song_id) => SongDetail { id: {song_id} },
                Route::SetlistDetail(setlist_id) => SetlistDetail { id: {setlist_id} },
                Route::Settings => Stub { title: "Settings", wireframe: "1q" },
                Route::AddSong => Stub { title: "New song", wireframe: "1j" },
                Route::Performance(_) => Stub { title: "Performance", wireframe: "1o" },
            }

            {bottom_nav(__scope, safe.bottom.max(NAV_MIN_GAP))}
        }
    }
}

/// Two tabs, and only two. Settings is a gear in each tab's header.
///
/// `gap` is the room below the labels: the gesture bar's inset on Android, the
/// design's own 22px on the desktop.
#[component]
fn bottom_nav(gap: f32) -> NodeHandle {
    rsx! {
        div {
            style: {format!(
                "display: flex; border-top: 1px solid var(--sla-hairline); \
                 padding: 10px 0 {gap}px; flex-shrink: 0;"
            )},
            {nav_item(__scope, Tab::Songs, "Songs", TablerIcon::Music)}
            {nav_item(__scope, Tab::Setlists, "Setlists", TablerIcon::List)}
        }
    }
}

#[component]
fn nav_item(tab: Tab, label: &str, glyph: TablerIcon) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let label = label.to_string();

    rsx! {
        div {
            onclick: move || nav.select_tab(tab),
            style: {move || {
                let active = nav.tab.get() == tab;
                let color = if active { "var(--sla-accent)" } else { "var(--sla-muted)" };
                format!(
                    "flex: 1; display: flex; flex-direction: column; align-items: center; gap: 4px; color: {color};"
                )
            }},
            {icon(__scope, glyph, 21)}
            span {
                style: {move || {
                    let weight = if nav.tab.get() == tab { 600 } else { 500 };
                    format!("{T_NAV_LABEL} font-weight: {weight};")
                }},
                {label.clone()}
            }
        }
    }
}

/// The theme the shell is started with, on either platform.
fn theme_props() -> ThemeProviderProps {
    ThemeProviderProps {
        font_family: Some(theme::FONT_UI.into()),
        ..Default::default()
    }
}

/// The desktop entry point.
///
/// Newsreader and Karla are picked up from the system font list. Run
/// `scripts/install-fonts.sh` once if the app falls back to Georgia and a
/// default sans.
#[cfg(not(target_os = "android"))]
pub fn run_desktop() {
    DataDir::desktop_default().install();
    run_with_theme("SetListArray", WIDTH, HEIGHT, app, theme_props());
}

#[cfg(test)]
mod manifest {
    /// The Android manifest, checked from the desktop test run because that is
    /// where the tests are run and because these two facts are worth failing
    /// a build over.
    const MANIFEST: &str = include_str!("../android/AndroidManifest.xml");

    /// The offline promise, as a test. This app has no account, no sync and one
    /// network call in the whole plan (a page the user explicitly pasted, which
    /// on Android goes through the system, not through us). A permission
    /// appearing here is a regression, not a feature.
    #[test]
    fn the_android_manifest_asks_for_no_permissions() {
        assert!(
            !without_comments(MANIFEST).contains("<uses-permission"),
            "SetListArray must ship with no Android permissions"
        );
    }

    /// The manifest's own comment says why there are no permissions, and so
    /// contains the string the test above looks for.
    fn without_comments(xml: &str) -> String {
        let mut out = String::with_capacity(xml.len());
        let mut rest = xml;
        while let Some(start) = rest.find("<!--") {
            out.push_str(&rest[..start]);
            match rest[start..].find("-->") {
                Some(end) => rest = &rest[start + end + 3..],
                None => return out,
            }
        }
        out.push_str(rest);
        out
    }

    /// `android.app.lib_name` has to match `[lib] name` in Cargo.toml, or the
    /// activity looks for a `.so` the build never produced — and it fails at
    /// launch on the device, not here.
    #[test]
    fn the_manifest_names_the_library_this_crate_builds() {
        assert!(
            MANIFEST.contains(r#"android:value="setlistarray""#),
            "android.app.lib_name must be the crate's [lib] name"
        );
    }
}
