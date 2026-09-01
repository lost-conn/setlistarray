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
pub mod capture;
pub mod db;
mod derive;
#[cfg(test)]
mod gesture_reachability;
mod menu;
mod model;
pub mod pdf;
pub mod picker;
pub mod platform;
mod screens;
mod seed;
mod store;
mod theme;
mod ui;

use std::sync::OnceLock;

use rinch::prelude::*;
use rinch::reactive::Effect;
use rinch_tabler_icons::TablerIcon;

use db::DataDir;
use platform::SafeArea;
use screens::{
    AddToSetlistSheet, AttachmentViewer, CaptureScreen, ChartEditor, Library, SetlistDetail,
    SetlistPicker, Setlists, SongDetail, SongForm, SortGroupSheet, Stub,
};
use store::{
    AttachmentsStore, LibraryViewStore, NavStore, PlaybackStore, Route, SettingsStore,
    SetlistsStore, SongsStore, Storage, Tab,
};
use theme::{DARK_NEUTRALS, T_NAV_LABEL, tokens};
use ui::icon;

/// A phone in the hand: 393×852 is the Pixel-class viewport the designs assume.
/// Ignored on Android, where the window is whatever the device is.
pub const WIDTH: u32 = 393;
pub const HEIGHT: u32 = 852;

/// The typefaces this build carries, on either platform.
///
/// The app used to rely on the machine having Newsreader and Karla installed
/// (`scripts/install-fonts.sh`), which a phone never does — so on Android the
/// whole identity fell back to Noto Serif and Roboto, and `--sla-font-mono`
/// resolved to nothing at all. Shipping the files is the only version of this
/// that holds on a device somebody else owns.
///
/// What each face answers to is what matters here, and it is not symmetric:
///
/// * **Newsreader** and **Karla** are named directly by `--sla-font-display`
///   and `--sla-font-ui`, so registering them is enough. They also take the
///   `serif` / `sans-serif` slots so that the *tails* of those same stacks land
///   back on the app's own faces rather than on whatever the device has.
/// * **DejaVu Sans Mono** is the one that has to claim `monospace`. It is
///   already the first name in `--sla-font-mono`, so bundling it would be
///   enough for this app's own charts — but `monospace` is what a chart *is*
///   asking for, and on Android nothing answers it. See `theme::FONT_MONO`.
///
/// The italic is registered by name only: it is the same family as the roman,
/// and `font-style: italic` picks it from within that family.
const FONTS: &[AppFont] = &[
    AppFont::serif(include_bytes!("../assets/fonts/Newsreader[opsz,wght].ttf")),
    AppFont::new(include_bytes!("../assets/fonts/Newsreader-Italic[opsz,wght].ttf")),
    AppFont::sans_serif(include_bytes!("../assets/fonts/Karla[wght].ttf")),
    AppFont::monospace(include_bytes!("../assets/fonts/DejaVuSansMono.ttf")),
];

/// The bottom nav's own breathing room, below which the gesture-bar inset is
/// not allowed to shrink it. A device with no gesture bar reports 0.
const NAV_MIN_GAP: f32 = 10.0;

/// What `main` worked out before the window existed. `app` is a component and
/// takes no arguments, so the command line arrives this way rather than as
/// props.
struct Startup {
    seed: bool,
}

static STARTUP: OnceLock<Startup> = OnceLock::new();

