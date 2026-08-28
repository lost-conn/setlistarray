//! Turning every remote image into a file in the attachment directory.
//!
//! This is the step that makes a capture worth having. A chord chart on a
//! guitar site is often a diagram — a chord box, a scan of a lead sheet, a
//! rhythm figure — and a saved page whose images still point at the site is
//! not saved at all.
//!
//! Failures here are deliberately **not** fatal. A page with nine images of
//! which one 404s is still the chart the user wanted; losing the whole capture
//! over it would be the wrong trade. Every miss is recorded and surfaced as
//! [`Outcome::Partial`](super::Outcome::Partial), which is E4's "partial
//! capture" screen.
//!
//! See the ordering note on [`sanitise`](super::sanitise): this pass runs
//! second, and it also finishes the sanitiser's job by removing the `srcset`
//! and `data-*` attributes it had to leave in place.

use markup5ever_rcdom::{Handle, RcDom};
use url::Url;

use super::dom::{self, Descend};
use super::{Limits, Missed};

/// One image, downloaded and named. The bytes are held in memory until
/// [`super::write_into`] puts them on disk, so that a capture that turns out
/// to be unusable never leaves a directory of orphans behind.
#[derive(Debug, Clone)]
pub struct Asset {
    /// Where it goes, relative to the attachment directory: `assets/003.png`.
    pub path: String,
    pub bytes: Vec<u8>,
    pub source: String,
}

/// Every attribute an image URL is known to hide in, best first.
///
/// `src` is not first. On a lazy-loading site `src` holds a grey placeholder
/// or a data-URI blur and the real image is in `data-src`; taking `src` would
/// capture the placeholder and call it a success.
const SRC_ATTRS: &[&str] = &[
    "data-src",
    "data-original",
    "data-lazy-src",
    "data-original-src",
    "data-full-src",
    "src",
];

/// Removed once the rewrite is done, whether or not anything was downloaded.
const REMOTE_ATTRS: &[&str] = &[
    "srcset",
    "imagesrcset",
    "data-srcset",
    "data-src",
    "data-original",
    "data-lazy-src",
    "data-original-src",
    "data-full-src",
    "loading",
];

/// The base every relative URL in the page resolves against, and the `<base>`
/// element removed on the way out.
///
/// `<base href>` wins over the requested URL because that is what a browser
/// does. When there is no `<base>` we fall back to the URL the user pasted —
/// which is subtly wrong for a page that redirected across hosts, because
/// `rinch-http` does not report where it ended up. In practice a cross-host
/// redirect that also serves host-relative image URLs is rare, and the failure
/// mode is a handful of missed assets (a partial capture) rather than a wrong
/// one.
pub fn base_url(dom: &RcDom, requested: &str) -> Option<Url> {
    let requested = Url::parse(requested).ok()?;
    let mut base = requested.clone();

    if let Some(element) = dom::find_element(&dom.document, &html5ever::local_name!("base")) {
        if let Some(href) = dom::attr(&element, "href") {
            if let Ok(parsed) = requested.join(href.trim()) {
                base = parsed;
            }
        }
        dom::detach(&element);
    }
    Some(base)
}

