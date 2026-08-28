//! Save a webpage offline — wireframe `1l`, card E2.
//!
//! Paste a URL, watch it come down, keep it forever. The engine that does the
//! coming-down is [`crate::capture`] and landed with E1; this file is the
//! screen on top of it, the worker thread that keeps that screen off the UI
//! thread, and — the part E4 is built on — an explicit state machine saying
//! what the screen can be in the middle of and what may happen next.
//!
//! ## The state machine
//!
//! The card asked for this to be modelled before it was written, because every
//! step of a capture can fail and E4 draws whatever states are named here. So
//! [`Flow`] is one enum rather than a handful of booleans, and the
//! combinations that would be a bug — running with a result already in hand,
//! attaching a capture that failed, a cancelled capture still ticking a
//! progress bar — cannot be constructed.
//!
//! ```text
//!                 Capture                 Step(run)
//!    ┌──────────┐──────────▶┌───────────┐◀────────┐
//!    │ Waiting  │           │  Running  │─────────┘
//!    └──────────┘◀──────────└───────────┘
//!         ▲        Cancel          │ Done(run)
//!         │                        ▼
//!         │  Cancel          ┌───────────┐   Attach   ┌──────────┐
//!         └──────────────────│  Settled  │───────────▶│ Attached │
//!                            └───────────┘            └──────────┘
//!                                  │ Capture (try again)
//!                                  └──────────▶ Running
//! ```
//!
//! [`Flow::NoWorker`] hangs off `Waiting` and is reached only when this device
//! will not give the app a thread to capture on.
//!
//! `Settled` carries the engine's [`Outcome`] whole, which is where the five
//! results live: `Captured`, `Partial`, `Blocked`, `Failed` and `Cancelled`.
//! E4's three failure screens are a `match` on that one field, and E4 does not
//! have to touch the machine above to add them.
//!
//! ### The two states that are deliberately *not* here
//!
//! **`Attaching`.** The write is [`crate::capture::write_into`] called straight
//! from the Attach handler, on the UI thread, and no frame is drawn while it
//! runs — so a state for it would be one nothing could ever observe. It is
//! bounded by `Limits::max_page_bytes` + `max_total_asset_bytes`, 12 MiB, which
//! is less than `pdf::import` already writes synchronously on the same thread.
//! If that write ever moves to a worker, this state comes back and the
//! double-tap guard below has to become a real one.
//!
//! **`Cancelling`.** The screen does not wait for a worker to acknowledge that
//! it has stopped. Cancel returns to `Waiting` immediately and the answer that
//! arrives afterwards is dropped on the floor by run id. A "stopping…" spinner
//! over something the user has already stopped is a screen apologising for its
//! own architecture.
//!
//! Nothing here is persisted before Attach, so there is also no `Restoring`:
//! an app killed mid-capture comes back with no capture and nothing on disk to
//! reconcile. That is what makes Cancel free, and it is the same reason
//! `capture` builds the whole page in memory before `write_into` touches the
//! library.
//!
//! ## Where the fetch runs, and how it talks back
//!
//! On a thread of its own, one per capture. It has to be: `capture` is a page
//! fetch followed by up to twenty-four image fetches, `rinch-http`'s timeouts
//! are 30s/60s *per request*, and Android calls five seconds of unresponsive
//! main thread an ANR and kills the process.
//!
//! D5's PDF prefetch is the local precedent for a worker and this one is
//! deliberately *not* built like it. That thread was allowed to lose every
//! race and tell nobody, because a page it failed to draw would simply be drawn
//! on demand later. This one has to report into a live screen, so it needs a
//! channel, and the channel is [`Signal::update_send`]:
//!
//! * **UI → worker** is one `AtomicBool`. The worker reads it at every
//!   checkpoint the engine offers and answers [`Wanted::No`] when it is set.
//! * **worker → UI** is `flow.update_send(…)`, which hops a closure onto the
//!   main thread and runs it against the live state. The *reduction happens on
//!   the main thread*, inside [`Flow::deliver`], which is what makes it safe to
//!   compare run ids there: a mailbox signal the worker `set` instead would
//!   lose a message whenever two arrived between frames, and the one it lost
//!   could be the final outcome.
//!
//! **If the screen is gone when the worker answers, nothing happens.** A write
//! to a signal whose scope has been disposed is a warn-once no-op in Rinch —
//! the same property `song_detail` already leans on for the file picker's
//! callback — so the update simply does not run and the `Flow` it would have
//! mutated no longer exists. In practice the worker usually never gets that
//! far, because of the next paragraph.
//!
//! ## One mechanism cancels everything
//!
//! [`StopOnDrop`] lives *inside* `Flow::Running` and sets the flag in its
//! `Drop`. That single fact covers every way a capture can stop being wanted:
//!
//! | What happens | Why the flag gets set |
//! | --- | --- |
//! | Cancel is pressed | `Running` is replaced by `Waiting` |
//! | Capture is pressed again | `Running` is replaced by a new `Running` |
//! | The worker finishes | `Running` is replaced by `Settled` |
//! | The user leaves the screen | the scope is disposed and the signal's value is dropped |
//!
//! Progress ticks mutate the state in place through `&mut Flow`, so a running
//! capture's token is moved rather than dropped and the flag is not set by its
//! own progress.
//!
//! **Correctness never depends on the flag arriving**, which is the one lesson
//! this does take from D5. A worker that never notices still cannot corrupt
//! anything: its answer carries a run id, [`Flow::deliver`] compares it, and a
//! stale answer changes nothing. The flag stops *work*; the run id keeps the
//! screen *right*.
//!
//! ## What Cancel means
//!
//! The header's ← and the footer's Cancel are **not** the same control, and
//! binding them to the same handler was the first thing the state machine
//! caught: with both leaving, or both stopping, one row of this table becomes
//! unreachable. ← is the screen's exit and always leaves. Cancel is the
//! capture's:
//!
//! | State | Cancel |
//! | --- | --- |
//! | `Waiting` / `NoWorker` | leave the screen — there is nothing to stop |
//! | `Running` | stop the capture, stay here with the URL still typed |
//! | `Settled` | throw the capture away and leave |
//! | `Attached` | unreachable; the screen has already left |
//!
//! What Cancel is *not* is an abort of the request in flight. `rinch-http`
//! exposes no way to cancel an open connection, so the socket already waiting
//! runs to its timeout on a thread nobody is listening to. Everything after it
//! is skipped, and nothing is written either way — a capture that is cancelled
//! at image twenty of twenty-four costs one abandoned request and zero bytes on
//! disk, because `write_into` has not been called and will not be.
//!
//! ## The numbers on the screen are measured
//!
//! `3 of 7` and `1.2 MB so far` come out of [`Progress`], which the engine
//! reports from inside its own loop: `total` is counted after the page has been
//! walked for images that carry a usable address, and `bytes` is the page as it
//! was served plus every image that actually landed. Nothing on this screen is
//! driven by a timer, and the only derived number is the progress bar's
//! fraction — see [`bar_fraction`], which says exactly what it is estimating
//! and why it does not pretend to estimate time.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::capture::{
    CaptureMode, CapturedPage, Limits, Outcome, Progress, Wanted, capture, write_into,
};
use crate::model::{Attachment, AttachmentId, AttachmentKind, Day, SongId, fmt_bytes};
use crate::store::{NavStore, Route, SongsStore};
use crate::theme::{SCREEN_PAD, T_CHART, T_META, T_META_SMALL, T_ROW_TITLE};
use crate::ui::{IconButton, icon};

/// Which capture a worker's message belongs to.
///
/// Monotonic, per screen, and never reused. It is the whole of the defence
/// against a late answer: a worker that has been cancelled, or superseded by a
/// second Capture, still holds a live `Signal` handle and will still deliver —
/// and its message is dropped on arrival because the number does not match.
pub type RunId = u64;

/// How much of a first line, or a page title, is worth keeping as a name.
///
/// The same 40 as `chart_editor::TITLE_LIMIT` and `pdf::TITLE_LIMIT`, and
/// deliberately: all three end up in the same collapsed row under the same
/// card, and three producers with three cut-off points would show as a ragged
/// column.
const TITLE_LIMIT: usize = 40;

/// What a captured page is called when the site gave it no title at all. The
/// register matches `chart_editor`'s `lyrics` and `pdf`'s `chart`: a lower-case
/// common noun, so the row reads `page · saved page`.
const FALLBACK_TITLE: &str = "page";

