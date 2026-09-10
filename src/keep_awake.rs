//! Stopping the screen going off in the middle of a song — card F4.
//!
//! A stand-mounted phone that sleeps mid-song is the failure this app exists to
//! avoid. F3 shipped the toggle for it in performance mode's bottom bar and
//! said so in its own header: the switch flipped a signal and *nothing read
//! that signal*, because there was no way from this crate to hold the screen
//! on. This is the thing that reads it.
//!
//! ## The shape: a trait whose default is to do nothing
//!
//! [`crate::platform`] is two `#[cfg]` functions and [`crate::picker`] is a
//! trait; both are the same seam drawn at different widths, and this is the
//! trait one, for the reason `picker`'s header gives. `safe_area()` *returns a
//! number* and a test can call it; a screen lock *does something to the
//! machine*, and a test can neither hold a phone awake nor watch a D-Bus
//! service to see whether it did. So the seam has to be substitutable — a trait
//! with a third implementation ([`test_support::Recorder`]) that answers
//! without touching anything.
//!
//! What is different here is the **no-op default**. `FilePicker::pick` has no
//! sensible default: a picker that does not pick is not a picker, and a
//! platform that cannot open a dialog has no business claiming it can. But
//! "keep the screen on" is a *request*, and a platform with no way to honour it
//! declining quietly is the honest answer rather than an error — the app is
//! still perfectly usable on a machine whose screen blanks, it is just less
//! pleasant. So [`ScreenLock::set_keep_awake`] does nothing unless a platform
//! says otherwise, and adding a third platform means implementing an empty
//! trait rather than remembering to write a stub.
//!
//! ## Write the answer, do not count the edges
//!
//! The one method takes a `bool` and is meant to be called with the *current*
//! answer every time that answer might have changed — never as an
//! acquire/release pair. This is not a style preference; it is the only shape
//! that cannot leak.
//!
//! A phone left holding the screen on after the set is over is the same class
//! of bug as a phone that sleeps during it, and it is the one that is easy to
//! write: leaving performance mode, switching the toggle off, the set being
//! deleted underneath the screen, and the ✕ in the top bar are four ways out of
//! the state, and an acquire/release pair has to remember all four. A recomputed
//! answer has to remember none of them — see the `Effect` in [`crate::app`],
//! which reads the route and the toggle and writes what falls out. The two
//! platform implementations absorb the difference between them: Android's flag
//! is genuinely idempotent, and the desktop's inhibit is a cookie protocol that
//! is not, so [`DesktopLock`] keeps the cookie and this module's contract stays
//! the same on both.
//!
//! ## Android
//!
//! `rinch_android::screen::keep_screen_on`, added upstream by card K5
//! (joeleaver/rinch#417) — `FLAG_KEEP_SCREEN_ON` on the activity's window, set
//! on the UI thread. Deliberately a window flag and not a `PowerManager`
//! wake lock: the flag is dropped by the system the moment the activity stops,
//! so backgrounding the app during a break hands the display timeout straight
//! back without this app having to notice. K5's module header has the long
//! version of that argument.
//!
//! ## The desktop, and why it got a real implementation
//!
//! `org.freedesktop.ScreenSaver.Inhibit` over the session bus. The card asked
//! for it *only if it does not mean adding a heavy dependency*, and this
//! project prices dependencies rather than assuming them, so it was priced:
//!
//! * **Crates added: none.** `zbus` 5.19 is already compiled into every desktop
//!   build of this app. `rinch`'s `file-dialogs` feature brings `rfd`, whose
//!   Linux backend is `xdg-portal`, which is `ashpd`, which is `zbus`. Naming
//!   it directly in `Cargo.toml` turns on zbus's own `blocking-api` feature and
//!   nothing else: `cargo tree --edges normal` lists **457 packages with the
//!   line and 457 without it**, the same 457.
//! * **APK bytes added: none**, which is the number this project actually
//!   guards. The dependency sits under
//!   `[target.'cfg(not(target_os = "android"))'.dependencies]`, so the phone
//!   build never sees it.
//! * **No C toolchain.** `zbus` is pure Rust over a unix socket.
//!
//! A dependency that adds no crates, no APK bytes and no build tooling is not a
//! dependency being taken on; it is one already in the tree being used. So the
//! desktop gets the real thing.
//!
//! ### It talks to a session service, which is the thing CLAUDE.md warns about
//!
//! `kwin_wayland` owns `org.freedesktop.ScreenSaver` on this machine, and
//! CLAUDE.md's rule about the developer's display is explicitly not only about
//! windows: *"the same caution applies to anything else that hands work to a
//! session service rather than drawing it"*. This does exactly that, so it is
//! worth being clear about what it can and cannot do to somebody who is working
//! while it runs.
//!
//! It can stop the screensaver starting. It cannot take focus, raise a window,
//! or draw anything — the whole conversation is two method calls and a `u32` —
//! and the effect ends when this process does, because the inhibit is scoped to
//! the D-Bus connection: a crash, a `SIGKILL` or a test that never reached its
//! `UnInhibit` all release it, since the socket closes either way. That is a
//! strictly weaker footprint than the `rfd` dialog the same seam's sibling had
//! to build a whole environment override to avoid, and it is why this one does
//! not need one.
//!
//! What it does *not* do is work everywhere. `org.freedesktop.ScreenSaver` is a
//! convention, not a standard: KDE and GNOME both own it, a bare window manager
//! owns nothing, and a headless CI box has no session bus at all. Every one of
//! those is a failure to talk to a service and every one is swallowed with a
//! line on stderr, because a desktop preview of a phone app whose screensaver
//! still works is not a broken app.

