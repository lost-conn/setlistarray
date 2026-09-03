//! Offline webpage capture — the feature the app exists for.
//!
//! Paste a URL, watch it come down, keep it forever. This module is the
//! engine: fetch, strip, download the images, decide whether what came back is
//! actually a chart, and hand back something [`write_into`] can put in an
//! attachment directory. The screens that drive it are cards E2–E6; nothing
//! here knows a [`Signal`](rinch::prelude::Signal) or a component, for the
//! same reason [`crate::db::repo`] does not: it makes the interesting half
//! testable without a window.
//!
//! ## The shape of a capture
//!
//! ```text
//!   fetch ──▶ parse ──▶ sanitise ──▶ judge ──▶ images ──▶ narrow ──▶ write
//!     │                     │           │         │         │
//!  Failure              Stripped    Outcome   Partial/   both readings
//!                                             Missed
//! ```
//!
//! **Judging happens before narrowing**, and that ordering is deliberate. The
//! question E4 has to answer is "is there a chart behind this URL", which is a
//! question about the page the site served, not about the fragment reader mode
//! chose to keep. A paywall notice sits in a banner that reader extraction
//! throws away; judging afterwards would lose it and save the teaser as though
//! it were the song.
//!
//! **Narrowing happens last, and produces a second reading rather than
//! replacing the first.** That is card E3's doing. The card asks for a preview
//! of what each mode keeps *before* anything is written, which is only
//! answerable if one fetch yields both — so `capture` serialises the full page,
//! re-parses it, narrows the copy, and hands back a [`CapturedPage`] carrying
//! one reading with the other in [`CapturedPage::alternate`]. E1 narrowed
//! before the images, which was cheaper by one masthead and made the second
//! reading impossible.
//!
//! ## The three failures E4 has to draw
//!
//! [`Outcome`] distinguishes them by construction, so the UI cannot forget one:
//!
//! | Outcome | What happened | E4 |
//! | --- | --- | --- |
//! | [`Captured`](Outcome::Captured) | Everything came down | — |
//! | [`Partial`](Outcome::Partial) | The page came down, some images did not | "partial capture" |
//! | [`Blocked`](Outcome::Blocked) | The page came down and is not a chart | "paywalled / JS-only" |
//! | [`Failed`](Outcome::Failed) | Nothing came down | "fetch failed" |
//! | [`Cancelled`](Outcome::Cancelled) | The caller said stop | — |
//!
//! `Blocked` still carries the page where there is one, because the user may
//! well want to keep a paywalled teaser — the app's job is to say what it got,
//! not to decide for them.
//!
//! `Cancelled` came with card E2, which needed Cancel to stop the *work* and
//! not merely stop the screen from watching. It is the answer to a
//! [`Wanted::No`] from the progress callback, it carries no page, and it is the
//! one outcome that is not a verdict on the site.
//!
//! ## The one place a site gets special treatment
//!
//! Card E7. [`site`] holds a small registry of per-site extractors, and
//! [`capture`] consults it in exactly one spot: the moment the generic engine
//! has already decided, from [`detect::verdict`], that a page is a
//! [`BlockReason::ScriptShell`] and is about to give up on it. Nowhere else —
//! a paywall or a plain no-chart page never reaches the registry, because
//! nowhere else is the generic path actually giving up. `site`'s own header
//! carries the argument against this existing at all and what happens on the
//! day Ultimate Guitar's markup changes underneath it; the short version is
//! that the fallback is silent and total, so a rotted extractor costs nothing
//! beyond the `Blocked(ScriptShell)` screen this app already draws.
//!
//! ## Where the bytes go
//!
//! Nothing is written until the whole capture is in hand, and then all of it
//! goes into one directory:
//!
//! ```text
//!   <data>/attachments/<id>/page.html
//!   <data>/attachments/<id>/assets/000.png
//! ```
//!
//! That is B4's convention as `crate::db::DataDir::attachment` already defines
//! it — `Repo::create_attachment` makes the directory and `delete_attachment`
//! removes it, so a capture inherits the lifecycle without a second one being
//! invented here. The rewritten `<img src>` is `assets/000.png`, relative, so
//! the file resolves wherever the library ends up: a desktop `$XDG_DATA_HOME`
//! today, `/data/data/<package>/files` on a phone, and whatever a restored
//! backup makes of it in Phase I.
//!
//! E2's sequence is therefore: capture into memory,
//! `SongsStore::attach` to mint an id and a directory, `write_into` that
//! directory, and `SongsStore::detach` again if the write fails. Capturing
//! first means a page that turns out to be a JavaScript shell never leaves a
//! row behind.
//!
//! Card D1 moved that first step. It used to be `AttachmentsStore::add`, which
//! wrote the row without telling the song about it; attaching is now the song's
//! operation, because the first chart a song gets becomes its primary one and
//! only the song can know that. `attach` takes the [`Attachment`] whole, so a
//! capture fills in `source_url`, `captured_at`, the extracted text as `body`
//! and its best guess at `bytes_on_disk`, then corrects the size through
//! `AttachmentsStore::update` once `write_into` has returned the real figure.
//!
//! [`Attachment`]: crate::model::Attachment

pub mod assets;
pub mod detect;
pub mod dom;
pub mod fetch;
pub mod reader;
// E5: turning a saved `page.html` back into something Rinch can lay out.
// Reads the attachment directory rather than the network, so it is the one
// file in here that runs long after a capture is over.
pub mod render;
pub mod sanitise;
pub mod site;

use std::io;
use std::path::Path;

pub use assets::Asset;
pub use detect::{BlockReason, Signals};
pub use fetch::{Fetched, Fetcher};
pub use sanitise::Stripped;

#[cfg(not(target_arch = "wasm32"))]
pub use fetch::HttpFetcher;

/// The file the page itself is written to, inside the attachment directory.
pub const PAGE_FILE: &str = "page.html";

/// What the user asked to keep — card E3's `Save as:`.
///
/// ## Reader is the default, and the measurement says so
///
/// E1 shipped this enum with `FullPage` as the default on the reasonable
/// assumption that fidelity is the safe choice: keep everything, decide later.
/// E3 captured the three sites `docs/CAPTURE.md` says the engine actually
/// works on, in both modes, and read what came out. The assumption does not
/// survive it.
///
/// The first words of a **full-page** capture, on all three:
///
/// | Site | What the saved page opens on |
/// | --- | --- |
/// | hymnal.net | `Login · Sign up · Follow us: · Classic · New Tunes · …` |
/// | cifraclub.com | `Skip to content · Home page · Search the website · Main menu` |
/// | guitaretab.com | `Add new tab · Help us to improve GuitareTab.com · Take our survey!` |
///
/// The same three in **reader** mode open on the hymn, on `[Intro] Em7 G`, and
/// on the first chord line of the tab. The only images full-page mode keeps on
/// any of them is the site's masthead — `hymnal.net/images/logo.png` and
/// `guitaretab.com/static/images/head.png` — so reader mode's zero images is
/// not a loss, it is the logo not being downloaded.
///
/// A user pasting a chord-site URL is the overwhelmingly common case, and for
/// that user full-page mode is a worse capture of the same fetch. So the
/// default is the one that answers the common case, and full page is the escape
/// hatch for when reader extraction picks the wrong node — which it can, it is
/// a heuristic, and it is why the screen previews both before anything is
/// written.
///
/// **A capture keeps both**, from one fetch: see [`CapturedPage::select`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptureMode {
    /// Just the chart, as this app reads the page: the article node, with any
    /// chord chart whose alignment lived in a dropped stylesheet rebuilt as
    /// preformatted text (see [`reader::rebuild_chord_blocks`]).
    ///
    /// Smaller, cleaner, and occasionally wrong — reader extraction is a
    /// heuristic, which is why E3 shows a preview before committing.
    #[default]
    Reader,
    /// The page as the site built it, minus everything that executes or phones
    /// home. Nothing is rewritten and nothing is chosen for the reader, which
    /// is the whole of what it offers over [`Reader`](CaptureMode::Reader) —
    /// and on a site whose chart was positioned in a linked stylesheet it is
    /// the honest answer that there is no fidelity left to keep.
    FullPage,
}