/// How many lines of the captured text the preview shows. The wireframe draws
/// seven bars of skeleton in a box about this tall.
const PREVIEW_LINES: usize = 7;

// ────────────────────────────────────────────────────────────────────────────
// The cancel flag
// ────────────────────────────────────────────────────────────────────────────

/// The worker's end of the cancel flag: it only ever reads.
///
/// Cheap to clone and safe to hold for as long as the thread lives — it keeps
/// the `AtomicBool` alive after the screen has gone, which is exactly what is
/// wanted, because the last thing the screen does on the way out is set it.
#[derive(Debug, Clone)]
pub struct StillWanted(Arc<AtomicBool>);

impl StillWanted {
    /// The answer to hand back to [`capture`]'s progress callback.
    pub fn answer(&self) -> Wanted {
        if self.0.load(Ordering::Relaxed) {
            Wanted::No
        } else {
            Wanted::Yes
        }
    }
}

/// The screen's end of the cancel flag. **Dropping it is the cancel.**
///
/// There is no `stop()` method on purpose. Every way a capture stops being
/// wanted is a way this value stops being reachable — the state is replaced,
/// or the screen is unmounted and the signal's value is dropped — so tying the
/// flag to `Drop` means no path can forget to set it. Rinch drops a disposed
/// scope's signal values deterministically, after everything that scope owned
/// has been freed, so leaving the screen mid-capture runs this.
///
/// `Relaxed` is the right ordering for both ends: the flag is the only thing
/// being communicated and nothing is being published behind it.
#[derive(Debug)]
pub struct StopOnDrop(Arc<AtomicBool>);

impl StopOnDrop {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// The read-only half to give the worker.
    pub fn watch(&self) -> StillWanted {
        StillWanted(Arc::clone(&self.0))
    }

    /// Whether the flag is set. Here for the tests, which cannot observe a
    /// `Drop` any other way.
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

impl Default for StopOnDrop {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        // One atomic store and nothing else, and that is a constraint rather
        // than a coincidence. A `Flow` replaced from inside `Signal::update` is
        // dropped while the signal store is still borrowed mutably, so anything
        // in here that read or wrote a signal would panic with "RefCell already
        // borrowed" — the same fault `Flow::take_settled` was written to avoid,
        // reached from a destructor where it would be much harder to find.
        self.0.store(true, Ordering::Relaxed);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// The machine
// ────────────────────────────────────────────────────────────────────────────

/// What a worker has to say. Both variants carry a [`RunId`] alongside them at
/// the call site rather than inside, because the id is about the *delivery* and
/// not about the message.
#[derive(Debug)]
pub enum Message {
    /// Where the capture has got to. Arrives many times per run.
    Step(Progress),
    /// The capture is over, whatever the engine made of it. Arrives once.
    Done(Outcome),
}

/// Whether a delivered message was for the capture this screen is watching.
///
/// Returned rather than logged so the tests can assert that a late worker
/// changes nothing — "it was ignored" is the interesting half of the
/// behaviour and an untested silent `return` is indistinguishable from a bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Applied,
    /// The message belonged to a capture that has been cancelled, superseded
    /// or already finished. Nothing was touched.
    Stale,
}

/// What the screen should do after a Cancel. See the table in the module
/// header: Cancel means two different things and the state decides which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterCancel {
    /// A capture was running and now is not. The screen stays, with the URL
    /// still in the field, because "stop" and "I did not want this screen" are
    /// different intentions and only one of them was expressed.
    StayHere,
    LeaveTheScreen,
}

/// Where the capture screen is. One value, so the illegal combinations cannot
/// be built. See the module header for the diagram and for the two states that
/// are deliberately missing.
#[derive(Debug)]
pub enum Flow {
    /// Nothing has been asked for. The URL field is the whole screen.
    Waiting,
    /// A worker is out for `run`.
    ///
    /// `url` is what that worker was actually given, kept here rather than read
    /// back off the field: the field is still editable, and a progress panel
    /// labelled with a URL nobody is fetching would be a lie the user could
    /// produce by typing.
    Running {
        run: RunId,
        url: String,
        /// The last thing the worker reported. Starts at
        /// [`Progress::Fetching`] because that is what the engine reports
        /// first, and the screen should not invent a stage the engine has not
        /// reached.
        step: Progress,
        /// Dropped when this state is replaced, which is the cancel.
        stop: StopOnDrop,
    },
    /// The worker for `run` answered. Everything E4 has to draw is in
    /// `outcome`.
    Settled {
        run: RunId,
        url: String,
        outcome: Outcome,
    },
    /// The capture is a chart on the song. Terminal — the screen navigates away
    /// the moment it sees this, and the state exists so that a second tap on
    /// Attach lands somewhere that refuses it rather than somewhere that
    /// attaches it again.
    Attached { attachment: AttachmentId },
    /// This device would not give the app a thread. Nothing was captured and
    /// nothing was written; the only way out is to try again or leave.
    NoWorker { url: String },
}

impl Flow {
    /// Begin a capture, throwing away whatever was here before.
    ///
    /// Replacing the old value is what stops an older worker: the `StopOnDrop`
    /// it was holding is dropped by the assignment. That is why this takes the
    /// new token by value rather than building it — the caller has to have
    /// handed the worker its `watch()` first, and threading it through here
    /// keeps the pairing in one place.
    pub fn start(&mut self, run: RunId, url: String, stop: StopOnDrop) {
        *self = Flow::Running {
            run,
            url,
            step: Progress::Fetching,
            stop,
        };
    }

    /// The capture could not be started, because the thread could not be.
    pub fn no_worker(&mut self, url: String) {
        *self = Flow::NoWorker { url };
    }

    /// A message from a worker.
    ///
    /// Every rejected edge in the machine is this function returning
    /// [`Delivery::Stale`]: a message for a superseded run, a message arriving
    /// after Cancel has put the screen back to `Waiting`, a second `Done` from
    /// a worker that somehow reported twice, anything at all once the capture
    /// has been attached. None of them mutate.
    pub fn deliver(&mut self, run: RunId, message: Message) -> Delivery {
        // Only a `Running` state is listening, and only to its own worker.
        // Written as a guard rather than folded into the match below so that
        // the two halves of "is this for me" — right state, right run — are
        // one condition and cannot drift apart.
        let Flow::Running {
            run: current, url, ..
        } = self
        else {
            return Delivery::Stale;
        };
        if *current != run {
            return Delivery::Stale;
        }

        match message {
            Message::Step(reported) => {
                // Mutated in place. Assigning a whole new `Running` here would
                // drop this run's own `StopOnDrop` and cancel the capture that
                // is reporting — the bug this comment exists to prevent
                // somebody reintroducing while tidying.
                if let Flow::Running { step, .. } = self {
                    *step = reported;
                }
            }
            Message::Done(outcome) => {
                let url = std::mem::take(url);
                *self = Flow::Settled { run, url, outcome };
            }
        }
        Delivery::Applied
    }

    /// What the footer's Cancel does here, and what the screen should do next.
    pub fn cancel(&mut self) -> AfterCancel {
        match self {
            // Stopping the worker is the whole of it: replacing the state drops
            // the flag's owner. The URL is not cleared — the most likely next
            // thing after stopping a capture is starting the same one again.
            Flow::Running { .. } => {
                *self = Flow::Waiting;
                AfterCancel::StayHere
            }
            // A capture nobody attached is a capture nobody keeps. There is
            // nothing on disk to undo, because nothing has been written.
            Flow::Waiting | Flow::Settled { .. } | Flow::NoWorker { .. } | Flow::Attached { .. } => {
                AfterCancel::LeaveTheScreen
            }
        }
    }

    /// The page this state is offering to attach, if it is offering one.
    ///
    /// This is what the Attach button draws itself from, and it is a method on
    /// the state rather than a flag beside it because the answer is different
    /// for reasons that only the state knows: `Attached` has no page, so a
    /// second tap finds nothing to attach; and `Blocked` *does* have one when
    /// the site served bytes, because a paywalled teaser is the user's to keep
    /// or discard and not the app's to decide about.
    ///
    /// **Reading only.** Pressing Attach goes through [`take_settled`] instead
    /// — see the warning there, which is written on the corpse of the first
    /// end-to-end run of this screen.
    ///
    /// [`take_settled`]: Flow::take_settled
    pub fn attachable(&self) -> Option<&CapturedPage> {
        match self {
            Flow::Settled { outcome, .. } => outcome.page(),
            _ => None,
        }
    }

