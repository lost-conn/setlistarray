//! Ultimate Guitar's chart, pulled out of the JSON the page ships anyway.
//!
//! The E1 spike measured this and left the finding in `docs/CAPTURE.md`: a
//! 150 KB fetch of a UG tab page reads as 57 characters of visible text and a
//! `Blocked(ScriptShell)` verdict — an empty React mount and a bundle — but
//! the chart itself is sitting in the same response, untouched, as a ~134 KB
//! JSON blob in the `data-content` attribute of `<div class="js-store">`.
//! `detect.rs` cannot see it because nothing in an attribute value is visible
//! text; this file goes and reads the attribute on purpose.
//!
//! Real shape, confirmed against one live page fetched by hand for this card
//! and not committed (see the report for that session): the attribute is the
//! whole client bootstrap, HTML-entity-encoded, and the chart is three levels
//! down:
//!
//! ```text
//! { "store": { "page": { "data": {
//!     "tab":      { "song_name": "…", "artist_name": "…" },
//!     "tab_view": { "wiki_tab": { "content": "…[ch]G[/ch]…" } }
//! }}}}
//! ```
//!
//! `content` is plain text, not markup: chords are wrapped `[ch]G[/ch]`,
//! every lyric-and-chord line is wrapped again in `[tab]…[/tab]`, and section
//! headers (`[Intro]`, `[Verse]`, `[Chorus]`) are bare bracketed words with no
//! closing tag. Only the first two are this extractor's business — bracketed
//! section headers are already this app's own convention for the same thing;
//! see the reader-mode captures in `docs/CAPTURE.md`, "Card E3", where
//! cifraclub's own reader text opens on `[Intro] Em7 G Em7 G`. Stripping them
//! here would make Ultimate Guitar read differently from every other captured
//! song for no reason, so they are left exactly as UG wrote them.

use serde_json::Value;

use super::{ExtractedChart, SiteExtractor};
use crate::capture::dom;

pub struct UltimateGuitar;

impl SiteExtractor for UltimateGuitar {
    /// `ultimate-guitar.com` itself and any subdomain — the pages the spike
    /// sampled were `tabs.ultimate-guitar.com`, and `www.` and a bare apex
    /// are the same site. The suffix match requires the leading dot, so a
    /// host merely containing the string (`ultimate-guitar.com.evil.example`,
    /// `not-ultimate-guitar.com`) does not claim.
    fn claims(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        host == "ultimate-guitar.com" || host.ends_with(".ultimate-guitar.com")
    }

    fn extract(&self, html: &str) -> Option<ExtractedChart> {
        let raw = js_store_data_content(html)?;
        let json: Value = serde_json::from_str(&raw).ok()?;

        let data = json.pointer("/store/page/data")?;
        let content = data.pointer("/tab_view/wiki_tab/content")?.as_str()?;
        let text = convert_chord_markers(content);
        if text.trim().is_empty() {
            // The JSON parsed and the path existed, but there was nothing in
            // it — an instrumental stub UG has not filled in yet, say. A
            // chart with no chords and no lyrics is not a chart.
            return None;
        }

        let song_name = data.pointer("/tab/song_name").and_then(Value::as_str);
        let artist_name = data.pointer("/tab/artist_name").and_then(Value::as_str);
        let title = match (song_name, artist_name) {
            (Some(song), Some(artist)) if !song.is_empty() && !artist.is_empty() => {
                Some(format!("{song} — {artist}"))
            }
            (Some(song), _) if !song.is_empty() => Some(song.to_string()),
            _ => None,
        };

        Some(ExtractedChart { title, text })
    }
}

/// Find `<div class="js-store">` and read its `data-content` attribute.
///
/// `dom::attr` hands back the value already entity-decoded — html5ever
/// decodes character references in an attribute value during tokenising, the
/// same as it does for text, so the `&quot;` the site wrote arrives here as
/// `"` and the JSON parses without this file doing any unescaping of its own.
///
/// `None` covers both "no such div" and "the div is there without the
/// attribute" — a redesign that renamed or dropped `data-content` is exactly
/// the rot the module header describes, and both cases fall back the same
/// way.
fn js_store_data_content(html: &str) -> Option<String> {
    let document = dom::parse(html.as_bytes());
    let mut found = None;
    dom::walk(&dom::root(&document), &mut |node| {
        if found.is_some() {
            return dom::Descend::No;
        }
        if dom::class_and_id(node).split_whitespace().any(|c| c == "js-store") {
            found = dom::attr(node, "data-content");
            return dom::Descend::No;
        }
        dom::Descend::Yes
    });
    found
}

