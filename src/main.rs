//! SetListArray — an offline-first book of the songs you know how to play.
//!
//! Nothing here talks to a network. There is no account and no sync.
//!
//! The handoff targets Android; Rinch currently ships desktop and wasm
//! backends, so this runs in a phone-shaped desktop window until an Android
//! backend exists. Nothing in the UI code assumes the desktop.

// The rsx! macro re-emits `let` bindings from control-flow bodies inside the
// closures it generates, but rustc lints the original spans — so bindings that
// demonstrably render come back as "unused". Silenced here rather than
// renaming half the screen code to `_x`.
#![allow(unused_variables)]
// The store API is complete ahead of the wireframe screens that will call it.
#![allow(dead_code)]

mod db;
mod derive;
mod model;
mod screens;
mod seed;
mod store;
mod theme;
mod ui;

use std::sync::OnceLock;

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use db::DataDir;
use screens::{Library, SetlistDetail, Setlists, SongDetail, Stub};
use store::{
    AttachmentsStore, LibraryViewStore, NavStore, PlaybackStore, Route, SettingsStore,
    SetlistsStore, SongsStore, Storage, Tab,
};
use theme::{T_NAV_LABEL, tokens};
use ui::icon;

/// A phone in the hand: 393×852 is the Pixel-class viewport the designs assume.
const WIDTH: u32 = 393;
const HEIGHT: u32 = 852;

/// What `main` worked out before the window existed. `app` is a component and
/// takes no arguments, so the directory arrives this way rather than as props.
struct Startup {
    dir: DataDir,
}

static STARTUP: OnceLock<Startup> = OnceLock::new();

#[component]
fn app() -> NodeHandle {
    let startup = STARTUP.get().expect("main sets this before the window opens");

    let storage = create_store(Storage::open(&startup.dir));
    let mut loaded = storage.load();

    // The demo content, until card B5 puts it behind a flag. Only into an
    // empty library: it must never land on top of real songs.
    if loaded.songs.is_empty() && loaded.setlists.is_empty() {
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
    create_store(SongsStore::restored(storage, loaded.songs));
    create_store(SetlistsStore::restored(storage, loaded.setlists));
    create_store(AttachmentsStore::restored(storage, loaded.attachments));
    create_store(LibraryViewStore::restored(storage));
    create_store(PlaybackStore::new());
    let nav = create_store(NavStore::new());

    rsx! {
        div {
            // Every token lives here; nothing downstream hard-codes a hex.
            style: {move || format!(
                "{} height: 100vh; display: flex; flex-direction: column;",
                tokens(settings.dark_mode.get(), settings.accent_resolved())
            )},

            // The OS draws the real status bar; this is the space it occupies.
            div { style: "height: 44px; flex-shrink: 0;" }

            match nav.route.get() {
                Route::Library => Library {},
                Route::Setlists => Setlists {},
                Route::SongDetail(song_id) => SongDetail { id: {song_id} },
                Route::SetlistDetail(setlist_id) => SetlistDetail { id: {setlist_id} },
                Route::Settings => Stub { title: "Settings", wireframe: "1q" },
                Route::AddSong => Stub { title: "New song", wireframe: "1j" },
                Route::Performance(_) => Stub { title: "Performance", wireframe: "1o" },
            }

            {bottom_nav(__scope)}
        }
    }
}

/// Two tabs, and only two. Settings is a gear in each tab's header.
#[component]
fn bottom_nav() -> NodeHandle {
    rsx! {
        div {
            style: "display: flex; border-top: 1px solid var(--sla-hairline); padding: 10px 0 22px; flex-shrink: 0;",
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

fn main() {
    let _ = STARTUP.set(Startup {
        dir: DataDir::desktop_default(),
    });

    // Newsreader and Karla are picked up from the system font list. Run
    // `scripts/install-fonts.sh` once if the app falls back to Georgia and a
    // default sans.
    run_with_theme(
        "SetListArray",
        WIDTH,
        HEIGHT,
        app,
        ThemeProviderProps {
            font_family: Some(theme::FONT_UI.into()),
            ..Default::default()
        },
    );
}