    /// Take the finished capture out of the state, on its way to the disk.
    ///
    /// **This exists because borrowing it instead is a crash**, and it is worth
    /// being blunt about how it was found: the first time this screen was
    /// driven end to end on the private display it captured hymnal.net
    /// perfectly, drew its preview, and then died on Attach with `RefCell
    /// already borrowed`. `Signal::with` holds the whole signal store borrowed
    /// for as long as its closure runs, and [`attach_captured`] writes to
    /// `SongsStore` — which is another signal, and `Signal::update` wants
    /// `borrow_mut`. So `flow.with(|state| attach_captured(…state.attachable()))`
    /// is a nested borrow of the same `RefCell`, and it panics on the main
    /// thread of the real app while every test passes. It is the same fault
    /// `AttachmentsStore::update` records having found in card D2, met from the
    /// other direction, and the fix is the same shape: get the value out of the
    /// borrow, then write.
    ///
    /// Moving rather than cloning also saves copying up to twelve megabytes of
    /// page and images, and — more usefully — *is* the double-tap guard. Once
    /// the capture has been taken the state is `Waiting`, [`attachable`] is
    /// `None`, and a second Attach has nothing to find. A write that fails puts
    /// it back with [`restore`].
    ///
    /// [`attachable`]: Flow::attachable
    /// [`restore`]: Flow::restore
    pub fn take_settled(&mut self) -> Option<(RunId, String, Outcome)> {
        match std::mem::replace(self, Flow::Waiting) {
            Flow::Settled { run, url, outcome } => Some((run, url, outcome)),
            // Put back whatever was actually there. Attach is only offered over
            // `Settled`, so every other state arriving here is a tap that
            // raced a redraw, and the answer to that is to change nothing.
            other => {
                *self = other;
                None
            }
        }
    }

    /// Put a capture back after a write that did not land.
    ///
    /// The preview and the checklist come back with it, because the complaint
    /// the screen is about to show is "this could not be written", and a
    /// complaint over an empty screen leaves the user with nothing to try
    /// again with. Same rule `chart_editor` states for a failed Save: a failed
    /// write costs the user nothing they had.
    pub fn restore(&mut self, run: RunId, url: String, outcome: Outcome) {
        *self = Flow::Settled { run, url, outcome };
    }

    /// The URL to put back in the field when the screen redraws itself from
    /// the state — after a Cancel, or a failed capture worth retrying.
    pub fn url(&self) -> &str {
        match self {
            Flow::Running { url, .. } | Flow::Settled { url, .. } | Flow::NoWorker { url } => url,
            Flow::Waiting | Flow::Attached { .. } => "",
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Flow::Running { .. })
    }
}

// ────────────────────────────────────────────────────────────────────────────
// What the state looks like
// ────────────────────────────────────────────────────────────────────────────

/// One line of `1l`'s checklist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// Not started. A plain circle.
    Waiting,
    /// Happening now. A dashed circle, and the line the count goes on.
    Doing,
    /// Done. A circle with a check in it.
    Done,
}

impl Tick {
    pub fn glyph(self) -> TablerIcon {
        match self {
            Tick::Waiting => TablerIcon::Circle,
            Tick::Doing => TablerIcon::CircleDashed,
            Tick::Done => TablerIcon::CircleCheck,
        }
    }
}

/// The three lines of the checklist, in the order `1l` draws them, with the
/// wording the wireframe uses where it gave any.
///
/// The image line is the one that has to say something different in four
/// situations, and every one of them is a real state rather than a decoration:
/// a page with no images at all is the common case on a chord site (CifraClub
/// captures perfectly with zero), and a page that finished with some missing is
/// `Outcome::Partial`, which the user is about to be asked to keep.
pub fn checklist(flow: &Flow) -> Vec<(String, Tick)> {
    let (step, finished) = match flow {
        Flow::Running { step, .. } => (*step, None),
        Flow::Settled { outcome, .. } => match outcome.page() {
            Some(page) => (
                Progress::Images {
                    done: page.assets.len(),
                    total: page.assets.len() + page.missed.len(),
                    bytes: page.bytes_on_disk(),
                },
                Some(page),
            ),
            // Nothing came down, or it was cancelled. The checklist stops
            // where the capture did rather than pretending the first two lines
            // succeeded — E4 draws the explanation underneath it.
            None => return failed_checklist(outcome),
        },
        Flow::Waiting | Flow::Attached { .. } | Flow::NoWorker { .. } => return Vec::new(),
    };

    let fetched = match step {
        Progress::Fetching => Tick::Doing,
        _ => Tick::Done,
    };
    let stripped = match step {
        Progress::Fetching => Tick::Waiting,
        Progress::Stripping { .. } => Tick::Doing,
        Progress::Images { .. } => Tick::Done,
    };
    let (images, images_label) = match (step, finished) {
        (Progress::Images { total: 0, .. }, _) => {
            (Tick::Done, "No images on this page".to_string())
        }
        (Progress::Images { done, total, .. }, None) if done < total => (
            Tick::Doing,
            format!("Downloading images ({done} of {total})"),
        ),
        (Progress::Images { done, total, .. }, _) if done == total => {
            (Tick::Done, format!("Downloaded {total} {}", images(total)))
        }
        (Progress::Images { done, total, .. }, _) => (
            // Finished with holes in it. Said plainly, because this is the
            // difference between `Captured` and `Partial` and the user is
            // about to decide whether to keep it.
            Tick::Done,
            format!("Downloaded {done} of {total} {}", images(total)),
        ),
        _ => (Tick::Waiting, "Images".to_string()),
    };

    vec![
        ("Fetched page".to_string(), fetched),
        ("Stripped ads & scripts".to_string(), stripped),
        (images_label, images),
    ]
}

/// `image` or `images`. A page with exactly one picture on it is not rare — it
/// is what hymnal.net has, and "Downloaded 1 images" was the first thing the
/// finished screen said out loud on the private display.
fn images(n: usize) -> &'static str {
    if n == 1 { "image" } else { "images" }
}

/// The checklist for a capture that never produced a page: it stops at the
/// line that did not finish, rather than ticking three boxes over an empty
/// result.
fn failed_checklist(outcome: &Outcome) -> Vec<(String, Tick)> {
    match outcome {
        // The fetch is the only step that runs before there is a page, so a
        // failure with no page is always a failure of that step.
        Outcome::Failed(_) => vec![("Fetched page".to_string(), Tick::Waiting)],
        // A cancel can land at any of them and the state does not record
        // which, because nothing is being kept and the distinction buys the
        // user nothing.
        _ => Vec::new(),
    }
}

/// How full the determinate bar is, from 0 to 1.
///
/// **This is a position in the checklist, not a prediction of time**, and the
/// distinction is the whole reason it is worth a doc comment. The three stages
/// are weighted equally because there is no honest basis for weighting them
/// otherwise: total bytes are unknowable until everything has arrived (nothing
/// in `Fetched` carries a `Content-Length`), and a page's images are not the
/// same size as each other, let alone as the page. What the bar can promise is
/// that it only ever moves on completed work and never moves backwards — which
/// is why the fetch contributes nothing while it is in flight and the images
/// share a third between them rather than each claiming a stage of their own.
///
/// The number underneath it is the one that is exact. See [`so_far`].
pub fn bar_fraction(step: Progress) -> f32 {
    match step {
        Progress::Fetching => 0.0,
        Progress::Stripping { .. } => 1.0 / 3.0,
        // A page with no images is finished the moment stripping is: there is
        // no third stage to be part-way through.
        Progress::Images { total: 0, .. } => 1.0,
        Progress::Images { done, total, .. } => {
            (1.0 + (done.min(total) as f32 / total as f32)) / 3.0 + 1.0 / 3.0
        }
    }
}

/// `1.2 MB so far · will work with no signal` — the line under the bar.
///
/// The promise on the right is the point of the feature and it is stated in the
/// present tense of something already true, so it is only ever printed beside a
/// figure that is already on this device.
pub fn so_far(bytes: u64) -> String {
    format!("{} so far · will work with no signal", fmt_bytes(bytes))
}

