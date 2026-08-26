//! The thin layer between html5ever's DOM and the rest of `capture`.
//!
//! Everything downstream — the sanitiser, the reader extractor, the detector —
//! walks the same `RcDom` and wants the same six things from it: the tag name,
//! an attribute, the children, the text underneath, a way to detach a node and
//! a way to write the tree back out. They live here so that no other file in
//! the module has to name `NodeData`, `QualName` or a `RefCell`, and so that
//! swapping the DOM out later (see `docs/CAPTURE.md` on `markup5ever_rcdom`'s
//! "unsupported" label) is a change to one file.

use std::cell::RefCell;
use std::rc::Rc;

use html5ever::serialize::{SerializeOpts, TraversalScope, serialize};
use html5ever::tendril::TendrilSink;
use html5ever::{Attribute, LocalName, QualName, local_name, ns, parse_document};
use markup5ever_rcdom::{Handle, NodeData, RcDom, SerializableHandle};

/// Parse a byte slice as HTML5.
///
/// html5ever's error recovery is the whole reason it is here: chord sites
/// serve unclosed `<p>`, stray `</div>` and 1990s table markup, and a parser
/// that gave up on any of that would fail on most of the web. `from_utf8` is
/// lossy in the same spirit — a page with one bad byte still captures.
pub fn parse(bytes: &[u8]) -> RcDom {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut &bytes[..])
        .expect("reading from a slice cannot fail")
}

/// The `<html>` element, or the document itself if the parse produced none.
pub fn root(dom: &RcDom) -> Handle {
    find_element(&dom.document, &local_name!("html")).unwrap_or_else(|| dom.document.clone())
}

/// Serialise a subtree back to HTML, the node included.
///
/// `scripting_enabled: false` matters: with it true the serialiser writes the
/// *raw text* of a `<noscript>` rather than its parsed children, and a
/// JS-only page's `<noscript>` fallback is sometimes the only real content on
/// it. We would rather keep that than throw it away.
pub fn to_html(node: &Handle) -> String {
    let mut out = Vec::new();
    let handle: SerializableHandle = node.clone().into();
    serialize(
        &mut out,
        &handle,
        SerializeOpts {
            scripting_enabled: false,
            traversal_scope: TraversalScope::IncludeNode,
            create_missing_parent: true,
        },
    )
    .expect("serialising into a Vec cannot fail");
    String::from_utf8_lossy(&out).into_owned()
}

/// This node's tag name, if it is an element at all.
pub fn tag(node: &Handle) -> Option<LocalName> {
    match &node.data {
        NodeData::Element { name, .. } => Some(name.local.clone()),
        _ => None,
    }
}

/// Whether this is the named element. The comparison is on the local name
/// only, so an `<img>` in a foreign (SVG) namespace matches too — which is
/// what a sanitiser wants, since the dangerous thing about `<script>` does not
/// depend on its namespace.
pub fn is(node: &Handle, name: &str) -> bool {
    tag(node).is_some_and(|t| t.eq_str_ignore_ascii_case(name))
}

pub fn attr(node: &Handle, name: &str) -> Option<String> {
    match &node.data {
        NodeData::Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .find(|a| a.name.local.eq_str_ignore_ascii_case(name))
            .map(|a| a.value.to_string()),
        _ => None,
    }
}

pub fn set_attr(node: &Handle, name: &str, value: &str) {
    let NodeData::Element { attrs, .. } = &node.data else {
        return;
    };
    let mut attrs = attrs.borrow_mut();
    match attrs
        .iter_mut()
        .find(|a| a.name.local.eq_str_ignore_ascii_case(name))
    {
        Some(existing) => existing.value = value.into(),
        None => attrs.push(Attribute {
            name: QualName::new(None, ns!(), LocalName::from(name)),
            value: value.into(),
        }),
    }
}

/// Drop every attribute the predicate accepts. Returns how many went, because
/// the detector counts what the sanitiser removed.
pub fn retain_attrs(node: &Handle, mut keep: impl FnMut(&str, &str) -> bool) -> usize {
    let NodeData::Element { attrs, .. } = &node.data else {
        return 0;
    };
    let mut attrs = attrs.borrow_mut();
    let before = attrs.len();
    attrs.retain(|a| keep(&a.name.local.to_ascii_lowercase(), &a.value));
    before - attrs.len()
}

/// `class` and `id` joined and lower-cased — the string every heuristic in
/// this module matches its keyword lists against.
pub fn class_and_id(node: &Handle) -> String {
    let mut s = attr(node, "class").unwrap_or_default();
    if let Some(id) = attr(node, "id") {
        s.push(' ');
        s.push_str(&id);
    }
    s.to_ascii_lowercase()
}

pub fn children(node: &Handle) -> Vec<Handle> {
    node.children.borrow().clone()
}

pub fn parent(node: &Handle) -> Option<Handle> {
    // `parent` is a Cell, so reading it means taking it and putting it back.
    let weak = node.parent.take();
    node.parent.set(weak.clone());
    weak.and_then(|w| w.upgrade())
}

