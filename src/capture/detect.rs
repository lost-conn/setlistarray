//! Deciding whether what came back is worth keeping.
//!
//! Three of E4's screens depend on this file. "Fetch failed" is easy — the
//! socket said so. The other two are judgement calls made on a page that
//! arrived with a cheerful `200 OK`:
//!
//! * **Paywalled.** The bytes are a teaser and a sign-in form.
//! * **JS-only.** The bytes are an empty mount point and 400 KB of bundle.
//!
//! Both look like success to an HTTP client, and both would be saved as a
//! useless attachment by anything that only checked the status code. The app
//! promises that a URL pasted on wifi still opens with the network off; a
//! capture that cannot honour that has to say so at capture time, not three
//! weeks later on a stage.
//!
//! Everything here is a heuristic and every heuristic is tuned to prefer a
//! false negative. Telling someone their perfectly good chart is a paywall is
//! worse than saving one page too many.

use markup5ever_rcdom::{Handle, RcDom};

use super::dom::{self, Descend};
use super::sanitise::Stripped;

/// What the page looked like, in numbers. Carried on every captured page so
/// that E2's checklist, E4's failure screens and a later "re-check saved
/// pages" pass all read the same measurements rather than re-deriving them.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Signals {
    /// Characters of visible text after sanitising.
    pub text_chars: usize,
    /// Lines that read as a row of chord symbols. The one measurement that
    /// answers "did the chart survive".
    pub chord_lines: usize,
    /// Elements whose class or id says `chord`, `tab` or `lyric`.
    pub chord_markup: usize,
    /// Characters inside `<pre>`.
    pub preformatted_chars: usize,
    /// Bytes of `<script>` the sanitiser removed.
    pub script_bytes: usize,
    /// An empty `<div id="root">`-shaped element with nothing in it.
    pub empty_mount: bool,
    /// A `<noscript>` telling the reader to turn JavaScript on.
    pub noscript_notice: bool,
    /// Structural paywall markers found, by name.
    pub paywall_markers: Vec<String>,
    /// A bot wall answered instead of the page.
    pub challenged: bool,
}

impl Signals {
    /// Whether a chart survived, in the one form the rest of the app asks for.
    ///
    /// Four lines rather than one: a single line of capital letters is an
    /// abbreviation, four in a row is a chart. Chord markup or a substantial
    /// `<pre>` will do instead, because plenty of sites render each chord as
    /// its own `<span>` and the text that falls out has no chord *lines* in it
    /// at all.
    pub fn has_chart(&self) -> bool {
        self.chord_lines >= 4 || self.chord_markup >= 8 || self.preformatted_chars >= 400
    }
}

/// Class and id fragments that a site only writes when it is holding content
/// back. Structural, not phrasing: "subscribe to our newsletter" appears in
/// the footer of every site on earth, `class="paywall"` does not.
const PAYWALL_MARKERS: &[&str] = &[
    "paywall",
    "pay-wall",
    "metered",
    "meter-wall",
    "subscriber-only",
    "subscription-wall",
    "premium-wall",
    "login-wall",
    "signin-wall",
    "regwall",
    "reg-wall",
    "gated-content",
    "piano-modal",
];

/// What a bot wall says while it is deciding about you. Cloudflare's is the
/// one this app will meet most often — two of the five sites in the E1 spike
/// served it — and it is *not* a paywall: the site has no objection to the
/// user, it has an objection to the client. Telling someone to buy a
/// subscription they already have would be exactly the wrong advice.
const CHALLENGE_MARKERS: &[&str] = &[
    "cf-browser-verification",
    "cf_chl_opt",
    "__cf_chl",
    "challenge-platform",
    "cf-challenge",
    "checking your browser",
    "enable javascript and cookies to continue",
    "attention required! | cloudflare",
    "just a moment...",
    "verifying you are human",
    "ddos protection by",
    "px-captcha",
];

/// The ids and roles single-page apps mount themselves into.
const MOUNT_POINTS: &[&str] = &["root", "app", "__next", "__nuxt", "react-root", "ember-app"];

/// Phrases that only carry weight on a page with almost no text — where they
/// *are* the page rather than a line in the footer.
const GATE_PHRASES: &[&str] = &[
    "to continue reading",
    "sign in to continue",
    "log in to continue",
    "subscribe to read",
    "subscribers only",
    "create a free account to",
    "this content is available to",
];