#[component]
pub fn app() -> NodeHandle {
    let startup = STARTUP.get().expect("an entry point sets this before the window opens");

    // Where the library is written. The entry point installed it; publishing it
    // as a context is what lets a repository reach it without knowing which
    // platform it is on.
    let dir = DataDir::current();
    create_context(dir.clone());

    // Read once, at mount: on Android these are JNI calls, and the app is
    // portrait-locked, so the insets do not change under us.
    let safe: SafeArea = platform::safe_area();

    let storage = create_store(Storage::open(&dir));
    let mut loaded = storage.load();

    // `--seed`, and only into an empty library: running it twice must not give
    // you two of everything, and it must never land on top of real songs.
    if startup.seed && loaded.songs.is_empty() && loaded.setlists.is_empty() {
        if storage.is_persistent() {
            if storage.write("writing the demo library", seed::install) {
                loaded = storage.load();
            }
        } else {
            // No database to write it to; the demo lives in memory for as long
            // as the window is open.
            loaded.songs = seed::songs();
            loaded.setlists = seed::setlists();
            loaded.attachments = seed::attachments();
        }
    }

    let settings = create_store(SettingsStore::restored(storage));

    // Attachments first: a song owns its charts, so `SongsStore` is handed the
    // store it mutates them through. The dependency runs one way (see the note
    // on `SongsStore::attachments`), so there is no wiring-up step and no
    // half-built store either of them can be observed in.
    let attachments = create_store(AttachmentsStore::restored(storage, loaded.attachments));
    create_store(SongsStore::restored(storage, attachments, loaded.songs));
    create_store(SetlistsStore::restored(storage, loaded.setlists));
    create_store(LibraryViewStore::restored(storage));
    create_store(PlaybackStore::new());
    let nav = create_store(NavStore::new());

    // The status bar and the gesture bar are the OS's to draw, but what they
    // are drawn *over* is `--sla-paper`, and Android has no way to see it: its
    // default is white glyphs, which on the light theme's cream is barely
    // there. An effect rather than a call, because dark mode is flipped at
    // runtime and the bars have to follow it, not just the mode the app
    // launched in. Nothing platform-specific here — `platform` is where the
    // desktop's version of this (nothing at all) lives.
    //
    // It follows the *route* as well as the theme, and D5 is why. The
    // attachment viewer is dark chrome regardless of the app's mode, so on a
    // light-themed phone the clock and the gesture pill would stay dark and
    // vanish into the viewer's own black bars the moment a chart was opened.
    // Reading both signals in one effect is what makes them come back when the
    // viewer is left again.
    Effect::new(move || {
        let light = !settings.dark_mode.get() && !nav.route.get().full_screen();
        platform::set_light_system_bars(light);
    });

    rsx! {
        div {
            // Every token lives here; nothing downstream hard-codes a hex.
            // The horizontal insets sit on the root so every screen inherits
            // them; a portrait phone reports 0 for both, a cutout in landscape
            // does not.
            style: {move || format!(
                "{} height: 100vh; display: flex; flex-direction: column; \
                 position: relative; overflow: hidden; \
                 padding-left: {left}px; padding-right: {right}px;",
                tokens(settings.dark_mode.get(), settings.accent_resolved()),
                left = safe.left,
                right = safe.right,
            )},

            // The OS draws the real status bar (and the notch); this is the
            // space it occupies.
            //
            // It takes a background of its own rather than inheriting the
            // root's, because of the one screen that is not the theme's colour:
            // the attachment viewer (`1k`, D5) is dark chrome whatever the app
            // is set to, and a cream strip above its black top bar would be a
            // seam across the top of every full-screen chart in light mode.
            div {
                style: {move || format!(
                    "height: {}px; flex-shrink: 0; {}",
                    safe.top,
                    // Re-declaring the dark neutrals here and then reading
                    // `paper` back out of them keeps the rule that no hex is
                    // written outside `theme` — this strip sits *above* the
                    // viewer's own root, so it cannot inherit the override the
                    // viewer makes for everything inside it.
                    if nav.route.get().full_screen() {
                        format!("{DARK_NEUTRALS} background: var(--sla-paper);")
                    } else {
                        "background: var(--sla-paper);".to_string()
                    },
                )}
            }

            match nav.route.get() {
                Route::Library => Library {},
                Route::Setlists => Setlists {},
                Route::SongDetail(song_id) => SongDetail { id: {song_id} },
                Route::SetlistDetail(setlist_id) => SetlistDetail { id: {setlist_id} },
                Route::Settings => Stub { title: "Settings", wireframe: "1q" },
                // One screen, two doors: adding starts blank, editing arrives
                // carrying the song it is about to overwrite.
                Route::AddSong => SongForm {},
                Route::EditSong(song_id) => SongForm { editing: {song_id} },
                // The typed-chart editor (D2). `chart` is `None` for a new
                // one and `Some` for the chart being corrected — the same
                // one-screen-two-doors shape as the form above it.
                Route::TypeChart { song: song_id, chart } => ChartEditor {
                    song: {song_id},
                    chart: {chart},
                },
                // The full-screen viewer (D5). Takes the song as well as the
                // chart: the top bar prints one under the other, and ← lands
                // back on the screen it was opened from.
                Route::CaptureWebpage { song: song_id } => CaptureScreen { song: {song_id} },
                Route::ViewAttachment { song: song_id, attachment } => AttachmentViewer {
                    song: {song_id},
                    attachment: {attachment},
                },
                Route::Performance(_) => Stub { title: "Performance", wireframe: "1o" },
            }

            {bottom_nav(__scope, safe.bottom.max(NAV_MIN_GAP))}

            // The three bottom sheets. All stay mounted for the life of the
            // app, parked below the fold, so that opening one has something to
            // slide. They sit last so they paint over the screen and the nav.
            SortGroupSheet {}
            AddToSetlistSheet {}
            SetlistPicker {}
        }
    }
}

