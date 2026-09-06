//! The one place a captured page is drawn — card E5.
//!
//! Not a screen and not a route, which is why it is the only file in here that
//! `screens::mod` exports without a `Route` pointing at it. It is a component
//! because of the sentence the card asked for: *the card and the viewer should
//! agree; a captured page should look like the same object in both places, the
//! way a PDF does.* A PDF gets that for free — `pdf::pages` rasterises one PNG
//! and both screens draw the same file. A captured page has no such artefact,
//! so the agreement has to be a shared component instead: `song_detail` mounts
//! this in its card and `attachment_viewer` mounts it full-screen, and the only
//! thing they pass differently is how big it is.
//!
//! What it draws, in order:
//!
//! | State | Drawn |
//! | --- | --- |
//! | `page.html` renders | the page, through [`crate::capture::render`] |
//! | no `page.html`, but the row has extracted text | that text, through the same renderer |
//! | neither | one sentence saying which of the two it is |
//!
//! The middle row is not a consolation prize, it is the honest answer to a real
//! state. `Attachment::body` lives in the database and `page.html` lives in a
//! directory on a phone, so the two go missing independently: a library folder
//! deleted by a file manager, a restore that brought the database back and not
//! the files, an `--seed` run where there was never a directory at all. In
//! every one of those the user still has the words, and showing them beats
//! showing an error about a file they did not know existed. When the files were
//! *expected* — there is a library on this device and the page is not in it —
//! the muted line above the text says so, because that is a thing the reader can
//! act on and E6's re-check is the thing that would fix it.
//!
//! ## Why the markup goes in through `set_inner_html`
//!
//! The long version is `crate::capture::render`'s header. The short version is
//! that `rsx!` needs its tags at compile time and a stranger's page does not
//! have any, so the choice is between flattening the page to a list of blocks —
//! which loses inline flow, and inline flow is where the `<b>` around each chord
//! symbol lives — and handing Rinch markup. `NodeHandle::set_inner_html` is the
//! documented way to do the second, the nodes it builds are ordinary DOM nodes
//! that Stylo and Taffy treat like any other, and the fragment it is given is
//! one this app generated rather than one a website served.
//!
//! It is called **once, from a component body**, which is the same rule
//! `pdf::pages` wrote for rasterising and for the same reason: the work is a
//! parse and a walk, and a render closure re-runs on every redraw. A component
//! body runs once per mount. That is also what makes the viewer's zoom work —
//! `attachment_viewer` already rebuilds its scrolling box on every zoom step,
//! for a scroll-offset fault that has nothing to do with this, so a new
//! `base_px` arrives as a remount and the page is simply drawn again at the new
//! size. Measured at **1.8 ms** for the largest page `docs/CAPTURE.md` captured
//! (CifraClub, 177 KB), against the 38 ms `pdf::pages` spends turning one PDF
//! page on the same phone.
//!
//! ## E6: re-checking a saved page, without touching the promise above
//!
//! The toggle lives in Settings (`SettingsStore::recheck_saved_pages`, off by
//! default) and the check itself lives here, fired from [`RecheckNote`] —
//! **a component of its own**, not logic folded into [`CapturedPageView`].
//! That split matters because of the paragraph just above it: this file's
//! whole discipline is that the expensive parse happens once and nothing
//! mounted here reads a signal that could make it happen again. A network
//! answer arriving is exactly such a signal, so it is read by a second,
//! separate component instead — its own body, its own signal, its own
//! re-render — which leaves [`CapturedPageView`]'s one true parse alone.
//! [`RecheckNote`] only mounts when this call counts as *opening* the page
//! rather than glancing at it — see `CapturedPageView`'s `opened` for what
//! tells the two apart.
//!
//! Everything else about how the check runs — the worker thread, the
//! `Signal::update_send` hop back to the main thread, why a late answer after
//! the screen is gone is harmless — is the same shape `screens::capture`
//! documents for the original fetch, reused rather than re-invented. The one
//! new piece is [`should_recheck`]'s rate limit and [`classify`]'s "what does
//! changed even mean", both written as plain functions so `cargo test` can
//! drive every answer without a socket.

use rinch::prelude::*;

use crate::capture::render::{self, Page, Sizing};
use crate::capture::{self, CaptureMode, HttpFetcher, Limits, Outcome, Wanted};
use crate::model::{AttachmentId, Day, SongId};
use crate::store::{AttachmentsStore, NavStore, Route, SettingsStore, SongsStore};
use crate::theme::T_META_SMALL;

/// The sentence for a capture whose files are not on this device.
///
/// Present tense and no blame: the row is still in the library, the text may
/// still be readable underneath, and the thing that fixes it is E6's re-check
/// rather than anything the reader can do right now.
pub const FILES_GONE: &str = "The saved page is no longer on this device.";

/// The sentence for a capture with a `page.html` that came to nothing.
///
/// Distinct from [`FILES_GONE`] because the two are different faults with
/// different fixes — a deleted directory against a file that is there and holds
/// no page — and `song_detail::note` makes the same argument about giving every
/// empty state its own words rather than a shared "couldn't load".
pub const UNREADABLE: &str = "This saved page could not be read.";

/// Said when the element budget ran out before the page did.
///
/// Only ever seen in the viewer. The card is a preview by construction and
/// stops early on every page long enough to be worth capturing, so saying it
/// there would be saying it always.
pub const TRUNCATED: &str = "This page was too long to draw in full.";

