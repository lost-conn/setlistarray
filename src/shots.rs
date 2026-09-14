//! The store-listing tour: a fixed list of screens, walked once, announced to
//! logcat so something outside the app can photograph each one.
//!
//! Behind the `shots` cargo feature, which nothing ships with. `build-apk.sh
//! --shots` builds the APK this drives, `scripts/store-shots.sh` runs it on a
//! headless emulator and grabs the frames, and card S2 turns the frames into
//! the eight pictures Google Play wants beside the description.
//!
//! ## Why the list is Rust and not a JSON file
//!
//! Because a shot is a [`Route`] and a route is a typed thing. `Route::
//! ViewAttachment { song, attachment }` carries two ids that only exist once
//! the demo library has been written through the database and remapped
//! (`seed::install` mints its own), so a shot is never "go to screen 7" — it
//! is "find the song called Wildwood Flower, take the chart it calls primary,
//! and open the viewer on the pair". That is a lookup against live stores, and
//! the moment it is written in a data file it is a lookup nothing type-checks:
//! a route renamed, a variant that grows a field, a store method that changes
//! shape, and the file is silently wrong until somebody runs the emulator and
//! looks at eight pictures of the library screen.
//!
//! Written here it is none of those things. A renamed route fails the build.
//!
//! The *captions* are a different question with a different answer, and card
//! S2 gives it: those go in `store/shots.json`, keyed by the ids below, because
//! a caption is prose somebody edits without touching Rust. Which is the whole
//! reason the ids in [`SHOTS`] are stable, unique and dull — they are a join
//! key between this file and that one, and a later test asserts the two sets
//! match. Renaming one here is a breaking change to a file in another language.
//!
//! ## Why the app dwells instead of being told when to fire
//!
//! There is no channel. Rinch's debug server — the thing `scripts/gesture-
//! probe.py` drives on the desktop — is `#[cfg(feature = "debug")]` in
//! `shell/rinch_runtime.rs` and is not referenced by `shell/android_runtime.rs`
//! at all, so on a phone there is nothing listening and nothing to ask. The
//! app could grow one, and that was considered for this card and rejected: an
//! IPC server compiled into a build whose whole job is to be photographed is a
//! great deal of machinery, and a listening socket, to save a few seconds.
//!
//! So the app sets a screen up, waits, says `SLA_SHOT_READY <id>` where logcat
//! will carry it, and waits again while the host grabs the framebuffer. That
//! is a timing assumption, and this file is where it is written down honestly
//! rather than buried.
//!
//! **This is not card K39's mistake wearing a different hat**, and the
//! difference is worth being exact about, because K39 is why `CLAUDE.md`
//! forbids timing a frame from inside the app. What K39 measured in-process
//! was *whether the app was fast*, and an in-process timer cannot see the
//! compositor, so it produced a number that was wrong in a direction nobody
//! could check and three cards were written on top of it. Nothing here
//! measures anything. The dwell is a bet about how long a screen takes to
//! settle, and if the bet is bad the result is a picture of a half-drawn
//! screen — in a PNG a human is about to look at before it goes on a store
//! page. A wrong guess here is visible; K39's was not. That is the whole of
//! why a dwell is allowed to be the mechanism.
//!
//! Which is also why it is generous rather than tight. [`SETTLE_MS`] is six
//! seconds for a reason that is not "six felt safe" — see its own note.

use rinch::prelude::*;

use crate::model::{SetlistId, SongId};
use crate::store::{
    AccentChoice, LibraryViewStore, NavStore, PlaybackStore, Route, SetlistsStore, SettingsStore,
    SongsStore, Tab, ThemeChoice,
};

/// The line the host greps logcat for, one per shot, id appended.
pub const READY: &str = "SLA_SHOT_READY";

/// The line that says the tour is over and the last frame has been sat on long
/// enough to be grabbed. Without it `scripts/store-shots.sh` could not tell
/// "finished" from "died after the last shot".
pub const DONE: &str = "SLA_SHOTS_DONE";