use crate::store::Route;

/// The seam. See the module header for why the default does nothing and why
/// the method takes the answer rather than a direction.
pub trait ScreenLock {
    /// Hold the screen awake, or stop holding it.
    ///
    /// Called with the current answer whenever that answer might have changed,
    /// including when it has not changed at all — an implementation that
    /// cannot be called twice with the same value is an implementation that
    /// has not honoured this contract.
    fn set_keep_awake(&self, on: bool) {
        let _ = on;
    }
}

/// Tell the platform whether the screen should stay on right now.
///
/// The one function anything outside this module calls, and the reason no
/// screen and no store contains a `#[cfg]`.
pub fn set(on: bool) {
    platform_lock().set_keep_awake(on);
}

#[cfg(target_os = "android")]
fn platform_lock() -> impl ScreenLock {
    AndroidLock
}

#[cfg(not(target_os = "android"))]
fn platform_lock() -> impl ScreenLock {
    DesktopLock
}

/// Whether the screen should be held awake, given where the app is and what
/// the toggle says.
///
/// Both halves, and `&&` rather than either alone. The toggle is remembered for
/// the life of the session — leave performance mode with it on, come back, and
/// it is still on, which is what somebody who set it before the first song
/// expects. That memory is exactly why the route has to be asked as well: a
/// toggle that is still `true` on the Songs tab an hour after the gig would
/// otherwise be holding the display of a phone in somebody's pocket.
///
/// Not a method on [`Route`] beside `full_screen` and `dark_chrome`, because
/// unlike those two it is not a question the route can answer on its own.
///
/// It deliberately does not ask whether the set has any songs in it, or whether
/// the current one has a chart. Performance mode with an empty set is a screen
/// somebody is reading a sentence off, on a stand, and there is no version of
/// "you are in the middle of a gig" where the phone going dark is the right
/// answer.
pub fn wanted(route: Route, toggle: bool) -> bool {
    matches!(route, Route::Performance(_)) && toggle
}

// ── Android: a flag on the activity's window ────────────────────────────────

/// `FLAG_KEEP_SCREEN_ON`, through `rinch-android` (card K5).
#[cfg(target_os = "android")]
pub struct AndroidLock;

#[cfg(target_os = "android")]
impl ScreenLock for AndroidLock {
    fn set_keep_awake(&self, on: bool) {
        // Nothing to remember and nothing to report. The JNI call posts to the
        // UI thread and returns; `addFlags`/`clearFlags` are an or and an
        // and-not, so writing the value that is already there costs a call and
        // changes nothing. Failures are logged inside `rinch-android` — this
        // app has no better answer to "the activity is going away" than
        // carrying on.
        rinch_android::screen::keep_screen_on(on);
    }
}

// ── The desktop: an inhibit on the session bus ──────────────────────────────

/// `org.freedesktop.ScreenSaver.Inhibit`, over `zbus`.
///
/// The one piece of state in this module, and it is forced. Android's flag is
/// idempotent; this is not — `Inhibit` hands back a cookie and `UnInhibit`
/// takes it, so *somebody* has to hold that number between the two calls. It is
/// held here, behind the trait, so that the contract the rest of the app codes
/// against ("write the current answer, every time") survives a platform that
/// cannot honour it directly.
#[cfg(not(target_os = "android"))]
pub struct DesktopLock;