/// Download what the page points at, write local paths back into it, and
/// report what did not come.
///
/// `progress` is called with *(images landed, images worth fetching, bytes
/// kept so far)* before each download and once more at the end, and answers
/// whether the capture is still wanted. Answering [`Wanted::No`] breaks the
/// loop where it stands: the images not yet attempted are not recorded as
/// [`Missed`], because they were never tried and the page is on its way to the
/// bin — see [`super::capture`], which turns that answer into
/// [`Outcome::Cancelled`](super::Outcome::Cancelled).
///
/// The byte figure is what is being **kept**: an image that arrived and was
/// then refused for being over [`Limits::max_asset_bytes`] cost the user's data
/// allowance and does not appear in it, because the sentence E2 puts it under
/// is about the file that will work with no signal.
pub fn rewrite(
    dom: &RcDom,
    base: &Url,
    fetcher: &dyn super::Fetcher,
    limits: &Limits,
    mut progress: impl FnMut(usize, usize, u64) -> super::Wanted,
) -> (Vec<Asset>, Vec<Missed>) {
    // Two passes. The first works out what there is to fetch and clears the
    // lazy-loading attributes; only then is the total in E2's "downloading
    // image 3 of 7" a real number rather than a count of `<img>` elements,
    // several of which routinely carry no usable address at all.
    let mut queue: Vec<(Handle, String)> = Vec::new();
    for node in collect_images(dom) {
        let candidate = SRC_ATTRS
            .iter()
            .filter_map(|name| dom::attr(&node, name))
            .map(|v| v.trim().to_string())
            .find(|v| !v.is_empty() && !v.starts_with("data:"));

        // Strip the lazy-loading attributes whatever happens next: a renderer
        // that honours `srcset` would otherwise prefer a remote URL over the
        // file we are about to write.
        for name in REMOTE_ATTRS {
            dom::retain_attrs(&node, |attr, _| attr != *name);
        }
        if let Some(raw) = candidate {
            queue.push((node, raw));
        }
    }

    let total = queue.len().min(limits.max_assets);
    let mut assets = Vec::new();
    let mut missed = Vec::new();
    let mut spent: u64 = 0;
    let started = std::time::Instant::now();

    for (index, (node, raw)) in queue.iter().enumerate() {
        let Ok(absolute) = base.join(raw) else {
            missed.push(Missed::new(raw, "the address did not parse"));
            continue;
        };
        if !matches!(absolute.scheme(), "http" | "https") {
            missed.push(Missed::new(absolute.as_str(), "not an http address"));
            continue;
        }

        if assets.len() >= limits.max_assets {
            missed.push(Missed::new(
                absolute.as_str(),
                "over the image count for one capture",
            ));
            drop_src(node);
            continue;
        }
        if spent >= limits.max_total_asset_bytes {
            missed.push(Missed::new(absolute.as_str(), "over the size budget"));
            drop_src(node);
            continue;
        }
        if started.elapsed() >= limits.deadline {
            missed.push(Missed::new(absolute.as_str(), "the capture ran out of time"));
            drop_src(node);
            continue;
        }

        // Asked here, immediately before the one blocking call in this loop,
        // so that a Cancel arriving during image four is acted on before image
        // five's socket is opened rather than after it has closed.
        if progress(assets.len(), total, spent) == super::Wanted::No {
            return (assets, missed);
        }

        match fetcher.get(absolute.as_str()) {
            Ok(response) if !(200..300).contains(&response.status) => {
                missed.push(Missed::new(
                    absolute.as_str(),
                    &format!("the site answered {}", response.status),
                ));
                drop_src(node);
            }
            Ok(response) if !response.is_image() => {
                // Overwhelmingly this is an HTML error page served with a 200.
                missed.push(Missed::new(absolute.as_str(), "did not come back an image"));
                drop_src(node);
            }
            Ok(response) if response.body.len() as u64 > limits.max_asset_bytes => {
                missed.push(Missed::new(absolute.as_str(), "larger than one image may be"));
                drop_src(node);
            }
            Ok(response) => {
                let path = format!(
                    "assets/{:03}.{}",
                    index,
                    extension(&response.content_type, absolute.path())
                );
                spent += response.body.len() as u64;
                dom::set_attr(node, "src", &path);
                // Keep a note of where it came from. It costs nothing, it
                // survives into the saved file, and it is what a
                // "re-check saved pages" pass (E6) would need.
                dom::set_attr(node, "data-captured-from", absolute.as_str());
                assets.push(Asset {
                    path,
                    bytes: response.body,
                    source: absolute.to_string(),
                });
            }
            Err(failure) => {
                missed.push(Missed::new(absolute.as_str(), &failure.to_string()));
                drop_src(node);
            }
        }
    }

    // The closing tick, so the checklist reads "7 of 7" rather than freezing on
    // "6 of 7" while the last image is written. Its answer is discarded: there
    // is nothing left to stop.
    let _ = progress(assets.len(), total, spent);
    (assets, missed)
}

/// An image we could not fetch keeps its `alt` and loses its `src`, so the
/// saved page shows a broken-image box with a caption rather than sitting
/// there trying to reach a network that is not there.
fn drop_src(node: &Handle) {
    dom::retain_attrs(node, |attr, _| attr != "src");
}

fn collect_images(dom: &RcDom) -> Vec<Handle> {
    let mut found = Vec::new();
    dom::walk(&dom::root(dom), &mut |node| {
        if dom::is(node, "img") {
            found.push(node.clone());
        }
        Descend::Yes
    });
    found
}