/// What a shot could not find, when the seed and this file have drifted apart.
/// Not a panic: a process that aborts leaves the host script guessing at a
/// dead pid, and the one thing worth knowing — *which* shot, and what it went
/// looking for — is exactly what a report can carry and a SIGABRT cannot.
pub const BROKEN: &str = "SLA_SHOT_BROKEN";

/// How long a screen is given to lay out and paint before its id is announced.
///
/// Six seconds is not a shrug. It is chosen to sit past
/// `screens::attachment_viewer`'s `CHROME_LINGER_MS`, which is four: the
/// viewer's top and bottom bars fade themselves away four seconds after it
/// opens, so any settle shorter than that photographs the viewer in whichever
/// of its two states the host happened to catch it in — bars on one run, bars
/// off the next, on the same build. Going *past* the linger makes that
/// deterministic, and the state it lands on is the better of the two for the
/// job: a chord chart edge to edge with nothing on top of it.
///
/// Every other shot is over-settled by several seconds and none of them mind.
/// These are static screens on a phone-shaped surface; there is no animation
/// still running at second six that was not running at second two. Spending
/// the seconds uniformly is worth more than a per-shot number would be,
/// because a per-shot number is a per-shot thing to get wrong.
const SETTLE_MS: u32 = 6_000;

/// How long the shot is held after its id goes out, for the host to see the
/// line, run `adb exec-out screencap` and get the bytes back.
///
/// Four seconds against a capture that takes well under one on this emulator.
/// The margin is for the other side: a logcat follower is a pipe through a
/// second process, and a laptop compiling something else in another terminal
/// is the ordinary case rather than the unlucky one.
const DWELL_MS: u32 = 4_000;

/// Before the first shot, on top of everything else: the surface has to exist,
/// the four bundled faces have to be registered and measured against, and the
/// demo library has to be written through the database and read back. On a
/// swiftshader emulator that first frame is by some distance the slowest one
/// of the run.
const WARM_UP_MS: u32 = 8_000;

/// The store handles a shot's set-up is allowed to touch.
///
/// A struct rather than six arguments because every closure in [`SHOTS`] is an
/// `fn` pointer and they all have to have the same signature; a struct also
/// means adding a seventh store later is one line here rather than an edit to
/// every shot. It is `Copy` for the same reason the stores themselves are —
/// each is a handful of `Signal` handles, and this is a handful of those.
#[derive(Clone, Copy)]
pub struct Stage {
    pub nav: NavStore,
    pub playback: PlaybackStore,
    pub songs: SongsStore,
    pub setlists: SetlistsStore,
    pub view: LibraryViewStore,
    pub settings: SettingsStore,
}

impl Stage {
    /// The seeded song with this title, by title, because the id it had in
    /// `seed.rs` is not the id it has here — `seed::install` writes every
    /// record through the repository and keeps whatever the database mints.
    /// The title is the one thing that survives the round trip unchanged, so
    /// the title is what a shot names.
    fn song(self, title: &str) -> Option<SongId> {
        self.songs.songs.get().iter().find(|s| s.title == title).map(|s| s.id)
    }

    /// The same, for a set, and for the same reason.
    fn setlist(self, name: &str) -> Option<SetlistId> {
        self.setlists.setlists.get().iter().find(|s| s.name == name).map(|s| s.id)
    }

    /// A song and the chart it calls primary, which is the pair
    /// [`Route::ViewAttachment`] is built out of. Both or neither: a song with
    /// no primary attachment cannot open a viewer, and a viewer opened on a
    /// song it does not belong to prints the wrong name under the file's.
    fn primary_chart(self, title: &str) -> Option<(SongId, crate::model::AttachmentId)> {
        let song = self.songs.songs.get().iter().find(|s| s.title == title)?.clone();
        Some((song.id, song.primary_attachment?))
    }
}

/// One frame of the tour.
///
/// `set_up` is a plain `fn` pointer rather than a boxed closure so the table
/// below can be a `const`: nothing here captures anything, everything it needs
/// arrives in the [`Stage`], and a const table is one less thing built at
/// runtime on a path whose entire job is to be predictable.
pub struct Shot {
    /// The join key with `store/shots.json` (card S2). Stable, unique, dull.
    pub id: &'static str,
    /// Put the stores where this shot wants them, or say what was missing.
    pub set_up: fn(Stage) -> Result<(), &'static str>,
}