/// What a captured page is called on the row under the card.
///
/// Chord sites put the whole world in a `<title>` — song, artist, site name,
/// "free guitar tabs, chords and lyrics" — so this gets the same treatment
/// `chart_editor::chart_title` and `pdf::title_for` give their titles:
/// whitespace collapsed, cut at [`TITLE_LIMIT`], and a plain word when the page
/// offered nothing. The engine has already fallen back to the URL when there
/// was no `<title>` and no `<h1>`, so the empty case here is rare — and the URL
/// is kept in `source_url` regardless, which is where E6 will read it.
pub fn page_title(page: &CapturedPage) -> String {
    let collapsed = page.title.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return FALLBACK_TITLE.to_string();
    }
    if collapsed.chars().count() <= TITLE_LIMIT {
        return collapsed;
    }
    let cut: String = collapsed.chars().take(TITLE_LIMIT).collect();
    format!("{}…", cut.trim_end())
}

/// The first lines of the captured text, for the preview box.
///
/// The preview is not decoration and it is not a loading skeleton with content
/// in it. The E1 spike found a songsterr URL that redirected to a *different
/// song*, and `rinch-http` does not report where a redirect ended up — so the
/// only thing standing between that and a chart filed under the wrong title is
/// the user reading a few lines before pressing Attach.
pub fn preview_lines(page: &CapturedPage) -> Vec<String> {
    page.text
        .lines()
        .skip_while(|line| line.trim().is_empty())
        .take(PREVIEW_LINES)
        // An empty text node collapses and takes the gap it stood for with it,
        // so a blank line is drawn as a space — the same trick `song_detail`'s
        // chart preview uses, and for the same reason: the blank lines between
        // verses are part of the chart.
        .map(|line| {
            if line.trim().is_empty() {
                " ".to_string()
            } else {
                line.to_string()
            }
        })
        .collect()
}

/// The sentence describing what came back, under the preview.
///
/// E4 owns the *screens* for the three failures; this is the one line E2 owes
/// each of them in the meantime, so that a capture that went wrong says so
/// rather than showing an empty box with a live Attach button under it. Every
/// string it can produce comes from the engine — `Failure`'s `Display` and
/// `BlockReason::explain` are both written next to the rule that fires them, so
/// that the reason and its explanation cannot drift apart.
pub fn verdict(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Captured(page) => format!(
            "{} on this device. Nothing in it reaches the network.",
            fmt_bytes(page.bytes_on_disk())
        ),
        Outcome::Partial(page) => format!(
            "{} kept, and {} image{} did not come down. The chart is still here.",
            fmt_bytes(page.bytes_on_disk()),
            page.missed.len(),
            if page.missed.len() == 1 { "" } else { "s" }
        ),
        Outcome::Blocked { reason, .. } => reason.explain().to_string(),
        Outcome::Failed(failure) => {
            let mut sentence = failure.to_string();
            // `Failure`'s `Display` is a fragment — "could not reach the site:
            // …" — written to be embedded. Capitalise it into a sentence here
            // rather than changing the engine's strings, which E4 and the
            // probe both read.
            if let Some(first) = sentence.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            format!("{sentence}.")
        }
        Outcome::Cancelled => "Stopped. Nothing was saved.".to_string(),
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Putting it on the song
// ────────────────────────────────────────────────────────────────────────────

/// Everything that can stop a finished capture becoming an attachment.
///
/// Shorter than `pdf::ImportError` because the judging half is already done:
/// by the time anything here runs, the engine has decided the bytes are a page
/// worth offering, and all that is left is the library saying no.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachError {
    /// There is no library on disk to put it in — the in-memory mode the
    /// `--seed` screenshots and the tests run in. Nothing was attached.
    NoLibrary,
    /// The row could not be written. `Storage` has already recorded the fault.
    NotAttached,
    /// The row went in and the bytes did not. The row has been taken out again.
    Write(String),
}

impl AttachError {
    /// The sentence the screen shows, in the register `pdf::ImportError` set.
    pub fn message(&self) -> String {
        match self {
            Self::NoLibrary => "There is no library on this device to save it in.".to_string(),
            Self::NotAttached => "That page could not be saved.".to_string(),
            Self::Write(_) => "That page could not be written to this device.".to_string(),
        }
    }
}