/// The file extension, from the content type first and the URL second.
///
/// The content type is the more trustworthy of the two — a chord site serving
/// a PNG from a path ending `.php` is not unusual — and an unrecognised type
/// gets `.img`, which no renderer sniffs but which keeps the file honest.
fn extension(content_type: &str, path: &str) -> String {
    let from_type = match content_type.split(';').next().unwrap_or("").trim() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        "image/bmp" => Some("bmp"),
        "image/avif" => Some("avif"),
        _ => None,
    };
    if let Some(ext) = from_type {
        return ext.to_string();
    }
    path.rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "img".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::fetch::test_support::Canned;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-- not really --";

    fn run(html: &str, net: Canned, limits: Limits) -> (String, Vec<Asset>, Vec<Missed>) {
        let doc = dom::parse(html.as_bytes());
        let base = base_url(&doc, "https://tabs.example/song/1").unwrap();
        let (assets, missed) = rewrite(&doc, &base, &net, &limits, |_, _, _| crate::capture::Wanted::Yes);
        (dom::to_html(&dom::root(&doc)), assets, missed)
    }

    #[test]
    fn a_relative_image_becomes_a_local_file() {
        let net = Canned::default().image("https://tabs.example/img/chord.png", PNG);
        let (html, assets, missed) = run(
            r#"<img src="../img/chord.png" alt="G major">"#,
            net,
            Limits::default(),
        );
        assert_eq!(assets.len(), 1);
        assert!(missed.is_empty());
        assert_eq!(assets[0].path, "assets/000.png");
        assert!(html.contains(r#"src="assets/000.png""#), "{html}");
        assert!(html.contains(r#"alt="G major""#), "the caption survives");
    }

    #[test]
    fn the_lazy_loaded_url_beats_the_placeholder() {
        let net = Canned::default().image("https://cdn.example/real.png", PNG);
        let (html, assets, _) = run(
            r#"<img src="/spacer.gif" data-src="https://cdn.example/real.png"
                    srcset="/spacer.gif 1x">"#,
            net,
            Limits::default(),
        );
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].source, "https://cdn.example/real.png");
        assert!(!html.contains("spacer.gif"), "the placeholder is not captured");
        assert!(!html.contains("srcset"), "and srcset cannot override us: {html}");
    }

    #[test]
    fn a_base_href_wins_over_the_url_we_asked_for() {
        let net = Canned::default().image("https://cdn.example/a/chord.png", PNG);
        let (html, assets, _) = run(
            r#"<base href="https://cdn.example/a/"><img src="chord.png">"#,
            net,
            Limits::default(),
        );
        assert_eq!(assets.len(), 1);
        assert!(!html.contains("<base"), "and the base element goes with it");
    }

    #[test]
    fn one_missing_image_is_a_partial_capture_not_a_failure() {
        let net = Canned::default()
            .image("https://tabs.example/a.png", PNG)
            .status("https://tabs.example/b.png", 404, "gone");
        let (html, assets, missed) = run(
            r#"<img src="/a.png"><img src="/b.png" alt="verse 2">"#,
            net,
            Limits::default(),
        );
        assert_eq!(assets.len(), 1);
        assert_eq!(missed.len(), 1);
        assert!(missed[0].why.contains("404"));
        assert!(html.contains(r#"alt="verse 2""#), "the caption is all that is left");
        assert!(!html.contains("/b.png"), "and it no longer points anywhere");
    }

    #[test]
    fn the_budget_is_enforced_and_the_overflow_is_reported() {
        let net = Canned::default()
            .image("https://tabs.example/a.png", PNG)
            .image("https://tabs.example/b.png", PNG);
        let limits = Limits {
            max_assets: 1,
            ..Limits::default()
        };
        let (_, assets, missed) = run(r#"<img src="/a.png"><img src="/b.png">"#, net, limits);
        assert_eq!(assets.len(), 1);
        assert_eq!(missed.len(), 1);
        assert!(missed[0].why.contains("count"));
    }

    #[test]
    fn running_out_of_time_is_a_partial_capture_not_a_failure() {
        let net = Canned::default()
            .image("https://tabs.example/a.png", PNG)
            .image("https://tabs.example/b.png", PNG);
        let limits = Limits {
            deadline: std::time::Duration::ZERO,
            ..Limits::default()
        };
        let (_, assets, missed) = run(r#"<img src="/a.png"><img src="/b.png">"#, net, limits);
        assert!(assets.is_empty());
        assert_eq!(missed.len(), 2);
        assert!(missed[0].why.contains("time"));
    }

    #[test]
    fn a_html_error_page_served_as_an_image_is_not_kept() {
        let net = Canned::default().status("https://tabs.example/a.png", 200, "<h1>oops</h1>");
        let (_, assets, missed) = run(r#"<img src="/a.png">"#, net, Limits::default());
        assert!(assets.is_empty());
        assert_eq!(missed.len(), 1);
    }

    #[test]
    fn the_extension_comes_from_the_content_type_first() {
        assert_eq!(extension("image/jpeg", "/render.php"), "jpg");
        assert_eq!(extension("", "/a/b/chord.PNG?x=1"), "img");
        assert_eq!(extension("", "/a/b/chord.PNG"), "png");
        assert_eq!(extension("application/octet-stream", "/x"), "img");
    }
}
