//! Making a stranger's HTML safe to render inside the app.
//!
//! The captured page is opened later by `E5` in the app's own renderer, so the
//! input is hostile by default: it is arbitrary markup from a site the user
//! pasted a link to, and nothing about a chord site makes it trustworthy.
//!
//! Two rules decide everything here.
//!
//! **Nothing may execute.** `<script>`, event-handler attributes,
//! `javascript:` URLs and the frame family go, whether or not Rinch's renderer
//! would have run them today. "Our renderer has no JS engine" is a property of
//! this month's Rinch, not a security boundary, and a file written to disk
//! outlives the assumption.
//!
//! **Nothing may reach the network.** This is the stricter of the two, and it
//! is what "offline-first" actually means: a captured page opened on a train
//! must look the same as it did on wifi. So every remote reference either
//! becomes a local file (`assets.rs`) or is removed. A same-host stylesheet is
//! no better than a third-party one — both are a request that will fail.
//!
//! ### What survives
//!
//! * `<style>` blocks, with `@import` and absolute `url()` neutralised. Chord
//!   sites position the chord row over the lyric row in CSS; throwing the
//!   stylesheet away turns a chart into a wall of words. Inline CSS needs no
//!   network once those two escapes are closed.
//! * `<a href>`, rewritten to absolute. It cannot be followed offline, but it
//!   is the record of where a "more from this artist" link went, and leaving
//!   it relative would silently resolve against the attachment directory.
//! * `<noscript>` contents. On a JS-only page this is sometimes the only real
//!   text on offer, which is why `dom::to_html` serialises with scripting
//!   disabled.
//!
//! ### Ordering
//!
//! [`sanitise`] runs **first** and [`assets::rewrite`](super::assets::rewrite)
//! second, and neither is optional. Sanitising first means the ad containers
//! and tracking pixels are already gone when the downloader picks its images,
//! so the app never fetches an advert. The price is that `srcset` and the
//! `data-src`/`data-original` attributes lazy-loaders hide the real URL in
//! have to survive this pass — they are inert data as far as a renderer is
//! concerned, but they are also remote references, so `assets::rewrite` is
//! what finally removes them. Calling `sanitise` on its own leaves a page
//! that cannot execute anything and can still name a remote image.

use html5ever::LocalName;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use super::dom::{self, Descend};

/// A tally of what came out, for the progress checklist in E2 and the
//  "this page was mostly JavaScript" verdict in E4.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stripped {
    pub scripts: usize,
    /// How much script *text* there was. A page whose scripts outweigh its
    /// prose by an order of magnitude is a JS shell, and `detect` says so.
    pub script_bytes: usize,
    pub frames: usize,
    pub media: usize,
    pub stylesheets: usize,
    pub ad_containers: usize,
    pub tracking_pixels: usize,
    pub event_handlers: usize,
    pub script_urls: usize,
    pub forms: usize,
}

/// Elements whose entire subtree goes.
///
/// `<template>` is here because its contents are inert markup the renderer
/// would never show but a future one might clone; `<canvas>` because without
/// a script it can only ever be a blank rectangle.
const DROP_SUBTREE: &[&str] = &[
    "script", "iframe", "frame", "frameset", "object", "embed", "applet", "canvas", "audio",
    "video", "source", "track", "template", "portal",
];

const FORM_CONTROLS: &[&str] = &["input", "button", "select", "textarea", "output", "progress"];

/// Substrings that mean "advertising" wherever they appear in a class or id.
/// Each is long enough that a false positive would have to be deliberate —
/// which is why the list has no bare `ad` or `banner` on it. Those match
/// "shadow", "headband" and half the class names on the web.
const AD_SUBSTRINGS: &[&str] = &[
    "advert",
    "adsbygoogle",
    "doubleclick",
    "googlesyndication",
    "taboola",
    "outbrain",
    "sharethrough",
    "sponsored",
    "-sponsor",
    "adslot",
    "ad-slot",
    "ad-container",
    "ad-wrapper",
    "ad-unit",
    "adunit",
    "ad-banner",
    "adsense",
    "interstitial",
];