impl CaptureMode {
    /// The label on E3's `Save as:` control, and the words in the provenance
    /// comment at the top of a saved page. One list, so a saved file and the
    /// screen that saved it cannot come to call the same thing two names.
    pub fn label(self) -> &'static str {
        match self {
            CaptureMode::Reader => "Reader text",
            CaptureMode::FullPage => "Full page",
        }
    }

    /// What each mode keeps, in one line, for the control that offers it.
    pub fn explain(self) -> &'static str {
        match self {
            CaptureMode::Reader => "Just the chart. The site's menus and logo are dropped.",
            CaptureMode::FullPage => "The whole page, navigation and all, exactly as served.",
        }
    }

    /// The stable name that survives a restart. Written into
    /// `Preferences::capture_mode` and into a saved page's provenance comment.
    pub fn name(self) -> &'static str {
        match self {
            CaptureMode::Reader => "Reader",
            CaptureMode::FullPage => "FullPage",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Reader" => Some(CaptureMode::Reader),
            "FullPage" => Some(CaptureMode::FullPage),
            _ => None,
        }
    }

    /// The other one. There are two, and every control that offers them is a
    /// toggle underneath.
    pub fn other(self) -> Self {
        match self {
            CaptureMode::Reader => CaptureMode::FullPage,
            CaptureMode::FullPage => CaptureMode::Reader,
        }
    }
}

/// The ceilings a capture will not go past.
///
/// Two of the numbers that matter are **not** here because `rinch-http` does
/// not expose them — the 30s/60s timeouts and the 10 MiB hard body ceiling are
/// fixed inside that crate. [`Limits::max_page_bytes`] can only tighten the
/// second, and does so after the bytes are already in memory. See
/// [`fetch`] for the full list of what is inherited.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Refuse an HTML document larger than this. 4 MiB is roughly ten times
    /// the largest chord page measured in the E1 spike.
    pub max_page_bytes: u64,
    pub max_assets: usize,
    pub max_asset_bytes: u64,
    pub max_total_asset_bytes: u64,
    /// How long the whole capture may take, images included.
    ///
    /// This is ours because `rinch-http`'s timeout is *per request*: a page
    /// with twenty-four images gets twenty-four independent sixty-second
    /// budgets, and a slow CDN can hold a capture open for twenty minutes
    /// without any one request misbehaving. Measured in the spike: one sheet
    /// music page took 21s of wall clock for its images alone. Running out
    /// stops the downloads and yields a partial capture rather than a failure
    /// — the page itself is already in hand by then.
    pub deadline: std::time::Duration,
    /// What the app calls itself. Honest by default — see `docs/CAPTURE.md`
    /// for what that costs on the sites that filter on it.
    pub user_agent: String,
}

/// Truthful, and the shape a well-behaved crawler has used since the 1990s:
/// `Mozilla/5.0 (compatible; Name/Version; +where-to-complain)`. Sites that
/// filter user agents are looking for the absence of `Mozilla/5.0` more often
/// than for anything else, and this app is not going to claim to be Chrome.
pub const DEFAULT_USER_AGENT: &str = concat!(
    "Mozilla/5.0 (compatible; SetListArray/",
    env!("CARGO_PKG_VERSION"),
    "; +https://github.com/lostconnection/setlistarray) offline-chart-capture"
);

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_page_bytes: 4 * 1024 * 1024,
            max_assets: 24,
            max_asset_bytes: 2 * 1024 * 1024,
            max_total_asset_bytes: 8 * 1024 * 1024,
            deadline: std::time::Duration::from_secs(90),
            user_agent: DEFAULT_USER_AGENT.to_string(),
        }
    }
}

/// Nothing came back. Each variant is a different sentence on E4's screen,
/// which is why a 404 is not folded in with a DNS failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Not a URL this app can fetch at all.
    NotAUrl(String),
    /// The socket never got there: DNS, TLS, refused, timed out.
    Unreachable(String),
    /// The site answered, and the answer was no.
    Refused { status: u16 },
    /// A PDF, an image, a zip. Not something this module can capture — though
    /// it may well be something card K4's file import should take.
    NotHtml { content_type: String },
    /// Past a ceiling, ours or `rinch-http`'s.
    TooLarge(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::NotAUrl(m) => write!(f, "that is not a web address: {m}"),
            Failure::Unreachable(m) => write!(f, "could not reach the site: {m}"),
            Failure::Refused { status } => write!(f, "the site answered {status}"),
            Failure::NotHtml { content_type } => {
                write!(f, "that address is a {content_type}, not a webpage")
            }
            Failure::TooLarge(m) => write!(f, "the page is too big to keep: {m}"),
        }
    }
}

/// An image that did not make it. Not fatal, and named so the user can see
/// what is missing from what they kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missed {
    pub source: String,
    pub why: String,
}

impl Missed {
    pub fn new(source: &str, why: &str) -> Self {
        Self {
            source: source.to_string(),
            why: why.to_string(),
        }
    }
}

/// The reading of a fetch that is not currently selected.
///
/// It exists so that E3's `Save as:` can be changed *after* the capture with no
/// second download. Only the two strings differ between the modes; the images,
/// the verdict and the byte count of what came off the wire are the fetch's,
/// not the reading's.
#[derive(Debug, Clone)]
pub struct Alternate {
    pub mode: CaptureMode,
    pub html: String,
    pub text: String,
}

/// A capture, complete, in memory, and not yet on disk.
///
/// ## One fetch, both readings
///
/// `html`/`text` are whichever mode is *selected*; [`alternate`] is the other
/// one, and [`select`] swaps them. That is the shape card E3 needs and the
/// reason it is not "capture again in the other mode": the card asks for a
/// preview of what each mode keeps **before** committing, and a preview that
/// cost a second trip to the site would be a preview nobody looked at on a
/// train. Both readings come out of the same parsed document, so the second one
/// costs a re-parse and a serialise of bytes already in memory — measured at a
/// few milliseconds against the hundreds the fetch takes.
///
/// The price is paid in images rather than in requests: the assets are
/// downloaded for the full page, so a reader-mode capture fetches a masthead it
/// then does not keep. That is bounded by `Limits::max_total_asset_bytes`, it
/// was 8.8 KB on hymnal.net and 9.6 KB on guitaretab, and it is what buys the
/// user the ability to look at both answers. **Only the selected reading's
/// assets are written** — see [`write_into`] — so nothing unreferenced reaches
/// the library.
///
/// [`alternate`]: CapturedPage::alternate
/// [`select`]: CapturedPage::select
#[derive(Debug, Clone)]
pub struct CapturedPage {
    /// The URL the user pasted. Goes into `Attachment::source_url`.
    pub url: String,
    /// `<title>`, or the first `<h1>`. Goes into `Attachment::title`.
    pub title: String,
    /// Which reading `html` and `text` currently hold.
    pub mode: CaptureMode,
    /// Sanitised HTML with local asset paths. Written to [`PAGE_FILE`].
    pub html: String,
    /// Visible text. Goes into `Attachment::body`, which is what card G2's
    /// search inside attachments reads.
    pub text: String,
    /// The other reading of the same fetch, or `None` when there is only one —
    /// which is what reader extraction finding nothing to narrow to looks like.
    pub alternate: Option<Alternate>,
    /// Every image that came down, whichever reading references it.
    pub assets: Vec<Asset>,
    pub missed: Vec<Missed>,
    pub stripped: Stripped,
    pub signals: Signals,
    /// Bytes of HTML as the site served them, before any of this ran. The
    /// number E2's "1.2 MB so far" counts up to.
    pub fetched_bytes: u64,
    /// Reader mode was asked for and found nothing to narrow to, so the full
    /// page was kept instead. E3 says so rather than quietly obliging.
    pub reader_fell_back: bool,
}

