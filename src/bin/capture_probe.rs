//! The harness that produced the site table in `docs/CAPTURE.md`.
//!
//! ```bash
//! cargo run --release --bin capture_probe -- https://example.com/song
//! cargo run --release --bin capture_probe -- --reader --out /tmp/caps URL...
//! cargo run --release --bin capture_probe -- --browser-ua URL...
//! ```
//!
//! It is here rather than in a scratch directory for two reasons. The spike's
//! headline result is a measurement, and a measurement nobody can repeat is an
//! anecdote; and card E6 ("re-check saved pages") is this program with a diff
//! bolted on. It builds as a binary, so `build-apk.sh --lib` never sees it.
//!
//! It prints **counts and byte totals only** — never the text it captured.
//! What comes down is somebody's copyrighted chart, and it belongs in the
//! user's own library, not in this repository's terminal scrollback.

use std::path::PathBuf;

use setlistarray::capture::{
    CaptureMode, HttpFetcher, Limits, Outcome, PAGE_FILE, Progress, capture, write_into,
};

fn main() {
    let mut mode = CaptureMode::FullPage;
    let mut limits = Limits::default();
    let mut out: Option<PathBuf> = None;
    let mut urls: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--reader" => mode = CaptureMode::Reader,
            "--out" => out = args.next().map(PathBuf::from),
            // What the honest User-Agent costs is itself a spike result, so
            // the comparison has to be one flag apart.
            "--browser-ua" => {
                limits.user_agent = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) \
                                     Gecko/20100101 Firefox/128.0"
                    .to_string()
            }
            "--help" | "-h" => {
                eprintln!("usage: capture_probe [--reader] [--browser-ua] [--out DIR] URL...");
                return;
            }
            other => urls.push(other.to_string()),
        }
    }

    if urls.is_empty() {
        eprintln!("nothing to capture. --help for usage.");
        std::process::exit(2);
    }

    let root = out.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("sla-capture-probe-{}", std::process::id()))
    });
    let fetcher = HttpFetcher::new(&limits);

    println!(
        "{:<44} {:>9} {:>7} {:>7} {:>6} {:>6} {:>5}  {}",
        "url", "fetched", "onDisk", "images", "chord", "pre", "js", "outcome"
    );

    for (index, url) in urls.iter().enumerate() {
        let started = std::time::Instant::now();
        let outcome = capture(url, mode, &limits, &fetcher, |step| {
            if let Progress::Images { done, total } = step {
                if total > 0 {
                    eprint!("\r  images {done}/{total}   ");
                }
            }
        });
        eprint!("\r                        \r");

        let verdict = match &outcome {
            Outcome::Captured(_) => "captured".to_string(),
            Outcome::Partial(page) => format!("PARTIAL ({} missed)", page.missed.len()),
            Outcome::Blocked { reason, .. } => format!("BLOCKED {reason:?}"),
            Outcome::Failed(failure) => format!("FAILED {failure}"),
        };

        match outcome.page() {
            Some(page) => {
                let directory = root.join(index.to_string());
                std::fs::create_dir_all(&directory).expect("scratch directory");
                let written = write_into(&directory, page).expect("writing the capture");
                println!(
                    "{:<44} {:>9} {:>7} {:>7} {:>6} {:>6} {:>5}  {verdict}",
                    short(url),
                    page.fetched_bytes,
                    written,
                    format!("{}/{}", page.assets.len(), page.assets.len() + page.missed.len()),
                    page.signals.chord_lines,
                    page.signals.preformatted_chars,
                    page.stripped.scripts,
                );
                for missed in page.missed.iter().take(3) {
                    eprintln!("      missed {}: {}", short(&missed.source), missed.why);
                }
                eprintln!(
                    "      title {:?} · text {} chars · chord markup {} · {} in {:?} · {}",
                    truncate(&page.title, 60),
                    page.signals.text_chars,
                    page.signals.chord_markup,
                    directory.join(PAGE_FILE).display(),
                    started.elapsed(),
                    if page.reader_fell_back {
                        "reader found nothing, kept the full page"
                    } else {
                        "ok"
                    }
                );
            }
            None => println!(
                "{:<44} {:>9} {:>7} {:>7} {:>6} {:>6} {:>5}  {verdict}",
                short(url),
                "-",
                "-",
                "-",
                "-",
                "-",
                "-"
            ),
        }
    }

    eprintln!("\ncaptures under {}", root.display());
}

fn short(url: &str) -> String {
    truncate(
        url.trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www."),
        44,
    )
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    s.chars().take(n - 1).collect::<String>() + "…"
}