/// What one mounting of this component decided to draw.
///
/// A plain value with a plain constructor, so `cargo test` can drive every
/// branch — including the two that need a directory on disk — without a window.
#[derive(Clone, Debug, PartialEq)]
pub enum Shown {
    /// The saved page itself.
    Page(Page),
    /// The extracted text. `gone` is true when there was a library to hold the
    /// files and they were not in it, which is what puts [`FILES_GONE`] above
    /// the text.
    Text { page: Page, gone: bool },
    /// Neither, and which of the two sentences says so.
    Nothing(&'static str),
}

impl Shown {
    /// The fragment to inject, if there is one.
    pub fn markup(&self) -> Option<&str> {
        match self {
            Shown::Page(page) | Shown::Text { page, .. } => Some(&page.markup),
            Shown::Nothing(_) => None,
        }
    }

    /// Whether the note belongs above the content rather than under it.
    ///
    /// Two different jobs, and putting both in the same place got one of them
    /// wrong. [`FILES_GONE`] explains *what you are looking at* — the words
    /// without the page — and on `:99` on 2026-08-28 it sat at the bottom of a
    /// two-thousand-pixel column of extracted text, which is a footnote nobody
    /// reaches to a question they had at the top. [`TRUNCATED`] is the opposite
    /// kind of sentence: it is about what came *after* what you can see, so it
    /// belongs where the page stops. [`UNREADABLE`] is the only thing on the
    /// screen and it does not matter which side it is on.
    pub fn note_before(&self) -> bool {
        !matches!(self, Shown::Page(_))
    }

    /// The muted line, above or under the content — see [`note_before`].
    ///
    /// [`note_before`]: Shown::note_before
    pub fn note(&self) -> Option<&'static str> {
        match self {
            Shown::Page(page) if page.truncated => Some(TRUNCATED),
            Shown::Page(_) => None,
            Shown::Text { gone: true, .. } => Some(FILES_GONE),
            Shown::Text { gone: false, .. } => None,
            Shown::Nothing(sentence) => Some(sentence),
        }
    }
}

/// Decide what to draw, reading the disk once.
///
/// Split out from the component so that the decision is testable and so that
/// both screens provably make it the same way. The `directory` is `None` in the
/// in-memory library the `--seed` screenshots and the tests run in — there was
/// never a file, so its absence is not a fault and no sentence is shown for it.
pub fn decide(
    directory: Option<&std::path::Path>,
    body: Option<&str>,
    sizing: Sizing,
    budget: usize,
) -> Shown {
    let saved = directory.and_then(render::read);
    if let Some(saved) = saved {
        let page = render::render(&saved, directory.unwrap_or(std::path::Path::new("")), sizing, budget);
        if !page.is_empty() {
            return Shown::Page(page);
        }
        // The file is there and nothing came out of it. Fall through to the
        // text, because a page with no markup this app will draw can still have
        // had its words extracted at capture time — and say `UNREADABLE` rather
        // than `FILES_GONE` if there is no text either, because the files are
        // exactly where they should be.
        let text = render::from_text(body.unwrap_or_default());
        if text.is_empty() {
            return Shown::Nothing(UNREADABLE);
        }
        return Shown::Text {
            page: text,
            gone: false,
        };
    }

    let text = render::from_text(body.unwrap_or_default());
    // `directory.is_some()` is the whole of "were the files expected": a
    // persistent library hands out a directory for every attachment whether or
    // not anything has been written into it, and an in-memory one hands out
    // none at all.
    let gone = directory.is_some();
    if text.is_empty() {
        return Shown::Nothing(if gone { FILES_GONE } else { UNREADABLE });
    }
    Shown::Text { page: text, gone }
}