impl CapturedPage {
    /// Switch which reading this capture is. `false` if the mode is not one
    /// this capture has — a page with no reader view asked for reader text.
    ///
    /// A swap rather than a clone, because the two strings are the largest
    /// things in the struct and there is no moment at which both need to be
    /// the selected one.
    pub fn select(&mut self, mode: CaptureMode) -> bool {
        if self.mode == mode {
            return true;
        }
        let Some(alternate) = self.alternate.as_mut() else {
            return false;
        };
        if alternate.mode != mode {
            return false;
        }
        std::mem::swap(&mut self.html, &mut alternate.html);
        std::mem::swap(&mut self.text, &mut alternate.text);
        alternate.mode = self.mode;
        self.mode = mode;
        true
    }

    /// Whether the other reading is there to switch to.
    pub fn has(&self, mode: CaptureMode) -> bool {
        self.mode == mode || self.alternate.as_ref().is_some_and(|a| a.mode == mode)
    }

    /// The markup a given mode would save, without switching to it. What E3's
    /// preview renders when the user opens the control to look.
    pub fn html_for(&self, mode: CaptureMode) -> Option<&str> {
        if self.mode == mode {
            return Some(&self.html);
        }
        self.alternate
            .as_ref()
            .filter(|a| a.mode == mode)
            .map(|a| a.html.as_str())
    }

    /// The images the selected reading actually references.
    ///
    /// Matching on the path rather than remembering which walk produced which
    /// image, because the path is what is *in* the markup: `assets::rewrite`
    /// wrote `assets/000.png` into the `src`, and a reading that dropped the
    /// element dropped the only mention of the file.
    pub fn kept_assets(&self) -> impl Iterator<Item = &Asset> {
        self.assets
            .iter()
            .filter(|asset| self.html.contains(&asset.path))
    }

    /// What [`write_into`] will put on disk **for the selected mode**. Goes into
    /// `Attachment::bytes_on_disk`.
    pub fn bytes_on_disk(&self) -> u64 {
        self.html.len() as u64 + self.kept_assets().map(|a| a.bytes.len() as u64).sum::<u64>()
    }

    /// What the *other* mode would take, so the control can price both. `None`
    /// when there is no other mode.
    pub fn bytes_for(&self, mode: CaptureMode) -> Option<u64> {
        let html = self.html_for(mode)?;
        let assets: u64 = self
            .assets
            .iter()
            .filter(|asset| html.contains(&asset.path))
            .map(|a| a.bytes.len() as u64)
            .sum();
        Some(html.len() as u64 + assets)
    }
}

#[derive(Debug, Clone)]
pub enum Outcome {
    Captured(CapturedPage),
    Partial(CapturedPage),
    Blocked {
        reason: BlockReason,
        /// `None` only when the site refused at the door — a 403 has no page
        /// behind it to keep.
        page: Option<Box<CapturedPage>>,
    },
    Failed(Failure),
    /// The caller said stop, and it was heard between two requests. Carries no
    /// page on purpose: the user asked for this not to happen, and half a
    /// capture handed back as though it were a result is exactly the lie the
    /// variant exists to prevent. Nothing is on disk either — a capture writes
    /// nothing until [`write_into`], which is what makes cancelling free.
    Cancelled,
}

impl Outcome {
    /// The page, whatever the verdict was. `Blocked` still has one when the
    /// site served bytes, and the user is allowed to keep them.
    pub fn page(&self) -> Option<&CapturedPage> {
        match self {
            Outcome::Captured(page) | Outcome::Partial(page) => Some(page),
            Outcome::Blocked { page, .. } => page.as_deref(),
            Outcome::Failed(_) | Outcome::Cancelled => None,
        }
    }

    /// The same page, to switch its mode through. E3's `Save as:` is the only
    /// caller: it changes which reading of an already-finished capture is the
    /// one that would be written, which is a change to the capture and not to
    /// the verdict wrapped around it.
    pub fn page_mut(&mut self) -> Option<&mut CapturedPage> {
        match self {
            Outcome::Captured(page) | Outcome::Partial(page) => Some(page),
            Outcome::Blocked { page, .. } => page.as_deref_mut(),
            Outcome::Failed(_) | Outcome::Cancelled => None,
        }
    }
}

/// The answer the caller gives every time the engine reports progress:
/// **is this capture still wanted?**
///
/// A blocking capture hands control back to its caller in exactly one place —
/// the progress callback — so that callback is the only place a Cancel can
/// possibly be heard. Card E2 wanted Cancel to mean something more than "the
/// screen stops looking", and this is the whole of the mechanism: the screen
/// answers [`Wanted::No`], the engine stops at its next checkpoint, and the
/// outcome is [`Outcome::Cancelled`].
///
/// What it is **not** is an abort. The checkpoints sit *between* HTTP
/// requests, because `rinch-http` exposes no way to stop one that is already
/// open (see [`fetch`]) — so a Cancel pressed while a socket is waiting stops
/// every request after this one and lets this one run to its timeout on a
/// thread nobody is listening to. That is honest and it is cheap: the bytes go
/// nowhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanted {
    Yes,
    No,
}

/// Where a capture has got to. E2's checklist reads these in order.
///
/// Every number on this enum is measured rather than estimated, which is the
/// point of it being here rather than on a timer in the screen: `done`/`total`
/// is E2's "3 of 7" and `bytes` is its "1.2 MB so far". `total` is counted
/// after the page has been walked for images that actually carry an address,
/// so it is not a count of `<img>` elements — see [`assets::rewrite`].
///
/// `bytes` counts **what is being kept**, not what the request cost: the page
/// as the site served it, plus every image that landed and was small enough to
/// keep. An image fetched and then refused for being over
/// [`Limits::max_asset_bytes`] spent the user's data and is not in this number,
/// because the sentence it sits under is "will work with no signal" and that
/// sentence is about the file, not the traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// The page has been asked for and nothing has come back. There is nothing
    /// to count yet, which is why this variant carries no bytes.
    Fetching,
    /// The page is in hand and is being made safe. `bytes` is exactly what the
    /// site served, before anything was thrown away.
    Stripping { bytes: u64 },
    /// Images. `done` have landed of `total` worth fetching.
    Images {
        done: usize,
        total: usize,
        bytes: u64,
    },
}

impl Progress {
    /// The running total behind "1.2 MB so far". Zero while the page itself is
    /// still in the air, because at that moment nothing has arrived.
    pub fn bytes(self) -> u64 {
        match self {
            Progress::Fetching => 0,
            Progress::Stripping { bytes } | Progress::Images { bytes, .. } => bytes,
        }
    }
}