/// Class *tokens* — matched whole, after splitting on whitespace. `ads` as a
/// token is an advert; `ads` inside "downloads" is not.
const AD_TOKENS: &[&str] = &["ad", "ads", "advertisement", "ad_slot", "sponsor", "promo"];

/// Hosts that exist only to count you.
const TRACKER_HOSTS: &[&str] = &[
    "google-analytics.com",
    "googletagmanager.com",
    "doubleclick.net",
    "facebook.com/tr",
    "scorecardresearch.com",
    "quantserve.com",
    "hotjar.com",
    "segment.io",
    "mixpanel.com",
    "amplitude.com",
];

pub fn sanitise(dom: &RcDom) -> Stripped {
    let mut out = Stripped::default();
    let root = dom::root(dom);
    scrub(&root, &mut out);
    out
}

fn scrub(node: &Handle, out: &mut Stripped) {
    dom::walk(node, &mut |node| {
        let Some(tag) = dom::tag(node) else {
            return Descend::Yes;
        };

        if let Some(reason) = subtree_verdict(node, &tag, out) {
            dom::replace_with_note(node, reason);
            return Descend::No;
        }

        clean_attributes(node, out);
        if tag.eq_str_ignore_ascii_case("style") {
            neutralise_style_element(node);
        }
        Descend::Yes
    });
}

/// Whether this whole subtree goes, and the note left in its place.
fn subtree_verdict(node: &Handle, tag: &LocalName, out: &mut Stripped) -> Option<&'static str> {
    let name = tag.as_ref();

    if name == "script" {
        out.scripts += 1;
        out.script_bytes += dom::children(node)
            .iter()
            .filter_map(|c| match &c.data {
                NodeData::Text { contents } => Some(contents.borrow().len()),
                _ => None,
            })
            .sum::<usize>();
        return Some(" script removed on capture ");
    }

    if DROP_SUBTREE.contains(&name) {
        if matches!(name, "iframe" | "frame" | "frameset" | "portal") {
            out.frames += 1;
            return Some(" frame removed on capture ");
        }
        out.media += 1;
        return Some(" embedded media removed on capture ");
    }

    if FORM_CONTROLS.contains(&name) {
        out.forms += 1;
        return Some(" control removed on capture ");
    }

    // A stylesheet link is a network fetch whichever host it names, and a
    // preload/prefetch is the same request under another name.
    if name == "link" {
        let rel = dom::attr(node, "rel").unwrap_or_default().to_ascii_lowercase();
        if rel.split_whitespace().any(|r| {
            matches!(
                r,
                "stylesheet" | "preload" | "prefetch" | "preconnect" | "dns-prefetch" | "modulepreload"
            )
        }) {
            out.stylesheets += 1;
            return Some(" stylesheet link removed on capture ");
        }
    }

    // `<meta http-equiv=refresh>` would navigate the viewer away from the
    // thing it just saved.
    if name == "meta" {
        let equiv = dom::attr(node, "http-equiv")
            .unwrap_or_default()
            .to_ascii_lowercase();
        if equiv == "refresh" {
            return Some(" meta refresh removed on capture ");
        }
    }

    if name == "img" && is_tracking_pixel(node) {
        out.tracking_pixels += 1;
        return Some(" tracking pixel removed on capture ");
    }

    if looks_like_advertising(node) {
        out.ad_containers += 1;
        return Some(" ad container removed on capture ");
    }

    None
}

/// A 1×1 image, or one served from a host in the analytics business.
fn is_tracking_pixel(node: &Handle) -> bool {
    let tiny = |name: &str| {
        dom::attr(node, name)
            .and_then(|v| v.trim().parse::<u32>().ok())
            .is_some_and(|n| n <= 1)
    };
    if tiny("width") || tiny("height") {
        return true;
    }
    let src = dom::attr(node, "src").unwrap_or_default().to_ascii_lowercase();
    TRACKER_HOSTS.iter().any(|h| src.contains(h))
}

fn looks_like_advertising(node: &Handle) -> bool {
    let marker = dom::class_and_id(node);
    if marker.is_empty() {
        return false;
    }
    if AD_SUBSTRINGS.iter().any(|s| marker.contains(s)) {
        return true;
    }
    marker
        .split(|c: char| c.is_whitespace())
        .any(|token| AD_TOKENS.contains(&token))
}