/// A captured page, drawn at the size the caller asks for.
///
/// `base_px` is the type size the fragment's `em` lengths resolve against — see
/// `render`'s header for why every length in it is relative — and `column_px`
/// is how wide the host is, which is what an image is fitted to. `budget` is
/// `render::CARD_ELEMENTS` from the card and `render::VIEWER_ELEMENTS` from the
/// viewer.
#[component]
pub fn CapturedPageView(
    attachment: Option<AttachmentId>,
    base_px: Option<f32>,
    column_px: Option<u32>,
    budget: Option<usize>,
    style: Option<String>,
) -> NodeHandle {
    let attachments = use_store::<AttachmentsStore>();
    let songs = use_store::<SongsStore>();
    let id = attachment.unwrap_or_default();
    let sizing = Sizing {
        base_px: base_px.unwrap_or(13.0),
        column_px: column_px.unwrap_or_else(|| crate::platform::viewport_width() as u32),
    };
    let budget = budget.unwrap_or(render::VIEWER_ELEMENTS);

    // Read and render once, here in the body, never in a render closure. Both
    // halves are expensive by this screen's standards — `body` is an object
    // read off the database and the render is a full html5ever parse — and
    // neither can change while this component is mounted: a captured page is a
    // file that was written once and a row that nothing edits.
    let shown = decide(
        attachments.directory(id).as_deref(),
        attachments.body(id).as_deref(),
        sizing,
        budget,
    );

    // Card E6: does this mount count as *opening* the page? `song_detail`'s
    // two card previews are the only callers in the app that pass
    // `render::CARD_ELEMENTS`, and they are glances at a library row, not an
    // open — the row's own footer says "Tap to open full screen" about
    // exactly this content. Reading `budget` for the answer rather than
    // adding a second prop that would mean the same thing keeps this
    // component honest about the rule its own header states: one number
    // already answers "how much of the page", and a fresh prop asking a
    // second, correlated question would be two knobs a caller could turn out
    // of step with each other. See `RecheckNote` for what "opened" unlocks.
    let opened = budget != render::CARD_ELEMENTS;

    let outer = format!(
        "display: flex; flex-direction: column; min-width: 0; {}",
        style.unwrap_or_default()
    );
    // The type size lives on the host and everything under it is relative to
    // this one number. `min-width: 0` is what lets a `<pre>` wider than the
    // column scroll inside itself instead of widening the whole screen.
    let host = format!(
        "font-size: {:.2}px; line-height: 1.5; min-width: 0;",
        sizing.base_px
    );
    let markup = shown.markup().unwrap_or_default().to_string();
    let note = shown.note();
    let before = shown.note_before();

    rsx! {
        div {
            style: {outer.clone()},
            if let Some(sentence) = note.filter(|_| before) {
                div {
                    style: {format!("{T_META_SMALL} margin-bottom: 6px;")},
                    {sentence}
                }
            }
            {injected(__scope, markup.clone(), host.clone())}
            if let Some(sentence) = note.filter(|_| !before) {
                div {
                    style: {format!("{T_META_SMALL} margin-top: 6px;")},
                    {sentence}
                }
            }
            if opened {
                RecheckNote { attachment: {id}, song: {song_of(songs, id)} }
            }
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Card E6: re-checking a saved page
// ────────────────────────────────────────────────────────────────────────────

/// What a re-check found, or why it never asked.
///
/// [`Quiet`](Self::Quiet) covers three different reasons — the toggle is off,
/// today's check already happened, there is nothing on this device to
/// compare against — and they are deliberately not distinguished any further
/// than that. A user who never turned the toggle on should see exactly
/// nothing, the same nothing a user sees whose page was already checked an
/// hour ago; a feature that stayed silent for one reason and hinted at
/// itself for another would be explaining its own plumbing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecheckState {
    Quiet,
    Unchanged,
    Changed,
    CouldNotCheck,
}

/// Said when a re-check found the site still matches what was saved.
///
/// Worth saying even though nothing happened, which is the card's own
/// argument: "checked, still the same" is the reassurance this feature exists
/// to give, not a no-op not worth mentioning.
pub const RECHECK_UNCHANGED: &str = "Checked — the site still matches what was saved.";

/// Said when the freshly fetched text does not match `Attachment::body`.
/// Paired on screen with a "Re-capture" action — never an automatic
/// replacement, which point 2 of the card rules out by name.
pub const RECHECK_CHANGED: &str = "The site has changed since this page was saved.";

/// Said when the fetch could not be completed — no connection, a timeout, the
/// site refusing every request today. Deliberately the same one sentence for
/// all of those: the reader has no way to act on the difference between a DNS
/// failure and a 403, and a saved page that still opens and still reads fine
/// must not be made to sound broken over a fetch it never needed in the first
/// place. See [`classify`] for why `Blocked` with no page lands here too.
pub const RECHECK_COULD_NOT_CHECK: &str = "Could not reach the site to check for changes.";

/// The line to show for a given [`RecheckState`], or nothing for
/// [`RecheckState::Quiet`].
pub fn recheck_sentence(state: RecheckState) -> Option<&'static str> {
    match state {
        RecheckState::Quiet => None,
        RecheckState::Unchanged => Some(RECHECK_UNCHANGED),
        RecheckState::Changed => Some(RECHECK_CHANGED),
        RecheckState::CouldNotCheck => Some(RECHECK_COULD_NOT_CHECK),
    }
}

/// Card E6's rate limit: **once a calendar day**, the same `Day` granularity
/// `Attachment::captured_at` already uses.
///
/// A day is generous in both directions. It is long enough that a rehearsal
/// spent opening the same song a dozen times over an evening costs one
/// request rather than a dozen — the card's own worry about a phone that
/// "wakes up and re-fetches thirty pages", scaled down to the one-page,
/// user-triggered version of the same mistake. It is short enough that a
/// chart the site actually changed is caught within days of the next
/// rehearsal rather than never, since nothing here sweeps a library that
/// stays closed.
///
/// `!=` rather than `<`: a clock that jumped backwards — a phone with no
/// network to sync against, coming back from a wrong-dated reboot — should
/// not be able to wedge a page into "already checked today" forever. The
/// worst `!=` can do with a skewed clock is check again sooner than once a
/// day, which is exactly the direction that costs nothing but a spare
/// request.
pub fn should_recheck(enabled: bool, rechecked_at: Option<Day>, today: Day) -> bool {
    enabled && rechecked_at != Some(today)
}

/// Card E6's classification: does the page just fetched match what is saved?
///
/// Reads [`Outcome::page`] rather than matching every variant by hand — the
/// same helper E4's screens use, because `Captured`, `Partial` and a
/// `Blocked` that still came with a page (a paywall's own teaser) all have
/// text worth comparing, and letting the three of them share one branch here
/// is exactly what that method is for. Nothing *without* a page — `Failed`, a
/// `Blocked` that was refused at the door, `Cancelled` (unreachable in
/// practice: this check never answers `Wanted::No`) — has any text to
/// compare, and all of them read the same to somebody with no way to act on
/// which one happened: [`RecheckState::CouldNotCheck`].
///
/// Compared trimmed rather than byte-for-byte: the extraction is
/// deterministic over identical bytes, so trimming only forgives the one kind
/// of drift worth forgiving — a site that changed nothing but its own
/// trailing whitespace — without so much as touching what counts as a real
/// change.
pub fn classify(outcome: &Outcome, stored_body: &str) -> RecheckState {
    match outcome.page() {
        Some(page) if page.text.trim() == stored_body.trim() => RecheckState::Unchanged,
        Some(_) => RecheckState::Changed,
        None => RecheckState::CouldNotCheck,
    }
}

/// The song a captured page belongs to, for the "Re-capture" action's route.
///
/// `AttachmentsStore` deliberately does not know this — its own header says
/// so: "there is nothing an attachment needs to know about a song." So this
/// asks `SongsStore` instead, which is where the ownership actually lives
/// (`Song::attachments`). A library runs to dozens or a few hundred songs,
/// not thousands, and this runs once per mount of an *opened* page rather
/// than once per redraw, so a linear scan costs nothing worth a second index
/// kept only for this.
fn song_of(songs: SongsStore, id: AttachmentId) -> Option<SongId> {
    songs
        .songs
        .get()
        .iter()
        .find(|s| s.attachments.contains(&id))
        .map(|s| s.id)
}

/// Which mode a re-check has to ask for, read off the saved file rather than
/// re-derived — see `capture::mode_of`'s own header for why the mode is
/// provenance baked into `page.html` rather than a database column, and why
/// re-fetching in a different mode than the one saved would diff apples
/// against oranges.
///
/// `None` when there is nothing on this device to read: an in-memory library
/// (`--seed`, every test), the files deleted out from under the row — the
/// exact case [`FILES_GONE`] already names — or, unreachably outside a
/// backup from before card E3, a `page.html` with no mode comment in it at
/// all.
fn recheck_mode(directory: Option<&std::path::Path>) -> Option<CaptureMode> {
    capture::mode_of(&render::read(directory?)?)
}

/// Everything a re-check needs before it can even try: the address to ask,
/// the mode to ask for it in, and the text already on file to compare the
/// answer against.
///
/// A struct rather than a tuple so the call site at [`maybe_recheck`] reads as
/// prose, and pulled out of that function so this half — "is there enough
/// here to attempt a check at all" — can be asserted on its own, with a real
/// directory and a real file on disk, without a thread or a socket anywhere
/// near the test.
struct RecheckTarget {
    url: String,
    mode: CaptureMode,
    stored_body: String,
}

fn recheck_target(attachments: AttachmentsStore, id: AttachmentId) -> Option<RecheckTarget> {
    let attachment = attachments.get(id)?;
    let mode = recheck_mode(attachments.directory(id).as_deref())?;
    let stored_body = attachments.body(id)?;
    let url = attachment.source_url?;
    Some(RecheckTarget { url, mode, stored_body })
}

/// The re-check thread's stack. Same size and same reason as
/// `screens::capture::WORKER_STACK`: this runs the identical `capture()`
/// engine, over the identical worry — a stranger's markup recursing through
/// `dom::walk` with no obligation to be shallow.
#[cfg(not(target_arch = "wasm32"))]
const RECHECK_STACK: usize = 4 * 1024 * 1024;

/// Fired once per mount of an *opened* [`CapturedPageView`] — see
/// [`RecheckNote`], its only caller.
///
/// The rate-limit stamp is written **before** the fetch starts and
/// regardless of what it finds, including "could not reach the site" — see
/// [`should_recheck`]'s own header for why an attempted-but-offline check
/// still has to count. Nothing is stamped when [`recheck_target`] comes back
/// empty, because a page with no directory, no mode or no address on file is
/// not a check that ran and found nothing to say; it is a check that could
/// not even be attempted, and stamping it would mean a library restored
/// without its files (or without a network yet) never gets tried again once
/// the files, or the network, come back.
fn maybe_recheck(
    settings: SettingsStore,
    attachments: AttachmentsStore,
    id: AttachmentId,
    state: Signal<RecheckState>,
) {
    let Some(attachment) = attachments.get(id) else {
        return;
    };
    if !should_recheck(
        settings.recheck_saved_pages.get(),
        attachment.rechecked_at,
        Day::today(),
    ) {
        return;
    }
    let Some(target) = recheck_target(attachments, id) else {
        return;
    };
    attachments.update(id, |a| a.rechecked_at = Some(Day::today()));

    #[cfg(not(target_arch = "wasm32"))]
    spawn_recheck(target, state);
}

/// The worker. Same shape `screens::capture::spawn_capture` documents in
/// full: off the main thread because `rinch-http`'s timeouts are 30s/60s and
/// nobody should hold a frame that long; delivered back through
/// [`Signal::update_send`], which is what makes a late answer harmless — a
/// signal whose scope has been disposed by the time this runs (the reader
/// left the page before the network answered) is a warn-once no-op on write,
/// never a panic, the same property `picker.rs`'s header records for its own
/// callback. There is no cancel token here the way `screens::capture` has
/// one: nobody pressed anything to start this, so there is nothing on screen
/// for a Cancel to mean, and the progress callback always answers
/// [`Wanted::Yes`] — a check nobody asked for cannot be too slow to wait for,
/// only too slow to matter, and by the time it answers this component may
/// simply no longer be there to hear it.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_recheck(target: RecheckTarget, state: Signal<RecheckState>) {
    let _ = std::thread::Builder::new()
        .name("sla-recheck".into())
        .stack_size(RECHECK_STACK)
        .spawn(move || {
            let limits = Limits::default();
            let fetcher = HttpFetcher::new(&limits);
            let outcome = capture::capture(&target.url, target.mode, &limits, &fetcher, |_| {
                Wanted::Yes
            });
            let classified = classify(&outcome, &target.stored_body);
            state.update_send(move |current| *current = classified);
        });
    // A device that will not hand this a thread gets exactly what a device
    // with no connection gets: silence. `Flow::no_worker` exists in
    // `screens::capture` because a user pressed Capture and is owed a screen
    // that says the attempt did not even start; nobody pressed anything to
    // start this one, so there is nobody owed that sentence — see point 5's
    // "no-op, not a failure", which this generalises to "no-op, not a
    // failure, not even at the level of finding a thread to run on."
}

