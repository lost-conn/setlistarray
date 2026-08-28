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

// ────────────────────────────────────────────────────────────────────────────
// Charts whose alignment lived in a stylesheet — card E3's answer to E8
// ────────────────────────────────────────────────────────────────────────────

/// Class and id fragments that mark a container as part of a chart.
const CHART_MARKERS: &[&str] = &["chord", "tablature"];

/// Elements that flow inside a line rather than starting a new one.
///
/// The list decides whether a chord block's element children are *lines* or the
/// insides of one, which is the whole difference between a chart and a column
/// of syllables. It is short on purpose: everything not on it is treated as a
/// block, which is what a browser with no stylesheet would do and what the
/// sites in question are relying on their stylesheet to override.
const INLINE: &[&str] = &[
    "span", "b", "i", "em", "strong", "a", "sup", "sub", "small", "u", "font", "code", "abbr",
    "mark", "label",
];

/// The longest a run of characters can be and still be a chord symbol.
///
/// `Cmaj7(#11)/G#` is thirteen and is the longest thing anybody writes over a
/// syllable; twelve covers everything seen on the sites in `docs/CAPTURE.md`
/// and is short enough that a paragraph in a `class="chord-text"` wrapper is
/// never mistaken for a chord. The rule that does most of the work is not the
/// length but the absence of whitespace: a chord symbol is one token.
const CHORD_SYMBOL_MAX: usize = 12;

/// How many chord symbols a page needs before this rebuild is attempted at all.
///
/// One is an accident — a stray `class="chords"` on a link, a "Chord version"
/// menu entry. Two in the same document is a chart. The gate is on the whole
/// page rather than on each block so that a chart split across six verses,
/// five of which are lyric-only, is rebuilt as six consistent blocks instead of
/// one monospaced verse followed by five proportional ones.
const MIN_CHORD_SYMBOLS: usize = 2;

/// Rebuild every chord chart whose layout was a stylesheet, as preformatted
/// text. Returns how many blocks were rebuilt.
///
/// ## The decision this function *is*
///
/// Card E5 found — and card E8 wrote up — that hymnal.net positions its chord
/// symbols with a `<link>`ed stylesheet, which E1 drops at capture time because
/// a stylesheet is a request that fails on a train. The saved page is therefore
/// a run of `<div class="chord-text">` boxes that the missing sheet would have
/// made `inline-block` with the chord `block` above the syllable — and without
/// it every one of them is a block, so the chart renders **one syllable per
/// line**. E8 offered two ways out and left the choice to E3:
///
/// 1. keep a same-origin `<style>` block and rewrite every selector in it to a
///    scoping prefix, so a stranger's CSS cannot escape into the app's single
///    Stylo stylist; or
/// 2. recognise the chord-above-syllable pattern structurally and re-emit it as
///    something that lays out on its own.
///
/// **This is the second, and the argument for it is not that the first is hard.**
/// It is that the first cannot work here even if the scoping were airtight.
/// Measured on the capture E5 was built against: hymnal.net's saved page
/// contains **zero** `<style>` blocks, because all of its CSS was in the linked
/// sheet that was dropped. Scoping preserves inline and same-document CSS; the
/// layout that is actually missing is in neither. To get it back the capture
/// would have to fetch and keep the site's stylesheets — which is a second
/// class of remote request to make offline, a second thing to re-check in E6,
/// and a bet that a sheet written for a 1200px desktop column degrades sanely
/// on a phone. The class that hides hymnal's whole chord scaffold is
/// `hidden`; the "high-fidelity" render of that page is a column of
/// one-syllable lines that the site never intended anybody to see.
///
/// The app wants a readable chart, not a faithful screenshot of somebody's
/// website. So the chart is *rebuilt*: each line is folded back into the two
/// rows it was drawn as — chord symbols over the column of the syllable they
/// belong to — and emitted as a `<pre>`. That is the same shape CifraClub and
/// guitaretab already serve, which means one rendering path serves all three
/// instead of two, and it is the shape `chart_editor` lets somebody type by
/// hand. It also survives everything downstream for free: `dom::text_of` copies
/// a `<pre>` verbatim, so the aligned chart is what lands in
/// `Attachment::body` for card G2's search and for the text `render::from_text`
/// falls back to.
///
/// ## Why only reader mode calls this
///
/// Because full-page mode's promise is *this is what the site served*, and a
/// mode that quietly rewrote the markup would be lying about the one thing it
/// exists to offer. The two modes now differ in kind rather than in size:
/// full page is the site, reader text is the app's reading of it. That is also
/// what makes reader the default — see [`super::CaptureMode`].
pub fn rebuild_chord_blocks(root: &Handle) -> usize {
    if count_symbols(root) < MIN_CHORD_SYMBOLS {
        return 0;
    }

    // Collected first, rebuilt afterwards. The rebuild replaces nodes and
    // detaches their siblings, and mutating a tree while walking it is how the
    // `Drop` fault in `narrow_to` above was found the first time.
    let mut blocks = Vec::new();
    collect_blocks(root, &mut blocks);

    let mut rebuilt = 0;
    for block in blocks {
        let Some(chart) = fold_block(&block) else {
            continue;
        };
        drop_plain_twin(&block, &chart.lyrics_only);
        let pre = dom::new_element("pre");
        dom::append(&pre, &dom::new_text(&chart.text));
        dom::replace_with(&block, &pre);
        rebuilt += 1;
    }
    rebuilt
}

