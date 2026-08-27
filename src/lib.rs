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
mod menu;
mod model;
pub mod platform;
mod screens;
mod seed;
mod store;
mod theme;
mod ui;

use std::sync::OnceLock;

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use db::DataDir;
use platform::SafeArea;
use screens::{
    AddToSetlistSheet, Library, SetlistDetail, SetlistPicker, Setlists, SongDetail, SongForm,
    SortGroupSheet, Stub,
};
use store::{
    AttachmentsStore, LibraryViewStore, NavStore, PlaybackStore, Route, SettingsStore,
    SetlistsStore, SongsStore, Storage, Tab,
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
            div { style: {format!("height: {}px; flex-shrink: 0;", safe.top)} }

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

/// Everything an entry point has to settle before the window exists.
fn start(dir: DataDir, seed: bool) {
    dir.install();
    let _ = STARTUP.set(Startup { seed });
}

/// The desktop entry point.
///
/// Newsreader and Karla are picked up from the system font list. Run
/// `scripts/install-fonts.sh` once if the app falls back to Georgia and a
/// default sans.
#[cfg(not(target_os = "android"))]
pub fn run_desktop() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // A fresh install opens an empty library. The demo content is behind this
    // flag, for screenshots and for anyone who wants something to look at
    // before they have typed a song in.
    start(DataDir::desktop_default(), args.iter().any(|a| a == "--seed"));
    run_with_theme("SetListArray", WIDTH, HEIGHT, app, theme_props());
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