/// Drop every attribute that can execute or that points somewhere we will not
/// follow. Kept deliberately small: an allowlist of attributes would be safer
/// still, but it would also throw away the `colspan`, `dir` and `lang` that
/// make a captured chart legible, so this is a denylist with a catch-all for
/// the one pattern that matters — a URL whose scheme is not `http`.
fn clean_attributes(node: &Handle, out: &mut Stripped) {
    let mut handlers = 0;
    let mut urls = 0;
    dom::retain_attrs(node, |name, value| {
        // Every event handler, present and future. `on` + anything is the
        // whole of the naming convention and there is no legitimate
        // attribute in that space we would miss.
        if name.starts_with("on") && name.len() > 2 {
            handlers += 1;
            return false;
        }
        // `ping` is a beacon; `integrity` describes bytes we are about to
        // replace with our own. Note what is *not* here: `srcset` and the
        // `data-*` attributes lazy-loaders hide the real image URL in.
        // `assets::rewrite` needs to read those and then removes them itself
        // — see the ordering note on `sanitise`.
        if matches!(name, "ping" | "integrity") {
            return false;
        }
        if URL_ATTRS.contains(&name) && !safe_url(value) {
            urls += 1;
            return false;
        }
        if name == "style" && value.to_ascii_lowercase().contains("url(") {
            // Rare enough not to be worth a partial rewrite; a background
            // image in an inline style is decoration, not a chart.
            return false;
        }
        true
    });
    out.event_handlers += handlers;
    out.script_urls += urls;
}

const URL_ATTRS: &[&str] = &[
    "href",
    "src",
    "action",
    "formaction",
    "background",
    "poster",
    "data",
    "codebase",
    "cite",
    "longdesc",
    "xlink:href",
];

/// `http`, `https`, `mailto`, an inline image, or a fragment. Everything else
/// — `javascript:`, `vbscript:`, `data:text/html`, `file:` — goes.
fn safe_url(value: &str) -> bool {
    let trimmed = value.trim_start();
    let Some(colon) = trimmed.find(':') else {
        // No scheme at all: a relative URL or a fragment. Harmless.
        return true;
    };
    // A slash or a question mark before the colon means it was never a
    // scheme — `/a:b`, `?x=a:b`.
    if trimmed[..colon].contains(['/', '?', '#']) {
        return true;
    }
    let scheme = trimmed[..colon].to_ascii_lowercase();
    match scheme.as_str() {
        "http" | "https" | "mailto" => true,
        "data" => trimmed[colon + 1..].to_ascii_lowercase().starts_with("image/"),
        _ => false,
    }
}

/// Close the two doors inline CSS has onto the network: `@import` pulls in
/// another stylesheet, and `url()` with an absolute reference fetches an
/// image. Relative `url()` is left alone — it resolves against the attachment
/// directory and fails locally, which costs nothing.
fn neutralise_style_element(node: &Handle) {
    for child in dom::children(node) {
        if let NodeData::Text { contents } = &child.data {
            let cleaned = neutralise_css(&contents.borrow());
            *contents.borrow_mut() = cleaned.into();
        }
    }
}