/// The chord symbol this element holds, if it is one.
///
/// Two rules, and the second is the one that does the work. A chord symbol is
/// marked as a chord by the site — that is what the class is for — and it is a
/// single token: `G`, `D7`, `A7(4)`, `Am/E`. Its wrapper is marked the same way
/// on every site that does this (hymnal's `chord-text` sits inside
/// `chord-container`), so without the token rule the wrapper, the container and
/// the whole chart would all answer to "is this a chord".
///
/// `dom::text_of` rather than the raw text because the symbol is not always one
/// text node: hymnal writes a seventh as `D<sup>7</sup>`, and the thing to put
/// over the syllable is `D7`.
///
/// The third rule — **the symbol is the innermost chord-marked element** — is
/// the one that had to be measured. Without it a wrapper whose syllable happens
/// to be short is a chord: `<div class="chord-text"><span
/// class="chord">D</span>Ev</div>` reads as the two-token-free string `DEv`,
/// and the whole line comes out as `DEv D7er, G` over nothing. Length and
/// whitespace cannot tell a wrapper from a symbol on their own, because a
/// wrapper is only ever as long as the syllable inside it.
fn chord_symbol(node: &Handle) -> Option<String> {
    dom::tag(node)?;
    let marker = dom::class_and_id(node);
    if !CHART_MARKERS.iter().any(|m| marker.contains(m)) {
        return None;
    }
    if holds_marked_element(node) {
        return None;
    }
    let text = dom::text_of(node);
    let symbol = text.trim();
    if symbol.is_empty() || symbol.chars().count() > CHORD_SYMBOL_MAX {
        return None;
    }
    if symbol.chars().any(char::is_whitespace) {
        return None;
    }
    Some(symbol.to_string())
}

/// Whether anything beneath this node is itself marked as part of a chart.
fn holds_marked_element(node: &Handle) -> bool {
    dom::children(node).iter().any(|child| {
        let marker = dom::class_and_id(child);
        CHART_MARKERS.iter().any(|m| marker.contains(m)) || holds_marked_element(child)
    })
}

fn count_symbols(root: &Handle) -> usize {
    let mut total = 0;
    dom::walk(root, &mut |node| {
        // Nothing inside a `<pre>` is a candidate. A site that already ships a
        // preformatted chart has done the alignment itself, and CifraClub marks
        // every chord in its `<pre>` with a `data-chord-name` — the day one of
        // them reaches for a class as well, this rule is what stops the chart
        // being taken apart and rebuilt worse.
        if dom::is(node, "pre") {
            return Descend::No;
        }
        if chord_symbol(node).is_some() {
            total += 1;
            return Descend::No;
        }
        Descend::Yes
    });
    total
}