/// Take a finished capture into the library as a chart on `song`.
///
/// The order is the one `crate::capture`'s header sets out and `pdf::import`
/// already follows: **attach, then write, then undo the attach if the write
/// failed.** `SongsStore::attach` mints a row *and* a directory *and* — if this
/// is the song's first chart — makes it primary, so a capture that left a row
/// behind after a failed write would put an empty ghost on the card in front of
/// a real chart. Nothing here creates a directory itself; `Repo::create_
/// attachment` does, which is what keeps every attachment directory owned by a
/// row that can delete it again.
///
/// Synchronous, on the UI thread, deliberately — see the module header's note
/// on the pruned `Attaching` state.
pub fn attach_captured(
    songs: SongsStore,
    song: SongId,
    page: &CapturedPage,
) -> Result<AttachmentId, AttachError> {
    let attachments = songs.attachments();
    let expected = page.bytes_on_disk();

    let attachment = Attachment {
        id: 0,
        kind: AttachmentKind::CapturedPage,
        title: page_title(page),
        bytes_on_disk: expected,
        // A saved page has no pages. `attachment_viewer::page_span` already
        // reads one for a `CapturedPage` whatever this says, and filling it in
        // with 1 would make `song_detail`'s subtitle read `primary · 1 page`,
        // which is a PDF's sentence.
        page_count: None,
        // What the user pasted, not where the request ended up: `rinch-http`
        // does not report the final URL after a redirect, so this is the only
        // address the app actually knows — and it is also the right thing for
        // E6 to re-fetch, because following the redirect again is what a
        // re-check should do.
        source_url: Some(page.url.clone()),
        captured_at: Some(Day::today()),
        // The extracted text, which is what card G2's search inside
        // attachments reads and what `song_detail` previews under the card
        // until E5 can render the page itself.
        body: Some(page.text.clone()),
    };

    let id = songs
        .attach(song, attachment)
        .ok_or(AttachError::NotAttached)?;

    let Some(directory) = attachments.directory(id) else {
        // In-memory mode: the row exists and there is nowhere for the files. A
        // saved page with no `page.html` behind it would be a chart the viewer
        // could never open, so the row comes back out.
        songs.detach(song, id);
        return Err(AttachError::NoLibrary);
    };

    match write_into(&directory, page) {
        Ok(written) => {
            if written == expected {
                // The row is already right — `write_into` returns the same sum
                // `bytes_on_disk()` computes — so this is a redraw and not a
                // save. The card has already drawn itself once against a
                // directory that was still empty when the row was minted; see
                // `AttachmentsStore::files_changed` for that whole story.
                attachments.files_changed(id);
            } else {
                // Unreachable today and cheap insurance for the day it is not:
                // if `write_into` ever puts something else in the directory —
                // E6's re-check manifest is the obvious candidate — the row's
                // size follows the disk rather than the guess.
                attachments.update(id, |a| a.bytes_on_disk = written);
            }
            Ok(id)
        }
        Err(e) => {
            songs.detach(song, id);
            Err(AttachError::Write(e.to_string()))
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// The worker
// ────────────────────────────────────────────────────────────────────────────

/// The capture thread's stack.
///
/// Bigger than D5's rasteriser thread, which got 1 MB, because the work is a
/// different shape: `dom::walk` and `dom::collect_text` recurse once per level
/// of element nesting, over markup that came off a stranger's server and has
/// no obligation to be shallow. 4 MB is the default a desktop main thread gets
/// and four times what Android hands a Java-created one, which is the floor
/// worth designing against.
#[cfg(not(target_arch = "wasm32"))]
const WORKER_STACK: usize = 4 * 1024 * 1024;

/// Start a capture on a thread of its own. `false` if the device would not
/// give us one.
///
/// Everything it needs is owned or `Copy`: `Signal` is a handle, and writing
/// through a handle whose scope has gone is a no-op rather than a fault, which
/// is what makes it safe to hand one to something that outlives the screen.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_capture(
    flow: Signal<Flow>,
    run: RunId,
    url: String,
    mode: CaptureMode,
    watch: StillWanted,
) -> bool {
    std::thread::Builder::new()
        .name("sla-capture".into())
        .stack_size(WORKER_STACK)
        .spawn(move || {
            let limits = Limits::default();
            let fetcher = crate::capture::HttpFetcher::new(&limits);

            let outcome = capture(&url, mode, &limits, &fetcher, |step| {
                // Asked before the hop, not after: if the answer is no there is
                // no reason to wake the main thread to tell it about a stage of
                // a capture it has stopped caring about.
                if watch.answer() == Wanted::No {
                    return Wanted::No;
                }
                flow.update_send(move |state| {
                    state.deliver(run, Message::Step(step));
                });
                Wanted::Yes
            });

            // Delivered even when the outcome is `Cancelled`. It costs one hop
            // and it means the machine has exactly one way to leave `Running`,
            // rather than one for a finished capture and a silent one for a
            // stopped worker that nothing could ever test.
            flow.update_send(move |state| {
                state.deliver(run, Message::Done(outcome));
            });
        })
        .is_ok()
}

// ────────────────────────────────────────────────────────────────────────────
// The screen
// ────────────────────────────────────────────────────────────────────────────

/// The progress panel and the preview box. `1l` draws both as outlined
/// rectangles; here they wear the panel fill the handoff's own "Before you
/// start" panel uses, because the wireframe's marker outlines are a sketch
/// convention and the app has a surface for a grouped block already.
const PANEL: &str = "background: var(--sla-fill); border-radius: 12px; padding: 13px 15px; \
    display: flex; flex-direction: column; gap: 10px;";

/// The URL field. `1j`'s underlined input, at body size rather than the display
/// face `song_form` uses for a title — a URL is not a name and setting one in
/// the display face makes it look like one.
const URL_FIELD: &str = "width: 100%; border: none; border-bottom: 2px solid var(--sla-ink); \
    outline: none; background: transparent; border-radius: 0; padding: 6px 0 11px; \
    margin-top: 2px; font-size: 15px; color: var(--sla-ink); word-break: break-all;";

#[component]
pub fn CaptureScreen(song: Option<SongId>) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();

    let song = song.unwrap_or_default();
    if songs.get(song).is_none() {
        return rsx! {
            div { style: {format!("padding: {SCREEN_PAD}; {T_META}")}, "This song is gone." }
        };
    }

    // What is in the field, which is not the same thing as what is being
    // captured — see `Flow::Running::url`.
    let draft = Signal::new(String::new());
    let flow = Signal::new(Flow::Waiting);
    // The run counter. A signal rather than a static so that two screens in one
    // process could never share it; there is only ever one today, and a
    // per-screen counter is the version of that fact that stays true.
    let runs = Signal::new(0 as RunId);
    // What the last Attach had to say, if it failed. Cleared by the next
    // attempt, so a complaint never outlives the problem — `song_detail`'s
    // rule for the same strip.
    let trouble = Signal::new(Option::<String>::None);

    let leave = move || nav.go(Route::SongDetail(song));

    let begin = move || {
        let url = draft.get().trim().to_string();
        if url.is_empty() {
            return;
        }
        trouble.set(None);
        let run = runs.get() + 1;
        runs.set(run);

        let stop = StopOnDrop::new();
        let watch = stop.watch();
        // The state goes in *before* the thread starts, so that any previous
        // run's flag is already set when the new worker's first checkpoint
        // comes round, and so that a spawn failure has somewhere to land.
        let started = url.clone();
        flow.update(move |state| state.start(run, started, stop));

        #[cfg(not(target_arch = "wasm32"))]
        if !spawn_capture(flow, run, url.clone(), CaptureMode::FullPage, watch) {
            // A machine that cannot spawn a thread is a machine under real
            // pressure. D5's prefetch could shrug and let the work happen
            // later; this one cannot, so it says so.
            flow.update(move |state| state.no_worker(url));
        }
    };

    // The footer's Cancel: the *capture's* cancel, which is a different thing
    // at different states — see the table in the module header.
    let cancel = move || {
        let next = flow.with_untracked_cancel();
        if next == AfterCancel::LeaveTheScreen {
            leave();
        }
    };

    // The header's ←: the *screen's* exit, always, whatever is running.
    //
    // Bound to `cancel` at first, and that was wrong in a way the state machine
    // made obvious: with both controls doing the same thing there was no way to
    // leave a screen with a capture running, and the "the user leaves the
    // screen" row of the cancel table was unreachable. They are two intentions
    // — "stop this download" and "I am done with this screen" — and the design
    // gave each of them its own control.
    //
    // It does not ask. `chart_editor`'s ✕ raises a question because what is on
    // that screen is a verse somebody typed out by hand and there is no undo
    // anywhere in this app; nothing here is typed by hand, and a capture
    // abandoned is a URL pasted and a button pressed again. Leaving disposes
    // the scope, which drops the state, which sets the flag, which stops the
    // worker at its next checkpoint.
    let back = move || leave();

    let attach = move || {
        trouble.set(None);

        // Out of the signal first, written afterwards. Doing it the obvious way
        // round — `flow.with(|state| attach_captured(…))` — panics with
        // "RefCell already borrowed" on the real app while every test passes;
        // see `Flow::take_settled`, which exists for that reason and nothing
        // else.
        let mut taken = None;
        flow.update(|state| taken = state.take_settled());

        // `None` is a tap on a state with nothing to attach — the second of a
        // double-tap, or a tap that raced a redraw. Not an error: the guard
        // working.
        let Some((run, url, outcome)) = taken else {
            return;
        };
        if outcome.page().is_none() {
            flow.update(move |state| state.restore(run, url, outcome));
            return;
        }

        let written = attach_captured(songs, song, outcome.page().expect("just checked"));
        match written {
            Ok(attachment) => {
                flow.update(move |state| *state = Flow::Attached { attachment });
                leave();
            }
            Err(e) => {
                flow.update(move |state| state.restore(run, url, outcome));
                trouble.set(Some(e.message()));
            }
        }
    };

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

            // ← · heading. `1l` draws a back arrow rather than `1j`'s ✕,
            // because this screen is reached from the song and goes back to it.
            div {
                style: "padding: 2px 14px 8px; display: flex; align-items: center; gap: 8px; \
                        border-bottom: 1px solid var(--sla-hairline);",
                IconButton { glyph: TablerIcon::ArrowLeft, size: 18, onclick: back }
                div { style: {format!("{T_ROW_TITLE} font-size: 19px; flex: 1;")}, "Save webpage" }
            }

            div {
                style: {format!("flex: 1; min-height: 0; overflow-y: auto; \
                                 padding: 16px {SCREEN_PAD} 0; display: flex; \
                                 flex-direction: column; gap: 14px;")},

                div {
                    div { style: {T_META_SMALL}, "Page address" }
                    input {
                        r#type: "text",
                        style: {URL_FIELD},
                        placeholder: "tabs.example.com/song",
                        value: {move || draft.get()},
                        oninput: move |value: String| draft.set(value),
                    }
                    // The expectation-setting line `docs/CAPTURE.md` asked E2
                    // to carry. Three of the eight sites the spike measured
                    // produced a usable offline chart, and the ones that did
                    // not were single-page apps and bot walls — so saying this
                    // before somebody pastes a Cloudflare-protected URL costs
                    // nothing and is the honest framing of the feature.
                    div { style: {format!("{T_META_SMALL} margin-top: 8px;")},
                        "Works with pages that are pages. A site that builds itself with \
                         JavaScript has nothing in it to save."
                    }
                }

                // The progress panel — `1l`'s checklist, bar and byte counter.
                // Drawn from `Flow` on every redraw rather than from signals of
                // its own, so there is exactly one thing that can be wrong.
                if flow.with(|state| !checklist(state).is_empty()) {
                    div { style: {PANEL},
                        for (index, label, tick) in flow.with(rows) {
                            div {
                                key: {index},
                                style: "display: flex; align-items: center; gap: 10px;",
                                span {
                                    style: {format!("display: flex; color: {};",
                                        if tick == Tick::Done { "var(--sla-accent)" } else { "var(--sla-muted)" })},
                                    {icon(__scope, tick.glyph(), 17)}
                                }
                                span { style: "font-size: 14px;", {label.clone()} }
                            }
                        }

                        // The determinate bar. A track and a fill, because
                        // `<progress>` is on the sanitiser's list of form
                        // controls and this app draws its own everything else.
                        div {
                            style: "height: 8px; border-radius: 4px; overflow: hidden; \
                                    background: var(--sla-hairline);",
                            div {
                                style: {move || format!(
                                    "height: 100%; border-radius: 4px; background: var(--sla-accent); \
                                     width: {:.0}%;",
                                    flow.with(|state| bar(state)) * 100.0
                                )},
                            }
                        }

                        div { style: {T_META_SMALL}, {move || flow.with(counter)} }
                    }
                }

                // The preview. Only ever drawn over a real page — a box of
                // skeleton lines with nothing behind it is the loading state
                // `1l` sketches, and this screen has a checklist for that.
                if flow.with(|state| state.attachable().is_some()) {
                    div {
                        div { style: {T_META}, "Preview" }
                        div { style: {format!("{PANEL} gap: 4px; margin-top: 6px;")},
                            div { style: {format!("{T_ROW_TITLE} font-size: 15px;")},
                                {move || flow.with(|state| state.attachable().map(page_title).unwrap_or_default())}
                            }
                            for (index, line) in flow.with(preview) {
                                div { key: {index}, style: {T_CHART}, {line.clone()} }
                            }
                        }
                    }
                }

                // What the engine made of it. E4 replaces this with its three
                // designed screens; until then a capture that went wrong says
                // one true sentence rather than nothing.
                if flow.with(|state| matches!(state, Flow::Settled { .. } | Flow::NoWorker { .. })) {
                    div { style: {format!("{T_META} color: {};",
                            if flow.with(|state| state.attachable().is_some()) {
                                "var(--sla-muted)"
                            } else {
                                "var(--sla-danger)"
                            })},
                        {move || flow.with(explanation)}
                    }
                }

                if trouble.get().is_some() {
                    div {
                        style: {format!("{T_META_SMALL} color: var(--sla-danger);")},
                        {move || trouble.get().unwrap_or_default()}
                    }
                }

                div { style: "height: 24px; flex-shrink: 0;" }
            }

            // Cancel · Attach. `1l`'s footer, and the only place on this screen
            // anything is written.
            div {
                style: {format!("flex-shrink: 0; display: flex; gap: 10px; \
                                 padding: 12px {SCREEN_PAD} 24px; \
                                 border-top: 1px solid var(--sla-hairline);")},
                div {
                    onclick: cancel,
                    style: "flex: 1; border: 1.5px solid var(--sla-hairline); border-radius: 999px; \
                            padding: 13px; text-align: center; font-weight: 600; font-size: 16px; \
                            color: var(--sla-ink-2);",
                    "Cancel"
                }
                div {
                    // One button, whose label says what it will do. `1l` drew
                    // the footer in one state only — mid-capture, reading
                    // Attach — and a screen needs a way to *start* as well as
                    // to keep. A second permanent button the wireframe never
                    // drew would be worse than one that is honest about which
                    // of its jobs is available.
                    onclick: move || {
                        if flow.with(|state| state.attachable().is_some()) {
                            attach();
                        } else if !flow.with(Flow::is_running) {
                            begin();
                        }
                    },
                    style: {move || {
                        let live = flow.with(|state| state.attachable().is_some())
                            || (!flow.with(Flow::is_running) && !draft.get().trim().is_empty());
                        let paint = if live {
                            "background: var(--sla-accent); color: var(--sla-on-accent);"
                        } else {
                            "background: var(--sla-fill); color: var(--sla-muted);"
                        };
                        format!("{paint} flex: 1; border-radius: 999px; padding: 13px; \
                                 text-align: center; font-weight: 600; font-size: 16px;")
                    }},
                    {move || flow.with(primary_label)}
                }
            }
        }
    }
}