/// Unhook a node from its parent. `markup5ever_rcdom` keeps its own
/// `remove_from_parent` private, so this is the one place that reaches into
/// the child vector by pointer identity.
pub fn detach(node: &Handle) {
    let Some(parent) = parent(node) else { return };
    parent.children.borrow_mut().retain(|c| !Rc::ptr_eq(c, node));
    node.parent.set(None);
}

/// Replace a node with a comment saying what used to be there.
///
/// Used where a silent deletion would be confusing to anyone who later opens
/// the saved file: a stripped `<script>` leaves a trace, so the capture is
/// self-documenting rather than mysteriously incomplete.
pub fn replace_with_note(node: &Handle, note: &str) {
    let Some(parent) = parent(node) else {
        detach(node);
        return;
    };
    let comment = markup5ever_rcdom::Node::new(NodeData::Comment {
        contents: note.into(),
    });
    let mut siblings = parent.children.borrow_mut();
    if let Some(index) = siblings.iter().position(|c| Rc::ptr_eq(c, node)) {
        comment.parent.set(Some(Rc::downgrade(&parent)));
        siblings[index] = comment;
    }
    node.parent.set(None);
}

/// Pre-order walk. The callback decides whether to descend, which is what lets
/// the sanitiser drop a whole subtree without walking into it first.
pub fn walk(node: &Handle, f: &mut impl FnMut(&Handle) -> Descend) {
    if f(node) == Descend::Yes {
        for child in children(node) {
            walk(&child, f);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Descend {
    Yes,
    No,
}

/// The text under a node, with block-level boundaries turned into newlines.
///
/// The newlines are not cosmetic. A chord chart *is* its line breaks — the
/// chord row has to stay above the lyric row — so the detector's
/// "does this look like a chart" heuristic and the `body` text we save for
/// search both depend on this collapsing whitespace inside a line and nowhere
/// else. `<pre>` is copied verbatim for the same reason.
pub fn text_of(node: &Handle) -> String {
    let mut out = String::new();
    collect_text(node, false, &mut out);
    tidy(&out)
}

fn collect_text(node: &Handle, preformatted: bool, out: &mut String) {
    match &node.data {
        NodeData::Text { contents } => {
            let raw = contents.borrow();
            if preformatted {
                out.push_str(&raw);
            } else {
                push_collapsed(&raw, out);
            }
        }
        NodeData::Element { name, .. } => {
            let local = &name.local;
            if local == &local_name!("script")
                || local == &local_name!("style")
                || local == &local_name!("template")
            {
                return;
            }
            let pre = preformatted || local == &local_name!("pre");
            let breaks = local == &local_name!("br") || is_block(local);
            if breaks && !out.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            for child in children(node) {
                collect_text(&child, pre, out);
            }
            if breaks && !out.ends_with('\n') {
                out.push('\n');
            }
        }
        _ => {
            for child in children(node) {
                collect_text(&child, preformatted, out);
            }
        }
    }
}

fn push_collapsed(raw: &str, out: &mut String) {
    let mut space = out.ends_with(' ') || out.ends_with('\n') || out.is_empty();
    for ch in raw.chars() {
        if ch.is_whitespace() {
            if !space {
                out.push(' ');
                space = true;
            }
        } else {
            out.push(ch);
            space = false;
        }
    }
}

/// Trailing spaces go, runs of three or more blank lines become two. A chart's
/// own blank line between verses is meaningful; twelve of them are markup.
fn tidy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blanks = 0usize;
    for line in s.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blanks += 1;
            if blanks > 2 {
                continue;
            }
        } else {
            blanks = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_start_matches('\n').to_string()
}

fn is_block(local: &LocalName) -> bool {
    matches!(
        local.as_ref(),
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "div"
            | "dd"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "tr"
            | "ul"
    )
}

/// Depth-first search for the first element with this tag name.
pub fn find_element(node: &Handle, name: &LocalName) -> Option<Handle> {
    let mut found = None;
    walk(node, &mut |candidate| {
        if found.is_some() {
            return Descend::No;
        }
        if tag(candidate).as_ref() == Some(name) {
            found = Some(candidate.clone());
            return Descend::No;
        }
        Descend::Yes
    });
    found
}

/// The document title, trimmed. Falls back to the first `<h1>`, because a
/// handful of chord sites leave `<title>` as the site name and put the song in
/// the heading.
pub fn title_of(dom: &RcDom) -> Option<String> {
    let from = |name: LocalName| {
        find_element(&dom.document, &name)
            .map(|node| text_of(&node).trim().to_string())
            .filter(|t| !t.is_empty())
    };
    from(local_name!("title")).or_else(|| from(local_name!("h1")))
}

/// A fresh detached element, for the wrapper the reader extractor builds.
pub fn new_element(name: &str) -> Handle {
    markup5ever_rcdom::Node::new(NodeData::Element {
        name: QualName::new(None, ns!(html), LocalName::from(name)),
        attrs: RefCell::new(Vec::new()),
        template_contents: RefCell::new(None),
        mathml_annotation_xml_integration_point: false,
    })
}

/// Move `child` under `parent`, detaching it from wherever it was.
pub fn append(parent: &Handle, child: &Handle) {
    detach(child);
    child.parent.set(Some(Rc::downgrade(parent)));
    parent.children.borrow_mut().push(child.clone());
}