/// The outermost chart containers, one per verse on the sites that do this.
///
/// "Outermost" is what stops the same chart being rebuilt three times from the
/// inside out: hymnal's `chord-container`, `chord-text` and `chord` all carry a
/// marker, and only the first of them is a block worth folding.
fn collect_blocks(node: &Handle, out: &mut Vec<Handle>) {
    for child in dom::children(node) {
        if dom::tag(&child).is_none() || dom::is(&child, "pre") {
            continue;
        }
        // A block that holds a `<pre>` is a wrapper around a chart the site
        // aligned itself. Leave it alone and keep looking underneath it.
        let marker = dom::class_and_id(&child);
        let marked = CHART_MARKERS.iter().any(|m| marker.contains(m));
        if marked && chord_symbol(&child).is_none() && !holds_pre(&child) {
            out.push(child);
            continue;
        }
        collect_blocks(&child, out);
    }
}

fn holds_pre(node: &Handle) -> bool {
    let mut found = false;
    dom::walk(node, &mut |candidate| {
        if dom::is(candidate, "pre") {
            found = true;
            return Descend::No;
        }
        Descend::Yes
    });
    found
}

/// One line of a chart, as the two rows it was drawn as.
#[derive(Default)]
struct Rows {
    chords: String,
    lyrics: String,
}

/// A rebuilt block: the preformatted chart, and the lyrics on their own.
struct Chart {
    /// What goes into the `<pre>`.
    text: String,
    /// The same words without the chord rows, for [`drop_plain_twin`].
    lyrics_only: String,
}

/// The lines of a chord block.
///
/// A line is an element child that starts a new one — anything not in
/// [`INLINE`]. A block whose element children are all inline, or all chord
/// symbols, is itself one line: that is the
/// `<span class="chord">G</span>syllable` shape with no wrapper, and treating
/// its two spans as two lines would produce exactly the one-syllable-per-line
/// output this function exists to undo.
fn lines_of(block: &Handle) -> Vec<Handle> {
    let children: Vec<Handle> = dom::children(block)
        .into_iter()
        .filter(|c| dom::tag(c).is_some())
        .collect();
    let all_inline = children.iter().all(|c| {
        chord_symbol(c).is_some()
            || dom::tag(c).is_some_and(|t| INLINE.contains(&t.as_ref()))
    });
    if children.is_empty() || all_inline {
        return vec![block.clone()];
    }
    children
}

fn fold_block(block: &Handle) -> Option<Chart> {
    let mut text = String::new();
    let mut lyrics_only = String::new();
    let mut any = false;

    for line in lines_of(block) {
        let mut folded = vec![Rows::default()];
        fold(&line, &mut folded);
        for rows in folded {
            let chords = rows.chords.trim_end();
            let lyrics = rows.lyrics.trim_end();
            if !chords.is_empty() {
                text.push_str(chords);
                text.push('\n');
                any = true;
            }
            if !lyrics.is_empty() {
                any = true;
            }
            text.push_str(lyrics);
            text.push('\n');
            lyrics_only.push_str(lyrics);
            lyrics_only.push('\n');
        }
    }

    // A block that folded to nothing is a wrapper the site left empty. Putting
    // an empty `<pre>` in its place would trade an invisible div for a visible
    // gap, and `render`'s own pruning would then have to throw it away again.
    any.then_some(Chart { text, lyrics_only })
}

/// Fold one line's subtree into its chord row and its lyric row.
///
/// `out` is a list rather than a single pair because a `<br>` inside the line
/// is a second line, and on a site that writes a whole verse as one `<div>`
/// with `<br>`s in it that is the only thing separating the lines at all.
fn fold(node: &Handle, out: &mut Vec<Rows>) {
    for child in dom::children(node) {
        if let Some(text) = dom::text_content(&child) {
            push_lyric(&text, out.last_mut().expect("one row to start with"));
            continue;
        }
        if dom::tag(&child).is_none() {
            continue;
        }
        if dom::is(&child, "br") {
            out.push(Rows::default());
            continue;
        }
        if let Some(symbol) = chord_symbol(&child) {
            place_chord(&symbol, out.last_mut().expect("one row to start with"));
            continue;
        }
        fold(&child, out);
    }
}