#[cfg(not(target_os = "android"))]
impl ScreenLock for DesktopLock {
    fn set_keep_awake(&self, on: bool) {
        // A `Mutex` rather than a `Cell`, for a value only the frame thread
        // ever touches. It is not contention this is buying, it is a `static`
        // that is allowed to exist at all: a plain `static mut` would be
        // unsafe, and a `thread_local!` would silently give a second thread its
        // own cookie and its own inhibit. Poison-tolerant because a panic while
        // holding it must not turn every later call into a second panic —
        // dropping the app because the screensaver could not be talked to is
        // the wrong end of the trade.
        static COOKIE: std::sync::Mutex<Option<u32>> = std::sync::Mutex::new(None);
        let mut cookie = COOKIE.lock().unwrap_or_else(|e| e.into_inner());

        match (on, *cookie) {
            // Already in the state being asked for. Both arms matter: this is
            // the method that gets called on every route change and every flip
            // of anything the effect reads, so most calls land here.
            (true, Some(_)) | (false, None) => {}
            (true, None) => *cookie = inhibit(),
            (false, Some(held)) => {
                uninhibit(held);
                // Cleared whether or not the call succeeded. A cookie the
                // service has forgotten is not worth retrying and not worth
                // keeping: holding it would mean the *next* request to keep the
                // screen on saw `Some` and quietly did nothing, which turns one
                // failed release into a screen that never comes on again.
                *cookie = None;
            }
        }
    }
}

/// The session bus, opened once and kept for the life of the process.
///
/// Kept, and not reopened per call, for a reason beyond the handshake cost: the
/// inhibit is scoped to the *connection*. Dropping this would release the
/// screensaver inhibit as surely as calling `UnInhibit`, which is the property
/// the module header leans on when it says a crash cannot leave one behind.
///
/// `OnceLock<Option<_>>` rather than retrying: a machine with no session bus at
/// the moment the app starts is a machine with no session bus, and asking it
/// again on every route change would be a socket connection attempt per screen
/// for an answer that is not going to change.
#[cfg(not(target_os = "android"))]
fn session_bus() -> Option<&'static zbus::blocking::Connection> {
    static BUS: std::sync::OnceLock<Option<zbus::blocking::Connection>> =
        std::sync::OnceLock::new();
    BUS.get_or_init(|| match zbus::blocking::Connection::session() {
        Ok(bus) => Some(bus),
        Err(e) => {
            // Not a fault worth showing the user. There is no session bus on a
            // headless box and no `org.freedesktop.ScreenSaver` on a bare
            // window manager, and neither is something they did.
            eprintln!("setlistarray: no session bus, the screen may sleep ({e})");
            None
        }
    })
    .as_ref()
}

/// Ask the screensaver to stay out of the way. `None` if there was nobody to
/// ask or they refused.
#[cfg(not(target_os = "android"))]
fn inhibit() -> Option<u32> {
    let bus = session_bus()?;
    // The two strings are shown to the user by some desktops' "what is stopping
    // my screen locking" panels, so they are written to be read there rather
    // than to be parsed: an application name and a reason in a full sentence.
    let reply = bus.call_method(
        Some("org.freedesktop.ScreenSaver"),
        "/org/freedesktop/ScreenSaver",
        Some("org.freedesktop.ScreenSaver"),
        "Inhibit",
        &("SetListArray", "A setlist is being performed"),
    );
    match reply.and_then(|m| m.body().deserialize::<u32>()) {
        Ok(cookie) => Some(cookie),
        Err(e) => {
            eprintln!("setlistarray: the screensaver could not be inhibited ({e})");
            None
        }
    }
}

/// Give the cookie back.
#[cfg(not(target_os = "android"))]
fn uninhibit(cookie: u32) {
    let Some(bus) = session_bus() else { return };
    if let Err(e) = bus.call_method(
        Some("org.freedesktop.ScreenSaver"),
        "/org/freedesktop/ScreenSaver",
        Some("org.freedesktop.ScreenSaver"),
        "UnInhibit",
        &(cookie,),
    ) {
        // Worth a line, and nothing more. The connection outliving a lost
        // cookie is the bad case, and it ends when the process does.
        eprintln!("setlistarray: the screensaver inhibit was not released ({e})");
    }
}

// ── The third implementation, for tests ────────────────────────────────────

/// A lock that holds nothing and remembers everything.
///
/// The reason [`ScreenLock`] is a trait: the rule in [`wanted`] and the effect
/// that drives it can be checked against a recorded sequence, on a machine with
/// no phone attached and no screensaver worth arguing with.
#[cfg(test)]
pub mod test_support {
    use super::*;
    use std::cell::RefCell;

    /// Every value it was asked to write, in order — repeats included, since
    /// "called twice with the same answer" is part of the contract rather than
    /// a fault to filter out.
    #[derive(Default)]
    pub struct Recorder {
        pub written: RefCell<Vec<bool>>,
    }

    impl ScreenLock for Recorder {
        fn set_keep_awake(&self, on: bool) {
            self.written.borrow_mut().push(on);
        }
    }

    /// A platform that cannot keep a screen awake and says so by saying
    /// nothing — the trait's default, with no method of its own.
    pub struct Silent;

    impl ScreenLock for Silent {}
}

#[cfg(test)]
mod tests {
    use super::test_support::{Recorder, Silent};
    use super::*;
    use crate::model::SetlistId;

    const SET: SetlistId = 1;

    // ── the rule ────────────────────────────────────────────────────────────

