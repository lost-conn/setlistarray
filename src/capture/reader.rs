//! Reader extraction: finding the chart and throwing the site away.
//!
//! E3 offers "Reader text" against "Full page", and this is the half that
//! makes the choice real. It is a readability-style heuristic — score the
//! blocks that hold text, roll those scores up into their containers, take the
//! best container — run over the DOM we have already parsed. No new dependency
//! and no network.
//!
//! ## Why not `dom_smoothie` or another readability port
//!
//! Because generic readability is actively wrong for a chord chart. Every
//! implementation of it descends from arc90's, and arc90's scoring rewards
//! long sentences and commas and punishes short lines — which is a precise
//! description of what it does to a chart. Four bars of chord symbols over a
//! line of lyric scores at the bottom of every candidate list, and a
//! `<pre>`-only page can score *zero*. The two rules below are the difference
//! between reader mode being useful here and being a way to delete the song:
//!
//! * **`<pre>` is content, heavily.** A monospaced block is the single
//!   strongest signal a page carries that it holds a chart.
//! * **`class` and `id` containing `chord`, `tab` or `lyric` are a bonus**,
//!   the mirror of the `comment`/`sidebar`/`share` penalty every readability
//!   carries.
//!
//! A page whose chart is in `<pre>` gets a hard override: whatever the scores
//! say, the extracted node must be an ancestor of the preformatted text. It is
//! better to keep a little furniture than to lose the reason the page was
//! saved.

use markup5ever_rcdom::{Handle, RcDom};
use std::rc::Rc;

use super::dom::{self, Descend};

/// Class and id fragments that mean "this is the article".
const POSITIVE: &[&str] = &[
    "article", "body", "content", "entry", "hentry", "main", "page", "post", "story", "text",
    "chord", "tab", "lyric", "song", "sheet", "verse", "chorus",
];

/// ...and the ones that mean "this is the site".
const NEGATIVE: &[&str] = &[
    "comment", "combx", "contact", "foot", "header", "masthead", "menu", "meta", "nav", "related",
    "remark", "rss", "shoutbox", "sidebar", "skyscraper", "sponsor", "share", "social", "promo",
    "pagination", "pager", "popup", "modal", "toolbar", "breadcrumb", "cookie", "newsletter",
    "subscribe", "signup", "banner", "widget", "tags", "author-box",
];

/// Elements that can hold a paragraph's worth of text.
const CANDIDATES: &[&str] = &[
    "p",
    "pre",
    "td",
    "blockquote",
    "article",
    "section",
    "div",
    "li",
    "dd",
];

/// Below this, a page has no reader view worth offering and the caller should
/// keep the full page instead.
const MIN_USEFUL_CHARS: usize = 140;

/// The node holding the article, still attached to `dom`.
///
/// `None` means "nothing here scored well enough" — a JS shell, a directory
/// page, a 404. The caller falls back to the full page rather than saving an
/// empty one.
pub fn extract(dom: &RcDom) -> Option<Handle> {
    let root = dom::root(dom);

    let mut scores: Vec<(Handle, f32)> = Vec::new();
    let mut add = |node: &Handle, amount: f32| {
        match scores.iter_mut().find(|(h, _)| Rc::ptr_eq(h, node)) {
            Some((_, existing)) => *existing += amount,
            None => scores.push((node.clone(), amount)),
        }
    };

    dom::walk(&root, &mut |node| {
        let Some(tag) = dom::tag(node) else {
            return Descend::Yes;
        };
        if !CANDIDATES.contains(&tag.as_ref()) {
            return Descend::Yes;
        }

        let text = dom::text_of(node);
        let own = text.chars().count();
        if own < 25 && tag.as_ref() != "pre" {
            return Descend::Yes;
        }

        let mut score = 1.0;
        score += text.matches(',').count() as f32;
        score += (own as f32 / 100.0).min(3.0);

        // The chord-chart thumb on the scale. A 600-character `<pre>` beats
        // any amount of surrounding prose, which is the whole point.
        if tag.as_ref() == "pre" {
            score += (own as f32 / 25.0).min(40.0);
        }

        // The score belongs to the *container*, not the paragraph: readability's
        // central trick. A div with eight scoring children beats any one of
        // them, which is how the article beats its own first paragraph.
        add(node, score);
        if let Some(parent) = dom::parent(node) {
            add(&parent, score);
            if let Some(grandparent) = dom::parent(&parent) {
                add(&grandparent, score / 2.0);
                if let Some(great) = dom::parent(&grandparent) {
                    add(&great, score / 3.0);
                }
            }
        }
        Descend::Yes
    });

    let mut best: Option<(Handle, f32)> = None;
    for (node, raw) in &scores {
        // A `<body>` or `<html>` always accumulates the most; taking it would
        // make reader mode a no-op.
        if dom::is(node, "body") || dom::is(node, "html") {
            continue;
        }
        let adjusted = raw * marker_multiplier(node) * (1.0 - link_density(node));
        if best.as_ref().is_none_or(|(_, top)| adjusted > *top) {
            best = Some((node.clone(), adjusted));
        }
    }

    let mut chosen = best.map(|(node, _)| node)?;

    // The override. If the page carries a chart at all, the thing we keep has
    // to contain it — no score is allowed to argue otherwise.
    match chart_anchor(&root) {
        // The chart is spread across the whole body: there is no smaller node
        // that holds it, so there is nothing for reader mode to narrow to.
        // Saying so and keeping the full page is the honest answer.
        Some(anchor) if dom::is(&anchor, "body") || dom::is(&anchor, "html") => return None,
        Some(anchor) => {
            if !contains(&chosen, &anchor) {
                chosen = anchor;
            }
        }
        None => {}
    }

    if dom::is(&chosen, "body") || dom::is(&chosen, "html") {
        return None;
    }
    if dom::text_of(&chosen).chars().count() < MIN_USEFUL_CHARS {
        return None;
    }
    Some(chosen)
}