/// Fetch a URL and turn it into something worth keeping.
///
/// Blocking, start to finish, and meant to be called on a worker thread — see
/// [`Fetcher`] for why this is a loop rather than a chain of callbacks. The
/// `progress` closure is called on that same thread, so E2 has to hop its
/// signal updates back to the main thread itself.
///
/// `progress` answers [`Wanted`], and answering [`Wanted::No`] once stops the
/// capture for good: the answer is latched here rather than asked again, so a
/// screen that has gone away cannot be talked back into a capture by a stale
/// closure returning `Yes` on the next call.
pub fn capture(
    url: &str,
    mode: CaptureMode,
    limits: &Limits,
    fetcher: &dyn Fetcher,
    mut progress: impl FnMut(Progress) -> Wanted,
) -> Outcome {
    let url = url.trim();
    // A pasted URL rarely has a scheme on it. Assume https rather than
    // rejecting it — and https rather than http, because falling back to
    // cleartext on a guess is not a thing this app should do.
    let url = if url.contains("://") {
        url.to_string()
    } else {
        format!("https://{url}")
    };

    if url::Url::parse(&url).is_err() {
        return Outcome::Failed(Failure::NotAUrl(url));
    }

    // One latch for the whole capture. `report` is what every stage below
    // calls instead of `progress` directly, and it does two things the raw
    // closure cannot: it remembers a `No` so the caller is never asked twice
    // (a screen that has been dropped should not get a second chance to
    // change its mind), and it gives `assets::rewrite` — which reports from
    // inside its own loop — somewhere to put the answer that this function can
    // read afterwards. A `Cell` rather than a `bool` because the closure
    // handed to `rewrite` borrows it while `report` is also live.
    let stopped = std::cell::Cell::new(false);
    let mut report = |step: Progress| -> Wanted {
        if stopped.get() {
            return Wanted::No;
        }
        let answer = progress(step);
        if answer == Wanted::No {
            stopped.set(true);
        }
        answer
    };

    if report(Progress::Fetching) == Wanted::No {
        return Outcome::Cancelled;
    }
    let response = match fetcher.get(&url) {
        Ok(response) => response,
        Err(failure) => return Outcome::Failed(failure),
    };

    // 401/402/403 fall through: the body is the teaser, and a teaser is worth
    // showing the user before they decide. Everything else in the 4xx/5xx
    // range is an error page nobody wants.
    if response.status >= 400 && !matches!(response.status, 401 | 402 | 403) {
        return Outcome::Failed(Failure::Refused {
            status: response.status,
        });
    }
    if !response.is_html() {
        return Outcome::Failed(Failure::NotHtml {
            content_type: response.content_type.clone(),
        });
    }
    if response.body.len() as u64 > limits.max_page_bytes {
        return Outcome::Failed(Failure::TooLarge(format!(
            "{} bytes, and the ceiling is {}",
            response.body.len(),
            limits.max_page_bytes
        )));
    }

    let fetched_bytes = response.body.len() as u64;

    if report(Progress::Stripping {
        bytes: fetched_bytes,
    }) == Wanted::No
    {
        return Outcome::Cancelled;
    }
    let document = dom::parse(&response.body);
    let base = assets::base_url(&document, &url);
    let title = dom::title_of(&document).unwrap_or_else(|| url.clone());
    let stripped = sanitise::sanitise(&document);

    // Judge the whole page, before reader mode gets a chance to throw the
    // evidence away.
    let full_text = dom::text_of(&dom::root(&document));
    let signals = detect::signals(&document, &full_text, &stripped);
    let verdict = detect::verdict(response.status, &signals, &full_text);

    // Card E7's registry, consulted in exactly the one place `mod.rs`'s
    // header promises: the generic engine has just decided this is a
    // JavaScript shell and is about to throw it away. `document` here is the
    // sanitised tree — sanitising never touches an attribute it does not
    // recognise as a URL or an event handler, so a site-specific extractor
    // reading `data-content` sees exactly what the site sent. Succeeding here
    // returns straight out of `capture`, before a single image is downloaded
    // for a page whose real chart does not need any: the images an ordinary
    // JS shell references belong to a bundle nobody is keeping.
    if verdict == Some(BlockReason::ScriptShell) {
        let host = url::Url::parse(&url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(str::to_string));
        if let Some(host) = host {
            let raw_html = dom::to_html(&dom::root(&document));
            if let Some(chart) = site::extract(&host, &raw_html) {
                return Outcome::Captured(site_captured_page(
                    url,
                    title,
                    chart,
                    stripped,
                    signals,
                    fetched_bytes,
                ));
            }
        }
    }

    let (downloaded, missed) = match &base {
        Some(base) => assets::rewrite(&document, base, fetcher, limits, |done, total, spent| {
            // The page's own bytes are added here rather than inside `rewrite`,
            // which has never seen them. What the screen shows is the size of
            // the thing it is about to keep, and that is both halves.
            report(Progress::Images {
                done,
                total,
                bytes: fetched_bytes + spent,
            })
        }),
        // No base means the URL did not parse, which `Url::parse` above has
        // already ruled out — but if it ever happens, a page with remote
        // images is a partial capture, not a crash.
        None => (Vec::new(), Vec::new()),
    };

    // Asked *after* `rewrite` rather than inside it, because `rewrite` breaks
    // its loop on a `No` and has no way to say so in a `(Vec, Vec)`. The images
    // it did not attempt are deliberately not recorded as `Missed`: they were
    // never tried, this page is being thrown away, and a list of things that
    // did not happen to a capture nobody is keeping is noise.
    if stopped.get() {
        return Outcome::Cancelled;
    }

    // Both readings, out of one fetch. The order matters: **narrowing happens
    // after the images are in hand**, which is a reversal of what E1 did and
    // the one cost of offering a preview of both.
    //
    // E1 narrowed first, so a reader capture never downloaded the site's
    // masthead. That is cheaper and it makes the second reading impossible: the
    // document has been gutted by the time anybody could ask what the full page
    // looked like, and there is no way back short of fetching the site again.
    // Card E3's whole point is that the user chooses *after* seeing both, so
    // the extra images are the price — 8.8 KB on hymnal.net, 9.6 KB on
    // guitaretab, nothing at all on CifraClub, and in every one of those three
    // cases the image was the site's own logo. `write_into` then keeps only
    // what the chosen reading references, so the phone's disk pays nothing.
    //
    // The reader reading is built on a **re-parse of the serialised full page**
    // rather than on a second `dom::parse` of the response body. Two reasons:
    // the sanitiser and `assets::rewrite` have both already run over this tree
    // and re-running them would download every image twice, and re-parsing what
    // will actually be written is the closest thing to a check that the two
    // readings are readings of the same document.
    let full_html = dom::to_html(&dom::root(&document));
    let full = Alternate {
        mode: CaptureMode::FullPage,
        text: dom::text_of(&dom::root(&document)),
        html: framed(&full_html, &url, CaptureMode::FullPage),
    };
    // `document` is dropped here on purpose: `RcDom`'s `Drop` empties the
    // children of everything it reaches (see `reader::narrow_to`), and holding
    // a gutted tree alive next to the one being narrowed is exactly the fault
    // that cost the E1 spike an afternoon.
    drop(document);

    let reader = read_narrowed(&full_html, &url);
    let reader_fell_back = mode == CaptureMode::Reader && reader.is_none();

    // The requested mode if it exists, and the full page if it does not. There
    // is no third state: a page with no reader view still has a full page.
    let (selected, alternate) = match (mode, reader) {
        (CaptureMode::Reader, Some(reader)) => (reader, Some(full)),
        (CaptureMode::Reader, None) => (full, None),
        (CaptureMode::FullPage, reader) => (full, reader),
    };

    let page = CapturedPage {
        mode: selected.mode,
        html: selected.html,
        text: selected.text,
        alternate,
        url,
        title,
        assets: downloaded,
        missed,
        stripped,
        signals,
        fetched_bytes,
        reader_fell_back,
    };

    match verdict {
        Some(reason) => Outcome::Blocked {
            reason,
            page: Some(Box::new(page)),
        },
        None if !page.missed.is_empty() => Outcome::Partial(page),
        None => Outcome::Captured(page),
    }
}