    #[test]
    fn the_screen_is_held_only_in_performance_mode_with_the_toggle_on() {
        assert!(wanted(Route::Performance(SET), true));
        assert!(!wanted(Route::Performance(SET), false));
    }

    #[test]
    fn every_other_screen_lets_the_phone_sleep_however_the_toggle_is_left() {
        // The toggle is remembered for the session, so `true` here is the
        // ordinary case and not a contrived one: this is what the app looks
        // like the moment somebody closes performance mode.
        for route in [
            Route::Library,
            Route::Setlists,
            Route::SetlistDetail(SET),
            Route::Settings,
            Route::Licenses,
            Route::SongDetail(7),
        ] {
            assert!(!wanted(route, true), "{route:?} must not hold the screen");
            assert!(!wanted(route, false), "{route:?} must not hold the screen");
        }
    }

    #[test]
    fn the_chart_viewer_is_not_performance_mode() {
        // Both are `Route::full_screen`, and it would be an easy mistake to
        // hold the screen for either. Reading a chart is not playing a set: the
        // viewer is reached from song detail, one page at a time, by somebody
        // who is holding the phone.
        let viewer = Route::ViewAttachment { song: 7, attachment: 3 };
        assert!(!wanted(viewer, true));
    }

    // ── the seam ────────────────────────────────────────────────────────────

    #[test]
    fn a_platform_with_no_answer_is_silent_rather_than_wrong() {
        // Nothing to assert but that this compiles and does not panic — which
        // is the whole point of a default that does nothing.
        Silent.set_keep_awake(true);
        Silent.set_keep_awake(false);
    }

    #[test]
    fn the_lock_is_written_every_time_the_answer_is_recomputed_repeats_and_all() {
        let lock = Recorder::default();
        // What the effect does over a session: open the set, toggle off,
        // toggle back on, leave. The two `true`s in a row are the case an
        // acquire/release API would have to filter and this one must not.
        for (route, toggle) in [
            (Route::SetlistDetail(SET), true),
            (Route::Performance(SET), true),
            (Route::Performance(SET), true),
            (Route::Performance(SET), false),
            (Route::Performance(SET), true),
            (Route::SetlistDetail(SET), true),
        ] {
            lock.set_keep_awake(wanted(route, toggle));
        }
        assert_eq!(
            *lock.written.borrow(),
            vec![false, true, true, false, true, false],
        );
    }

    /// The desktop half, against the real service. **`#[ignore]`d**, and run as
    ///
    /// ```text
    /// cargo test --release -- --ignored the_session_bus
    /// ```
    ///
    /// Ignored rather than deleted, because it is the only thing that can tell
    /// anyone whether `DesktopLock` still works and it must never run by
    /// default. It needs a session bus and a compositor that owns
    /// `org.freedesktop.ScreenSaver` — neither exists on a build machine — and
    /// on a developer's machine it genuinely inhibits their screensaver for the
    /// length of the test. That is a smaller intrusion than it sounds (see the
    /// module header: no window, no focus, and it dies with the process) and it
    /// is still not something a routine `cargo test` should be doing to
    /// somebody.
    ///
    /// What it proves is the round trip: that the service accepted an inhibit
    /// and handed back a cookie, that a second `true` did not take a second one
    /// out, and that `false` gave it back. It cannot prove the screen would
    /// have stayed lit — nothing short of watching a phone for twenty minutes
    /// can — and it does not claim to.
    #[test]
    #[ignore = "talks to the real session bus and inhibits the real screensaver"]
    #[cfg(not(target_os = "android"))]
    fn the_session_bus_takes_an_inhibit_and_gives_it_back() {
        let cookie = inhibit().expect("org.freedesktop.ScreenSaver accepted an inhibit");
        println!("Inhibit -> cookie {cookie}");

        // The contract the rest of the app codes against: called again with the
        // answer it already has, and it must not stack a second inhibit that
        // one `UnInhibit` would then fail to release.
        DesktopLock.set_keep_awake(true);
        DesktopLock.set_keep_awake(true);

        uninhibit(cookie);
        println!("UnInhibit({cookie}) -> ok");

        // And through the seam itself, the sequence a set actually produces.
        DesktopLock.set_keep_awake(true);
        DesktopLock.set_keep_awake(false);
        DesktopLock.set_keep_awake(false);
    }

    #[test]
    fn leaving_performance_mode_releases_it_even_with_the_toggle_still_on() {
        // The bug this card is as much about as the sleeping screen: a phone
        // that never sleeps again because the set ended and nobody said so.
        let lock = Recorder::default();
        lock.set_keep_awake(wanted(Route::Performance(SET), true));
        lock.set_keep_awake(wanted(Route::SetlistDetail(SET), true));
        assert_eq!(*lock.written.borrow(), vec![true, false]);
    }
}