pub fn signals(dom: &RcDom, text: &str, stripped: &Stripped) -> Signals {
    let root = dom::root(dom);
    let mut out = Signals {
        text_chars: text.chars().count(),
        chord_lines: count_chord_lines(text),
        script_bytes: stripped.script_bytes,
        challenged: is_challenge(dom, text),
        ..Signals::default()
    };

    dom::walk(&root, &mut |node| {
        let marker = dom::class_and_id(node);
        if !marker.is_empty() {
            // Not `tab-`. Bootstrap names its tab component `tab-content` /
            // `tab-pane`, and matching that scored a tab-*aggregator* page —
            // an index of links, with no chart on it anywhere — as a chart.
            if marker.contains("chord") || marker.contains("lyric") || marker.contains("tablature")
            {
                out.chord_markup += 1;
            }
            for found in PAYWALL_MARKERS.iter().filter(|m| marker.contains(**m)) {
                if !out.paywall_markers.iter().any(|m| m == found) {
                    out.paywall_markers.push((*found).to_string());
                }
            }
        }
        if dom::is(node, "pre") {
            out.preformatted_chars += dom::text_of(node).chars().count();
            return Descend::No;
        }
        if dom::is(node, "noscript") && mentions_javascript(&dom::text_of(node)) {
            out.noscript_notice = true;
        }
        if is_empty_mount(node) {
            out.empty_mount = true;
        }
        Descend::Yes
    });

    out
}

/// A challenge page is small, and it gives itself away in its `<title>`, in
/// the id of the div it mounts into, and in the URL of the script that runs
/// the check — none of which are visible text, which is why this looks at the
/// markup rather than at what the reader would see.
fn is_challenge(dom: &RcDom, text: &str) -> bool {
    // Anything with a page's worth of prose in it is a page, whatever words
    // happen to appear in it.
    if text.chars().count() > 2_000 {
        return false;
    }
    let haystack = dom::to_html(&dom::root(dom)).to_ascii_lowercase();
    CHALLENGE_MARKERS.iter().any(|m| haystack.contains(m))
}

fn mentions_javascript(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("javascript") && (lower.contains("enable") || lower.contains("turn on"))
}

fn is_empty_mount(node: &Handle) -> bool {
    let Some(id) = dom::attr(node, "id") else {
        return false;
    };
    if !MOUNT_POINTS.contains(&id.to_ascii_lowercase().as_str()) {
        return false;
    }
    dom::text_of(node).trim().is_empty()
}

/// Why a page came back but is not worth keeping as a chart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockReason {
    /// A bot wall stood in front of the page. Nothing about the user or their
    /// subscription is the problem — the site will not talk to a client that
    /// is not a browser.
    Challenged,
    /// The site asked for money or a login before it would show the page.
    Paywalled,
    /// The bytes are a JavaScript application, not a document. Nothing this
    /// app can render will find a chart in them.
    ScriptShell,
    /// A real page, with real prose, that has no chart in it. Usually a
    /// search result, an artist index, or the wrong URL.
    NoChart,
}

impl BlockReason {
    /// The line E4 puts on the screen. Written here rather than in the UI so
    /// that the reason and its explanation cannot drift apart.
    pub fn explain(&self) -> &'static str {
        match self {
            BlockReason::Challenged => {
                "This site checks that you are using a web browser before it \
                 will show a page, and it did not accept this app. Opening the \
                 page in a browser and saving it as text is the way round it."
            }
            BlockReason::Paywalled => {
                "This site keeps the chart behind a subscription. \
                 Only the part it shows to everyone came down."
            }
            BlockReason::ScriptShell => {
                "This page builds itself with JavaScript after it loads, so \
                 there is nothing in the file to save. Try the site's print \
                 view, or copy the chart in as text."
            }
            BlockReason::NoChart => {
                "The page saved, but there is no chord chart in it — this \
                 looks like a search result or an index rather than a song."
            }
        }
    }
}