/// One piece of evidence that a chart lives here, and how much it weighs.
///
/// Two kinds count, because chord sites come in two kinds. Some put the chart
/// in a `<pre>` and let the monospace do the aligning; others wrap every chord
/// in its own `<span class="chord">` and position it in CSS. The second kind
/// is invisible to any measurement of text — the spans are two characters each
/// — which is why they are weighted by count rather than by length.
///
/// The 25 is arbitrary and only has to be in the right order of magnitude: it
/// makes one chord marker worth about one short line of preformatted text, so
/// that neither kind of page can drown out the other.
const CHORD_MARKER_WEIGHT: usize = 25;

/// The smallest node holding most of the page's chart.
///
/// Measured, not guessed: without this, reader extraction dropped the chart on
/// three of the four capturable sites in the E1 spike. Scoring alone keeps the
/// *article*, and on a chord site the article is the write-up around the chart
/// rather than the chart itself.
///
/// "Most" rather than "all" because sites put a transposed second copy, or a
/// key legend, somewhere else on the page entirely. Insisting on every last
/// marker walks straight back up to `<body>`.
fn chart_anchor(root: &Handle) -> Option<Handle> {
    let total = chart_weight(root);
    if total < 200 {
        return None;
    }

    let mut blocks: Vec<(Handle, usize)> = Vec::new();
    dom::walk(root, &mut |node| {
        if dom::is(node, "pre") {
            blocks.push((node.clone(), dom::text_of(node).chars().count()));
            return Descend::No;
        }
        if is_chord_marked(node) {
            blocks.push((node.clone(), CHORD_MARKER_WEIGHT));
        }
        Descend::Yes
    });

    let (heaviest, _) = blocks.into_iter().max_by_key(|(_, weight)| *weight)?;
    let mut node = heaviest;
    loop {
        if chart_weight(&node) * 5 >= total * 4 || dom::is(&node, "body") {
            return Some(node);
        }
        match dom::parent(&node) {
            Some(parent) => node = parent,
            None => return Some(node),
        }
    }
}

fn chart_weight(node: &Handle) -> usize {
    let mut total = 0;
    dom::walk(node, &mut |candidate| {
        if dom::is(candidate, "pre") {
            total += dom::text_of(candidate).chars().count();
            return Descend::No;
        }
        if is_chord_marked(candidate) {
            total += CHORD_MARKER_WEIGHT;
        }
        Descend::Yes
    });
    total
}

/// Whether `node` is `ancestor` or sits somewhere beneath it.
fn contains(ancestor: &Handle, node: &Handle) -> bool {
    let mut current = Some(node.clone());
    while let Some(step) = current {
        if Rc::ptr_eq(&step, ancestor) {
            return true;
        }
        current = dom::parent(&step);
    }
    false
}

fn is_chord_marked(node: &Handle) -> bool {
    let marker = dom::class_and_id(node);
    !marker.is_empty()
        && (marker.contains("chord") || marker.contains("lyric") || marker.contains("tablature"))
}