/// The tour, in the order it is walked.
///
/// Chosen to be the screens that answer "what is this app for?" rather than
/// the screens that exist. Settings, the add/edit form, the importer and the
/// capture screen are all real work and none of them is a reason to install
/// anything, so none of them is here.
///
/// The order is a sentence: here is your book, here is one song in it, here is
/// the chart with nothing on top of it, here is finding a line *inside* a
/// chart, here are your sets, here is one of them, here is a gig.
pub const SHOTS: &[Shot] = &[
    // The book. The seeded library is grouped by confidence and sorted by
    // artist, which is the resting state a fresh install is in, so this is
    // also the only shot that needs nothing set up beyond the tab.
    Shot {
        id: "library",
        set_up: |stage| {
            stage.nav.select_tab(Tab::Songs);
            Ok(())
        },
    },
    // One song, everything known about it: key, capo, tempo, confidence, the
    // sets it is in, and the chart card. Wildwood Flower because it is one of
    // the three that carries real text — see `seed.rs` for why those three.
    Shot {
        id: "song-detail",
        set_up: |stage| {
            let song = stage.song("Wildwood Flower").ok_or("no song titled Wildwood Flower")?;
            stage.nav.select_tab(Tab::Songs);
            stage.nav.go(Route::SongDetail(song));
            Ok(())
        },
    },
    // The chart, full screen, in the viewer's own dark chrome. By the time
    // this is photographed the chrome has faded — deliberately, and
    // `SETTLE_MS` is where that is argued.
    Shot {
        id: "chart-viewer",
        set_up: |stage| {
            let (song, attachment) = stage
                .primary_chart("Wildwood Flower")
                .ok_or("Wildwood Flower has no primary chart")?;
            stage.nav.go(Route::ViewAttachment { song, attachment });
            Ok(())
        },
    },
    // The thing no paper folder does: a word found inside a chart you typed.
    // "fair" hits one song by title (Scarborough Fair) and two charts by their
    // text — Scarborough Fair's own first line and Wildwood Flower's "roses so
    // fair" — so the screen shows both halves of what search is for rather
    // than a list of titles, which is the half people assume.
    Shot {
        id: "search-in-chart",
        set_up: |stage| {
            stage.view.query.set("fair".into());
            stage.nav.select_tab(Tab::Songs);
            stage.nav.go(Route::Search);
            Ok(())
        },
    },
    // The other tab, and the other half of the app's promise.
    Shot {
        id: "setlists",
        set_up: |stage| {
            stage.nav.select_tab(Tab::Setlists);
            Ok(())
        },
    },
    // One set, in running order, with its cumulative time.
    Shot {
        id: "setlist-detail",
        set_up: |stage| {
            let setlist = stage.setlist("Campfire set").ok_or("no setlist named Campfire set")?;
            stage.nav.select_tab(Tab::Setlists);
            stage.nav.go(Route::SetlistDetail(setlist));
            Ok(())
        },
    },
    // A gig, mid-set. `Campfire set` because its first three songs are the
    // three with charts behind them (see `seed::setlists`), and the second
    // rather than the first because "2 / 5" with a progress strip a third of
    // the way along says *what this screen is* in a way "1 / 5" does not.
    Shot {
        id: "performance",
        set_up: |stage| {
            let setlist = stage.setlist("Campfire set").ok_or("no setlist named Campfire set")?;
            stage.playback.start(setlist, true);
            stage.playback.index.set(1);
            stage.nav.go(Route::Performance(setlist));
            Ok(())
        },
    },
];

/// Start the tour. Called once, from the Android entry point, and only in a
/// build that asked for this feature.
pub fn run(stage: Stage) {
    pin_the_look(stage);
    // Armed from `crate::app`'s body, which is a render — and `set_timeout`
    // cancels a timeout armed during a render if the component that armed it
    // unmounts first. The component that armed this one is the root, which
    // unmounts when the process ends, so there is nothing here to lose.
    set_timeout(WARM_UP_MS, move || advance(stage, 0));
}