/// Add a run of a site's characters to the lyric row.
///
/// One rule, and it is the rule HTML itself already imposes: **a non-breaking
/// space is content and every other kind of whitespace is markup.** A site
/// cannot indent a chart with ordinary spaces, because the browser collapses
/// them — which is precisely why these pages are full of `&nbsp;`. So the
/// non-breaking spaces are kept one for one, and they are the only thing
/// carrying the indent of a continuation line; everything else collapses to a
/// single space the way a browser would collapse it.
///
/// Getting this wrong is not subtle and was found by writing the test fixture
/// rather than by reading the site. The real hymnal.net markup pretty-prints
/// with bare newlines, which an earlier version of this stripped outright — and
/// that version passed against the live page and produced
/// `               G                                         C` against a
/// fixture indented with spaces, because eight spaces of somebody's HTML
/// formatter had become eight spaces of chart.
fn push_lyric(raw: &str, rows: &mut Rows) {
    for ch in raw.chars() {
        match ch {
            '\u{a0}' => rows.lyrics.push(' '),
            other if other.is_whitespace() => {
                if !rows.lyrics.is_empty() && !rows.lyrics.ends_with(' ') {
                    rows.lyrics.push(' ');
                }
            }
            other => rows.lyrics.push(other),
        }
    }
}

/// Put a chord symbol over the column the words after it start at.
///
/// The one interesting case is a syllable shorter than the chord above it —
/// `D` and `D7` over `Ev` / `er` in hymn 1. The chord row cannot simply be
/// padded, because the second chord would start before the first had finished;
/// so the *lyric* is pushed along instead, which is what every chord-chart
/// renderer does and what somebody typing one by hand does. The alternative —
/// letting the two chords run together — loses which syllable each belongs to,
/// and that is the only information the chart carries.
fn place_chord(symbol: &str, rows: &mut Rows) {
    let mut column = rows.lyrics.chars().count();
    let so_far = rows.chords.chars().count();
    if so_far > 0 && so_far + 1 > column {
        for _ in 0..(so_far + 1 - column) {
            rows.lyrics.push(' ');
        }
        column = so_far + 1;
    }
    while rows.chords.chars().count() < column {
        rows.chords.push(' ');
    }
    rows.chords.push_str(symbol);
}

/// Drop the plain copy of a verse the site keeps beside the chord copy.
///
/// hymnal.net ships every verse twice: a `text-container` of lyrics, and a
/// `chord-container` of the same lyrics with the chords positioned over them,
/// hidden behind a "show chords" toggle whose `hidden` class was in the
/// stylesheet that went. Both are in the saved page, so without this every
/// verse is read out twice — once without chords and once with.
///
/// The test is the text rather than the class, because `hidden` is the wrong
/// signal in the worst possible way: the copy the site hides is the one this
/// app wants. Two siblings that say the same words are one verse, and the copy
/// that carries the chords is the one to keep.
fn drop_plain_twin(block: &Handle, lyrics_only: &str) {
    let wanted = normalised(lyrics_only);
    if wanted.is_empty() {
        return;
    }
    let Some(parent) = dom::parent(block) else {
        return;
    };
    for sibling in dom::children(&parent) {
        if Rc::ptr_eq(&sibling, block) || dom::tag(&sibling).is_none() {
            continue;
        }
        if normalised(&dom::text_of(&sibling)) == wanted {
            dom::detach(&sibling);
        }
    }
}