pub(crate) fn neutralise_css(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let lower = css.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0;

    while i < css.len() {
        if lower[i..].starts_with("@import") {
            // Skip to the end of the at-rule: the next `;` or `}`.
            let end = bytes[i..]
                .iter()
                .position(|&b| b == b';' || b == b'}')
                .map(|p| i + p + 1)
                .unwrap_or(css.len());
            out.push_str("/* @import removed on capture */");
            i = end;
            continue;
        }
        if lower[i..].starts_with("url(") {
            let end = bytes[i..]
                .iter()
                .position(|&b| b == b')')
                .map(|p| i + p + 1)
                .unwrap_or(css.len());
            let inner = css[i + 4..end.saturating_sub(1)].trim().trim_matches(['"', '\'']);
            if inner.contains("//") || inner.contains(':') {
                out.push_str("url(about:blank)");
            } else {
                out.push_str(&css[i..end]);
            }
            i = end;
            continue;
        }
        let ch = css[i..].chars().next().expect("in bounds");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::dom;

    fn scrubbed(html: &str) -> (String, Stripped) {
        let doc = dom::parse(html.as_bytes());
        let tally = sanitise(&doc);
        (dom::to_html(&dom::root(&doc)), tally)
    }

    #[test]
    fn a_script_leaves_a_note_and_nothing_else() {
        let (html, tally) = scrubbed(r#"<p>keep</p><script>fetch('/x')</script>"#);
        assert!(html.contains("keep"));
        assert!(!html.contains("fetch("), "the script body is gone: {html}");
        assert!(html.contains("script removed on capture"));
        assert_eq!(tally.scripts, 1);
        assert!(tally.script_bytes > 0, "the weight of the script is recorded");
    }

    #[test]
    fn every_way_of_executing_something_is_closed() {
        let (html, tally) = scrubbed(
            r#"<div onclick="steal()" onmouseover="x()">
                 <a href="javascript:alert(1)">t</a>
                 <a href="HTTPS://example.com/ok">u</a>
                 <iframe src="https://ads.example/frame"></iframe>
                 <img src="data:text/html;base64,PHNjcmlwdD4=">
               </div>"#,
        );
        assert!(!html.contains("onclick"));
        assert!(!html.contains("onmouseover"));
        assert!(!html.contains("javascript:"));
        assert!(!html.contains("data:text/html"));
        assert!(!html.contains("<iframe"));
        assert!(html.contains("example.com/ok"), "a real link survives");
        assert_eq!(tally.event_handlers, 2);
        assert_eq!(tally.frames, 1);
    }

    #[test]
    fn nothing_left_behind_can_reach_the_network() {
        let (html, _) = scrubbed(
            r#"<link rel="stylesheet" href="https://cdn.example/site.css">
               <link rel="preload" href="/local.css" as="style">
               <link rel="icon" href="/favicon.ico">
               <style>@import url("https://cdn.example/more.css");
                      .chord { background: url(https://cdn.example/bg.png); }
                      .lyric { background: url(local.png); }</style>"#,
        );
        assert!(!html.contains("site.css"));
        assert!(!html.contains("local.css"), "a preload is the same request");
        assert!(!html.contains("more.css"));
        assert!(!html.contains("cdn.example/bg.png"));
        assert!(html.contains("url(local.png)"), "a relative url costs nothing");
        assert!(html.contains(".chord"), "the rules themselves survive");
    }

    #[test]
    fn advertising_goes_and_content_stays() {
        let (html, tally) = scrubbed(
            r#"<div class="ad-container"><span>buy</span></div>
               <div class="adsbygoogle">x</div>
               <div class="downloads">the chart</div>
               <div class="headband">a heading</div>
               <img src="https://www.google-analytics.com/collect?v=1">
               <img src="/pixel.gif" width="1" height="1">
               <img src="/chart.png" width="640">"#,
        );
        assert!(!html.contains("buy"));
        assert!(!html.contains("adsbygoogle"));
        assert!(html.contains("the chart"), "'downloads' is not 'ads'");
        assert!(html.contains("a heading"), "'headband' is not 'banner'");
        assert!(html.contains("/chart.png"));
        assert_eq!(tally.ad_containers, 2);
        assert_eq!(tally.tracking_pixels, 2);
    }

    #[test]
    fn a_meta_refresh_cannot_navigate_the_viewer_away() {
        let (html, _) = scrubbed(r#"<meta http-equiv="refresh" content="0;url=https://x/">"#);
        assert!(!html.contains("http-equiv"));
    }

    #[test]
    fn a_url_with_a_colon_in_its_path_is_not_a_scheme() {
        assert!(safe_url("/songs/a:b"));
        assert!(safe_url("#verse"));
        assert!(safe_url("?q=a:b"));
        assert!(safe_url("  HtTpS://example.com"));
        assert!(!safe_url("JaVaScRiPt:alert(1)"));
        assert!(!safe_url("vbscript:x"));
        assert!(!safe_url("file:///etc/passwd"));
        assert!(safe_url("data:image/png;base64,aa"));
        assert!(!safe_url("data:text/html,<script>"));
    }
}