/// 1.25 for a container that names itself content, 0.4 for one that names
/// itself furniture, 1.0 for the anonymous majority. Multiplicative rather
/// than additive so it scales with how much text is actually there — a
/// sidebar with one line does not need a penalty to lose.
fn marker_multiplier(node: &Handle) -> f32 {
    let marker = dom::class_and_id(node);
    if marker.is_empty() {
        return 1.0;
    }
    let mut factor = 1.0;
    if POSITIVE.iter().any(|p| marker.contains(p)) {
        factor *= 1.25;
    }
    if NEGATIVE.iter().any(|n| marker.contains(n)) {
        factor *= 0.4;
    }
    factor
}

/// The share of this node's text that sits inside a link. A navigation block
/// is nearly all link; an article is nearly none.
fn link_density(node: &Handle) -> f32 {
    let total = dom::text_of(node).chars().count();
    if total == 0 {
        return 0.0;
    }
    let mut linked = 0usize;
    dom::walk(node, &mut |candidate| {
        if dom::is(candidate, "a") {
            linked += dom::text_of(candidate).chars().count();
            return Descend::No;
        }
        Descend::Yes
    });
    (linked as f32 / total as f32).min(0.95)
}

/// Replace the document body with the extracted node.
///
/// `<head>` is kept as it is, which keeps the `<style>` blocks the sanitiser
/// left in place — the chord-over-lyric positioning on most of these sites is
/// CSS, and a reader view that dropped it would stack the chord row on top of
/// the wrong words.
///
/// ## The move has to happen before the sweep
///
/// `markup5ever_rcdom::Node` has a hand-written `Drop` that avoids blowing the
/// stack on a deep tree by walking its descendants iteratively and
/// `mem::take`-ing the `children` vector out of **every node it reaches** — and
/// it does that whether or not somebody else still holds a strong reference to
/// one. So detaching an ancestor of the node we are keeping, and letting it
/// fall out of scope, silently empties the node we are keeping. It stays
/// alive, it keeps its tag and its attributes, and its entire subtree is gone.
///
/// That is exactly what happened in the E1 spike: reader mode wrote out
/// `<div class="row main-content"></div>` and a 3.7 KB file where the chart
/// had been, on a page whose extraction had picked precisely the right node.
///
/// Moving the keeper under `<body>` first takes it out of its old parent's
/// child list, so the sweep that follows can never walk into it.
pub fn narrow_to(dom: &RcDom, chosen: &Handle) {
    let Some(body) = dom::find_element(&dom.document, &html5ever::local_name!("body")) else {
        return;
    };
    // Appending the body to itself empties the document. `extract` never
    // returns one, but this function is public and the failure is silent.
    if Rc::ptr_eq(&body, chosen) || dom::is(chosen, "html") {
        return;
    }

    dom::append(&body, chosen);
    for child in dom::children(&body) {
        if !Rc::ptr_eq(&child, chosen) {
            dom::detach(&child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extracted(html: &str) -> String {
        let doc = dom::parse(html.as_bytes());
        let node = extract(&doc).expect("something scored");
        dom::text_of(&node)
    }

    const NAV: &str = r#"<nav class="site-nav"><a href="/a">Songs</a><a href="/b">Artists</a>
        <a href="/c">Submit</a><a href="/d">Forum</a><a href="/e">Sign in</a></nav>"#;
    const SIDEBAR: &str = r#"<aside class="sidebar related"><a href="/1">More by this artist</a>
        <a href="/2">Similar songs</a><a href="/3">Top 100 of the week on this site</a></aside>"#;

    #[test]
    fn the_article_wins_over_the_furniture() {
        let text = extracted(&format!(
            r#"<body>{NAV}
               <div class="post-content"><p>{}</p><p>{}</p></div>
               {SIDEBAR}</body>"#,
            "A paragraph about how the song is played, with commas, clauses, and length. "
                .repeat(3),
            "A second paragraph, also of some substance, to give the container something to add up."
        ));
        assert!(text.contains("A paragraph about how the song is played"));
        assert!(!text.contains("More by this artist"), "sidebar dropped: {text}");
        assert!(!text.contains("Submit"), "nav dropped: {text}");
    }

    #[test]
    fn a_chart_in_a_pre_beats_a_wall_of_prose() {
        // The case generic readability gets wrong: short lines, no commas,
        // against three paragraphs of chatty prose that would out-score it.
        let chart: String = (1..=12)
            .map(|n| format!("G       C       D\nline {n} of the words here\n"))
            .collect();
        let prose = "Some background about the recording session, with commas, and clauses, and a good deal of length to it. ".repeat(6);
        let text = extracted(&format!(
            r#"<body>{NAV}<div class="story"><p>{prose}</p></div>
               <div class="chord-sheet"><pre>{chart}</pre></div>{SIDEBAR}</body>"#
        ));
        assert!(text.contains("line 12 of the words here"), "the chart survived");
    }

    #[test]
    fn a_pre_only_page_still_extracts() {
        let chart: String = (1..=20)
            .map(|n| format!("Am      F       C\nline {n}\n"))
            .collect();
        let text = extracted(&format!("<body><div id=\"tab\"><pre>{chart}</pre></div></body>"));
        assert!(text.contains("line 20"));
    }

    #[test]
    fn a_chart_marked_up_as_spans_survives_with_no_pre_anywhere() {
        // The shape that broke reader mode in the spike: no `<pre>`, every
        // chord its own two-character span, positioned in CSS. Nothing about
        // it scores, and scoring alone kept the write-up and dropped the song.
        let sheet: String = (1..=30)
            .map(|n| {
                format!(
                    r#"<div class="line"><span class="chord">G</span><span class="lyric">l{n}</span></div>"#
                )
            })
            .collect();
        let prose = "Notes on the arrangement, with commas, and clauses, and length. ".repeat(10);
        let doc = dom::parse(
            format!(
                r#"<body>{NAV}<div class="entry-content"><p>{prose}</p></div>
                   <div id="chordsheet">{sheet}</div>{SIDEBAR}</body>"#
            )
            .as_bytes(),
        );
        let node = extract(&doc).expect("something scored");
        let kept = dom::to_html(&node);
        assert!(kept.matches("class=\"chord\"").count() >= 24, "the chords survived");
        assert!(!kept.contains("More by this artist"));
    }

    #[test]
    fn a_chart_spread_across_the_whole_body_falls_back_to_the_full_page() {
        // No container holds the chart, so there is nothing to narrow to and
        // reader mode says so rather than saving half a song.
        let sheet: String = (1..=30)
            .map(|n| format!(r#"<span class="chord">C</span><span class="lyric">l{n}</span>"#))
            .collect();
        let doc = dom::parse(format!("<body>{NAV}{sheet}{SIDEBAR}</body>").as_bytes());
        assert!(extract(&doc).is_none());
    }

    #[test]
    fn a_javascript_shell_extracts_nothing() {
        let doc = dom::parse(br#"<body><div id="__next"></div></body>"#);
        assert!(extract(&doc).is_none());
    }

    #[test]
    fn narrowing_does_not_gut_the_node_it_is_keeping() {
        // The regression that cost the spike an afternoon. `RcDom`'s iterative
        // `Drop` empties the `children` of every descendant it walks, so
        // detaching the keeper's own ancestor first leaves the keeper alive
        // and hollow. It only shows up when the chart is nested — a chart that
        // happens to be a direct child of `<body>` passes either way.
        let chart: String = (1..=20).map(|n| format!("Am  F  C\nline {n}\n")).collect();
        let doc = dom::parse(
            format!(
                r#"<html><body class="site"><div class="wrapper"><div class="container">
                   {NAV}<div id="tab"><pre>{chart}</pre></div>{SIDEBAR}
                   </div></div></body></html>"#
            )
            .as_bytes(),
        );
        let chosen = extract(&doc).expect("something scored");
        assert!(!dom::children(&chosen).is_empty(), "the node starts with content");
        narrow_to(&doc, &chosen);
        assert!(
            !dom::children(&chosen).is_empty(),
            "and still has it after narrowing"
        );

        let html = dom::to_html(&dom::root(&doc));
        assert!(html.contains("line 20"), "the chart is in the document: {html}");
        assert!(!html.contains("More by this artist"));
    }

    #[test]
    fn narrowing_keeps_the_head_and_replaces_the_body() {
        let chart: String = (1..=20).map(|n| format!("Am  F  C\nline {n}\n")).collect();
        let doc = dom::parse(
            format!(
                r#"<html><head><style>.chord{{color:red}}</style></head>
                   <body>{NAV}<div id="tab"><pre>{chart}</pre></div>{SIDEBAR}</body></html>"#
            )
            .as_bytes(),
        );
        let chosen = extract(&doc).unwrap();
        narrow_to(&doc, &chosen);
        let html = dom::to_html(&dom::root(&doc));
        assert!(html.contains(".chord{color:red}"), "the stylesheet stays: {html}");
        assert!(html.contains("line 20"));
        assert!(!html.contains("More by this artist"));
    }
}
