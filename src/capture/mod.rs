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
//!   fetch ──▶ parse ──▶ sanitise ──▶ judge ──▶ [narrow] ──▶ images ──▶ write
//!     │                     │           │                     │
//!  Failure              Stripped    Outcome              Partial/Missed
//! ```
//!
//! **Judging happens before narrowing**, and that ordering is deliberate. The
//! question E4 has to answer is "is there a chart behind this URL", which is a
//! question about the page the site served, not about the fragment reader mode
//! chose to keep. A paywall notice sits in a banner that reader extraction
//! throws away; judging afterwards would lose it and save the teaser as though
//! it were the song.
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

/// What the user asked to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptureMode {
    /// The page as the site built it, minus everything that executes or
    /// phones home. Fidelity: the chart looks like it did in the browser.
    #[default]
    FullPage,
    /// Just the article. Smaller, cleaner, and occasionally wrong — reader
    /// extraction is a heuristic, which is why E3 shows a preview before
    /// committing.
    Reader,
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

/// A capture, complete, in memory, and not yet on disk.
#[derive(Debug, Clone)]
pub struct CapturedPage {
    /// The URL the user pasted. Goes into `Attachment::source_url`.
    pub url: String,
    /// `<title>`, or the first `<h1>`. Goes into `Attachment::title`.
    pub title: String,
    pub mode: CaptureMode,
    /// Sanitised HTML with local asset paths. Written to [`PAGE_FILE`].
    pub html: String,
    /// Visible text. Goes into `Attachment::body`, which is what card G2's
    /// search inside attachments reads.
    pub text: String,
    pub assets: Vec<Asset>,
    pub missed: Vec<Missed>,
    pub stripped: Stripped,
    pub signals: Signals,
    /// Bytes of HTML as the site served them, before any of this ran. The
    /// number E2's "1.2 MB so far" counts up to.
    pub fetched_bytes: u64,
    /// Reader mode was asked for and found nothing to narrow to, so the full
    /// page was kept instead. E3 should say so rather than quietly obliging.
    pub reader_fell_back: bool,
}

impl CapturedPage {
    /// What [`write_into`] will put on disk. Goes into
    /// `Attachment::bytes_on_disk`.
    pub fn bytes_on_disk(&self) -> u64 {
        self.html.len() as u64 + self.assets.iter().map(|a| a.bytes.len() as u64).sum::<u64>()
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

    let mut reader_fell_back = false;
    if mode == CaptureMode::Reader {
        match reader::extract(&document) {
            Some(chosen) => reader::narrow_to(&document, &chosen),
            None => reader_fell_back = true,
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

    let page = CapturedPage {
        text: dom::text_of(&dom::root(&document)),
        html: render(&document, &url, mode),
        url,
        title,
        mode,
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

/// The saved file: a doctype, a comment saying where it came from, the tree.
///
/// The provenance comment is not decoration. Somebody will find one of these
/// files in a backup in three years, and a saved page that does not say what
/// it is or what was done to it is a mystery. It is also what card E6's
/// re-check would read to know what to re-fetch.
fn render(document: &markup5ever_rcdom::RcDom, url: &str, mode: CaptureMode) -> String {
    let mode = match mode {
        CaptureMode::FullPage => "full page",
        CaptureMode::Reader => "reader text",
    };
    format!(
        "<!doctype html>\n<!-- Captured by SetListArray ({mode}) from {}.\n     \
         Scripts, frames and remote references were removed; images were \
         rewritten to local files. -->\n{}\n",
        url.replace("--", "&#45;&#45;"),
        dom::to_html(&dom::root(document))
    )
}

/// Write a capture into an attachment directory, returning the bytes it took.
///
/// The directory is expected to exist — `Repo::create_attachment` makes it, and
/// this function deliberately does not, so that a capture can never write into
/// a directory no row points at. Assets go first: a `page.html` that exists is
/// the app's signal that the capture is complete, so it must not appear before
/// the files it references.
pub fn write_into(directory: &Path, page: &CapturedPage) -> io::Result<u64> {
    let mut written = 0u64;

    if !page.assets.is_empty() {
        std::fs::create_dir_all(directory.join("assets"))?;
    }
    for asset in &page.assets {
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