/// Two copies of a verse, reduced to the thing they have in common: their
/// letters.
///
/// **Whitespace is removed rather than collapsed**, and the typographic
/// punctuation is folded to ASCII. Both rules were written against the live
/// page after a version that only collapsed whitespace failed to drop a single
/// twin on hymnal.net, while passing against a fixture:
///
/// * The chord copy splits a word wherever the chord changes inside it —
///   `Ever` is `Ev` under a `D` and `er` under a `D7`, so the rebuilt line
///   says `Ev er` and the plain copy says `Ever`. Where the site broke the
///   word is not part of what the words *are*.
/// * The two copies do not even use the same apostrophe. The plain verse has
///   `e’en` (U+2019) and the chord scaffold has `e'en`, because they were
///   typed into different fields at different times.
///
/// What is left is strict enough that a collision would need two siblings of a
/// chord block with the same letters in the same order and nothing else in
/// them, which is a description of the duplicate this is looking for.
fn normalised(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| {
            match c {
                '\u{2018}' | '\u{2019}' => '\'',
                '\u{201c}' | '\u{201d}' => '"',
                '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
                other => other,
            }
            .to_lowercase()
        })
        .collect()
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

    // ── rebuilding a chart whose alignment lived in a stylesheet (E3/E8) ────

    /// hymnal.net's real shape, reduced: a plain copy of the verse, then the
    /// same verse again with a chord symbol above each syllable group, every
    /// piece of it a `<div>` that the site's linked stylesheet — the one E1
    /// drops — would have made `inline-block`.
    fn hymn() -> String {
        r#"<body><div class="verse">
             <div class="text-container">Glory be to God the Father,<br>&nbsp;&nbsp;And to Christ the Son,</div>
             <div class="chord-container hidden"><div class="line">
               <div class="chord-text"><span class="chord">G</span>
               Glory&nbsp;be&nbsp;to&nbsp;</div>
               <div class="chord-text"><span class="chord">C</span>
               God&nbsp;the&nbsp;Father,</div>
             </div><div class="line">&nbsp;
               <div class="chord-text"><span class="chord">G</span>
               And&nbsp;to&nbsp;Christ&nbsp;the&nbsp;</div>
               <div class="chord-text"><span class="chord">D<sup>7</sup></span>
               Son,</div>
             </div></div>
           </div></body>"#
            .to_string()
    }

    fn rebuilt(html: &str) -> (usize, String) {
        let doc = dom::parse(html.as_bytes());
        let root = dom::root(&doc);
        let count = rebuild_chord_blocks(&root);
        (count, dom::to_html(&root))
    }

    #[test]
    fn a_chart_positioned_in_a_dropped_stylesheet_is_rebuilt_as_preformatted_text() {
        // Without this the saved page is one syllable per line, because every
        // `chord-text` is a block and the sheet that made it `inline-block` is
        // gone. That is card E8, and this is E3's answer to it.
        let (count, html) = rebuilt(&hymn());
        assert_eq!(count, 1, "one block, the chord container: {html}");
        assert!(html.contains("<pre>"), "{html}");

        let text = dom::text_of(&dom::root(&dom::parse(html.as_bytes())));
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(
            lines,
            vec![
                "G           C",
                "Glory be to God the Father,",
                " G                 D7",
                " And to Christ the Son,",
            ],
            "the whole chart, in two rows a line: {text:?}"
        );
    }

    /// The alignment is the only thing the chart carries, so it is asserted by
    /// column rather than by eye: each chord has to sit over the first letter
    /// of the syllable it belongs to.
    #[test]
    fn every_chord_lands_over_the_column_its_syllable_starts_at() {
        let (_, html) = rebuilt(&hymn());
        let text = dom::text_of(&dom::root(&dom::parse(html.as_bytes())));
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();

        assert_eq!(lines[0].find('C'), lines[1].find("God"), "C over God");
        assert_eq!(lines[2].find('G'), lines[3].find("And"), "G over And");
        assert_eq!(lines[2].find("D7"), lines[3].find("Son,"), "D7 over Son");
    }

    /// `D<sup>7</sup>` is one chord, not a `D` and a stray `7`.
    #[test]
    fn a_chord_split_across_elements_is_still_one_symbol() {
        let (_, html) = rebuilt(&hymn());
        assert!(html.contains("D7"), "{html}");
        assert!(!html.contains("<sup>"), "the markup went with it: {html}");
    }

    /// hymnal ships every verse twice — plain, and again with the chords over
    /// it behind a "show chords" toggle. Both are in the saved page, so without
    /// this the reader gets the verse, and then the verse again.
    #[test]
    fn the_plain_copy_of_a_verse_the_site_ships_twice_is_dropped() {
        let (_, html) = rebuilt(&hymn());
        assert!(
            !html.contains("text-container"),
            "the copy without the chords went: {html}"
        );
        let text = dom::text_of(&dom::root(&dom::parse(html.as_bytes())));
        assert_eq!(
            text.matches("Glory be to God the Father,").count(),
            1,
            "and the words are there exactly once: {text:?}"
        );
    }

    /// The version of the twin test that fixture-driven development missed.
    ///
    /// The plain copy and the chord copy on the live hymnal.net page are not
    /// the same string: the chord scaffold breaks `Ever` across two chord
    /// symbols, and the two copies were typed with two different apostrophes.
    /// A comparison that only collapsed whitespace matched neither, so every
    /// verse was read out twice on the private display while every test passed.
    #[test]
    fn a_twin_that_breaks_a_word_or_curls_an_apostrophe_is_still_a_twin() {
        // The apostrophe is built rather than typed, so that this file stays
        // ASCII the way card K26 wants the app's own prose to be.
        let curly = char::from_u32(0x2019).expect("U+2019");
        let html = format!(
            r#"<body><div class="verse">
              <div class="text-container">Ever, e{curly}en so.</div>
              <div class="chord-container"><div class="line">
                <div class="chord-text"><span class="chord">D</span>Ev</div>
                <div class="chord-text"><span class="chord">D7</span>er,&nbsp;</div>
                <div class="chord-text"><span class="chord">G</span>e'en&nbsp;so.</div>
              </div></div>
            </div></body>"#
        );
        let (count, out) = rebuilt(&html);
        assert_eq!(count, 1);
        assert!(
            !out.contains("text-container"),
            "the plain copy went even though it is not the same string: {out}"
        );
    }

    /// The guard that keeps CifraClub and guitaretab out of this entirely.
    /// Their charts are `<pre>` and the site did the alignment; taking one
    /// apart and rebuilding it could only make it worse.
    #[test]
    fn a_chart_the_site_already_preformatted_is_left_alone() {
        let chart: String = (1..=12)
            .map(|n| format!("G       C\nline {n} of the words\n"))
            .collect();
        let html = format!(
            r#"<body><div class="tab"><pre>{chart}<b class="chord">Em7</b>  <b class="chord">G</b>
</pre></div></body>"#
        );
        let (count, out) = rebuilt(&html);
        assert_eq!(count, 0, "nothing was rebuilt");
        assert!(out.contains("line 12 of the words"), "{out}");
        assert!(out.contains(r#"<b class="chord">Em7</b>"#), "{out}");
    }

    /// One `class="chords"` on a link is not a chart. Two symbols in the same
    /// document is, which is the whole of the gate.
    #[test]
    fn a_single_stray_chord_marker_is_not_a_chart() {
        let html = r#"<body><nav><a class="chords-link">Chords</a></nav>
                      <p>Some prose about the song, at length, with commas.</p></body>"#;
        let (count, out) = rebuilt(html);
        assert_eq!(count, 0);
        assert!(out.contains("chords-link"), "{out}");
    }

    /// A syllable shorter than the chord above it. Two chords cannot run
    /// together — which of the two words each belonged to would be lost — so
    /// the lyric is pushed along instead.
    #[test]
    fn a_syllable_shorter_than_its_chord_pushes_the_words_along() {
        let html = r#"<body><div class="chord-sheet"><div class="line">
              <span class="chord">Dmaj7</span><span class="lyric">Ev</span>
              <span class="chord">G</span><span class="lyric">er</span>
            </div></div></body>"#;
        let (count, out) = rebuilt(html);
        assert_eq!(count, 1);
        let text = dom::text_of(&dom::root(&dom::parse(out.as_bytes())));
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines[0].find('G'), lines[1].find("er"), "{lines:?}");
        assert!(
            lines[0].starts_with("Dmaj7 G"),
            "the two symbols are separate: {lines:?}"
        );
    }

    /// The spans-with-no-wrapper shape `extract` already had a test for. Its
    /// chart survived extraction and still rendered one syllable per line,
    /// because nothing had put the chords back over the words.
    #[test]
    fn a_span_marked_chart_with_no_wrapper_folds_into_lines_not_syllables() {
        let sheet: String = (1..=8)
            .map(|n| {
                format!(
                    r#"<div class="line"><span class="chord">G</span><span class="lyric">word{n} </span><span class="chord">C</span><span class="lyric">after{n}</span></div>"#
                )
            })
            .collect();
        let (count, out) = rebuilt(&format!(r#"<body><div id="chordsheet">{sheet}</div></body>"#));
        assert_eq!(count, 1, "one block, not eight");
        let text = dom::text_of(&dom::root(&dom::parse(out.as_bytes())));
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 16, "two rows for each of eight lines: {lines:?}");
        assert_eq!(lines[0].find('C'), lines[1].find("after1"));
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