/// The verdict, or `None` if the page is fine.
///
/// Order matters and is deliberate: a paywalled page is *also* usually thin on
/// text, and calling that a JavaScript problem would send the user off to look
/// for a print view that does not exist.
pub fn verdict(status: u16, signals: &Signals, text: &str) -> Option<BlockReason> {
    // Before the paywall check, because a challenge answers 403 too and the
    // two need opposite advice.
    if signals.challenged {
        return Some(BlockReason::Challenged);
    }
    if matches!(status, 401 | 402 | 403) || !signals.paywall_markers.is_empty() {
        return Some(BlockReason::Paywalled);
    }

    // A gate phrase only counts on a page too thin to be the article. On a
    // full page it is a line in the footer.
    if signals.text_chars < 1_500 {
        let lower = text.to_ascii_lowercase();
        if GATE_PHRASES.iter().any(|p| lower.contains(p)) {
            return Some(BlockReason::Paywalled);
        }
    }

    if signals.has_chart() {
        return None;
    }

    // An empty mount point is conclusive on its own. Otherwise it takes both
    // halves of the shape: almost no text, and a great deal of script. The
    // 20:1 ratio is what separates a real page that happens to be short from
    // a bundle with a loading spinner in it.
    let shell = signals.empty_mount
        || (signals.text_chars < 800
            && (signals.noscript_notice || signals.script_bytes > signals.text_chars * 20));
    if shell {
        return Some(BlockReason::ScriptShell);
    }

    Some(BlockReason::NoChart)
}

/// How many lines read as a row of chord symbols.
fn count_chord_lines(text: &str) -> usize {
    text.lines().filter(|line| is_chord_line(line)).count()
}

/// A chord line is short, has at least two tokens, and nearly all of them are
/// chord names.
///
/// The "nearly" is doing real work: sites interleave a bar count, a repeat
/// mark or an `x4` into the chord row, and demanding purity would score those
/// lines as lyrics. Requiring two tokens keeps a stray "A" or "Am" in a
/// sentence from counting.
fn is_chord_line(line: &str) -> bool {
    let line = line.trim();
    if line.is_empty() || line.len() > 120 {
        return false;
    }
    // Bar lines and repeat marks are punctuation, not tokens. Counting them
    // in the denominator would fail `C/G  F/A  |  Dm7`, which is as plainly a
    // chord row as anything gets.
    let tokens: Vec<&str> = line
        .split_whitespace()
        .filter(|t| t.chars().any(char::is_alphanumeric))
        .collect();
    if tokens.len() < 2 {
        return false;
    }
    let chords = tokens.iter().filter(|t| is_chord(t)).count();
    // Three quarters rather than all of them: `Bb Eb F x2` is a chord row and
    // the `x2` is not a chord.
    chords >= 2 && chords * 4 >= tokens.len() * 3
}