/// The reader reading of a page, from the markup the full page would save.
///
/// `None` means reader extraction found nothing to narrow to — a JavaScript
/// shell, a directory page, a chart spread across the whole body — and the
/// caller keeps the full page instead.
///
/// The rebuild in the middle is card E3's answer to card E8, and
/// [`reader::rebuild_chord_blocks`] carries the argument for it. In one
/// sentence: a chart whose alignment lived in a stylesheet this app dropped is
/// re-emitted as preformatted text rather than having somebody else's CSS
/// preserved and scoped, because the app wants a readable chart and not a
/// screenshot of a website.
fn read_narrowed(full_html: &str, url: &str) -> Option<Alternate> {
    let document = dom::parse(full_html.as_bytes());
    let chosen = reader::extract(&document)?;
    reader::narrow_to(&document, &chosen);
    reader::rebuild_chord_blocks(&dom::root(&document));
    Some(Alternate {
        mode: CaptureMode::Reader,
        text: dom::text_of(&dom::root(&document)),
        html: framed(&dom::to_html(&dom::root(&document)), url, CaptureMode::Reader),
    })
}

/// Turn a registry hit into the same [`CapturedPage`] shape a generic capture
/// produces, so nothing downstream — attaching, rendering, G2's search, E6's
/// re-check — has to know a site extractor was ever involved.
///
/// `stripped` and `signals` are the measurements the generic pass already
/// took of what actually came down over the wire; they stay true regardless
/// of which path decided what to do with the page, so there is no reason to
/// re-derive them. There is no `alternate` reading — a site extractor has one
/// answer, not two — and no assets: the images an ordinary JavaScript shell
/// references belong to its bundle, not to the chart the registry found
/// inside it, so `write_into` has nothing to save alongside the text.
fn site_captured_page(
    url: String,
    fallback_title: String,
    chart: site::ExtractedChart,
    stripped: Stripped,
    signals: Signals,
    fetched_bytes: u64,
) -> CapturedPage {
    let pre = dom::new_element("pre");
    dom::append(&pre, &dom::new_text(&chart.text));
    let html = framed(&dom::to_html(&pre), &url, CaptureMode::Reader);
    CapturedPage {
        title: chart.title.unwrap_or(fallback_title),
        mode: CaptureMode::Reader,
        html,
        text: chart.text,
        alternate: None,
        assets: Vec::new(),
        missed: Vec::new(),
        stripped,
        signals,
        fetched_bytes,
        reader_fell_back: false,
        url,
    }
}

/// The saved file: a doctype, a comment saying where it came from, the tree.
///
/// The provenance comment is not decoration. Somebody will find one of these
/// files in a backup in three years, and a saved page that does not say what
/// it is or what was done to it is a mystery. It is also what card E6's
/// re-check would read to know what to re-fetch — and, since E3, **it is the
/// only place the mode is recorded**: see [`mode_of`].
fn framed(body_html: &str, url: &str, mode: CaptureMode) -> String {
    format!(
        "<!doctype html>\n<!-- Captured by SetListArray ({}) from {}.\n     \
         Scripts, frames and remote references were removed; images were \
         rewritten to local files. -->\n{body_html}\n",
        mode.name(),
        url.replace("--", "&#45;&#45;"),
    )
}

/// Which mode a saved page was written in, read back off the file.
///
/// ## What the mode means once a page is captured
///
/// It is **a decision taken once and baked into what was saved**, not a
/// property that can be re-derived later. A saved attachment is one `page.html`
/// and one `Attachment::body`; the other reading needed the fetched bytes, and
/// those are gone the moment the capture screen leaves. Switching an existing
/// attachment from reader text to full page is therefore not a re-render, it is
/// a re-capture — which is card E6's territory, not a viewer control.
///
/// That is also why the mode is recorded in the file rather than in the
/// database row. `Attachment` is the library's schema and every column on it is
/// something the library queries, sorts or counts; the mode is none of those,
/// it is provenance, and provenance belongs with the artefact so that a
/// `page.html` recovered from a backup still says what it is. E6 re-fetching a
/// page reads this to know which reading to produce again, so that a re-check
/// diffs like against like.
pub fn mode_of(saved: &str) -> Option<CaptureMode> {
    let comment = saved.split_once("Captured by SetListArray (")?.1;
    let name = comment.split_once(')')?.0;
    CaptureMode::from_name(name)
}

