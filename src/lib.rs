//! SetListArray — an offline-first book of the songs you know how to play.
//!
//! Nothing here talks to a network. There is no account and no sync.
//!
//! One crate, two entry points. The desktop binary (`src/main.rs`) calls
//! [`run_desktop`]; on Android the crate is built as a `cdylib` and the
//! platform calls `android_main` (`src/android.rs`). Everything between those
//! two doors is shared — the screens contain no `#[cfg]` at all. Where the
//! platforms genuinely differ, each difference has its own seam rather than a
//! `#[cfg]` in a screen:
//!
//! * [`platform::safe_area`] — the status bar and gesture bar.
//! * [`db::DataDir`] — where the library is written.
//! * [`picker::FilePicker`] — a dialog on the desktop, an activity on Android.
//! * [`keep_awake::ScreenLock`] — a window flag on Android, a D-Bus inhibit on
//!   the desktop, and nothing at all anywhere else.
//!
//! This list said "the two places" for a long time after it had become four,
//! which is the usual fate of a counted list in a comment. It is a list now.

// The rsx! macro re-emits `let` bindings from control-flow bodies inside the
// closures it generates, but rustc lints the original spans — so bindings that
// demonstrably render come back as "unused". Silenced here rather than
// renaming half the screen code to `_x`.
#![allow(unused_variables)]
// The store API is complete ahead of the wireframe screens that will call it.
#![allow(dead_code)]

#[cfg(target_os = "android")]
mod android;
/// The guard that keeps a new screen from becoming a Back-key dead end. Tests
/// only — it reads this crate's own source rather than running any of it.
#[cfg(test)]
mod back_coverage;
pub mod capture;
pub mod db;
mod derive;
pub mod export;
#[cfg(test)]
mod gesture_reachability;
#[cfg(test)]
mod glyph_coverage;
pub mod import;
pub mod keep_awake;
mod menu;
mod model;
pub mod pdf;
pub mod picker;
pub mod platform;
mod screens;
mod seed;
/// The app as an Android share target — what another app's share sheet hands
/// over, and what this app makes of it. Compiled on both platforms on purpose:
/// only the registration and the `content://` read are Android's, and every
/// decision above them is testable on a laptop.
pub mod share;
mod store;
mod theme;
mod ui;

use std::sync::OnceLock;

use rinch::core::{KeyEventData, set_keyboard_interceptor};
use rinch::prelude::*;
use rinch::reactive::Effect;
use rinch_tabler_icons::TablerIcon;

use db::DataDir;
use screens::{
    AddToSetlistSheet, AttachmentViewer, CaptureScreen, ChartEditor, FilterSheet, FirstRun,
    Library, Performance, RunningOrderSheet, SaveShared, Search, SetlistDetail, SetlistPicker,
    Settings, Setlists, SongDetail, SongForm, SortGroupSheet, TuningSheet,
};
use store::{
    AttachmentsStore, BackPress, LibraryViewStore, NavStore, PlaybackStore, Route, SettingsStore,
    SetlistsStore, SongsStore, Storage, SystemStore, Tab,
};
use theme::{DARK_NEUTRALS, T_META, T_NAV_LABEL, tokens};
use ui::icon;