// The reactive closures above can only capture `Copy` values, and several of
// them want the same view of the state, so each one is a free function taking
// `&Flow`. They are also where every string on this screen is decided, which
// makes them the things worth testing.

fn rows(state: &Flow) -> Vec<(usize, String, Tick)> {
    checklist(state)
        .into_iter()
        .enumerate()
        .map(|(index, (label, tick))| (index, label, tick))
        .collect()
}

fn preview(state: &Flow) -> Vec<(usize, String)> {
    state
        .attachable()
        .map(|page| preview_lines(page).into_iter().enumerate().collect())
        .unwrap_or_default()
}

/// The bar's fill for whatever the state is. A settled capture is a full bar
/// whatever it settled as: the work stopped, and a bar frozen at two thirds
/// under a sentence explaining the failure would read as still running.
fn bar(state: &Flow) -> f32 {
    match state {
        Flow::Running { step, .. } => bar_fraction(*step),
        Flow::Settled { .. } => 1.0,
        _ => 0.0,
    }
}

/// The line under the bar, which counts two different true things and says
/// which one it is counting.
///
/// While a capture is running the figure is **what has come down** — the page
/// exactly as the site served it, plus the images that have landed. When it
/// settles the figure is **what is being kept**, which is smaller, because
/// sanitising has thrown the scripts and the ad containers away. The real
/// hymnal.net run on the private display went `78 KB so far` and then
/// `66 KB on this device`, and that step down looks like a bug until you read
/// the two labels.
///
/// The alternative — running a counter that predicted the sanitised size — is
/// not available: the strip happens once, the serialised size is not known
/// until the last image has landed, and a number that guessed would be the one
/// thing this panel is not allowed to be.
fn counter(state: &Flow) -> String {
    match state {
        Flow::Running { step, .. } => so_far(step.bytes()),
        Flow::Settled { outcome, .. } => match outcome.page() {
            Some(page) => format!(
                "{} on this device · will work with no signal",
                fmt_bytes(page.bytes_on_disk())
            ),
            None => "Nothing was saved.".to_string(),
        },
        _ => String::new(),
    }
}

fn explanation(state: &Flow) -> String {
    match state {
        Flow::Settled { outcome, .. } => verdict(outcome),
        Flow::NoWorker { .. } => {
            "This device would not start the download. Close some apps and try again.".to_string()
        }
        _ => String::new(),
    }
}

fn primary_label(state: &Flow) -> String {
    match state {
        Flow::Waiting => "Capture".to_string(),
        Flow::Running { .. } => "Capturing…".to_string(),
        Flow::Settled { outcome, .. } if outcome.page().is_some() => "Attach".to_string(),
        // Nothing came down. The button becomes the retry, which is the
        // transition E4 will keep when it draws these properly.
        Flow::Settled { .. } | Flow::NoWorker { .. } => "Try again".to_string(),
        Flow::Attached { .. } => "Attach".to_string(),
    }
}

/// `Flow::cancel` through the signal, in one place.
///
/// An extension trait rather than a closure because it has to be callable from
/// two handlers (the header's ← and the footer's Cancel) that cannot share a
/// non-`Copy` capture.
trait CancelThrough {
    fn with_untracked_cancel(self) -> AfterCancel;
}