/// Card E6's line under an opened captured page. A component of its own —
/// see the module header's "E6" section for why the split from
/// [`CapturedPageView`] is load-bearing rather than tidiness.
#[component]
fn RecheckNote(attachment: AttachmentId, song: Option<SongId>) -> NodeHandle {
    let settings = use_store::<SettingsStore>();
    let attachments = use_store::<AttachmentsStore>();
    let nav = use_store::<NavStore>();

    let state = Signal::new(RecheckState::Quiet);
    maybe_recheck(settings, attachments, attachment, state);

    // Wrapped in a div that is always mounted, even with nothing inside it —
    // `Quiet` is the common case (the toggle off, or today's check already
    // done) and an empty, unstyled div costs no space and draws nothing, so
    // this stays out of the way exactly as completely as drawing nothing at
    // all would.
    rsx! {
        div {
            if let Some(sentence) = recheck_sentence(state.get()) {
                div {
                    style: {format!(
                        "{T_META_SMALL} margin-top: 6px; color: var(--sla-muted); \
                         display: flex; align-items: center; gap: 8px;"
                    )},
                    span { {sentence} }
                    // Only ever drawn for `Changed`, and only when the
                    // attachment still belongs to a song — the ordinary case,
                    // and the one this action needs to build its route. Never
                    // auto-replaces anything: this is a link to the same
                    // screen E2/E4 already own, carrying the song and the URL
                    // the way `CaptureWebpage` always has, and the saved copy
                    // underneath is untouched whether or not anybody ever
                    // taps it.
                    if let Some(song) = song.filter(|_| state.get() == RecheckState::Changed) {
                        span {
                            onclick: move || nav.go(Route::CaptureWebpage { song }),
                            style: "color: var(--sla-accent); font-weight: 600; cursor: pointer;",
                            "Re-capture"
                        }
                    }
                }
            }
        }
    }
}