/// `A`, `Bb`, `C#m7`, `Dsus4`, `F#m7b5`, `G/B`. A note name, an optional
/// accidental, and a quality built only out of the pieces a quality is built
/// out of.
///
/// The quality is checked by consuming known fragments rather than by
/// allowing a set of letters, and the difference is not academic: a
/// letters-based check accepts "Bring", "Cars", "Dogs" and "Ban" as chords,
/// which turns any line of lyrics starting with a capital note name into a
/// false chord row.
fn is_chord(token: &str) -> bool {
    let token = token.trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | ',' | '|' | '*'));
    if token.is_empty() {
        return false;
    }
    // A slash chord is two chords; both halves have to check out.
    if let Some((upper, bass)) = token.split_once('/') {
        return is_chord(upper) && is_chord(bass);
    }
    if token.chars().filter(|c| c.is_alphabetic()).count() > 7 {
        return false;
    }

    let mut chars = token.chars();
    let root = chars.next().expect("non-empty");
    if !('A'..='G').contains(&root) {
        return false;
    }
    let rest: String = chars.collect();
    let mut rest = rest
        .strip_prefix(['#', 'b', '♯', '♭'])
        .unwrap_or(&rest)
        .to_string();

    // Longest first, so `maj` is not read as `m` followed by nonsense.
    const QUALITIES: &[&str] = &[
        "maj", "min", "dim", "aug", "sus", "add", "alt", "m", "M", "b", "#", "+", "-", "°", "ø",
        "∆", "(", ")", "♯", "♭",
    ];
    while !rest.is_empty() {
        if rest.starts_with(|c: char| c.is_ascii_digit()) {
            rest = rest.trim_start_matches(|c: char| c.is_ascii_digit()).to_string();
            continue;
        }
        match QUALITIES.iter().find(|q| rest.starts_with(**q)) {
            Some(quality) => rest = rest[quality.len()..].to_string(),
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chord_row_is_a_chord_row_and_a_lyric_is_not() {
        assert!(is_chord_line("G       C       D       G"));
        assert!(is_chord_line("Am7  D7sus4  Gmaj7"));
        assert!(is_chord_line("C/G  F/A  |  Dm7"));
        assert!(is_chord_line("Bb   Eb   F   x2"));

        assert!(!is_chord_line("Chorus"));
        assert!(!is_chord_line("Capo on the second fret"));
        assert!(!is_chord_line("Ever after all these years"));
        assert!(!is_chord_line("A"), "one token is not a row");
        assert!(!is_chord_line("Bring a friend and dance"));
    }

    #[test]
    fn the_root_has_to_be_a_note_and_the_quality_has_to_be_a_quality() {
        assert!(is_chord("F#m7b5"));
        assert!(is_chord("Gsus2"));
        assert!(is_chord("Cmaj7"));
        assert!(is_chord("Emadd9"));
        assert!(is_chord("Gaug"));
        assert!(is_chord("C"));
        assert!(is_chord("(Am)"));
        assert!(!is_chord("Hm"));
        assert!(!is_chord("Cathedral"));
        assert!(!is_chord("Dangerous"));
    }

    #[test]
    fn a_capitalised_word_starting_with_a_note_name_is_not_a_chord() {
        // The whole reason the quality is matched by fragment rather than by
        // letter set. Every one of these passes a letters-based check.
        for word in ["Bring", "Cars", "Dogs", "Ban", "Grin", "Dim", "Add", "Bomb", "Fond"] {
            assert!(!is_chord(word), "{word} is a word, not a chord");
        }
    }

    #[test]
    fn a_paywall_class_is_conclusive_on_its_own() {
        let signals = Signals {
            text_chars: 4_000,
            chord_lines: 20,
            paywall_markers: vec!["paywall".into()],
            ..Signals::default()
        };
        assert_eq!(verdict(200, &signals, ""), Some(BlockReason::Paywalled));
    }

    #[test]
    fn a_gate_phrase_only_counts_on_a_thin_page() {
        let thin = Signals {
            text_chars: 200,
            ..Signals::default()
        };
        assert_eq!(
            verdict(200, &thin, "Sign in to continue reading"),
            Some(BlockReason::Paywalled)
        );

        let full = Signals {
            text_chars: 6_000,
            chord_lines: 30,
            ..Signals::default()
        };
        assert_eq!(verdict(200, &full, "... subscribers only ..."), None);
    }

    #[test]
    fn a_bot_wall_is_not_a_paywall() {
        // Cloudflare answers 403, which the paywall rule would otherwise
        // claim — and "buy a subscription" is the wrong thing to tell someone
        // whose only problem is that they are not a browser.
        let challenged = Signals {
            text_chars: 120,
            challenged: true,
            ..Signals::default()
        };
        assert_eq!(verdict(403, &challenged, ""), Some(BlockReason::Challenged));

        let plain = Signals {
            text_chars: 120,
            ..Signals::default()
        };
        assert_eq!(verdict(403, &plain, ""), Some(BlockReason::Paywalled));
    }

    #[test]
    fn a_long_article_that_mentions_a_browser_check_is_not_a_challenge() {
        let doc = dom::parse(
            format!(
                "<html><head><title>Checking your browser</title></head><body><p>{}</p></body></html>",
                "a paragraph about verification pages, at length, ".repeat(80)
            )
            .as_bytes(),
        );
        let text = dom::text_of(&dom::root(&doc));
        assert!(!is_challenge(&doc, &text));
    }

    #[test]
    fn an_empty_mount_point_is_a_shell() {
        let signals = Signals {
            text_chars: 40,
            empty_mount: true,
            ..Signals::default()
        };
        assert_eq!(verdict(200, &signals, ""), Some(BlockReason::ScriptShell));
    }

    #[test]
    fn a_bundle_with_no_prose_is_a_shell_and_a_short_real_page_is_not() {
        let bundle = Signals {
            text_chars: 120,
            script_bytes: 400_000,
            ..Signals::default()
        };
        assert_eq!(verdict(200, &bundle, ""), Some(BlockReason::ScriptShell));

        // Same length of text, no bundle behind it: a real page with no chart.
        let index = Signals {
            text_chars: 120,
            script_bytes: 900,
            ..Signals::default()
        };
        assert_eq!(verdict(200, &index, ""), Some(BlockReason::NoChart));
    }

    #[test]
    fn a_page_with_a_chart_is_never_blocked_for_being_short() {
        let signals = Signals {
            text_chars: 300,
            chord_lines: 12,
            script_bytes: 500_000,
            ..Signals::default()
        };
        assert_eq!(verdict(200, &signals, ""), None);
    }

    #[test]
    fn span_per_chord_markup_counts_as_a_chart_when_the_text_does_not() {
        let signals = Signals {
            text_chars: 900,
            chord_markup: 24,
            ..Signals::default()
        };
        assert!(signals.has_chart());
        assert_eq!(verdict(200, &signals, ""), None);
    }
}
