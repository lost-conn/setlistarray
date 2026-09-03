//! Site-specific extractors — the registry card E7 asked for, and only that.
//!
//! ## The argument against this file, which the card that authorised it makes
//! itself
//!
//! Every other file in `src/capture/` earns its keep by being generic:
//! `detect.rs` scores *any* page for chord content, `reader.rs` narrows *any*
//! document to its article. A per-site parser is the opposite kind of thing —
//! it knows one company's React props, and it will keep knowing them for
//! exactly as long as that company does not redeploy. Ultimate Guitar owes
//! this module nothing and will change `js-store`'s shape, or drop it, with no
//! notice and no obligation to this app's test suite. That is not a risk to
//! manage down to zero; it is the deal, made with eyes open, because the
//! prize is the single most-used chord site on the internet going from
//! `Blocked(ScriptShell)` to a real chart — see the E1 spike's numbers in
//! `docs/CAPTURE.md`.
//!
//! **So the file that names Ultimate Guitar is not allowed to be load-bearing.**
//! [`extract`] is consulted from exactly one place in [`super::capture`] — the
//! moment the generic engine has already decided, on its own evidence, that a
//! page is a JavaScript shell and is about to say so. Nothing here changes
//! what the generic engine sees, nothing here is consulted for a paywall or a
//! no-chart verdict, and a page from a host nothing here claims never reaches
//! an extractor at all. When Ultimate Guitar's markup rots — and the header
//! above says it will — [`UltimateGuitar::extract`] starts returning `None`
//! on every page, this module goes silent, and the app falls straight back to
//! the honest `Blocked(ScriptShell)` screen card E4 already drew: "this page
//! builds itself with JavaScript... type the chart instead." No panic, no
//! half-page, nobody has to notice the day it happens. That silent, total
//! fallback *is* the risk mitigation the card asked for, which is why it is
//! covered by tests below rather than left to be discovered on a phone.
//!
//! ## The seam a second extractor goes through
//!
//! One trait, [`SiteExtractor`], and one list, [`extractors`]. To add a
//! second site: write a new file beside [`ultimate_guitar`], implement
//! [`SiteExtractor`] for it — `claims` says which hosts are yours, `extract`
//! takes the page's own HTML (already fetched, already in memory — no second
//! request) and returns `Some` only when you are sure, `None` for everything
//! else, including "I am not sure" — and add one line to [`extractors`]. That
//! is the whole of the contract. Nothing outside this directory needs to
//! change: [`super::capture`] calls [`extract`] by host and does not know or
//! care how many extractors are behind it.

pub mod ultimate_guitar;

/// What an extractor pulled out of a page, in the shape any other captured
/// page hands to the pipeline.
///
/// `text` is the important field. It is what becomes
/// [`CapturedPage::text`](super::CapturedPage), which is what
/// `Attachment::body` holds — the field card G2 made load-bearing for search
/// and card E6 for change detection. An extractor that gets the chord markup
/// right but hands back something other than plain readable text has not
/// actually done this card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedChart {
    /// A better title than the page's `<title>`, if the extractor found one.
    /// `None` keeps whatever the generic fetch already read off the page.
    pub title: Option<String>,
    pub text: String,
}

/// A site-specific extractor: claims a set of hosts, and offers a fallible
/// pull of a chart out of bytes the generic fetch already has in hand.
///
/// Both methods are allowed to be wrong in only one direction. `claims`
/// returning `true` for a host that turns out not to have this site's markup
/// is fine — `extract` will simply return `None` and the caller falls back.
/// `extract` returning `Some` for something that is not actually a chart is
/// not fine, because the caller trusts it completely: whatever comes back is
/// presented as a captured page with no second check. When in doubt, return
/// `None`.
pub trait SiteExtractor: Sync {
    fn claims(&self, host: &str) -> bool;
    fn extract(&self, html: &str) -> Option<ExtractedChart>;
}

/// The registry. One entry today; see the module header for where a second
/// one goes.
fn extractors() -> &'static [&'static dyn SiteExtractor] {
    &[&ultimate_guitar::UltimateGuitar]
}

/// Try every registered extractor that claims `host`, in the order they are
/// listed, and return what the first one finds.
///
/// `None` covers two different situations — nothing claims this host, or the
/// thing that does could not make sense of this particular page — and the
/// caller does not need to tell them apart: either way its answer is the same
/// `Blocked(ScriptShell)` the generic engine already had ready.
pub fn extract(host: &str, html: &str) -> Option<ExtractedChart> {
    extract_with(extractors(), host, html)
}

/// [`extract`] over an explicit list, so a test can hand it an extractor that
/// panics if it is ever asked and prove the claim/extract split actually
/// gates the call rather than merely documenting an intention.
fn extract_with(list: &[&dyn SiteExtractor], host: &str, html: &str) -> Option<ExtractedChart> {
    list.iter()
        .find(|extractor| extractor.claims(host))
        .and_then(|extractor| extractor.extract(html))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_nothing_claims_is_never_offered_to_an_extractor() {
        struct PanicsIfAskedToExtract;
        impl SiteExtractor for PanicsIfAskedToExtract {
            fn claims(&self, host: &str) -> bool {
                host == "chords.example"
            }
            fn extract(&self, _html: &str) -> Option<ExtractedChart> {
                panic!("extract was called on a host this extractor never claimed");
            }
        }

        let list: Vec<&dyn SiteExtractor> = vec![&PanicsIfAskedToExtract];
        // A different host: the panic above must not fire. If `extract_with`
        // ever changed to try every extractor regardless of `claims`, this
        // test fails loudly instead of the real registry failing quietly on
        // somebody else's server.
        assert!(extract_with(&list, "other.example", "<html></html>").is_none());
    }

    #[test]
    fn the_real_registry_has_nothing_for_a_host_ultimate_guitar_never_owned() {
        assert!(extract("azlyrics.com", "<html><body>lyrics only</body></html>").is_none());
    }
}