/// Two tabs, and only two. Settings is a gear in each tab's header.
///
/// `gap` is the room below the labels: the gesture bar's inset on Android, the
/// design's own 22px on the desktop.
///
/// ## Why it takes itself off screen rather than not being mounted
///
/// One route is full-screen (`Route::full_screen` — the attachment viewer,
/// D5), and this bar has to be gone for it. `display: none` in a reactive
/// style, rather than wrapping the call site in an `if`, for the same reason
/// the three bottom sheets stay mounted below the fold: the nav owns nothing
/// worth tearing down, unmounting and remounting it on every trip into a chart
/// would rebuild both tab items for nothing, and a style closure is the one
/// mechanism in this file that is already known to work on both platforms.
#[component]
fn bottom_nav(gap: f32) -> NodeHandle {
    let nav = use_store::<NavStore>();

    rsx! {
        div {
            style: {move || format!(
                "display: {}; border-top: 1px solid var(--sla-hairline); \
                 padding: 10px 0 {gap}px; flex-shrink: 0;",
                if nav.route.get().full_screen() { "none" } else { "flex" },
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

/// Everything an entry point has to settle before the window exists.
fn start(dir: DataDir, seed: bool) {
    dir.install();
    let _ = STARTUP.set(Startup { seed });
}

/// The desktop entry point.
///
/// The typefaces come from [`FONTS`], not from the system font list, so
/// `scripts/install-fonts.sh` is no longer what stands between this and
/// Georgia.
#[cfg(not(target_os = "android"))]
pub fn run_desktop() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // A fresh install opens an empty library. The demo content is behind this
    // flag, for screenshots and for anyone who wants something to look at
    // before they have typed a song in.
    start(DataDir::desktop_default(), args.iter().any(|a| a == "--seed"));
    run_with_fonts(
        "SetListArray",
        WIDTH,
        HEIGHT,
        app,
        Some(theme_props()),
        FONTS,
    );
}

/// The Android entry point's half of the same work: the platform hands us the
/// app-private directory, and there is no command line to read a flag from.
#[cfg(target_os = "android")]
pub fn start_android(dir: DataDir) {
    start(dir, false);
}

#[cfg(test)]
mod tests {
    /// The permission promise, in the shape it now has.
    ///
    /// This test used to be `the_android_manifest_asks_for_no_permissions`,
    /// and it asserted that `android/AndroidManifest.xml` contained no
    /// `<uses-permission>` element at all. That was the promise the README
    /// made, the manifest's own header comment made, and — from card E1 — this
    /// test made. It is not the promise any more, and the narrowing was
    /// deliberate rather than accidental, so it is written down here.
    ///
    /// **What happened.** Card E1 built the offline webpage capture engine
    /// (`src/capture/`) and ran into the one thing that cannot live inside an
    /// empty manifest: `android.permission.INTERNET` is required to open a
    /// socket on Android. It is not a runtime prompt and there is no way round
    /// it from app code — the installer puts a package in the `inet` group
    /// only when the manifest asks, and without that the kernel refuses the
    /// socket. So the app could have capture, or it could have a manifest with
    /// nothing in it, and not both. The owner chose capture, on 2026-08-26,
    /// with the reasoning recorded in `docs/CAPTURE.md`.
    ///
    /// **Read this as narrowing, not as erosion.** The promise did not become
    /// "we ask for what we need"; it became a specific, checkable claim: *one
    /// permission, one call site, nothing else reaches the network.* INTERNET
    /// is a normal permission — granted at install, never prompted for, absent
    /// from the app's permission screen — and it grants access to none of the
    /// user's data. Everything that would (camera, location, external storage,
    /// contacts) is still refused here, and this test is what refuses it: the
    /// allowlist is exactly one entry long and adding a second fails the
    /// build. The floor under the other half of the claim — the single call
    /// site — is `the_http_client_is_named_in_exactly_one_file` below.
    ///
    /// Deleting this test is still not the way to pass it.
    #[test]
    fn the_android_manifest_asks_only_for_internet() {
        let manifest = include_str!("../android/AndroidManifest.xml");
        // The header comment discusses `<uses-permission>` at length, so scan
        // the markup rather than the prose about it.
        let markup = strip_xml_comments(manifest);
        let asked: Vec<&str> = markup
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("<uses-permission"))
            .collect();

        assert_eq!(
            asked.len(),
            1,
            "the manifest asks for {} permissions; this app promises exactly one: {asked:?}",
            asked.len()
        );
        assert!(
            asked[0].contains("android.permission.INTERNET"),
            "the one permission this app asks for is INTERNET, and this is not it: {asked:?}"
        );
    }

    /// Three edits in three files that only work as one, and card K33 is the
    /// reason any of them exist.
    ///
    /// The app is essentially its native library — `classes.dex` is 18,900 B
    /// and everything else in the APK is under 3 KB — so where that library
    /// lives on the phone is the whole of the installed footprint. Measured on
    /// 2026-08-28: an 11.5 MiB APK occupying **46.6 MB** installed, because
    /// the installer had deflated `libsetlistarray.so` out of the APK into
    /// `/data/app/…/lib/arm64/` and the phone was keeping both copies.
    ///
    /// Not extracting it takes all three of these together:
    ///
    /// 1. `android:extractNativeLibs="false"` — the manifest asking the loader
    ///    to map the library where it already is;
    /// 2. `zip -0` over `lib/` in `build-apk.sh` — the bytes in the APK being
    ///    the bytes of the ELF, because a deflated entry cannot be mapped;
    /// 3. `zipalign -P 16` — that entry starting on a page boundary, 16 KB
    ///    because NDK r27c links every LOAD segment with `p_align 0x4000` and
    ///    Android 15 requires 16 KB-page support of a targetSdk-35 app.
    ///
    /// Any one of them alone is worse than none of them. The attribute over a
    /// compressed library is an `INSTALL_FAILED_INVALID_APK` from the
    /// installer; the attribute over a misaligned one is a device that
    /// installs and then cannot load the library at all. Both failures land a
    /// long way from the edit that caused them, on a phone, which is why they
    /// are asserted here on a laptop instead — the pattern cards K15 and K20
    /// established.
    ///
    /// This test reads the two files as text on purpose. There is no build
    /// system here to ask; `build-apk.sh` *is* the build system.
    #[test]
    fn the_apk_maps_its_native_library_instead_of_extracting_it() {
        let manifest = strip_xml_comments(include_str!("../android/AndroidManifest.xml"));
        assert!(
            manifest.contains(r#"android:extractNativeLibs="false""#),
            "AndroidManifest.xml no longer asks the loader to map the library out of the APK; \
             without it the installer extracts a second 25 MB copy (card K33)"
        );

        let script = strip_shell_comments(include_str!("../build-apk.sh"));
        assert!(
            script.contains("zip -qr -0 base.apk lib/"),
            "build-apk.sh no longer stores lib/ uncompressed; a deflated library cannot be \
             mapped, and the manifest above says it will be (card K33)"
        );
        assert!(
            script.contains("zipalign\" -f -P 16 4"),
            "build-apk.sh no longer page-aligns the stored library to 16 KB; NDK r27c links \
             it with p_align 0x4000 and a 16 KB-page device cannot map it otherwise (card K33)"
        );
    }

    /// The phone gets the GPU painter, and nothing about that is visible in a
    /// test run on a laptop — which is the reason to assert it here.
    ///
    /// A revert to the software shell would build, install, run, and draw the
    /// identical screen. The only symptom is that the app is slower on a
    /// device nobody is holding at the time. Measured on the moto g stylus 5G
    /// with `scripts/frame-probe.sh`, two runs each: 84.8-85.7fps on the GPU
    /// path against 70.6-70.9fps on the software one, and the frames that miss
    /// take two refreshes rather than three (p95 16.69ms against 25.18ms).
    ///
    /// Four cards paid for this default — K35 removed the readback that made
    /// K27 measure the GPU path *slower*, K42 found the present block was the
    /// GPU not fitting rather than a swapchain setting, K43 cut a third of the
    /// rasterisation, and K36 fixed the silent clipping difference that made
    /// shipping the faster path a bad trade whatever its speed. It is worth a
    /// line of test to keep it.
    #[test]
    fn the_phone_gets_the_gpu_painter_by_default() {
        let script = strip_shell_comments(include_str!("../build-apk.sh"));
        // The *first* `FEATURES=` is the default; the ones after it are the
        // flags that override it, and `--software` legitimately assigns an
        // empty string. Reading only the first assignment is what makes this
        // assertion about the default rather than about the flags.
        let default = script
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("FEATURES="))
            .expect("build-apk.sh sets FEATURES");
        assert_eq!(
            default, r#"FEATURES="rinch/android-gpu""#,
            "build-apk.sh no longer defaults to rinch's android-gpu shell; the app is \
             slower on the phone and nothing else says so (card K41)"
        );
        assert!(
            script.contains("--software)"),
            "build-apk.sh no longer offers --software; the GPU path is proven on exactly \
             one driver and the painter with years behind it should stay one flag away \
             (card K41)"
        );
    }

    /// Link-time optimisation is asked for by `build-apk.sh`, not by
    /// `Cargo.toml`, and that is deliberate enough to be worth holding still.
    ///
    /// `lto = "fat"` and `codegen-units = 1` take 3.29 MB off the library
    /// (25,509,032 → 22,223,448 B) and cost 38 seconds on an APK build. Put
    /// them in `[profile.release]` and the desktop build pays too, where they
    /// buy nothing: an incremental rebuild after touching this very file went
    /// **7.28s → 124.74s** — and `scripts/screenshot.sh` runs a release build
    /// every time the visual net is checked.
    ///
    /// So the two settings are environment variables in the one script that
    /// builds for the phone. This test is what stops them drifting back into
    /// `Cargo.toml`, where the seventeen-fold slowdown would arrive silently
    /// and be blamed on the framework.
    #[test]
    fn link_time_optimisation_is_asked_for_by_the_apk_build_alone() {
        let script = strip_shell_comments(include_str!("../build-apk.sh"));
        assert!(
            script.contains(r#": "${CARGO_PROFILE_RELEASE_LTO:=fat}""#)
                && script.contains(r#": "${CARGO_PROFILE_RELEASE_CODEGEN_UNITS:=1}""#),
            "build-apk.sh no longer asks for fat LTO; that is 3.29 MB back on the phone \
             (card K33)"
        );

        let cargo_toml = include_str!("../Cargo.toml");
        let profile: Vec<&str> = cargo_toml
            .lines()
            .map(str::trim)
            .skip_while(|line| *line != "[profile.release]")
            .take_while(|line| !line.starts_with('[') || *line == "[profile.release]")
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        assert!(
            !profile
                .iter()
                .any(|line| line.starts_with("lto") || line.starts_with("codegen-units")),
            "[profile.release] sets an LTO knob, which makes every desktop rebuild 17x \
             slower for a saving only the phone sees (card K33). Found: {profile:?}"
        );
    }

    /// 10.1 MB of symbol names that nothing on the phone reads.
    ///
    /// `Cargo.toml` had no `[profile.release]` section at all until card K33,
    /// so `libsetlistarray.so` shipped with `.symtab` (4,646,160 B) and
    /// `.strtab` (5,497,880 B) intact. Measured either side of adding two
    /// lines: the library went 35,665,688 → 25,509,032 B on the device and the
    /// APK 12,018,187 → 10,187,275 B.
    ///
    /// Nothing fails visibly if this disappears — the app builds, installs and
    /// runs exactly as before, ten megabytes heavier — which is precisely why
    /// it is worth a test. `panic = "abort"` is checked for too, in the other
    /// direction: `src/pdf/pages.rs` catches unwinds out of hayro because a
    /// chart is a file from the internet, and aborting would turn a malformed
    /// one into a crashed app.
    #[test]
    fn the_release_profile_strips_symbols_and_still_unwinds() {
        let cargo_toml = include_str!("../Cargo.toml");
        let settings: Vec<&str> = cargo_toml
            .lines()
            .map(str::trim)
            .skip_while(|line| *line != "[profile.release]")
            .skip(1)
            .take_while(|line| !line.starts_with('['))
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();

        assert!(
            settings.contains(&"strip = \"symbols\""),
            "[profile.release] no longer strips symbols; that is 10 MB back on the phone \
             for nothing (card K33). Found: {settings:?}"
        );
        assert!(
            !settings.iter().any(|line| line.starts_with("panic")),
            "[profile.release] sets a panic strategy; src/pdf/pages.rs relies on unwinding \
             to survive a malformed PDF (card D4). Found: {settings:?}"
        );
    }

    /// The floor under "one network call exists in the entire app".
    ///
    /// `docs/PLAN.md` makes that claim in its cross-cutting section and card
    /// X2 is on the backlog to assert it properly — walk the code, or watch a
    /// running app, and prove nothing else dials out. This is not X2. It is
    /// the cheap half that can be written today: rinch-http is the only HTTP
    /// client in the dependency tree this crate names, and `src/capture/
    /// fetch.rs` is the only file allowed to name it.
    ///
    /// What that catches is somebody adding a second fetch site — an update
    /// check, a font download, a crash reporter — because they would have to
    /// name the client to do it, and this fails when they do. What it does not
    /// catch is a transitive dependency opening its own socket, or this crate
    /// growing a *different* HTTP client. Those are X2's job. Treat a failure
    /// here as a question about the permission in `AndroidManifest.xml`: it is
    /// declared for one call site, and this is the test that counts them.
    #[test]
    fn the_http_client_is_named_in_exactly_one_file() {
        // Spelled in two halves so that this file, which is one of the files
        // being searched, is not itself a hit.
        let needle = concat!("rinch", "_http");
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut naming: Vec<String> = Vec::new();
        collect_rust_files(&src, &mut |path| {
            let text = std::fs::read_to_string(path).expect("a source file this crate compiles");
            if text.contains(needle) {
                naming.push(
                    path.strip_prefix(src.parent().unwrap())
                        .unwrap_or(path)
                        .display()
                        .to_string(),
                );
            }
        });
        naming.sort();

        assert_eq!(
            naming,
            vec!["src/capture/fetch.rs".to_string()],
            "the HTTP client is named outside the one file that is allowed to open a socket"
        );
    }

    /// Remove `<!-- ... -->` regions, so a test can read markup without
    /// reading the commentary around it. Good enough for a file this crate
    /// owns; not an XML parser.
    fn strip_xml_comments(xml: &str) -> String {
        let mut out = String::with_capacity(xml.len());
        let mut rest = xml;
        while let Some(open) = rest.find("<!--") {
            out.push_str(&rest[..open]);
            match rest[open..].find("-->") {
                Some(close) => rest = &rest[open + close + 3..],
                None => return out,
            }
        }
        out.push_str(rest);
        out
    }

    /// Drop whole-line `#` comments, so a test can read a shell script's code
    /// without reading the prose above it. Deliberately does not touch a `#`
    /// that follows code on the same line — there are none in `build-apk.sh`,
    /// and guessing at quoting would be worse than not trying.
    fn strip_shell_comments(script: &str) -> String {
        script
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Every `.rs` file under `dir`, depth first.
    fn collect_rust_files(dir: &std::path::Path, visit: &mut impl FnMut(&std::path::Path)) {
        for entry in std::fs::read_dir(dir).expect("the crate's own source directory") {
            let path = entry.expect("a readable directory entry").path();
            if path.is_dir() {
                collect_rust_files(&path, visit);
            } else if path.extension().is_some_and(|e| e == "rs") {
                visit(&path);
            }
        }
    }
}