/// Strip UG's `[ch]…[/ch]` chord wrapper and `[tab]…[/tab]` line wrapper,
/// leaving the chord names and lyrics exactly where they sat.
///
/// A plain literal replace rather than a tag-stack parser, and that is
/// deliberate rather than a shortcut: replacing the four literal tokens
/// wherever they occur is correct however the tags are nested or how close
/// together they sit, because nothing here needs to know they were ever a
/// pair. `[ch]G[/ch][ch]C[/ch]` (adjacent, no separator) and
/// `[ch]G[ch]7[/ch][/ch]` (malformed, tag-shaped text a real UG page should
/// never emit but a rotted or hand-edited one might) both come out with
/// exactly the chord characters kept and none of the brackets — a stack
/// parser would have to special-case the second, this does not.
fn convert_chord_markers(content: &str) -> String {
    content
        .replace("[ch]", "")
        .replace("[/ch]", "")
        .replace("[tab]", "")
        .replace("[/tab]", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page shaped like Ultimate Guitar's, with placeholder content of our
    /// own rather than anybody's real song — see the module header on why
    /// nothing from a live fetch is committed here. `data_content` is the raw
    /// JSON text; this function does the same entity-encoding the real site's
    /// server-rendered attribute carries, so the extractor exercises its own
    /// decoding path rather than reading a value nobody had to unescape.
    fn synthetic_ug_page(js_store: Option<&str>) -> String {
        let div = match js_store {
            Some(data_content) => format!(
                r#"<div class="js-store" data-content="{}"></div>"#,
                data_content.replace('&', "&amp;").replace('"', "&quot;")
            ),
            None => String::new(),
        };
        format!(
            r#"<!doctype html><html><head><title>Placeholder Song Chords@ Example Tab Co</title></head>
               <body><div id="root"></div>{div}
               <script>{filler}</script></body></html>"#,
            div = div,
            filler = "var boot=1;".repeat(200),
        )
    }

    fn well_formed_json() -> String {
        serde_json::json!({
            "store": {
                "page": {
                    "data": {
                        "tab": {
                            "song_name": "Placeholder Song",
                            "artist_name": "The Example Band"
                        },
                        "tab_view": {
                            "wiki_tab": {
                                "content": "[Intro]\r\n[ch]G[/ch]   [ch]C[/ch]\r\n\r\n[Verse]\r\n[tab][ch]G[/ch]        [ch]D[/ch]\r\n  Lorem ipsum, a placeholder line[/tab]\r\n[tab][ch]C[/ch][ch]Am[/ch]\r\n  Consectetur, and a second placeholder[/tab]\r\n"
                            }
                        }
                    }
                }
            }
        })
        .to_string()
    }

    #[test]
    fn a_well_formed_ultimate_guitar_page_extracts_a_real_chart() {
        let html = synthetic_ug_page(Some(&well_formed_json()));
        let chart = UltimateGuitar.extract(&html).expect("should extract");

        assert_eq!(chart.title.as_deref(), Some("Placeholder Song — The Example Band"));
        assert!(chart.text.contains("[Intro]"), "section headers are kept: {}", chart.text);
        assert!(chart.text.contains("G   C"), "chord row survives without [ch] markers: {}", chart.text);
        assert!(
            chart.text.contains("Lorem ipsum, a placeholder line"),
            "lyric survives without [tab] markers: {}",
            chart.text
        );
        assert!(!chart.text.contains("[ch]"), "{}", chart.text);
        assert!(!chart.text.contains("[/ch]"), "{}", chart.text);
        assert!(!chart.text.contains("[tab]"), "{}", chart.text);
        assert!(!chart.text.contains("[/tab]"), "{}", chart.text);
    }

    #[test]
    fn a_page_whose_js_store_div_has_no_data_content_falls_back() {
        // The div is there, the way a redesign that moved the payload
        // somewhere else but kept the class name would leave it.
        let html = r#"<html><body><div id="root"></div>
            <div class="js-store" data-other-thing="1"></div></body></html>"#;
        assert!(UltimateGuitar.extract(html).is_none());
    }

    #[test]
    fn a_page_with_no_js_store_div_at_all_falls_back() {
        let html = synthetic_ug_page(None);
        assert!(UltimateGuitar.extract(&html).is_none());
    }

    #[test]
    fn a_data_content_attribute_that_is_not_valid_json_falls_back() {
        let html = synthetic_ug_page(Some("this is not { json"));
        assert!(UltimateGuitar.extract(&html).is_none());
    }

    #[test]
    fn valid_json_with_no_tab_content_falls_back() {
        let sparse = serde_json::json!({
            "store": { "page": { "data": {
                "tab": { "song_name": "Placeholder Song" }
            }}}
        })
        .to_string();
        let html = synthetic_ug_page(Some(&sparse));
        assert!(
            UltimateGuitar.extract(&html).is_none(),
            "the JSON is valid but there is no tab_view.wiki_tab.content anywhere in it"
        );
    }

    #[test]
    fn tab_content_that_is_only_chord_markers_with_nothing_else_falls_back() {
        let empty = serde_json::json!({
            "store": { "page": { "data": {
                "tab_view": { "wiki_tab": { "content": "[ch][/ch][tab][/tab]\r\n  \r\n" } }
            }}}
        })
        .to_string();
        let html = synthetic_ug_page(Some(&empty));
        assert!(
            UltimateGuitar.extract(&html).is_none(),
            "stripping the markers leaves nothing, which is not a chart"
        );
    }

    #[test]
    fn chord_markers_convert_whether_adjacent_or_nested_looking() {
        assert_eq!(convert_chord_markers("[ch]G[/ch]"), "G");
        assert_eq!(
            convert_chord_markers("[ch]G[/ch][ch]C[/ch]"),
            "GC",
            "adjacent markers with no separator"
        );
        assert_eq!(
            convert_chord_markers("[ch]G[ch]7[/ch][/ch]"),
            "G7",
            "a chord tag that appears to nest inside another"
        );
        assert_eq!(
            convert_chord_markers("[tab][ch]G[/ch]  lyric[/tab]"),
            "G  lyric",
            "the line wrapper goes too"
        );
    }

    #[test]
    fn ultimate_guitar_and_its_subdomains_are_claimed_and_lookalikes_are_not() {
        assert!(UltimateGuitar.claims("ultimate-guitar.com"));
        assert!(UltimateGuitar.claims("tabs.ultimate-guitar.com"));
        assert!(UltimateGuitar.claims("www.Ultimate-Guitar.com"));
        assert!(!UltimateGuitar.claims("not-ultimate-guitar.com"));
        assert!(!UltimateGuitar.claims("ultimate-guitar.com.evil.example"));
        assert!(!UltimateGuitar.claims("azlyrics.com"));
    }
}