/// A phone in the hand: 393×852 is the Pixel-class viewport the designs assume.
/// Ignored on Android, where the window is whatever the device is.
///
/// So this is the window to *ask* for and not the window the app is in, and
/// card K31 is what the difference cost: everything that rasterised a page to a
/// pixel width read this constant, and on a 432-logical handset drew 393 wide
/// with a strip of backdrop down each side. Anything asking how wide the screen
/// actually is wants [`platform::viewport_width`], which is this number on the
/// desktop — where the shell honours it — and the real surface on Android. The
/// two entry points below are the only callers left that mean this one.
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
///
/// `pub(crate)` since F3, because one screen has to pad itself: performance
/// mode is `Route::full_screen`, so the nav that normally holds this gap is
/// `display: none` under it, and its progress strip would otherwise be drawn
/// beneath the phone's gesture pill.
pub(crate) const NAV_MIN_GAP: f32 = 10.0;

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

    // What the platform says about itself: the night mode, the system's own
    // Material You palette, the wallpaper colour, the insets and the window's
    // own width, all five taken here and then held in signals rather than in
    // `let` bindings.
    //
    // **They used to be `let` bindings, and card K53 is why they are not.**
    // The safe area was read once at mount with a comment saying the app is
    // portrait-locked so the insets cannot change under it, and the viewport
    // width was asked for here and thrown away purely to fill
    // `platform::viewport_width`'s process-long cache before the first frame
    // wanted it. Both were true and both were frozen, for the same reason:
    // there was nothing to listen to. There is now, and it is registered a few
    // lines below — see `SystemStore` for what one re-read reaches and the one
    // thing it deliberately does not.
    //
    // First, before any store that resolves a colour: `SettingsStore` is handed
    // this handle and asks it what the system looks like every time anything
    // reads a theme.
    let system = create_store(SystemStore::read());

    let storage = create_store(Storage::open(&dir));
    let mut loaded = storage.load();

    // Card J6: a directory under `attachments/` that no row names, left by a
    // crash between `SongsStore::attach` minting the row (and, with it, the
    // directory `Repo::create_attachment` makes in the same call) and a
    // producer — `capture::write_into`, `pdf::import`, the typed editor —
    // finishing the bytes inside it. Every *ordinary* failure on that path
    // was audited for this card and none of them leak: `AttachmentsStore::
    // forget` already deletes a row's directory along with its row, which is
    // what `attach`/`detach`/`capture::attach_captured`/`pdf::import` all call
    // on their own failure exits. Only an actual crash, or a directory
    // removal that itself failed partway, leaves one behind — which is
    // exactly what this sweep is for.
    //
    // Right here and nowhere else: `loaded.attachments` is the read this
    // session is trusting, before the `--seed` block below can add its own
    // and before any store exists that could attach a chart of its own.
    // `Storage::sweep_orphaned_attachments` carries its own two refusals —
    // no repository, or a fault already on record from the load just above —
    // see its doc comment for why a failed read must never be mistaken for an
    // empty library.
    let orphaned = storage.sweep_orphaned_attachments(
        &loaded.attachments.iter().map(|a| a.id).collect::<Vec<_>>(),
    );
    if !orphaned.is_empty() {
        eprintln!(
            "setlistarray: swept {} orphaned attachment director{} with no row: {:?}",
            orphaned.len(),
            if orphaned.len() == 1 { "y" } else { "ies" },
            orphaned
        );
    }

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

    let settings = create_store(SettingsStore::restored(storage, system));

    // Setlists before songs: `SongsStore::delete` reaches into `SetlistsStore`
    // to drop a deleted song's id out of every running order the instant the
    // delete lands (card J5), which means `SongsStore` has to be handed a
    // `SetlistsStore` that already exists. `SetlistsStore` itself depends on
    // nothing, so it can move first without anything left half-built.
    //
    // Attachments before songs for the same reason: a song owns its charts,
    // so `SongsStore` is handed the store it mutates them through too. The
    // dependency runs one way in both cases (see the notes on
    // `SongsStore::attachments` and `SongsStore::setlists`), so there is no
    // wiring-up step and no half-built store any of the three can be observed
    // in.
    let attachments = create_store(AttachmentsStore::restored(storage, loaded.attachments));
    let setlists = create_store(SetlistsStore::restored(storage, loaded.setlists));
    let songs = create_store(SongsStore::restored(storage, attachments, setlists, loaded.songs));
    create_store(LibraryViewStore::restored(storage));
    let playback = create_store(PlaybackStore::new());
    let nav = create_store(NavStore::new());

    // Being shared *to* (`crate::share`). Here, at the top, and not from the
    // screen that answers a share — because the share that matters most is the
    // one that *started the app*, and by the rules `rinch_android::intent` sets
    // out, that intent is queued before a line of app code runs and is held
    // rather than discarded until something registers for it. A handler
    // installed from `SaveShared`'s component body would therefore be installed
    // by the screen the share was supposed to open, which is the wrong way
    // round: nothing would ever open it.
    //
    // Registering once here also means there is exactly one of these for the
    // life of the process, which is what the `set_keyboard_interceptor` and
    // `set_configuration_change_handler` notes below both argue for on their
    // own slots — except that this registry *appends* rather than replacing, so
    // a second registration would not overwrite this one, it would deliver
    // every share twice.
    #[cfg(target_os = "android")]
    share::listen(nav);

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
    //
    // `dark_chrome` rather than `full_screen`, since F1: performance mode is
    // full-screen too and is *not* dark by default — it follows the app theme,
    // which the handoff asked for — so asking the wrong one of the two
    // questions here would have put white system glyphs on a cream `1o` in
    // light mode.
    //
    // Since H1 it is `derive::dark_chrome` and takes two arguments, because the
    // route stopped being able to answer on its own the moment the Settings
    // screen grew the switch that forces `1o` dark. Three signals in one effect
    // now: the mode, the route, and the performance theme. `Route::dark_chrome`
    // is still the route half of it and still says the viewer is dark whatever
    // anybody sets.
    Effect::new(move || {
        let light = !settings.dark_active()
            && !derive::dark_chrome(nav.route.get(), settings.performance_theme.get());
        platform::set_light_system_bars(light);
    });

    // The screen must not go off in the middle of a song (F4). The toggle in
    // performance mode's bottom bar flips `PlaybackStore::keep_awake`, and this
    // is the thing that reads it — until now nothing did, which
    // `screens::performance`'s header said out loud for as long as it was true.
    //
    // **Here, and not inside the screen**, which is the obvious place and is
    // wrong. Performance mode has four ways out — the ✕, the running-order
    // sheet's own dismissal into a different route, a set deleted underneath it,
    // and the two entry points navigating somewhere else entirely — and a
    // release written into the screen has to be on every one of them. Up here
    // the question is asked of the *route*, so leaving performance mode by any
    // means at all is a recomputation that comes out `false`; there is no path
    // to forget because there is no path. `keep_awake::wanted` is that rule and
    // it is unit-tested against every route the app has.
    //
    // The same shape as the bars above it, deliberately: two signals read in one
    // effect, an answer written to a `platform`-style seam that is a real call
    // on one platform and nothing on the other. Nothing here is
    // platform-specific — `keep_awake` is where Android's window flag and the
    // desktop's D-Bus inhibit part company.
    Effect::new(move || {
        keep_awake::set(keep_awake::wanted(nav.route.get(), playback.keep_awake.get()));
    });

    // A route can outlive the record it is a screen for (card J7). Delete a
    // song from its own detail screen and, before this effect existed, the
    // delete committed, the library underneath was correct, and the screen
    // stayed open on a row the store would no longer return — because the
    // handler that deletes it (`SongMenuItems`, in `crate::menu`) is shared
    // with a library row's long-press, which must *not* navigate, so putting
    // `nav.back()` there would have fixed one caller by breaking the other.
    //
    // Up here the question is asked of the *route* instead, the same move
    // the two effects above it both make: "does this screen still have
    // something to show" is a recomputation, not an event, so every way the
    // record underneath a route can vanish — the ⋮ menu on the very screen
    // showing it, a delete from the list it was opened from, one day an undo
    // that goes the wrong way — is the same recomputation rather than a
    // handler somewhere that has to remember to call `nav.back()`.
    // `derive::route_orphaned` is the pure half of that rule and is
    // unit-tested against every route the app has; what only a live effect
    // can add is the two stores it is asked to check against.
    //
    // Performance mode's own ✕ (`close`, in `screens::performance`) lands on
    // `Route::SetlistDetail(id)` on purpose, because there is a set to go back
    // to. Here there is not — the setlist that route names is the one that is
    // gone — so this stops the gig the ordinary way (`PlaybackStore::stop`,
    // clearing `index` along with it) and then falls through to `nav.back()`
    // like every other orphaned route, landing on the Setlists tab rather than
    // on a "This setlist is gone." screen for a setlist nobody can get back.
    Effect::new(move || {
        let route = nav.route.get();
        let orphaned = derive::route_orphaned(
            route,
            |id| songs.get(id).is_some(),
            |id| setlists.get(id).is_some(),
        );
        if orphaned {
            if matches!(route, Route::Performance(_)) {
                playback.stop();
            }
            nav.back();
        }
    });

    // The phone's Back key and Back gesture, which until this card did nothing
    // at all.
    //
    // Rinch's Android shell maps `AK::Back` to `KeyCode::Escape` and returns
    // `InputStatus::Handled` for every key it sees, so the OS never got its own
    // default handling of Back and this app never acted on it either: Back on
    // song detail left you on song detail, and Back on the library left you in
    // the library. The key was arriving and falling on the floor.
    //
    // `set_keyboard_interceptor` is rinch-core's one document-level keyboard
    // hook — one per thread, consulted before the focus arbiter routes the key
    // anywhere (`rinch/src/app/event_dispatch.rs`), returning `true` to say the
    // key is handled and must not propagate. Installed here rather than in a
    // screen for the reason there is only one of it: a hook a screen installed
    // on mount would be silently overwritten by the next screen to mount and
    // never reinstated when that one left. What varies per screen is *what Back
    // does*, and that lives in `NavStore` (`register_back`), read by
    // `press_back` below — a signal, which is a thing the app already knows how
    // to have one of.
    //
    // Escape rather than a name of Android's own, because Escape is what
    // reaches this hook: the shell has already translated the keycode by the
    // time anything app-side can see it. That makes the desktop's Escape key
    // Back as well, which is not a compromise — it is the same key on the same
    // seam, and it is what lets the whole of this behaviour be driven on a
    // laptop instead of only on a phone.
    //
    // `press_back` decides and acts; the one thing it will not do is end the
    // app, because a function that can end the process cannot be unit-tested.
    // So that last step is here, and it is the only line of this card that a
    // `cargo test` cannot reach.
    // The platform saying that something the app read at mount is now stale —
    // the system flipped to dark at sunset, the wallpaper changed, a cutout
    // moved, the window resized. Cards K8 and K53, and the reason both of them
    // could be done at once: they are the same event.
    //
    // Registered here for the same reason `set_keyboard_interceptor` below is:
    // there is exactly one slot, last write wins, and a handler installed from
    // inside a screen would be silently replaced by the next screen to mount
    // and never reinstated. What varies is not *who* listens, it is what has to
    // be re-read, and that is `SystemStore::reread` — one function, five
    // readings (K57 added the system palette), no `#[cfg]`.
    //
    // No `#[cfg]` at this call site either, and that is deliberate on rinch's
    // part rather than luck: the slot lives in `rinch-core`, which is compiled
    // on every target, so a desktop build registers a handler that is simply
    // never dispatched. See `rinch_core::events::set_configuration_change_handler`.
    //
    // **It runs on the main thread**, which is what makes the `Signal::set`
    // inside `reread` legal — the shell defers the callback out of Android's
    // `onConfigurationChanged` into its own loop body precisely so that it
    // does. A change that arrives while the app is backgrounded is deferred to
    // the first iteration after the surface comes back rather than dropped.
    //
    // And the other half of this feature is one word in
    // `android/AndroidManifest.xml`: without `uiMode` in `configChanges`,
    // Android does not deliver a configuration change at all — it destroys the
    // activity and rebuilds it *in the same process*, `android_main` is entered
    // a second time, and `rinch_android::init`'s `OnceLock` panics "already
    // initialized". The pid does not change, so it does not look like a crash;
    // it looks like the app quietly stopping. That comment lives in the
    // manifest, where somebody editing the attribute will see it.
    set_configuration_change_handler(move || system.reread());

    set_keyboard_interceptor(move |key: &KeyEventData| {
        if key.key != "Escape" {
            return false;
        }
        match nav.press_back() {
            BackPress::ClosedASheet(_) | BackPress::RanTheScreensBackAction => true,
            // A tab root with nothing in front of it. Android's own default
            // Back at the root of a task is `finish()`, so leaving is what a
            // phone already teaches; on Android this call is
            // `std::process::exit(0)` (rinch's `windows_stub`), and on the
            // desktop it closes the only window there is. Still `true`: the key
            // was ours, whatever the platform does with the request.
            BackPress::NothingLeftToDoButExit => {
                close_current_window();
                true
            }
        }
    });

    rsx! {
        div {
            // Every token lives here; nothing downstream hard-codes a hex.
            // The horizontal insets sit on the root so every screen inherits
            // them; a portrait phone reports 0 for both, a cutout in landscape
            // does not.
            style: {move || {
                // `safe_area` is read here rather than captured from a `let`
                // above, which is the whole of K53's edit in this closure: a
                // cutout inset that changes underneath the app now repaints
                // the root instead of being the number that was true at mount.
                let safe = system.safe_area.get();
                format!(
                    "{} height: 100vh; display: flex; flex-direction: column; \
                     position: relative; overflow: hidden; \
                     padding-left: {left}px; padding-right: {right}px;",
                    tokens(settings.dark_active(), settings.accent_resolved()),
                    left = safe.left,
                    right = safe.right,
                )
            }},

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
                    system.safe_area.get().top,
                    // Re-declaring the dark neutrals here and then reading
                    // `paper` back out of them keeps the rule that no hex is
                    // written outside `theme` — this strip sits *above* the
                    // viewer's own root, so it cannot inherit the override the
                    // viewer makes for everything inside it.
                    //
                    // `dark_chrome`, not `full_screen`: performance mode (F1)
                    // is the other full-screen route and it takes the app's own
                    // theme, so this strip stays cream above a cream `1o` —
                    // unless somebody has set Settings → Performance mode theme
                    // to Always dark, which is the second argument and the
                    // reason this is `derive::dark_chrome` rather than the
                    // method on `Route`. Get that pair wrong and a forced-dark
                    // gig wears a cream band under the phone's clock.
                    if derive::dark_chrome(nav.route.get(), settings.performance_theme.get()) {
                        format!("{DARK_NEUTRALS} background: var(--sla-paper);")
                    } else {
                        "background: var(--sla-paper);".to_string()
                    },
                )}
            }

            // Card J4: the library either opened or it did not, and a user
            // whose library is running in memory only has to be told — see
            // `derive::library_unavailable_note`'s own header for why this
            // has no button on it. It sits above every route rather than
            // inside one screen, because the fact it states is true of the
            // whole session and not of whichever tab happens to be open when
            // it becomes true; a banner that lived inside `Library` would say
            // nothing on the very first frame of a fresh install that landed
            // in `FirstRun` instead, which is exactly the launch this state
            // is most likely to occur on.
            for sentence in derive::library_unavailable_note(storage.is_persistent()) {
                div {
                    style: "padding: 10px 22px; display: flex; align-items: center; \
                            gap: 8px; flex-shrink: 0; background: var(--sla-fill); \
                            border-bottom: 1px solid var(--sla-hairline);",
                    span { style: "color: var(--sla-danger); display: flex; flex-shrink: 0;",
                        {icon(__scope, TablerIcon::AlertCircle, 15)}
                    }
                    span { style: {format!("{T_META} color: var(--sla-danger); font-weight: 600;")},
                        {sentence}
                    }
                }
            }

            match nav.route.get() {
                // An empty book has one job, and it is not this screen's
                // (card H3). `derive::first_run_active` is the pure half of
                // that decision — a testable `usize -> bool`, in the same
                // spirit as `derive::route_orphaned` just below it in that
                // file — and this `if`, not a signal anywhere, is the whole
                // of the wiring: it re-reads `SongsStore::songs` like any
                // other reactive condition, so the very tap that creates the
                // first song swaps `FirstRun` back out for `Library` with
                // nothing here having to ask it to.
                Route::Library => if derive::first_run_active(songs.songs.get().len()) {
                    FirstRun {}
                } else {
                    Library {}
                },
                Route::Setlists => Setlists {},
                Route::SongDetail(song_id) => SongDetail { id: {song_id} },
                Route::SetlistDetail(setlist_id) => SetlistDetail { id: {setlist_id} },
                // The real screen since card H1; its header is where the
                // rows `1q` draws that are *not* here are accounted for.
                Route::Settings => Settings {},
                // Search & filter (`1p`, G1). Carries nothing: the query it is
                // a screen for is `LibraryViewStore::query`, for the reasons
                // the variant's own doc comment gives.
                Route::Search => Search {},
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
                // Performance mode (`1o`, F1). Takes the setlist and nothing
                // else: where in it we are is `PlaybackStore::index`, which
                // the two entry points set with `start` before they navigate.
                Route::Performance(setlist_id) => Performance { setlist: {setlist_id} },
                // "Save this to…" — the screen a share lands on. It takes
                // nothing, because what was shared is `NavStore::pending_share`
                // and the song it will be filed under is the question the
                // screen is there to ask. On the desktop nothing ever navigates
                // here: there is no share sheet to be launched from, and
                // `crate::share::listen` — the one thing that sets this route —
                // is Android-only. The arm is not `#[cfg]`-ed all the same, so
                // that `Route` stays one enum with one set of match arms
                // everywhere it is read.
                Route::SaveShared => SaveShared {},
            }

            {bottom_nav(__scope)}

            // The six bottom sheets. All stay mounted for the life of the
            // app, parked below the fold, so that opening one has something to
            // slide. They sit last so they paint over the screen and the nav.
            //
            // Being out here, rather than inside the screen that opens each one,
            // is what lets a sheet cover a `Route::full_screen` screen: the
            // running order (F3) slides over the whole of performance mode,
            // scrim and all, and a sheet nested in that screen's own column
            // could only ever slide up inside the chart. See its header.
            SortGroupSheet {}
            FilterSheet {}
            AddToSetlistSheet {}
            SetlistPicker {}
            RunningOrderSheet {}
            TuningSheet {}
        }
    }
}