/// Take the emulator's opinion about colour out of the pictures.
///
/// `AccentChoice::FromSystem` is what a fresh install starts on, and on a real
/// phone it is the right default — it derives the app's accent from the
/// system's Material You palette, or failing that from the primary colour of
/// the wallpaper (`theme::DerivedAccent`, `platform::wallpaper_primary`). On
/// an emulator that means the *stock AVD wallpaper* picks the colour of every
/// screenshot on the store page, and a different system image would pick a
/// different one, silently, between one run of this tour and the next.
///
/// So both are pinned: `Named(0)` is Rust, the accent the handoff authored and
/// the one `scripts/screenshot.sh` has photographed on the desktop since card
/// A3, and `Light` is the cream paper this app's identity is built around.
/// Pinning them here rather than in the seed is deliberate — this is a fact
/// about *photographing* the app, not about the demo library, and a user who
/// installs the real build still gets their own wallpaper's colour.
fn pin_the_look(stage: Stage) {
    stage.settings.theme.set(ThemeChoice::Light);
    stage.settings.accent.set(AccentChoice::Named(0));
}

/// Set up shot `index`, let it paint, announce it, hold it, move on.
///
/// Recursive through [`set_timeout`] rather than a loop, because there is no
/// loop available: the app has one thread and it is the one drawing, so the
/// only way to wait is to hand the rest of the tour to the timer and return.
fn advance(stage: Stage, index: usize) {
    let Some(shot) = SHOTS.get(index) else {
        announce(&format!("{DONE} {} shot(s)", SHOTS.len()));
        return;
    };

    if let Err(missing) = (shot.set_up)(stage) {
        // Stop here rather than skipping on. The tour is a sequence and a gap
        // in it is a store page with a hole; the host script is waiting for
        // this id, will not get it, and will time out and print the logcat
        // this line is sitting in — which names the shot and what it went
        // looking for, and is the whole of what somebody needs to fix it.
        announce(&format!("{BROKEN} {}: {missing}", shot.id));
        return;
    }

    set_timeout(SETTLE_MS, move || {
        announce(&format!("{READY} {}", shot.id));
        set_timeout(DWELL_MS, move || advance(stage, index + 1));
    });
}

/// Say something where the host can hear it.
///
/// A seam of the same shape as the four `crate`-level ones (`platform`,
/// `db::DataDir`, `picker`, `keep_awake`): on Android the only channel out of
/// a process is the logger rinch's shell installs — `println!` goes nowhere on
/// a phone, which is what `Cargo.toml`'s Android-only `log` dependency already
/// says out loud — and on a desktop build there is no logcat and stdout is
/// right there. The desktop arm exists so this file compiles and can be read
/// on a laptop, not because anyone photographs a window this way.
#[cfg(target_os = "android")]
fn announce(line: &str) {
    log::info!("{line}");
}

#[cfg(not(target_os = "android"))]
fn announce(line: &str) {
    println!("{line}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The ids are a join key with a file in another language (card S2's
    /// `store/shots.json`), so two shots sharing one would silently give one
    /// caption to two pictures and none to the third.
    #[test]
    fn every_shot_id_is_unique() {
        let mut seen: HashSet<&str> = HashSet::new();
        for shot in SHOTS {
            assert!(seen.insert(shot.id), "two shots share the id {:?}", shot.id);
        }
    }

    /// An id ends up in a filename (`<out>/<id>.png`) and in a JSON key, so
    /// the alphabet is the intersection of what a shell, a filesystem and a
    /// person reading a diff all handle without thinking about it.
    #[test]
    fn every_shot_id_is_a_safe_filename() {
        for shot in SHOTS {
            assert!(!shot.id.is_empty(), "a shot has no id");
            assert!(
                shot.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{:?} is not lowercase-ascii-and-hyphens",
                shot.id
            );
        }
    }

    /// The tour has to be worth taking. A list that has quietly lost its
    /// contents still walks, still announces nothing, and still prints
    /// `SLA_SHOTS_DONE` — so the host script would report a clean run of zero
    /// pictures.
    #[test]
    fn the_tour_is_not_empty() {
        assert!(SHOTS.len() >= 5, "only {} shot(s); the listing wants more", SHOTS.len());
    }
}