/// Write a capture into an attachment directory, returning the bytes it took.
///
/// The directory is expected to exist — `Repo::create_attachment` makes it, and
/// this function deliberately does not, so that a capture can never write into
/// a directory no row points at. Assets go first: a `page.html` that exists is
/// the app's signal that the capture is complete, so it must not appear before
/// the files it references.
///
/// **Only the selected reading's images are written.** A capture holds every
/// image the full page referenced, because card E3 needs both readings out of
/// one fetch — but a reader capture that put the site's masthead in the library
/// would be storing a file nothing on disk mentions, and card E6's re-check
/// would then have an image to re-fetch that no saved page asks for. See
/// [`CapturedPage::kept_assets`].
pub fn write_into(directory: &Path, page: &CapturedPage) -> io::Result<u64> {
    let mut written = 0u64;

    let keeping: Vec<&Asset> = page.kept_assets().collect();
    if !keeping.is_empty() {
        std::fs::create_dir_all(directory.join("assets"))?;
    }
    for asset in keeping {
        // `path` is built by this module as `assets/NNN.ext` and never comes
        // from the page, so it cannot traverse — but a capture writes into the
        // user's library, and the check costs a comparison.
        let target = directory.join(&asset.path);
        if !target.starts_with(directory) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("asset path escaped the attachment directory: {}", asset.path),
            ));
        }
        std::fs::write(&target, &asset.bytes)?;
        written += asset.bytes.len() as u64;
    }

    std::fs::write(directory.join(PAGE_FILE), page.html.as_bytes())?;
    written += page.html.len() as u64;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::fetch::test_support::Canned;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n stand-in";

    fn chart_html(extra: &str) -> String {
        let chart: String = (1..=16)
            .map(|n| format!("G       C       D\nplaceholder line {n}\n"))
            .collect();
        format!(
            r#"<html><head><title>A Song — Example Tabs</title></head>
               <body><nav><a href="/x">Songs</a></nav>
               <div class="chord-sheet"><pre>{chart}</pre>{extra}</div>
               <script>window.ads = [];</script></body></html>"#
        )
    }

    fn run(net: Canned, mode: CaptureMode) -> Outcome {
        capture(
            "https://tabs.example/song/1",
            mode,
            &Limits::default(),
            &net,
            |_| Wanted::Yes,
        )
    }

    #[test]
    fn a_good_page_captures_whole() {
        let net = Canned::default().html("https://tabs.example/song/1", &chart_html(""));
        let Outcome::Captured(page) = run(net, CaptureMode::FullPage) else {
            panic!("expected a clean capture");
        };
        assert_eq!(page.title, "A Song — Example Tabs");
        assert!(page.signals.has_chart(), "{:?}", page.signals);
        assert!(page.signals.chord_lines >= 4);
        assert_eq!(page.stripped.scripts, 1);
        assert!(!page.html.contains("window.ads"));
        assert!(page.html.contains("Captured by SetListArray"));
        assert!(page.text.contains("placeholder line 16"));
        assert!(page.fetched_bytes > 0);
    }

    #[test]
    fn a_missing_image_makes_it_partial_and_keeps_the_chart() {
        let net = Canned::default()
            .html(
                "https://tabs.example/song/1",
                &chart_html(r#"<img src="/diagram.png" alt="G shape">"#),
            )
            .status("https://tabs.example/diagram.png", 500, "no");
        let Outcome::Partial(page) = run(net, CaptureMode::FullPage) else {
            panic!("expected a partial capture");
        };
        assert_eq!(page.missed.len(), 1);
        assert!(page.signals.has_chart(), "the chart is still there");
    }

    #[test]
    fn an_image_that_arrives_is_written_beside_the_page() {
        let dir = std::env::temp_dir().join(format!("sla-capture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let net = Canned::default()
            .html(
                "https://tabs.example/song/1",
                &chart_html(r#"<img src="/diagram.png">"#),
            )
            .image("https://tabs.example/diagram.png", PNG);
        let Outcome::Captured(page) = run(net, CaptureMode::FullPage) else {
            panic!("expected a clean capture");
        };

        let written = write_into(&dir, &page).unwrap();
        assert_eq!(written, page.bytes_on_disk());
        assert!(dir.join(PAGE_FILE).exists());
        assert!(dir.join("assets/000.png").exists());

        let saved = std::fs::read_to_string(dir.join(PAGE_FILE)).unwrap();
        assert!(saved.contains(r#"src="assets/000.png""#));
        assert!(
            !saved.contains(r#"src="http"#),
            "no src left pointing at the network"
        );
        assert!(
            saved.contains(r#"data-captured-from="https://tabs.example/diagram.png""#),
            "but the provenance E6 would re-check is recorded"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_javascript_shell_is_blocked_and_says_which_kind() {
        let shell = format!(
            r#"<html><head><title>Loading…</title></head><body><div id="root"></div>
               <script>{}</script></body></html>"#,
            "var x=1;".repeat(4_000)
        );
        let net = Canned::default().html("https://tabs.example/song/1", &shell);
        let Outcome::Blocked { reason, page } = run(net, CaptureMode::FullPage) else {
            panic!("expected a block");
        };
        assert_eq!(reason, BlockReason::ScriptShell);
        assert!(page.is_some(), "the shell is still handed back");
        assert!(reason.explain().contains("JavaScript"));
    }

    /// A page shaped the way Ultimate Guitar's actually is: an empty React
    /// mount (`signals.empty_mount`, conclusive on its own per
    /// `detect::verdict`) and, elsewhere in the same response, a
    /// `div.js-store` carrying the chart as JSON nobody's reader would see as
    /// text. `js_store` is `None` for the "the registry finds nothing"
    /// tests below.
    fn ultimate_guitar_shaped_html(js_store: Option<&str>) -> String {
        let div = js_store
            .map(|data_content| {
                format!(
                    r#"<div class="js-store" data-content="{}"></div>"#,
                    data_content.replace('&', "&amp;").replace('"', "&quot;")
                )
            })
            .unwrap_or_default();
        format!(
            r#"<html><head><title>Placeholder Chords @ Example Tab Co</title></head>
               <body><div id="root"></div>{div}
               <script>{filler}</script></body></html>"#,
            div = div,
            filler = "var boot=1;".repeat(200),
        )
    }

    fn placeholder_tab_json() -> String {
        serde_json::json!({
            "store": { "page": { "data": {
                "tab": { "song_name": "Placeholder Song", "artist_name": "The Example Band" },
                "tab_view": { "wiki_tab": { "content":
                    "[Intro]\r\n[ch]G[/ch]   [ch]C[/ch]\r\n\r\n[Verse]\r\n[tab][ch]G[/ch]  [ch]D[/ch]\r\n  A placeholder lyric line[/tab]\r\n"
                } }
            }}}
        })
        .to_string()
    }

    #[test]
    fn ultimate_guitar_bytes_produce_a_real_chart_through_the_registry() {
        let html = ultimate_guitar_shaped_html(Some(&placeholder_tab_json()));
        let net = Canned::default().html("https://tabs.ultimate-guitar.com/tab/example/song-chords-1", &html);
        let outcome = capture(
            "https://tabs.ultimate-guitar.com/tab/example/song-chords-1",
            CaptureMode::Reader,
            &Limits::default(),
            &net,
            |_| Wanted::Yes,
        );
        let Outcome::Captured(page) = outcome else {
            panic!("expected the registry to turn this into a capture, got {outcome:?}");
        };
        assert_eq!(page.title, "Placeholder Song — The Example Band");
        assert!(page.text.contains("G   C"), "{}", page.text);
        assert!(page.text.contains("A placeholder lyric line"), "{}", page.text);
        assert!(!page.text.contains("[ch]"), "{}", page.text);
        assert!(page.assets.is_empty(), "no bundle image is part of this chart");
        assert!(page.missed.is_empty());
    }

    #[test]
    fn a_page_from_a_host_the_registry_does_not_claim_stays_blocked_even_with_ultimate_guitar_shaped_bytes() {
        // Same bytes, same JSON, a different host. Proves the registry is
        // gated on the host and not merely on finding a `js-store` div —
        // nobody else on earth should have their markup read this closely.
        let html = ultimate_guitar_shaped_html(Some(&placeholder_tab_json()));
        let net = Canned::default().html("https://tabs.example/song/1", &html);
        let Outcome::Blocked { reason, page } = run(net, CaptureMode::Reader) else {
            panic!("expected a block: this host is not in the registry");
        };
        assert_eq!(reason, BlockReason::ScriptShell);
        assert!(page.is_some());
    }

    #[test]
    fn broken_ultimate_guitar_bytes_fall_back_to_the_honest_script_shell_verdict() {
        // The right host, but no `js-store` div at all — the shape a
        // redesign leaves behind. The registry finds nothing and today's
        // behaviour, unchanged, is what the user sees.
        let html = ultimate_guitar_shaped_html(None);
        let net = Canned::default().html("https://tabs.ultimate-guitar.com/tab/example/song-chords-1", &html);
        let outcome = capture(
            "https://tabs.ultimate-guitar.com/tab/example/song-chords-1",
            CaptureMode::Reader,
            &Limits::default(),
            &net,
            |_| Wanted::Yes,
        );
        let Outcome::Blocked { reason, page } = outcome else {
            panic!("expected the honest fallback, got {outcome:?}");
        };
        assert_eq!(reason, BlockReason::ScriptShell);
        assert!(page.is_some(), "the shell is still handed back, exactly as before this card");
    }

    #[test]
    fn a_paywall_is_a_different_verdict_from_a_shell() {
        let walled = r#"<html><head><title>Members only</title></head>
            <body><div class="paywall-overlay"><p>Subscribe for the full chart.</p></div>
            <pre>G  C  D</pre></body></html>"#;
        let net = Canned::default().html("https://tabs.example/song/1", walled);
        let Outcome::Blocked { reason, .. } = run(net, CaptureMode::FullPage) else {
            panic!("expected a block");
        };
        assert_eq!(reason, BlockReason::Paywalled);
    }

    #[test]
    fn the_verdict_survives_reader_mode_throwing_the_banner_away() {
        // The banner is furniture and reader extraction drops it. If judging
        // ran after narrowing, this page would be saved as a clean capture.
        let walled = format!(
            r#"<html><body><div class="regwall-banner"><p>Sign in to see the rest.</p></div>
               {}</body></html>"#,
            r#"<div class="chord-sheet"><pre>"#.to_string()
                + &(1..=16)
                    .map(|n| format!("G  C  D\nplaceholder line {n}\n"))
                    .collect::<String>()
                + "</pre></div>"
        );
        let net = Canned::default().html("https://tabs.example/song/1", &walled);
        let Outcome::Blocked { reason, .. } = run(net, CaptureMode::Reader) else {
            panic!("expected a block even in reader mode");
        };
        assert_eq!(reason, BlockReason::Paywalled);
    }

    #[test]
    fn reader_mode_drops_the_site_and_keeps_the_song() {
        let net = Canned::default().html("https://tabs.example/song/1", &chart_html(""));
        let Outcome::Captured(page) = run(net, CaptureMode::Reader) else {
            panic!("expected a clean capture");
        };
        assert!(!page.reader_fell_back);
        assert!(page.text.contains("placeholder line 1"));
        assert!(!page.html.contains("<nav"), "the navigation is gone");
        assert!(
            page.html.len() < capture_full_len(),
            "reader mode is the smaller of the two"
        );
    }

    fn capture_full_len() -> usize {
        let net = Canned::default().html("https://tabs.example/song/1", &chart_html(""));
        run(net, CaptureMode::FullPage).page().unwrap().html.len()
    }

    // ── one fetch, two readings (E3) ────────────────────────────────────────

    #[test]
    fn the_default_mode_is_reader_text() {
        // The measurement behind it is in `CaptureMode`'s own docs: on all
        // three sites this engine actually captures, a full-page save opens on
        // the site's login links and a reader save opens on the chart.
        assert_eq!(CaptureMode::default(), CaptureMode::Reader);
    }

    #[test]
    fn a_capture_carries_both_readings_of_one_fetch() {
        let net = Canned::default().html("https://tabs.example/song/1", &chart_html(""));
        let Outcome::Captured(page) = run(net, CaptureMode::Reader) else {
            panic!("expected a clean capture");
        };
        assert_eq!(page.mode, CaptureMode::Reader);
        assert!(page.has(CaptureMode::FullPage), "the other one is there too");
        assert!(!page.html.contains("<nav"), "reader dropped the navigation");
        assert!(
            page.html_for(CaptureMode::FullPage)
                .is_some_and(|html| html.contains("<nav")),
            "and full page kept it, without a second fetch"
        );
    }

    /// The control the card asks for. Switching it after the capture must not
    /// go back to the site — that is the whole reason both readings are built.
    #[test]
    fn switching_the_mode_swaps_the_reading_and_the_size() {
        let net = Canned::default().html("https://tabs.example/song/1", &chart_html(""));
        let Outcome::Captured(mut page) = run(net, CaptureMode::Reader) else {
            panic!("expected a clean capture");
        };
        let reader_bytes = page.bytes_on_disk();
        let full_bytes = page
            .bytes_for(CaptureMode::FullPage)
            .expect("the full page is priced too");
        assert!(reader_bytes < full_bytes, "{reader_bytes} < {full_bytes}");

        assert!(page.select(CaptureMode::FullPage));
        assert_eq!(page.mode, CaptureMode::FullPage);
        assert_eq!(page.bytes_on_disk(), full_bytes);
        assert!(page.html.contains("<nav"));

        // And back again, with both still in hand.
        assert!(page.select(CaptureMode::Reader));
        assert_eq!(page.bytes_on_disk(), reader_bytes);
    }

    /// A capture holds every image the full page referenced, because both
    /// readings come out of one fetch. Only the chosen one's images are put in
    /// the library — otherwise a reader capture files the site's masthead where
    /// nothing on disk mentions it, and card E6 has an image to re-check that
    /// no saved page asks for.
    #[test]
    fn only_the_selected_readings_images_are_written() {
        let dir = std::env::temp_dir().join(format!("sla-modes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // A masthead in the navigation, a chord box inside the chart. Reader
        // mode keeps the second and drops the first — which is exactly what
        // hymnal.net and guitaretab do in real life.
        let body = chart_html("").replace(
            r#"<nav><a href="/x">Songs</a></nav>"#,
            r#"<nav><a href="/x">Songs</a><img src="/logo.png"></nav>"#,
        );
        let body = body.replace("</pre>", r#"</pre><img src="/shape.png">"#);
        let net = Canned::default()
            .html("https://tabs.example/song/1", &body)
            .image("https://tabs.example/logo.png", PNG)
            .image("https://tabs.example/shape.png", PNG);

        let Outcome::Captured(mut page) = run(net, CaptureMode::Reader) else {
            panic!("expected a clean capture");
        };
        assert_eq!(page.assets.len(), 2, "both came down");
        assert_eq!(page.kept_assets().count(), 1, "one is referenced");

        let written = write_into(&dir, &page).unwrap();
        assert_eq!(written, page.bytes_on_disk());
        assert_eq!(
            std::fs::read_dir(dir.join("assets")).unwrap().count(),
            1,
            "and only one is on disk"
        );

        // Switching to the full page before attaching writes the other one too.
        assert!(page.select(CaptureMode::FullPage));
        write_into(&dir, &page).unwrap();
        assert_eq!(std::fs::read_dir(dir.join("assets")).unwrap().count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The mode is not in the database row. It is in the file, which is what
    /// makes a `page.html` recovered from a backup still say what it is — and
    /// what card E6 reads to know which reading to produce when it re-fetches.
    #[test]
    fn the_saved_file_records_which_mode_it_was_written_in() {
        let net = Canned::default().html("https://tabs.example/song/1", &chart_html(""));
        let Outcome::Captured(mut page) = run(net, CaptureMode::Reader) else {
            panic!("expected a clean capture");
        };
        assert_eq!(mode_of(&page.html), Some(CaptureMode::Reader));
        page.select(CaptureMode::FullPage);
        assert_eq!(mode_of(&page.html), Some(CaptureMode::FullPage));
        assert_eq!(mode_of("<!doctype html><p>somebody else's file</p>"), None);
    }

    /// Reader text asked for on a page with no article in it. The full page is
    /// kept, there is no second reading to switch to, and the screen is told so
    /// rather than being left with a control whose other half does nothing.
    #[test]
    fn a_page_with_no_article_has_one_reading_and_says_so() {
        // A chart spread across `<body>` itself: `reader::extract` returns
        // `None` because there is no smaller node that holds it.
        let sheet: String = (1..=30)
            .map(|n| format!(r#"<span class="chord">C</span><span class="lyric">l{n}</span>"#))
            .collect();
        let net = Canned::default().html(
            "https://tabs.example/song/1",
            &format!("<html><body>{sheet}</body></html>"),
        );
        let page = run(net, CaptureMode::Reader)
            .page()
            .expect("something was kept")
            .clone();
        assert!(page.reader_fell_back);
        assert_eq!(page.mode, CaptureMode::FullPage);
        assert!(!page.has(CaptureMode::Reader));
        assert!(page.bytes_for(CaptureMode::Reader).is_none());
    }

    /// The E8 fault, end to end: a chart positioned by a stylesheet the capture
    /// drops comes back as preformatted text in reader mode — and full-page
    /// mode leaves it exactly as the site served it, because that is the only
    /// thing full page promises.
    #[test]
    fn reader_mode_rebuilds_a_stylesheet_positioned_chart_and_full_page_does_not() {
        let verse: String = (1..=8)
            .map(|n| {
                format!(
                    r#"<div class="line"><div class="chord-text"><span class="chord">G</span>Line&nbsp;{n}&nbsp;</div><div class="chord-text"><span class="chord">C</span>of&nbsp;the&nbsp;words</div></div>"#
                )
            })
            .collect();
        let net = Canned::default().html(
            "https://tabs.example/song/1",
            &format!(
                r#"<html><head><link rel="stylesheet" href="/chart.css"></head>
                   <body><nav><a href="/x">Songs</a></nav>
                   <div class="chord-container">{verse}</div></body></html>"#
            ),
        );
        let Outcome::Captured(mut page) = run(net, CaptureMode::Reader) else {
            panic!("expected a clean capture");
        };
        assert!(page.html.contains("<pre>"), "rebuilt: {}", page.html);
        assert!(
            page.text.contains("G      C\nLine 1 of the words"),
            "the chords are over their words: {:?}",
            page.text
        );

        page.select(CaptureMode::FullPage);
        assert!(
            !page.html.contains("<pre>"),
            "full page is the site's markup, untouched"
        );
        assert!(page.html.contains("chord-text"));
    }

    #[test]
    fn a_404_never_becomes_an_attachment() {
        let net = Canned::default().status("https://tabs.example/song/1", 404, "<h1>gone</h1>");
        assert_eq!(
            run(net, CaptureMode::FullPage).page().map(|_| ()),
            None,
            "there is nothing to keep"
        );
    }

    #[test]
    fn a_pdf_is_refused_by_this_module_rather_than_mangled() {
        let mut net = Canned::default();
        net = net.html("https://tabs.example/song/1", "x");
        // Rewrite the content type by hand: the canned helper only serves HTML.
        let outcome = capture(
            "https://tabs.example/song/1",
            CaptureMode::FullPage,
            &Limits::default(),
            &PdfSite,
            |_| Wanted::Yes,
        );
        let Outcome::Failed(Failure::NotHtml { content_type }) = outcome else {
            panic!("expected a NotHtml failure");
        };
        assert_eq!(content_type, "application/pdf");
        let _ = net;
    }

    struct PdfSite;
    impl Fetcher for PdfSite {
        fn get(&self, _url: &str) -> Result<Fetched, Failure> {
            Ok(Fetched {
                status: 200,
                content_type: "application/pdf".into(),
                body: b"%PDF-1.4".to_vec(),
            })
        }
    }

    #[test]
    fn a_bare_host_is_assumed_to_be_https() {
        let net = Canned::default().html("https://tabs.example/song/1", &chart_html(""));
        let outcome = capture(
            "  tabs.example/song/1 ",
            CaptureMode::FullPage,
            &Limits::default(),
            &net,
            |_| Wanted::Yes,
        );
        assert!(matches!(outcome, Outcome::Captured(_)), "{outcome:?}");
    }

    /// Two images and a page, so that every number E2 prints has something to
    /// be checked against.
    fn two_image_site() -> (Canned, u64) {
        let body = chart_html(r#"<img src="/a.png"><img src="/b.png">"#);
        let bytes = body.len() as u64;
        let net = Canned::default()
            .html("https://tabs.example/song/1", &body)
            .image("https://tabs.example/a.png", PNG)
            .image("https://tabs.example/b.png", PNG);
        (net, bytes)
    }

    fn watch(net: &Canned, answer: impl FnMut(Progress) -> Wanted) -> (Outcome, Vec<Progress>) {
        let seen = std::cell::RefCell::new(Vec::new());
        let mut answer = answer;
        let outcome = capture(
            "https://tabs.example/song/1",
            CaptureMode::FullPage,
            &Limits::default(),
            net,
            |step| {
                seen.borrow_mut().push(step);
                answer(step)
            },
        );
        (outcome, seen.into_inner())
    }

    #[test]
    fn the_progress_checklist_reports_every_stage_in_order() {
        let (net, page_bytes) = two_image_site();
        let (_, seen) = watch(&net, |_| Wanted::Yes);

        assert_eq!(seen[0], Progress::Fetching);
        assert_eq!(seen[1], Progress::Stripping { bytes: page_bytes });
        assert_eq!(
            seen.last(),
            Some(&Progress::Images {
                done: 2,
                total: 2,
                bytes: page_bytes + 2 * PNG.len() as u64,
            })
        );
    }

    /// The whole argument for `Progress` carrying bytes rather than E2 running
    /// a timer: "1.2 MB so far" has to be a measurement. Nothing before the
    /// page lands counts anything, and from then on the figure only grows, by
    /// exactly the size of each image that arrived.
    #[test]
    fn the_byte_counter_is_measured_and_never_goes_backwards() {
        let (net, page_bytes) = two_image_site();
        let (_, seen) = watch(&net, |_| Wanted::Yes);

        assert_eq!(seen[0].bytes(), 0, "nothing has arrived yet");
        let mut previous = 0;
        for step in &seen {
            assert!(step.bytes() >= previous, "{seen:?} went backwards");
            previous = step.bytes();
        }
        assert_eq!(previous, page_bytes + 2 * PNG.len() as u64);

        // And the figure the screen ends on is the figure the file takes,
        // give or take what sanitising threw away — never larger.
        let Outcome::Captured(page) = watch(&net, |_| Wanted::Yes).0 else {
            panic!("expected a clean capture");
        };
        assert_eq!(page.fetched_bytes, page_bytes);
    }

    // ── cancelling ──────────────────────────────────────────────────────────

    /// A Cancel during the images stops the remaining downloads and throws the
    /// page away. It does *not* come back as `Partial`: the user did not get a
    /// capture with holes in it, they got no capture, and E4 must not offer to
    /// keep one.
    #[test]
    fn a_cancel_during_the_images_stops_the_downloads_and_keeps_nothing() {
        let (net, _) = two_image_site();
        let (outcome, seen) = watch(&net, |step| match step {
            Progress::Images { done: 1, .. } => Wanted::No,
            _ => Wanted::Yes,
        });

        assert!(matches!(outcome, Outcome::Cancelled), "{outcome:?}");
        assert!(outcome.page().is_none(), "a cancelled capture has no page");
        // The second image was never asked for: the last thing reported is the
        // tick that was answered `No`.
        assert_eq!(
            seen.last(),
            Some(&Progress::Images {
                done: 1,
                total: 2,
                bytes: seen.last().unwrap().bytes(),
            })
        );
    }

    #[test]
    fn a_cancel_before_the_page_lands_never_opens_the_socket() {
        struct Loud(std::cell::Cell<usize>);
        impl Fetcher for Loud {
            fn get(&self, _url: &str) -> Result<Fetched, Failure> {
                self.0.set(self.0.get() + 1);
                Err(Failure::Unreachable("should never be asked".into()))
            }
        }
        let net = Loud(std::cell::Cell::new(0));
        let outcome = capture(
            "https://tabs.example/song/1",
            CaptureMode::FullPage,
            &Limits::default(),
            &net,
            |_| Wanted::No,
        );
        assert!(matches!(outcome, Outcome::Cancelled), "{outcome:?}");
        assert_eq!(net.0.get(), 0, "the fetch was never attempted");
    }

    /// The latch. A screen that has gone away answers `No` once; a stale
    /// closure that answered `Yes` afterwards must not be able to restart a
    /// capture that has already been abandoned.
    #[test]
    fn saying_no_once_is_not_taken_back_by_a_later_yes() {
        let (net, _) = two_image_site();
        let mut answers = vec![Wanted::Yes, Wanted::No, Wanted::Yes, Wanted::Yes].into_iter();
        let (outcome, seen) = watch(&net, move |_| answers.next().unwrap_or(Wanted::Yes));

        assert!(matches!(outcome, Outcome::Cancelled), "{outcome:?}");
        // Fetching was answered `Yes`, Stripping `No`, and nothing was asked
        // after that — the engine never got as far as an image.
        assert_eq!(seen.len(), 2, "{seen:?}");
    }
}