/// Two tabs, and only two. Settings is a gear in each tab's header.
///
/// The room below the labels is the gesture bar's inset on Android and the
/// design's own 22px on the desktop, floored at [`NAV_MIN_GAP`]. It used to be
/// a prop, computed once in `app()` from the safe area read at mount; since
/// K53 the inset is a signal and this reads it in its own style closure, for
/// the reason every other reading in this file moved the same way — a number
/// handed over as a prop is a number frozen at the moment it was handed over.
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
///
/// It dims for the same reason it does not hide, on the other route that
/// changes its look (card H3): `1r` draws the nav at `opacity: .4` while the
/// book is empty, and the wireframe's own words are "visible but dimmed" —
/// emphasis, not availability. `onclick` is untouched below; a dimmed-and-dead
/// nav would strand someone on an empty Songs tab with no way to reach
/// Settings at all, since the gear lives only in the Library and Setlists tab
/// headers and `1r` draws neither. Tapping `Setlists` from here still opens
/// on a screen with its own gear, dimmed nav and all.
#[component]
fn bottom_nav() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let system = use_store::<SystemStore>();

    rsx! {
        div {
            style: {move || {
                let route = nav.route.get();
                let dimmed = matches!(route, Route::Library)
                    && derive::first_run_active(songs.songs.get().len());
                let gap = system.safe_area.get().bottom.max(NAV_MIN_GAP);
                format!(
                    "display: {}; opacity: {}; border-top: 1px solid var(--sla-hairline); \
                     padding: 10px 0 {gap}px; flex-shrink: 0;",
                    if route.full_screen() { "none" } else { "flex" },
                    if dimmed { "0.4" } else { "1" },
                )
            }},
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

    /// The share filter, and the promise it makes to every chooser on the
    /// phone.
    ///
    /// An `<intent-filter>` is a claim Android has no way to check. It puts an
    /// app in a share sheet because the manifest said so, and the person who
    /// picks it finds out whether that was true afterwards — which makes the
    /// MIME list in that file a user-facing promise sitting in XML, exactly the
    /// shape of thing `the_android_manifest_asks_only_for_internet` above
    /// already watches, and exactly the shape of thing that gets edited by
    /// somebody adding "just one more type".
    ///
    /// So this asserts the whole of the claim rather than its presence:
    ///
    /// 1. The filter exists at all. Delete it and the app is simply not in the
    ///    share sheet — no error, no crash, nothing in `logcat`, just an app
    ///    that stopped being offered.
    /// 2. It carries `CATEGORY_DEFAULT`. An implicit intent is only ever
    ///    matched against filters that have it, so the same silent absence
    ///    follows from leaving out one line that looks like boilerplate.
    /// 3. The MIME types are `text/plain` and `application/pdf`, those two and
    ///    no others. `image/*` is the one that will be proposed — a photo of a
    ///    chart is a real thing a musician has — and this app has nowhere to
    ///    put an image: `AttachmentKind` is `Pdf | CapturedPage | Text`, and
    ///    `pdf::import` would refuse the bytes as `NotAPdf` after copying them
    ///    through Java. Advertising it would put SetListArray in the share
    ///    sheet of every photograph on the device and then fail in front of
    ///    whoever picked it.
    /// 4. The activity is `singleTask`. This is the half that fails *loudly*
    ///    and a long way from the edit: with the default launch mode, a share
    ///    from another app builds a second activity in that app's task, in the
    ///    same process, so `android_main` runs twice and `rinch_android::init`
    ///    panics `already initialized` — the identical failure the `uiMode`
    ///    note in that file describes, reached from a different direction.
    #[test]
    fn the_android_manifest_offers_itself_as_a_share_target() {
        let manifest = strip_xml_comments(include_str!("../android/AndroidManifest.xml"));

        assert!(
            manifest.contains(r#"android:launchMode="singleTask""#),
            "the share activity is no longer singleTask; a second share would build a second \
             activity in the sending app's task, in this process, and `rinch_android::init` \
             panics `already initialized` when android_main is entered twice"
        );

        // The `<intent-filter>` holding the SEND action, as text, so the three
        // assertions below are about *that* filter and not about the LAUNCHER
        // one above it.
        let filter = manifest
            .split("<intent-filter>")
            .find(|block| block.contains("android.intent.action.SEND"))
            .map(|block| block.split("</intent-filter>").next().unwrap_or(block))
            .expect(
                "AndroidManifest.xml no longer declares an ACTION_SEND filter, so this app is \
                 not in the share sheet at all — and nothing about that failure is visible \
                 anywhere except an app that stopped being offered",
            );

        assert!(
            filter.contains("android.intent.category.DEFAULT"),
            "the share filter has lost CATEGORY_DEFAULT; an implicit intent is only matched \
             against filters that carry it, so the app silently vanishes from the chooser"
        );

        let types: Vec<&str> = filter
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("<data"))
            .collect();
        assert_eq!(
            types,
            vec![
                r#"<data android:mimeType="text/plain" />"#,
                r#"<data android:mimeType="application/pdf" />"#,
            ],
            "the share filter advertises {} MIME types; this app can store exactly two kinds \
             of thing and promises exactly two: {types:?}",
            types.len()
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
    /// `docs/PLAN.md` makes that claim in its cross-cutting section. This is
    /// the half of it that a single crate's source tree can prove: rinch-http
    /// is the only HTTP client in the dependency tree this crate names, and
    /// `src/capture/fetch.rs` is the only file allowed to name it.
    ///
    /// What that catches is somebody adding a second fetch site — an update
    /// check, a font download, a crash reporter — because they would have to
    /// name the client to do it, and this fails when they do. What it does not
    /// catch is a transitive dependency opening its own socket, or this crate
    /// growing a *different* HTTP client without naming rinch-http to do it.
    /// That is the other half of card X2, and
    /// `no_network_capable_crate_enters_the_dependency_graph_outside_the_sanctioned_path`
    /// below is what closes it — it reads the whole resolved graph instead of
    /// this crate's own source. Treat a failure here as a question about the
    /// permission in `AndroidManifest.xml`: it is declared for one call site,
    /// and this is the test that counts them.
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

    /// Crate names, exactly as `Cargo.lock` spells them, that this test
    /// treats as evidence a *second* way of talking to a network has entered
    /// the tree: other HTTP clients, async runtimes whose reason to exist is
    /// scheduling network I/O, raw-socket and websocket crates, and
    /// alternative TLS backends to the one `ureq` already uses.
    ///
    /// This cannot be, and does not try to be, exhaustive — see the doc
    /// comment on `no_network_capable_crate_enters_the_dependency_graph_
    /// outside_the_sanctioned_path` for exactly what that means. It is a list
    /// of names someone reaching for a network call from Rust would plausibly
    /// `cargo add`, or would arrive transitively behind one of those. A crate
    /// that opens a socket under a name not on this list — a bespoke
    /// mio-alike, an obscure vendor SDK, anything hand-rolled over
    /// `std::net` — passes this test and is exactly the gap the comment
    /// below is honest about.
    const DENIED_NETWORK_CRATES: &[&str] = &[
        // Other HTTP clients. `reqwest` is the one that would actually get
        // reached for; the rest are its less common siblings.
        "reqwest",
        "hyper",
        "hyper-util",
        "hyper-rustls",
        "hyper-tls",
        "curl",
        "curl-sys",
        "isahc",
        "attohttpc",
        "surf",
        "minreq",
        "ehttp",
        "http-req",
        // Async runtimes whose selling point, unlike this app's need for one,
        // is concurrent network I/O.
        "tokio",
        "async-std",
        "smol",
        // Raw sockets and the lowest-level async I/O reactors, one layer
        // under an HTTP client.
        "mio",
        "socket2",
        "net2",
        // WebSocket.
        "tungstenite",
        "async-tungstenite",
        "tokio-tungstenite",
        "websocket",
        // HTTP/2 and HTTP/3 as their own transports rather than as part of
        // an HTTP client above.
        "h2",
        "h3",
        "quinn",
        "quinn-proto",
        // Alternative TLS backends. `ureq` here runs on `rustls`; a second
        // stack (a system TLS binding, or OpenSSL) arriving is a strong
        // signal that something else is about to make its own connection.
        "native-tls",
        "openssl",
        "openssl-sys",
        "boring",
        "boring-sys",
        "schannel",
        "security-framework",
    ];

    /// The names `Cargo.lock` gives every crate that `cargo tree -p ureq
    /// --edges normal --prefix none | sort -u` lists — `ureq`'s own
    /// transitive closure, the one network path this app allows. Most of
    /// these (`itoa`, `smallvec`, `syn`, the `icu_*` family used by `idna`'s
    /// Unicode normalisation, and so on) would never collide with
    /// `DENIED_NETWORK_CRATES` above; they are listed anyway so this constant
    /// is a complete, checkable answer to "what does the one network call in
    /// this app actually run on", not a hand-picked subset. Re-run that
    /// `cargo tree` command and diff it against this list before adding
    /// anything here — the point of naming these individually, rather than
    /// exempting `ureq` and trusting whatever comes with it, is that growing
    /// this list is the deliberate act the card is about.
    const ALLOWED_NETWORK_CRATES: &[&str] = &[
        "adler2",
        "base64",
        "bytes",
        "cfg-if",
        "cookie",
        "cookie_store",
        "crc32fast",
        "deranged",
        "displaydoc",
        "document-features",
        "equivalent",
        "flate2",
        "form_urlencoded",
        "getrandom",
        "hashbrown",
        "http",
        "httparse",
        "icu_collections",
        "icu_locale_core",
        "icu_normalizer",
        "icu_normalizer_data",
        "icu_properties",
        "icu_properties_data",
        "icu_provider",
        "idna",
        "idna_adapter",
        "indexmap",
        "itoa",
        "libc",
        "litemap",
        "litrs",
        "log",
        "miniz_oxide",
        "num-conv",
        "once_cell",
        "percent-encoding",
        "potential_utf",
        "powerfmt",
        "proc-macro2",
        "quote",
        "ring",
        "rustls",
        "rustls-pki-types",
        "rustls-webpki",
        "serde",
        "serde_core",
        "serde_derive",
        "simd-adler32",
        "smallvec",
        "stable_deref_trait",
        "subtle",
        "syn",
        "synstructure",
        "time",
        "time-core",
        "time-macros",
        "tinystr",
        "unicode-ident",
        "untrusted",
        "ureq",
        "ureq-proto",
        "url",
        "utf8_iter",
        "utf8-zero",
        "webpki-roots",
        "writeable",
        "yoke",
        "yoke-derive",
        "zerofrom",
        "zerofrom-derive",
        "zeroize",
        "zerotrie",
        "zerovec",
        "zerovec-derive",
        "zlib-rs",
    ];

    /// The desktop half of card X2, and the stronger of the two floors under
    /// "one network call exists in the entire app" — `the_http_client_is_
    /// named_in_exactly_one_file` above can only see this crate's own
    /// source; this reads `Cargo.lock`, which is the *whole* resolved
    /// dependency graph, including everything pulled in transitively that no
    /// line of this crate's own code ever names.
    ///
    /// **Why `Cargo.lock` and not `cargo metadata` or `cargo tree`.**
    /// `Cargo.lock` is already sitting on disk as the answer to "what is in
    /// the graph" — it needs no subprocess, no network, and no assumption
    /// that `cargo` is even the binary running this test suite under. It is
    /// also exactly where a new transport would show up: nobody adds a
    /// dependency by editing `Cargo.lock` directly, so a name appearing here
    /// that was not here before is, without exception, downstream of an edit
    /// to some `Cargo.toml`.
    ///
    /// **Why `Cargo.lock` can be `include_str!`'d safely even though it is
    /// untracked.** `.gitignore` excludes it — checked directly, `git
    /// ls-files Cargo.lock` prints nothing — which makes it reasonable to ask
    /// what this test does on a checkout that has never had one. The answer
    /// is: nothing, because there is no such checkout by the time a test
    /// binary exists. Cargo resolves dependencies and writes `Cargo.lock`
    /// before it invokes `rustc` on anything at all, on every build, lock
    /// file present or not going in; `include_str!` reads the file at that
    /// same compile step, after Cargo has already written it and before this
    /// test can run. So the two ways this could go wrong on a fresh
    /// checkout — the crate fails to compile, or the test silently reports
    /// nothing — collapse to the same thing a plain `assert!` cannot offer: a
    /// compile error naming the missing file, for the whole crate, not a test
    /// that quietly passed because it had nothing to check.
    ///
    /// **What this cannot catch**, stated plainly rather than implied by
    /// silence: a crate that opens a socket under a name this test does not
    /// recognise as network-shaped (`DENIED_NETWORK_CRATES` is a denylist,
    /// and a denylist is only ever as good as the list); a dependency that is
    /// declared but never actually called, which this test would still flag
    /// — a false positive in the safe direction; and anything that reaches
    /// the network without going through Cargo's dependency graph at all,
    /// such as a shell command shelling out to `curl`. `the_http_client_is_
    /// named_in_exactly_one_file` and this test together answer "does the
    /// tree build one transport for one call site" — neither one, nor both
    /// together, is a runtime guarantee that nothing dials out. Card K1's
    /// Android manifest test is the same kind of floor from a different
    /// angle: `AndroidManifest.xml` cannot get INTERNET back if the OS never
    /// granted it, no matter what the dependency graph does.
    #[test]
    fn no_network_capable_crate_enters_the_dependency_graph_outside_the_sanctioned_path() {
        let lock = include_str!("../Cargo.lock");
        let packages = parse_lockfile_packages(lock);

        // A parser this small breaking silently against a future `Cargo.lock`
        // format change would turn every case above into a false pass — the
        // one failure mode worse than the test never existing. This tree has
        // 619 packages in it at the time of writing; anything far short of
        // that means the parser stopped matching, not that the tree shrank by
        // hundreds of crates.
        assert!(
            packages.len() > 300,
            "parsed only {} packages out of Cargo.lock; that is far fewer than this tree \
             actually resolves, so the small hand-written reader above has probably stopped \
             matching Cargo.lock's format rather than the tree having shrunk — fix \
             parse_lockfile_packages before trusting this test's silence",
            packages.len()
        );

        for pkg in &packages {
            if !DENIED_NETWORK_CRATES.contains(&pkg.name.as_str()) {
                continue;
            }
            if ALLOWED_NETWORK_CRATES.contains(&pkg.name.as_str()) {
                continue;
            }

            let parents: Vec<&str> = packages
                .iter()
                .filter(|p| p.dependencies.iter().any(|dep| dep == &pkg.name))
                .map(|p| p.name.as_str())
                .collect();
            let brought_in = if parents.is_empty() {
                "Cargo.lock shows nothing else depending on it, so it was most likely added \
                 directly to a Cargo.toml in this workspace."
                    .to_string()
            } else {
                format!(
                    "Cargo.lock shows it required by: {}.",
                    parents.join(", ")
                )
            };

            panic!(
                "`{name}` v{version} is in Cargo.lock, and this test's denylist treats that name \
                 as a second way of talking to a network — a second HTTP client, a second async \
                 runtime, a raw socket, or a websocket. {brought_in}\n\
                 \n\
                 This app makes exactly one network request: offline webpage capture \
                 (src/capture/), fetching a page the user has explicitly pasted, over \
                 rinch-http, which runs on ureq on both platforms this app ships to (see \
                 Cargo.toml's comment on the rinch-http dependency). Settings' footer promises \
                 \"works with no connection. Nothing is uploaded anywhere\", and \
                 AndroidManifest.xml backs that with exactly one <uses-permission> line — \
                 INTERNET, for this one call site, checked by \
                 the_android_manifest_asks_only_for_internet above.\n\
                 \n\
                 A second transport is a decision, not a side effect of `cargo add` for \
                 something unrelated. If `{name}` genuinely belongs — ureq picked up a new TLS \
                 backend, a new DNS crate, whatever it is this time — the fix is to re-run \
                 `cargo tree -p ureq --edges normal --prefix none` and fold the new names into \
                 ALLOWED_NETWORK_CRATES above, on purpose, not to delete this test or add just \
                 enough to make it pass.",
                name = pkg.name,
                version = pkg.version,
            );
        }
    }

    /// One `[[package]]` table read out of `Cargo.lock`: its name, its
    /// version, and the names of the packages it depends on. Where more than
    /// one version of a crate is in the graph, Cargo spells a dependency on
    /// it as `"name version"` to disambiguate; the version suffix is dropped
    /// here because every use in this file only ever needs the name.
    struct LockedPackage {
        name: String,
        version: String,
        dependencies: Vec<String>,
    }

    /// A reader for exactly the shape Cargo writes `[[package]]` tables in —
    /// not a TOML parser, and not trying to be one. `Cargo.lock` is a
    /// generated file this crate never hand-edits, so the format it needs to
    /// survive is "whatever `cargo` itself writes", which has been stable
    /// long enough that this is a reasonable bet; the size sanity check in
    /// the test above is the guard against that bet going bad quietly.
    fn parse_lockfile_packages(lock: &str) -> Vec<LockedPackage> {
        let mut packages = Vec::new();
        let mut lines = lock.lines().peekable();

        while let Some(line) = lines.next() {
            if line.trim() != "[[package]]" {
                continue;
            }

            let mut name = String::new();
            let mut version = String::new();
            let mut dependencies = Vec::new();

            while let Some(&next) = lines.peek() {
                let trimmed = next.trim();
                if trimmed.starts_with("[[") {
                    break;
                }
                lines.next();

                if let Some(rest) = trimmed.strip_prefix("name = \"") {
                    name = rest.trim_end_matches('"').to_string();
                } else if let Some(rest) = trimmed.strip_prefix("version = \"") {
                    version = rest.trim_end_matches('"').to_string();
                } else if trimmed == "dependencies = [" {
                    for dep_line in lines.by_ref() {
                        let dep_trimmed = dep_line.trim();
                        if dep_trimmed == "]" {
                            break;
                        }
                        let dep = dep_trimmed.trim_matches(',').trim_matches('"');
                        let dep_name = dep.split_whitespace().next().unwrap_or(dep);
                        dependencies.push(dep_name.to_string());
                    }
                }
            }

            if !name.is_empty() {
                packages.push(LockedPackage {
                    name,
                    version,
                    dependencies,
                });
            }
        }

        packages
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