/// The host element, with the fragment already inside it.
///
/// The one imperative call in the app, and it is the framework's own escape
/// hatch rather than a way round `rsx!`: `rsx!` builds the element, this hands
/// its children over as markup, and everything Rinch does with them afterwards
/// — the cascade, the layout, the inline flow — is what it does with any other
/// node. An empty fragment is still injected, which is a no-op that clears
/// nothing, so there is no branch here for the case with no page.
///
/// Visible to the crate because card E3's capture screen previews a page that
/// is not an attachment yet — there is no `AttachmentId` for it and no
/// directory behind it — so it cannot mount [`CapturedPageView`] and reaches
/// for the host instead. That keeps the "one place decides what a captured page
/// looks like" rule the module header states: the preview and the library draw
/// the same fragment through the same element.
pub(crate) fn injected(scope: &mut RenderScope, markup: String, style: String) -> NodeHandle {
    let __scope = scope;
    let host = rsx! { div { style: {style.clone()} } };
    host.set_inner_html(&markup);
    host
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn sizing() -> Sizing {
        Sizing {
            base_px: 13.0,
            column_px: 313,
        }
    }

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sla-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn decided(directory: Option<&Path>, body: Option<&str>) -> Shown {
        decide(directory, body, sizing(), render::VIEWER_ELEMENTS)
    }

    #[test]
    fn a_saved_page_is_drawn_in_preference_to_the_text_beside_it() {
        let dir = tempdir("captured-page");
        std::fs::write(dir.join("page.html"), "<body><pre>G  C\nline</pre></body>").expect("write");
        let shown = decided(Some(&dir), Some("the extracted text"));
        let Shown::Page(page) = &shown else {
            panic!("expected the page, got {shown:?}");
        };
        assert!(page.markup.contains("G  C\nline"));
        assert!(!page.markup.contains("the extracted text"));
        assert_eq!(shown.note(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The deletion case the card asks about: the row is still in the library
    /// and the files under it are not.
    #[test]
    fn files_deleted_underneath_fall_back_to_the_text_and_say_so() {
        let dir = tempdir("captured-gone");
        let shown = decided(Some(&dir), Some("Verse one\nVerse two"));
        let Shown::Text { page, gone } = &shown else {
            panic!("expected the text, got {shown:?}");
        };
        assert!(*gone);
        assert!(page.markup.contains("Verse one"));
        assert_eq!(shown.note(), Some(FILES_GONE));
        assert!(
            shown.note_before(),
            "it explains what the reader is looking at, so it goes first"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// In-memory mode — `--seed`, and every test in this crate that builds a
    /// library without a repository. There was never a file, so nothing is
    /// missing and nothing is said.
    #[test]
    fn a_library_with_no_directories_shows_the_text_and_no_complaint() {
        let shown = decided(None, Some("Verse one"));
        let Shown::Text { gone, .. } = &shown else {
            panic!("expected the text, got {shown:?}");
        };
        assert!(!*gone);
        assert_eq!(shown.note(), None);
    }

    #[test]
    fn a_page_that_will_not_parse_into_anything_says_which_fault_it_is() {
        let dir = tempdir("captured-unreadable");
        // Well-formed, and every element in it is one the app drops.
        std::fs::write(dir.join("page.html"), "<html><head><title>x</title></head></html>")
            .expect("write");
        assert_eq!(decided(Some(&dir), None), Shown::Nothing(UNREADABLE));
        assert_eq!(decided(Some(&dir), None).note(), Some(UNREADABLE));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nothing_at_all_names_the_files_rather_than_the_parse() {
        let dir = tempdir("captured-empty");
        assert_eq!(decided(Some(&dir), None), Shown::Nothing(FILES_GONE));
        assert_eq!(decided(None, None), Shown::Nothing(UNREADABLE));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A capture written before this card existed has a `page.html` and no
    /// extracted text; one written by an older build that only kept the text
    /// has the text and no file. Both draw something.
    #[test]
    fn either_half_of_a_capture_is_enough_to_draw_it() {
        let dir = tempdir("captured-half");
        std::fs::write(dir.join("page.html"), "<body><p>from the file</p></body>").expect("write");
        assert!(matches!(decided(Some(&dir), None), Shown::Page(_)));
        std::fs::remove_file(dir.join("page.html")).expect("remove");
        assert!(matches!(
            decided(Some(&dir), Some("from the row")),
            Shown::Text { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_page_too_long_for_the_budget_says_so_and_still_draws() {
        let dir = tempdir("captured-long");
        let body: String = (0..80).map(|n| format!("<p>line {n}</p>")).collect();
        std::fs::write(dir.join("page.html"), format!("<body>{body}</body>")).expect("write");
        let shown = decide(Some(&dir), None, sizing(), 12);
        assert!(matches!(shown, Shown::Page(_)));
        assert_eq!(shown.note(), Some(TRUNCATED));
        assert!(
            !shown.note_before(),
            "it is about what came after, so it goes where the page stops"
        );
        assert!(shown.markup().unwrap().contains("line 0"));
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── E6: re-checking a saved page ────────────────────────────────────────

    use crate::capture::{Alternate, BlockReason, CapturedPage, Failure, Signals, Stripped};
    use crate::db::scratch;
    use crate::model::{Attachment, AttachmentKind, Song};
    use crate::store::{SetlistsStore, Storage};

    fn today() -> Day {
        Day::today()
    }

    fn yesterday() -> Day {
        Day::from_days_since_epoch(today().days_since_epoch() - 1)
    }

    // ── should_recheck: the rate limit ──────────────────────────────────────

    #[test]
    fn the_toggle_off_means_no_is_the_answer_no_matter_what_else_is_true() {
        assert!(!should_recheck(false, None, today()));
        assert!(!should_recheck(false, Some(yesterday()), today()));
    }

    #[test]
    fn a_page_never_checked_before_is_due_the_moment_the_toggle_is_on() {
        assert!(should_recheck(true, None, today()));
    }

    #[test]
    fn a_page_already_checked_today_is_not_checked_again() {
        assert!(!should_recheck(true, Some(today()), today()));
    }

    #[test]
    fn a_page_last_checked_yesterday_is_due_again() {
        assert!(should_recheck(true, Some(yesterday()), today()));
    }

    #[test]
    fn a_clock_that_jumped_backwards_does_not_wedge_a_page_forever() {
        // `today` reads as being *before* the recorded `rechecked_at` — the
        // clock skew case `should_recheck`'s own header argues for `!=`
        // rather than `<`. The worst this does is check a day early; a `<`
        // comparison would instead refuse to check again until the clock
        // caught back up, which could be never.
        let tomorrow = Day::from_days_since_epoch(today().days_since_epoch() + 1);
        assert!(should_recheck(true, Some(tomorrow), today()));
    }

    // ── classify: what "changed" means ──────────────────────────────────────

    fn page_with_text(text: &str) -> CapturedPage {
        CapturedPage {
            url: "https://tabs.example/song".to_string(),
            title: "A Song".to_string(),
            mode: CaptureMode::Reader,
            html: "<html></html>".to_string(),
            text: text.to_string(),
            alternate: None::<Alternate>,
            assets: Vec::new(),
            missed: Vec::new(),
            stripped: Stripped::default(),
            signals: Signals::default(),
            fetched_bytes: 1_000,
            reader_fell_back: false,
        }
    }

    #[test]
    fn identical_text_is_unchanged() {
        let outcome = Outcome::Captured(page_with_text("G  C\nCarolina in my mind"));
        assert_eq!(
            classify(&outcome, "G  C\nCarolina in my mind"),
            RecheckState::Unchanged
        );
    }

    #[test]
    fn only_the_trailing_whitespace_differing_still_reads_as_unchanged() {
        let outcome = Outcome::Captured(page_with_text("G  C\nCarolina in my mind\n\n  "));
        assert_eq!(
            classify(&outcome, "G  C\nCarolina in my mind"),
            RecheckState::Unchanged
        );
    }

    #[test]
    fn different_text_is_changed() {
        let outcome = Outcome::Captured(page_with_text("a whole new set of words"));
        assert_eq!(classify(&outcome, "the old words"), RecheckState::Changed);
    }

    #[test]
    fn a_partial_capture_with_the_same_text_is_still_unchanged() {
        // A few missing images must not read as a content change — G2's own
        // field, `body`, never held the images in the first place.
        let mut partial = page_with_text("same words either way");
        partial.missed = vec![crate::capture::Missed::new("https://x/img.png", "timed out")];
        let outcome = Outcome::Partial(partial);
        assert_eq!(
            classify(&outcome, "same words either way"),
            RecheckState::Unchanged
        );
    }

    #[test]
    fn a_paywalled_teaser_with_different_text_is_changed_not_could_not_check() {
        let outcome = Outcome::Blocked {
            reason: BlockReason::Paywalled,
            page: Some(Box::new(page_with_text("subscribe to keep reading"))),
        };
        assert_eq!(
            classify(&outcome, "G  C\nCarolina in my mind"),
            RecheckState::Changed
        );
    }

    #[test]
    fn a_wall_with_no_page_behind_it_could_not_be_checked() {
        let outcome = Outcome::Blocked {
            reason: BlockReason::Challenged,
            page: None,
        };
        assert_eq!(classify(&outcome, "anything"), RecheckState::CouldNotCheck);
    }

    #[test]
    fn nothing_reaching_the_site_at_all_could_not_be_checked() {
        let outcome = Outcome::Failed(Failure::Unreachable("dns failure".into()));
        assert_eq!(classify(&outcome, "anything"), RecheckState::CouldNotCheck);
    }

    // ── the three sentences ──────────────────────────────────────────────────

    #[test]
    fn quiet_says_nothing() {
        assert_eq!(recheck_sentence(RecheckState::Quiet), None);
    }

    #[test]
    fn each_non_quiet_state_names_its_own_sentence() {
        assert_eq!(recheck_sentence(RecheckState::Unchanged), Some(RECHECK_UNCHANGED));
        assert_eq!(recheck_sentence(RecheckState::Changed), Some(RECHECK_CHANGED));
        assert_eq!(
            recheck_sentence(RecheckState::CouldNotCheck),
            Some(RECHECK_COULD_NOT_CHECK)
        );
    }

    // ── recheck_target: is there even enough to try ─────────────────────────

    /// One launch's worth of stores over a real, throwaway library — the same
    /// shape `store::storage`'s own `Session` test helper builds, minus the
    /// pieces this file's tests never touch.
    struct Library {
        songs: SongsStore,
        attachments: AttachmentsStore,
        settings: SettingsStore,
    }

    fn library(dir: &crate::db::DataDir) -> Library {
        let storage = Storage::open(dir);
        assert!(storage.is_persistent(), "a scratch dir must open for real");
        let loaded = storage.load();
        let attachments = AttachmentsStore::restored(storage, loaded.attachments);
        let setlists = SetlistsStore::restored(storage, loaded.setlists);
        let songs = SongsStore::restored(storage, attachments, setlists, loaded.songs);
        Library {
            songs,
            attachments,
            settings: SettingsStore::restored(storage),
        }
    }

    /// A `CapturedPage` attachment with everything E6 needs already on disk:
    /// a `page.html` carrying a mode comment, a stored `body`, and a
    /// `source_url` to re-fetch. Individual tests knock one of these away to
    /// prove `recheck_target` refuses to guess at the missing piece.
    fn captured_chart(songs: SongsStore, mode: CaptureMode) -> AttachmentId {
        let song = songs.add("Angel From Montgomery", "John Prine");
        let attachment = Attachment {
            id: 0,
            kind: AttachmentKind::CapturedPage,
            title: "Angel From Montgomery — chords".into(),
            bytes_on_disk: 900,
            page_count: None,
            source_url: Some("https://tabs.example/angel".into()),
            captured_at: Some(Day::new(2026, 5, 9)),
            body: Some("G  C\nCarolina in my mind".into()),
            rechecked_at: None,
        };
        let id = songs.attach(song, attachment).expect("attached");
        let directory = songs.attachments().directory(id).expect("persistent library");
        std::fs::write(
            directory.join(crate::capture::PAGE_FILE),
            format!(
                "<!doctype html>\n<!-- Captured by SetListArray ({}) from https://tabs.example/angel. -->\n<body>hi</body>\n",
                mode.name()
            ),
        )
        .expect("write page.html");
        id
    }

    #[test]
    fn a_fully_saved_page_has_everything_a_recheck_needs() {
        let dir = scratch("recheck-target-full");
        let lib = library(&dir);
        let id = captured_chart(lib.songs, CaptureMode::Reader);

        let target = recheck_target(lib.attachments, id).expect("everything is on disk");
        assert_eq!(target.url, "https://tabs.example/angel");
        assert_eq!(target.mode, CaptureMode::Reader);
        assert_eq!(target.stored_body, "G  C\nCarolina in my mind");
        std::fs::remove_dir_all(dir.path()).ok();
    }

    #[test]
    fn an_in_memory_library_has_no_directory_to_read_a_mode_from() {
        // `--seed` and every other test in this crate: `SongsStore::new`
        // never opens a repository, so `AttachmentsStore::directory` hands
        // back `None` for every id.
        let songs = SongsStore::new(vec![Song::new(1, "Song", "Artist")]);
        let attachments = songs.attachments();
        let id = attachments
            .items
            .get()
            .first()
            .map(|a| a.id)
            .unwrap_or_default();
        // No attachment at all yet, but the point stands either way: there is
        // no directory, so there is nothing to read a mode off of.
        assert!(recheck_target(attachments, id).is_none());
    }

    #[test]
    fn a_page_html_with_no_mode_comment_cannot_be_diffed_like_for_like() {
        let dir = scratch("recheck-target-no-mode");
        let lib = library(&dir);
        let id = captured_chart(lib.songs, CaptureMode::Reader);
        let directory = lib.attachments.directory(id).unwrap();
        // Overwrite with a file that has no "Captured by SetListArray (...)"
        // comment at all — a backup restored from before card E3, say.
        std::fs::write(directory.join(crate::capture::PAGE_FILE), "<body>no comment here</body>")
            .unwrap();

        assert!(recheck_target(lib.attachments, id).is_none());
        std::fs::remove_dir_all(dir.path()).ok();
    }

    // ── song_of ──────────────────────────────────────────────────────────────

    #[test]
    fn song_of_finds_the_song_that_owns_the_attachment() {
        let dir = scratch("recheck-song-of");
        let lib = library(&dir);
        let id = captured_chart(lib.songs, CaptureMode::Reader);

        let owner = lib.songs.get(1).map(|s| s.id);
        assert_eq!(song_of(lib.songs, id), owner);
        std::fs::remove_dir_all(dir.path()).ok();
    }

    // ── maybe_recheck: the card's own title ──────────────────────────────────

    #[test]
    fn with_the_toggle_off_nothing_is_ever_fetched() {
        let dir = scratch("recheck-toggle-off");
        let lib = library(&dir);
        let id = captured_chart(lib.songs, CaptureMode::Reader);
        // `SettingsStore::restored` defaults `recheck_saved_pages` to `false`
        // — this is the library's normal, untouched state.
        assert!(!lib.settings.recheck_saved_pages.get());

        maybe_recheck(lib.settings, lib.attachments, id, Signal::new(RecheckState::Quiet));

        // Nothing was even attempted: no stamp, which is the only thing that
        // could have happened on the main thread before a fetch nobody asked
        // for was allowed to start. A test that hung here, or took longer
        // than a function call, would itself be the proof this rule failed —
        // it does not, because `should_recheck` returns before either.
        assert_eq!(
            lib.attachments.get(id).unwrap().rechecked_at,
            None,
            "the toggle is off — this page must not have been touched at all"
        );
        std::fs::remove_dir_all(dir.path()).ok();
    }

    /// Stamping the rate-limit day is the one write `maybe_recheck` makes on
    /// the main thread before a fetch is even allowed to start, and it goes
    /// through the same `AttachmentsStore::update` that D4's page count and
    /// E2's byte count already use. That door is safe for exactly the reason
    /// `strip_body`'s own comment gives: the in-memory copy `update` reads
    /// back has already had its `body` stripped for a persistent library, and
    /// `attachment_fields`'s `put_some` skips a field entirely rather than
    /// writing it as empty when it is `None` — so the write this test drives
    /// never so much as mentions `body` to the database, and the extracted
    /// text stays exactly what `captured_chart` wrote.
    #[test]
    fn stamping_the_rate_limit_never_touches_the_saved_text() {
        let dir = scratch("recheck-stamp-preserves-body");
        let lib = library(&dir);
        let id = captured_chart(lib.songs, CaptureMode::Reader);
        lib.settings.set_recheck_saved_pages(true);

        // `recheck_target` will be `None` here — `example.invalid` in
        // `captured_chart` resolves nowhere on any network worth trusting a
        // test to reach — but the stamp happens before that is even asked,
        // so this exercises the one write this path makes without needing a
        // socket to fail politely.
        lib.attachments.update(id, |a| a.rechecked_at = Some(Day::today()));

        assert_eq!(
            lib.attachments.body(id).as_deref(),
            Some("G  C\nCarolina in my mind"),
            "the saved page's text must survive a metadata-only write untouched"
        );
        std::fs::remove_dir_all(dir.path()).ok();
    }

    #[test]
    fn an_in_memory_library_is_never_stamped_even_with_the_toggle_on() {
        let songs = SongsStore::new(Vec::new());
        let settings = SettingsStore::restored(Storage::in_memory());
        settings.set_recheck_saved_pages(true);
        let song = songs.add("Song", "Artist");
        let attachments = songs.attachments();
        let id = songs
            .attach(
                song,
                Attachment {
                    id: 0,
                    kind: AttachmentKind::CapturedPage,
                    title: "page".into(),
                    bytes_on_disk: 10,
                    page_count: None,
                    source_url: Some("https://tabs.example/x".into()),
                    captured_at: None,
                    body: Some("text".into()),
                    rechecked_at: None,
                },
            )
            .expect("attached");

        maybe_recheck(settings, attachments, id, Signal::new(RecheckState::Quiet));

        assert_eq!(
            attachments.get(id).unwrap().rechecked_at,
            None,
            "no directory means no mode to diff against, so nothing was attempted"
        );
    }
}