impl CancelThrough for Signal<Flow> {
    fn with_untracked_cancel(self) -> AfterCancel {
        let mut answer = AfterCancel::LeaveTheScreen;
        self.update(|state| answer = state.cancel());
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{BlockReason, Failure, Missed, Stripped};

    fn page(url: &str) -> CapturedPage {
        CapturedPage {
            url: url.to_string(),
            title: "A Song — Example Tabs".to_string(),
            mode: CaptureMode::FullPage,
            html: "<html></html>".repeat(10),
            text: "G       C\nCarolina in my mind\n\nsecond verse".to_string(),
            assets: Vec::new(),
            missed: Vec::new(),
            stripped: Stripped::default(),
            signals: Default::default(),
            fetched_bytes: 4_000,
            reader_fell_back: false,
        }
    }

    fn running() -> Flow {
        let mut flow = Flow::Waiting;
        flow.start(1, "https://tabs.example/song".to_string(), StopOnDrop::new());
        flow
    }

    // ── the states ──────────────────────────────────────────────────────────

    #[test]
    fn a_fresh_screen_is_waiting_with_nothing_to_attach() {
        let flow = Flow::Waiting;
        assert!(flow.attachable().is_none());
        assert!(!flow.is_running());
        assert!(checklist(&flow).is_empty(), "no panel before anything runs");
        assert_eq!(primary_label(&flow), "Capture");
    }

    #[test]
    fn starting_a_capture_puts_the_screen_in_running_at_the_first_stage() {
        let flow = running();
        assert!(flow.is_running());
        assert_eq!(flow.url(), "https://tabs.example/song");
        assert!(flow.attachable().is_none(), "nothing to attach yet");
        assert_eq!(primary_label(&flow), "Capturing…");
        assert_eq!(bar(&flow), 0.0, "the fetch has not finished");
    }

    #[test]
    fn a_finished_capture_is_settled_and_offers_its_page() {
        let mut flow = running();
        assert_eq!(
            flow.deliver(1, Message::Done(Outcome::Captured(page("https://x/1")))),
            Delivery::Applied
        );
        assert!(flow.attachable().is_some());
        assert_eq!(primary_label(&flow), "Attach");
        assert_eq!(bar(&flow), 1.0);
    }

    /// A paywalled teaser still has a page, and the engine hands it back on
    /// purpose. The screen must offer to keep it rather than deciding for the
    /// user — the decision `crate::capture`'s header spends a paragraph on.
    #[test]
    fn a_blocked_page_with_bytes_behind_it_can_still_be_attached() {
        let mut flow = running();
        flow.deliver(
            1,
            Message::Done(Outcome::Blocked {
                reason: BlockReason::Paywalled,
                page: Some(Box::new(page("https://x/1"))),
            }),
        );
        assert!(flow.attachable().is_some());
        assert_eq!(primary_label(&flow), "Attach");
        assert!(verdict_of(&flow).contains("subscription"));
    }

    #[test]
    fn a_block_with_no_page_offers_nothing_and_becomes_a_retry() {
        let mut flow = running();
        flow.deliver(
            1,
            Message::Done(Outcome::Blocked {
                reason: BlockReason::Challenged,
                page: None,
            }),
        );
        assert!(flow.attachable().is_none());
        assert_eq!(primary_label(&flow), "Try again");
    }

    #[test]
    fn a_failed_fetch_offers_nothing_and_says_why_in_the_engines_own_words() {
        let mut flow = running();
        flow.deliver(
            1,
            Message::Done(Outcome::Failed(Failure::Refused { status: 404 })),
        );
        assert!(flow.attachable().is_none());
        assert_eq!(primary_label(&flow), "Try again");
        assert_eq!(verdict_of(&flow), "The site answered 404.");
        // The checklist stops at the step that did not finish rather than
        // ticking three boxes over an empty result.
        assert_eq!(checklist(&flow), vec![("Fetched page".to_string(), Tick::Waiting)]);
    }

    #[test]
    fn a_cancelled_run_settles_with_nothing_and_says_so() {
        let mut flow = running();
        flow.deliver(1, Message::Done(Outcome::Cancelled));
        assert!(flow.attachable().is_none());
        assert_eq!(counter(&flow), "Nothing was saved.");
        assert_eq!(verdict_of(&flow), "Stopped. Nothing was saved.");
    }

    // ── the transitions that must be rejected ───────────────────────────────

    /// The whole reason a run id exists. A worker that was cancelled, or
    /// superseded by a second Capture, is still alive and will still deliver.
    #[test]
    fn a_message_from_a_superseded_run_changes_nothing() {
        let mut flow = running();
        flow.start(2, "https://tabs.example/other".to_string(), StopOnDrop::new());

        assert_eq!(
            flow.deliver(1, Message::Step(Progress::Stripping { bytes: 99 })),
            Delivery::Stale
        );
        assert_eq!(
            flow.deliver(1, Message::Done(Outcome::Captured(page("https://x/1")))),
            Delivery::Stale
        );
        assert!(flow.is_running(), "still run 2, still running");
        assert_eq!(flow.url(), "https://tabs.example/other");
        assert!(flow.attachable().is_none(), "run 1's page was not kept");
    }

    #[test]
    fn a_worker_answering_after_cancel_changes_nothing() {
        let mut flow = running();
        assert_eq!(flow.cancel(), AfterCancel::StayHere);
        assert_eq!(
            flow.deliver(1, Message::Done(Outcome::Captured(page("https://x/1")))),
            Delivery::Stale
        );
        assert!(matches!(flow, Flow::Waiting), "{flow:?}");
        assert!(flow.attachable().is_none());
    }

    #[test]
    fn a_second_done_from_the_same_worker_is_ignored() {
        let mut flow = running();
        flow.deliver(1, Message::Done(Outcome::Captured(page("https://first/"))));
        assert_eq!(
            flow.deliver(1, Message::Done(Outcome::Failed(Failure::Refused { status: 500 }))),
            Delivery::Stale
        );
        assert_eq!(
            flow.attachable().map(|p| p.url.clone()),
            Some("https://first/".to_string()),
            "the first answer stands"
        );
    }

    /// Attach twice. The guard is the state, not a flag: `Attached` has no page
    /// to offer, so a second tap finds nothing to do. Without this the two taps
    /// would be two rows, two directories and two copies of the same page.
    #[test]
    fn attach_is_not_offered_twice() {
        let mut flow = running();
        flow.deliver(1, Message::Done(Outcome::Captured(page("https://x/1"))));
        assert!(flow.attachable().is_some());

        flow = Flow::Attached { attachment: 7 };
        assert!(flow.attachable().is_none(), "the second tap has nothing to attach");
    }

    #[test]
    fn nothing_a_worker_says_can_reach_an_attached_capture() {
        let mut flow = Flow::Attached { attachment: 7 };
        assert_eq!(
            flow.deliver(1, Message::Done(Outcome::Captured(page("https://x/1")))),
            Delivery::Stale
        );
        assert!(matches!(flow, Flow::Attached { attachment: 7 }), "{flow:?}");
    }

    // ── what Cancel means ───────────────────────────────────────────────────

    #[test]
    fn cancel_means_stop_the_capture_while_one_is_running() {
        let mut flow = running();
        assert_eq!(flow.cancel(), AfterCancel::StayHere);
        assert!(matches!(flow, Flow::Waiting), "{flow:?}");
    }

    #[test]
    fn cancel_means_leave_the_screen_everywhere_else() {
        let mut flow = Flow::Waiting;
        assert_eq!(flow.cancel(), AfterCancel::LeaveTheScreen);

        let mut flow = running();
        flow.deliver(1, Message::Done(Outcome::Captured(page("https://x/1"))));
        assert_eq!(flow.cancel(), AfterCancel::LeaveTheScreen);

        let mut flow = Flow::NoWorker { url: "x".into() };
        assert_eq!(flow.cancel(), AfterCancel::LeaveTheScreen);
    }

    // ── the flag that stops the work ────────────────────────────────────────

    /// Every way a capture stops being wanted is a way the state stops holding
    /// its `StopOnDrop`. These are the four rows of the table in the module
    /// header, checked one at a time through a watch that outlives the state.
    #[test]
    fn dropping_the_state_is_what_stops_the_worker() {
        // Cancel.
        let stop = StopOnDrop::new();
        let watch = stop.watch();
        let mut flow = Flow::Waiting;
        flow.start(1, "u".into(), stop);
        assert_eq!(watch.answer(), Wanted::Yes);
        flow.cancel();
        assert_eq!(watch.answer(), Wanted::No, "cancel stops the worker");

        // Superseded by a second Capture.
        let stop = StopOnDrop::new();
        let watch = stop.watch();
        let mut flow = Flow::Waiting;
        flow.start(1, "u".into(), stop);
        flow.start(2, "v".into(), StopOnDrop::new());
        assert_eq!(watch.answer(), Wanted::No, "the first run is abandoned");

        // The worker finishing.
        let stop = StopOnDrop::new();
        let watch = stop.watch();
        let mut flow = Flow::Waiting;
        flow.start(1, "u".into(), stop);
        flow.deliver(1, Message::Done(Outcome::Captured(page("https://x/1"))));
        assert_eq!(watch.answer(), Wanted::No);

        // The screen going away, which Rinch performs by dropping the signal's
        // value when the scope is disposed.
        let stop = StopOnDrop::new();
        let watch = stop.watch();
        let mut flow = Flow::Waiting;
        flow.start(1, "u".into(), stop);
        drop(flow);
        assert_eq!(watch.answer(), Wanted::No);
    }

    /// The bug this guards against is subtle and was designed out rather than
    /// found: if a progress tick rebuilt the `Running` state instead of
    /// mutating it, the state's own `StopOnDrop` would be dropped and the
    /// capture would cancel itself on its first report.
    #[test]
    fn a_progress_tick_does_not_cancel_the_capture_reporting_it() {
        let stop = StopOnDrop::new();
        let watch = stop.watch();
        let mut flow = Flow::Waiting;
        flow.start(1, "u".into(), stop);

        for step in [
            Progress::Stripping { bytes: 1_000 },
            Progress::Images { done: 0, total: 3, bytes: 1_000 },
            Progress::Images { done: 2, total: 3, bytes: 9_000 },
        ] {
            assert_eq!(flow.deliver(1, Message::Step(step)), Delivery::Applied);
            assert_eq!(watch.answer(), Wanted::Yes, "still wanted after {step:?}");
        }
    }

    // ── the checklist and the numbers on it ─────────────────────────────────

    #[test]
    fn the_checklist_reads_1l_and_advances_a_line_at_a_time() {
        let mut flow = running();
        assert_eq!(
            checklist(&flow),
            vec![
                ("Fetched page".into(), Tick::Doing),
                ("Stripped ads & scripts".into(), Tick::Waiting),
                ("Images".into(), Tick::Waiting),
            ]
        );

        flow.deliver(1, Message::Step(Progress::Stripping { bytes: 4_000 }));
        assert_eq!(
            checklist(&flow),
            vec![
                ("Fetched page".into(), Tick::Done),
                ("Stripped ads & scripts".into(), Tick::Doing),
                ("Images".into(), Tick::Waiting),
            ]
        );

        flow.deliver(
            1,
            Message::Step(Progress::Images { done: 3, total: 7, bytes: 1_200_000 }),
        );
        assert_eq!(
            checklist(&flow),
            vec![
                ("Fetched page".into(), Tick::Done),
                ("Stripped ads & scripts".into(), Tick::Done),
                // The wireframe's own words, from the engine's own counts.
                ("Downloading images (3 of 7)".into(), Tick::Doing),
            ]
        );
    }

    /// The line the wireframe put under the bar, with a real number in it.
    #[test]
    fn the_byte_counter_is_the_wireframes_line_and_the_engines_number() {
        let mut flow = running();
        flow.deliver(
            1,
            Message::Step(Progress::Images { done: 3, total: 7, bytes: 1_200_000 }),
        );
        assert_eq!(counter(&flow), "1.2 MB so far · will work with no signal");
    }

    /// A chord site with no images at all is the common case, not an edge one —
    /// CifraClub captures perfectly with zero. The checklist must not sit on
    /// "Downloading images (0 of 0)" forever.
    #[test]
    fn a_page_with_no_images_finishes_its_checklist() {
        let mut flow = running();
        flow.deliver(1, Message::Step(Progress::Images { done: 0, total: 0, bytes: 500 }));
        assert_eq!(checklist(&flow)[2], ("No images on this page".into(), Tick::Done));
        assert_eq!(bar(&flow), 1.0);
    }

    #[test]
    fn a_partial_capture_says_how_many_images_are_missing() {
        let mut flow = running();
        let mut partial = page("https://x/1");
        partial.missed = vec![Missed::new("https://x/a.png", "the site answered 404")];
        flow.deliver(1, Message::Done(Outcome::Partial(partial)));

        assert_eq!(checklist(&flow)[2], ("Downloaded 0 of 1 image".into(), Tick::Done));
        assert!(verdict_of(&flow).contains("1 image did not come down"), "{}", verdict_of(&flow));
        assert!(flow.attachable().is_some(), "a chart with a missing photo is still the chart");
    }

    #[test]
    fn the_bar_only_moves_forwards() {
        let steps = [
            Progress::Fetching,
            Progress::Stripping { bytes: 4_000 },
            Progress::Images { done: 0, total: 7, bytes: 4_000 },
            Progress::Images { done: 3, total: 7, bytes: 900_000 },
            Progress::Images { done: 7, total: 7, bytes: 1_200_000 },
        ];
        let mut previous = -1.0;
        for step in steps {
            let now = bar_fraction(step);
            assert!(now > previous, "{step:?} went backwards: {now} after {previous}");
            assert!((0.0..=1.0).contains(&now), "{step:?} is off the bar: {now}");
            previous = now;
        }
        assert_eq!(bar_fraction(steps[4]), 1.0, "and it reaches the end");
    }

    // ── the name and the preview ────────────────────────────────────────────

    #[test]
    fn a_page_is_named_after_its_title_collapsed_and_cut() {
        let mut p = page("https://x/1");
        assert_eq!(page_title(&p), "A Song — Example Tabs");

        p.title = "  Bron-Yr-Aur   Stomp  ".to_string();
        assert_eq!(page_title(&p), "Bron-Yr-Aur Stomp");

        p.title = "Free Guitar Tabs, Chords And Lyrics For Everybody Everywhere".to_string();
        let cut = page_title(&p);
        assert!(cut.chars().count() <= TITLE_LIMIT + 1, "{cut}");
        assert!(cut.ends_with('…'));
        assert!(!cut.contains(" …"), "no space dangling before the ellipsis");
    }

    #[test]
    fn a_page_with_no_title_at_all_falls_back_to_a_plain_word() {
        let mut p = page("https://x/1");
        p.title = "   ".to_string();
        assert_eq!(page_title(&p), FALLBACK_TITLE);
    }

    /// The preview is the only thing standing between a redirect and a chart
    /// filed under the wrong song, so it has to show the words rather than a
    /// skeleton.
    #[test]
    fn the_preview_shows_the_first_lines_of_what_came_down() {
        let p = page("https://x/1");
        let lines = preview_lines(&p);
        assert_eq!(lines[0], "G       C");
        assert_eq!(lines[1], "Carolina in my mind");
        // The blank line between verses survives as a space, so the gap it
        // stands for does not collapse away.
        assert_eq!(lines[2], " ");
        assert!(lines.len() <= PREVIEW_LINES);
    }

    /// "Downloaded 1 images" is what the finished screen said out loud the
    /// first time it ran against a real site.
    #[test]
    fn one_image_is_an_image() {
        let mut flow = running();
        flow.deliver(1, Message::Step(Progress::Images { done: 1, total: 1, bytes: 66_000 }));
        assert_eq!(checklist(&flow)[2], ("Downloaded 1 image".into(), Tick::Done));

        let mut flow = running();
        flow.deliver(1, Message::Step(Progress::Images { done: 7, total: 7, bytes: 66_000 }));
        assert_eq!(checklist(&flow)[2], ("Downloaded 7 images".into(), Tick::Done));
    }

    // ── attaching ───────────────────────────────────────────────────────────

    /// Taking the capture out of the state is what keeps `attach_captured`
    /// outside `Signal::with`'s borrow — see `Flow::take_settled`. It is also
    /// the double-tap guard: after the take there is nothing left to attach.
    #[test]
    fn taking_the_capture_out_leaves_nothing_to_attach_twice() {
        let mut flow = running();
        flow.deliver(1, Message::Done(Outcome::Captured(page("https://x/1"))));

        let (run, url, outcome) = flow.take_settled().expect("a settled capture");
        assert_eq!(run, 1);
        assert_eq!(url, "https://tabs.example/song");
        assert!(outcome.page().is_some());
        assert!(flow.attachable().is_none(), "the second tap finds nothing");
        assert!(flow.take_settled().is_none(), "and takes nothing");
    }

    #[test]
    fn taking_from_a_state_that_is_not_settled_changes_nothing() {
        let mut flow = running();
        assert!(flow.take_settled().is_none());
        assert!(flow.is_running(), "the running capture was not disturbed");

        let mut flow = Flow::Attached { attachment: 7 };
        assert!(flow.take_settled().is_none());
        assert!(matches!(flow, Flow::Attached { attachment: 7 }), "{flow:?}");
    }

    /// A write that does not land costs the user nothing they had: the preview
    /// and the checklist come back, so there is something to try again with.
    #[test]
    fn a_failed_write_puts_the_capture_back() {
        let mut flow = running();
        flow.deliver(1, Message::Done(Outcome::Captured(page("https://x/1"))));
        let (run, url, outcome) = flow.take_settled().unwrap();

        flow.restore(run, url, outcome);
        assert!(flow.attachable().is_some());
        assert_eq!(primary_label(&flow), "Attach");
        assert_eq!(flow.url(), "https://tabs.example/song");
    }

    fn verdict_of(flow: &Flow) -> String {
        match flow {
            Flow::Settled { outcome, .. } => verdict(outcome),
            other => panic!("not settled: {other:?}"),
        }
    }
}
