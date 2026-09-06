//! Design tokens from the handoff, expressed as CSS custom properties.
//!
//! The neutrals are fixed in both modes — they carry the identity. Only the
//! accent varies. Every text token clears 4.5:1 against its own background;
//! do not lighten `muted` in either mode.

/// The six values derived from a single accent choice, per mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accent {
    pub name: &'static str,
    /// Light-mode base.
    pub base: &'static str,
    /// Dark-mode variant, lightened for contrast.
    pub base_dark: &'static str,
    /// Accent at ~12% over paper, for selected metadata chips.
    pub tint: &'static str,
    pub tint_dark: &'static str,
    /// Readable accent-family text on the tint.
    pub on_tint: &'static str,
    pub on_tint_dark: &'static str,
    /// Text/glyph colour on top of the accent itself.
    pub on_accent: &'static str,
    pub on_accent_dark: &'static str,
    /// Desaturated accent for "rusty" confidence dots.
    pub dim: &'static str,
    pub dim_dark: &'static str,
}

/// Default. Every value here is authored in the handoff.
pub const RUST: Accent = Accent {
    name: "Rust",
    base: "#B54724",
    base_dark: "#E8845C",
    tint: "#F7E6DE",
    tint_dark: "#3A2318",
    on_tint: "#8E3419",
    on_tint_dark: "#F0A483",
    on_accent: "#FBF7F0",
    on_accent_dark: "#1A1310",
    dim: "#D8B4A2",
    dim_dark: "#8B5540",
};

// The alternates below have only their `base` authored in the handoff ("proof
// the base holds"). The remaining five values are derived here on the same
// relationships as Rust and should be checked against the 4.5:1 rule if they
// ever become more than a demo.
pub const PINE: Accent = Accent {
    name: "Pine",
    base: "#2F6F4E",
    base_dark: "#6FB68F",
    tint: "#DEEBE4",
    tint_dark: "#16291F",
    on_tint: "#245740",
    on_tint_dark: "#93CDAC",
    on_accent: "#FBF7F0",
    on_accent_dark: "#101A14",
    dim: "#A9C6B6",
    dim_dark: "#3F6B54",
};

pub const INDIGO: Accent = Accent {
    name: "Indigo",
    base: "#3D5A9E",
    base_dark: "#8AA3E0",
    tint: "#E1E6F3",
    tint_dark: "#1A2136",
    on_tint: "#31487E",
    on_tint_dark: "#A9BCEB",
    on_accent: "#FBF7F0",
    on_accent_dark: "#111624",
    dim: "#B0BBD6",
    dim_dark: "#4E5F8C",
};

pub const PLUM: Accent = Accent {
    name: "Plum",
    base: "#7A4C86",
    base_dark: "#C08FCB",
    tint: "#EFE3F1",
    tint_dark: "#2A1B2E",
    on_tint: "#623C6C",
    on_tint_dark: "#D3ABDB",
    on_accent: "#FBF7F0",
    on_accent_dark: "#1B1119",
    dim: "#CBB2D1",
    dim_dark: "#6E4E76",
};

pub const ACCENTS: [Accent; 4] = [RUST, PINE, INDIGO, PLUM];

/// Card K34: a live capture of hymnal.net rendered `A♭ Major` as `A□ Major` on
/// the moto g stylus 5G. The flat sign is U+266D, and it is not text a hymn
/// site controls the font for — a captured page's headings and paragraphs go
/// through `--sla-font-ui`/`--sla-font-display` (`src/capture/render.rs`'s
/// `ELEMENT_STYLES` gives only `pre`/`code`/`kbd`/`samp` the mono stack), so
/// whatever family answers those two custom properties is what a chord
/// annotation sitting in body prose gets drawn in. Neither Newsreader nor
/// Karla has a glyph for it — checked with `fc-query --format '%{charset}'` —
/// and on the laptop that was invisible: fontconfig sits behind every stack
/// here and hands Parley a system face with the glyph the moment the bundled
/// families run out. On Android there is no fontconfig, so the same cluster
/// falls off the end of the stack onto nothing and comes back .notdef —
/// tofu, on the phone only, for a character the app never had to draw before.
///
/// `'DejaVu Sans Mono'` is appended to both stacks as the fix, and it is
/// deliberately *last* and deliberately not a `script_fallback` registration
/// (`../rinch-fixes/crates/rinch/src/font.rs` — that flag replaces the
/// platform fallback for every script, which would take CJK and emoji down
/// with it). Parley's `FontSelector::select_font` walks a stack one cluster at
/// a time and only moves on when the current family's coverage is incomplete
/// (`parley/src/shape/mod.rs` around line 520, in the pinned checkout), so a
/// name at the tail is never in competition with Newsreader or Karla for a
/// letter either of them can draw — it is asked only about the clusters nothing
/// before it could answer. That is a coverage tail, not a fallback chain: the
/// font it names is already in the binary for `FONT_MONO`'s own reasons, and
/// this is the same fourth file being read a second time for the three glyphs
/// (♭ ♮ ♯) nothing else here carries, rather than a fifth font bundled just for
/// them.
pub const FONT_DISPLAY: &str = "Newsreader, Georgia, serif, 'DejaVu Sans Mono'";
pub const FONT_UI: &str = "Karla, 'Helvetica Neue', sans-serif, 'DejaVu Sans Mono'";
/// Charts only. A chord chart is written with the chord names sitting over the
/// syllable they land on, and that alignment is the notation — in a
/// proportional face it is noise. The handoff never names a family for it (it
/// draws the preview as grey bars), so this is the standard fallback chain.
///
/// The first name in it is bundled (`crate::FONTS`) and so is always the one
/// that answers. That is not belt-and-braces: on Android *every* name in this
/// list resolves to nothing — `monospace` included, because the platform's
/// generic map looks for a family literally called `monospace` and no font
/// file is called that — so before the app carried the file, a chart on a
/// phone came out proportional and stopped meaning anything.
pub const FONT_MONO: &str =
    "'DejaVu Sans Mono', 'Liberation Mono', 'Courier New', ui-monospace, monospace";

/// Horizontal padding for every screen.
pub const SCREEN_PAD: &str = "22px";

/// The dark mode's neutral half, on its own.
///
/// Split out of [`tokens`] because one screen wants these without wanting the
/// mode: the attachment viewer (`1k`, card D5) is dark chrome *regardless of
/// theme*, so it re-declares this block on its own root and everything under it
/// — including the `rinch-components` menu, which reads the same `var(--sla-*)`
/// through `menu::MENU_SURFACE` — resolves to the dark values without a single
/// hex being written twice. CSS custom properties inherit and a nested
/// declaration wins, which is the whole mechanism.
///
/// fill-2 is not authored for dark in the handoff — derived one step below
/// `fill` so the empty-thumb still reads as a hole, not a chip.
///
/// `--sla-danger` is not authored either — the handoff never reaches a screen
/// with a destructive action. Added for the song/setlist overflow menus'
/// "Delete" rows (C2/C7): a true red, clearly apart from every accent hue (all
/// warm oranges/greens/blues/purples), each variant checked against its own
/// paper for 4.5:1.
pub const DARK_NEUTRALS: &str = "--sla-paper: #181512;\
     --sla-card: #211C18;\
     --sla-fill: #241F1A;\
     --sla-fill-2: #1F1A16;\
     --sla-hairline: #2C2620;\
     --sla-hairline-soft: #241F1A;\
     --sla-muted: #9B9188;\
     --sla-ink-2: #D6CCC1;\
     --sla-ink: #F5EFE6;\
     --sla-skeleton: #2E2822;\
     --sla-skeleton-2: #42392F;\
     --sla-card-shadow: 0 0 0 1px rgba(255,255,255,.05);\
     --sla-danger: #FFB4AB;";

/// The light mode's neutral half. Only [`tokens`] wants this one — nothing in
/// the app is light chrome regardless of theme.
pub const LIGHT_NEUTRALS: &str = "--sla-paper: #FBF7F0;\
     --sla-card: #FFFFFF;\
     --sla-fill: #F1E9DC;\
     --sla-fill-2: #F6F0E6;\
     --sla-hairline: #E7DFD4;\
     --sla-hairline-soft: #EFE8DD;\
     --sla-muted: #6E645A;\
     --sla-ink-2: #4A423B;\
     --sla-ink: #1C1917;\
     --sla-skeleton: #E4DACB;\
     --sla-skeleton-2: #EFE8DD;\
     --sla-card-shadow: 0 2px 10px -4px rgba(28,25,23,.14), 0 0 0 1px rgba(28,25,23,.06);\
     --sla-danger: #BA1B1B;";

/// The neutrals, pulled out of [`LIGHT_NEUTRALS`]/[`DARK_NEUTRALS`] one value
/// at a time. Those two blocks are one CSS-ready string each, so pulling the
/// same hex out of them at runtime is a parse for no reason — these exist so
/// `contrast_ratio`'s tests have something to check text-on-background
/// against directly, at the cost of every hex appearing twice in the file.
/// Change one, change both — a mismatch here would make the audit below lie
/// about a token it never actually re-read.
///
/// H2 only needed `paper`, to check the four accent bases as text against it.
/// Card J3 widens the audit to the neutral pairs the screens actually draw —
/// `ink`/`ink-2`/`muted`/`danger` as text on `paper`/`card`/`fill` — so it
/// needed the rest of the row spelled out here too.
const LIGHT_PAPER: &str = "#FBF7F0";
const DARK_PAPER: &str = "#181512";
const LIGHT_CARD: &str = "#FFFFFF";
const DARK_CARD: &str = "#211C18";
const LIGHT_FILL: &str = "#F1E9DC";
const DARK_FILL: &str = "#241F1A";
const LIGHT_MUTED: &str = "#6E645A";
const DARK_MUTED: &str = "#9B9188";
const LIGHT_INK_2: &str = "#4A423B";
const DARK_INK_2: &str = "#D6CCC1";
const LIGHT_INK: &str = "#1C1917";
const DARK_INK: &str = "#F5EFE6";
const LIGHT_DANGER: &str = "#BA1B1B";
const DARK_DANGER: &str = "#FFB4AB";

/// The full token block, as an inline `style` value for the app root.
///
/// Everything downstream reads `var(--sla-*)`; nothing hard-codes a hex.
pub fn tokens(dark: bool, accent: Accent) -> String {
    let neutrals = if dark { DARK_NEUTRALS } else { LIGHT_NEUTRALS };

    let (base, tint, on_tint, on_accent, dim) = if dark {
        (
            accent.base_dark,
            accent.tint_dark,
            accent.on_tint_dark,
            accent.on_accent_dark,
            accent.dim_dark,
        )
    } else {
        (
            accent.base,
            accent.tint,
            accent.on_tint,
            accent.on_accent,
            accent.dim,
        )
    };

    format!(
        "{neutrals}\
         --sla-accent: {base};\
         --sla-accent-tint: {tint};\
         --sla-accent-on-tint: {on_tint};\
         --sla-on-accent: {on_accent};\
         --sla-accent-dim: {dim};\
         --sla-font-display: {FONT_DISPLAY};\
         --sla-font-ui: {FONT_UI};\
         --sla-font-mono: {FONT_MONO};\
         background: var(--sla-paper);\
         color: var(--sla-ink);\
         font-family: var(--sla-font-ui);"
    )
}

// ---------------------------------------------------------------------------
// Type scale — each entry is a complete font declaration, used as-is in styles.
// ---------------------------------------------------------------------------

pub const T_SCREEN_TITLE: &str =
    "font-family: var(--sla-font-display); font-weight: 400; font-size: 34px; line-height: 1.0; letter-spacing: -0.01em;";
pub const T_DETAIL_TITLE: &str =
    "font-family: var(--sla-font-display); font-weight: 500; font-size: 32px; line-height: 1.12; letter-spacing: -0.015em;";
pub const T_SETLIST_TITLE: &str =
    "font-family: var(--sla-font-display); font-weight: 500; font-size: 30px; line-height: 1.12; letter-spacing: -0.015em;";
pub const T_ROW_TITLE: &str =
    "font-family: var(--sla-font-display); font-weight: 500; font-size: 18px; line-height: 1.25;";
pub const T_BODY: &str = "font-size: 15px; font-weight: 400;";
pub const T_META: &str = "font-size: 13px; line-height: 1.4; color: var(--sla-muted);";
pub const T_META_SMALL: &str = "font-size: 12px; color: var(--sla-muted);";
pub const T_LABEL_CAPS: &str =
    "font-weight: 500; font-size: 12px; letter-spacing: 0.16em; text-transform: uppercase;";
pub const T_SECTION_CAPS: &str =
    "font-weight: 600; font-size: 12px; letter-spacing: 0.10em; text-transform: uppercase;";
pub const T_CHIP: &str = "font-weight: 500; font-size: 13px;";
/// One line of a chart, in the attachment card and in an expanded row.
/// `white-space: pre` and a monospaced face together are what keep a chord
/// over its syllable; `overflow: hidden` keeps a long line from widening the
/// card rather than wrapping in the middle of a lyric.
pub const T_CHART: &str = "font-family: var(--sla-font-mono); font-size: 12.5px; \
     line-height: 1.5; white-space: pre; overflow: hidden;";
pub const T_NAV_LABEL: &str = "font-size: 11px; letter-spacing: 0.04em;";

// ---------------------------------------------------------------------------
// Contrast — the handoff's rule as arithmetic, not just as a comment.
// ---------------------------------------------------------------------------
//
// The header above states the rule in prose ("every text token clears 4.5:1
// against its own background") and until now nothing checked it. Card H2
// ships three accents whose card explicitly demands a contrast check before
// they ship, which is the immediate reason this exists — but it is written to
// be the one piece of arithmetic card J3 needs and not a one-off: J3's whole
// job is "assert every text token clears 4.5:1 on its background, in both
// modes and for every accent, as a unit test over the token table", and it
// should import `contrast_ratio` from here rather than re-deriving WCAG
// relative luminance a second time. H2's own tests below use it for the
// accent family only — the pairs `theme::Accent` actually defines as text on
// a background — and leave the neutrals (muted, ink, ink-2 on paper/card) for
// J3 to widen this into.
//
// J3 is that widening. Its tests sit at the bottom of the `tests` module
// below, after H2's, and its own comment there says where every pair in its
// table was found.

/// Parses a `#RRGGBB` string into its three channels. Every hex in this file
/// is authored in that exact shape, so a value that is not is a typo in the
/// token table worth a panic rather than a silently-wrong contrast number.
fn hex_rgb(hex: &str) -> (u8, u8, u8) {
    let digits = hex
        .strip_prefix('#')
        .unwrap_or_else(|| panic!("token hex must start with '#': {hex}"));
    assert_eq!(digits.len(), 6, "token hex must be #RRGGBB: {hex}");
    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&digits[range], 16).unwrap_or_else(|_| panic!("bad hex digit in {hex}"))
    };
    (channel(0..2), channel(2..4), channel(4..6))
}

/// WCAG 2.x relative luminance of one sRGB channel (0..=255 in, 0.0..=1.0 out).
fn channel_luminance(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG 2.x relative luminance of a `#RRGGBB` colour.
fn relative_luminance(hex: &str) -> f64 {
    let (r, g, b) = hex_rgb(hex);
    0.2126 * channel_luminance(r) + 0.7152 * channel_luminance(g) + 0.0722 * channel_luminance(b)
}

/// WCAG 2.x contrast ratio between two `#RRGGBB` colours. Order of the two
/// arguments does not matter — the lighter one is always the numerator — so a
/// call site can read `contrast_ratio(text, background)` without having to
/// know or care which of the two is lighter.
pub fn contrast_ratio(a: &str, b: &str) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (lighter, darker) = if la >= lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The handoff's own numbers, as a sanity check on the arithmetic itself
    /// before trusting it with the accents: `muted` on light `paper` is
    /// claimed as roughly 5.4:1 and dark `muted` on dark `paper` as 5.9:1 (the
    /// handoff states 5.9:1 outright for the dark pairing).
    #[test]
    fn contrast_ratio_matches_the_handoffs_worked_numbers_for_muted() {
        let light = contrast_ratio("#6E645A", "#FBF7F0");
        let dark = contrast_ratio("#9B9188", "#181512");
        assert!((light - 5.4).abs() < 0.2, "light muted ratio drifted: {light}");
        assert!((dark - 5.9).abs() < 0.2, "dark muted ratio drifted: {dark}");
    }

    #[test]
    fn contrast_ratio_does_not_care_which_argument_is_lighter() {
        assert_eq!(
            contrast_ratio("#FBF7F0", "#1C1917"),
            contrast_ratio("#1C1917", "#FBF7F0")
        );
    }

    /// Every accent's two text-on-accent pairs — `accent-on-tint` on
    /// `accent-tint`, and `on-accent` on `accent` itself — clear the
    /// handoff's 4.5:1, in both modes. This is the contrast check card H2's
    /// own card text demands before Pine, Indigo and Plum ship: their `base`
    /// is authored in the handoff ("proof the base holds") but the other five
    /// values per accent are derived here, and derived is exactly what this
    /// test is allowed to send back for adjustment if it ever fails.
    #[test]
    fn every_accents_text_on_accent_pairs_clear_4_5_to_1_in_both_modes() {
        for accent in ACCENTS {
            let on_tint_light = contrast_ratio(accent.on_tint, accent.tint);
            assert!(
                on_tint_light >= 4.5,
                "{}: on_tint vs tint (light) is only {on_tint_light:.2}:1",
                accent.name
            );

            let on_tint_dark = contrast_ratio(accent.on_tint_dark, accent.tint_dark);
            assert!(
                on_tint_dark >= 4.5,
                "{}: on_tint_dark vs tint_dark (dark) is only {on_tint_dark:.2}:1",
                accent.name
            );

            let on_accent_light = contrast_ratio(accent.on_accent, accent.base);
            assert!(
                on_accent_light >= 4.5,
                "{}: on_accent vs base (light) is only {on_accent_light:.2}:1",
                accent.name
            );

            let on_accent_dark = contrast_ratio(accent.on_accent_dark, accent.base_dark);
            assert!(
                on_accent_dark >= 4.5,
                "{}: on_accent_dark vs base_dark (dark) is only {on_accent_dark:.2}:1",
                accent.name
            );
        }
    }

    /// `accent` itself is also used as text — group labels and "Show 38 more"
    /// sit directly on `paper`/`card`, per the handoff's step 2 rule ("Material
    /// You primary … darkened until it clears 4.5:1 against paper"), which this
    /// applies to every accent rather than only the one it was written about.
    #[test]
    fn every_accents_base_clears_4_5_to_1_as_text_on_paper_in_both_modes() {
        for accent in ACCENTS {
            let light = contrast_ratio(accent.base, LIGHT_PAPER);
            assert!(
                light >= 4.5,
                "{}: base vs light paper is only {light:.2}:1",
                accent.name
            );

            let dark = contrast_ratio(accent.base_dark, DARK_PAPER);
            assert!(
                dark >= 4.5,
                "{}: base_dark vs dark paper is only {dark:.2}:1",
                accent.name
            );
        }
    }

    // -----------------------------------------------------------------------
    // J3 — widening the audit from the accent family to the whole token table.
    // -----------------------------------------------------------------------
    //
    // H2 asserted the three pairs `Accent` itself defines. Everything below is
    // the rest of the table: the neutral text tokens (`ink`, `ink-2`, `muted`,
    // `danger`) against the neutral backgrounds they are actually drawn on
    // (`paper`, `card`, `fill`), plus one accent pairing that is real but is
    // *not* one of `Accent`'s three — found the same way, and it does not
    // clear the bar.
    //
    // Method: every `color: var(--sla-...)` in every file under `src/screens`,
    // `src/ui.rs` and `src/menu.rs` (`grep -rn "color: var(--sla-" src/`), read
    // one at a time to find the `background` the enclosing row, card, chip or
    // sheet actually paints — the app has no CSS cascade of its own worth
    // trusting for this, so "what sits behind it" means walking up the `div`s
    // in the source, not assuming a token pairs with the one background it
    // sounds like it should. Twelve neutral pairings came out of that walk,
    // cited below one at a time. Two categories of hit were *not* turned into
    // an assertion, and are named rather than just missing:
    //
    // - **Non-text.** `--sla-hairline` colours `ConfidenceDots`' empty dots
    //   and `SheetHandle`'s grab bar (both plain fills, no glyph), and it
    //   colours the glyph in `setlist_detail`'s `MOVE_DEAD` — the disabled
    //   Move-up/-down chevron at either end of a set, deliberately drawn
    //   "visibly unavailable" rather than hidden (see that file's own
    //   comment). `--sla-accent-dim` only ever fills the same dots. None of
    //   these draw a word or a number; WCAG's 4.5:1 is a *text* rule and does
    //   not apply to a decorative dot or a bar, so there is nothing to assert.
    // - **Large text.** Checked for and not found. `T_SCREEN_TITLE` (34px),
    //   `T_DETAIL_TITLE` (32px) and `T_SETLIST_TITLE` (30px) would clear
    //   WCAG's large-text carve-out (18pt/24px regular) if any of them were
    //   ever coloured something other than the inherited `ink` — grepping
    //   every call site of all three (`grep -rn "T_SCREEN_TITLE\|
    //   T_DETAIL_TITLE\|T_SETLIST_TITLE" src/screens`) shows none is. Same
    //   check for `T_ROW_TITLE` (18px, weight 500): 18px clears the carve-out
    //   only at 24px regular or roughly 18.66px at genuine `bold` (700+), and
    //   500 is not that, so it stays held to the full 4.5:1 below regardless —
    //   moot in practice, since every place it is coloured at all uses `ink`
    //   or `muted`, both of which clear 4.5:1 by a wide margin against every
    //   background they are asked to sit on. If a future screen ever *does*
    //   colour a 24px+ (or true-bold 18.66px+) run with `muted`, `danger` or a
    //   bare accent, that pairing earns 3:1 here, not 4.5:1 — and should say
    //   so in a comment next to the assertion the way this one does, not by
    //   silently lowering the threshold for everything else.
    //
    // `paper`-on-`ink` is drawn too — `ui::Chip`'s `active` state and the
    // "Add to setlist" / record-style buttons in `song_detail.rs` and
    // `ui::SheetFooter`'s sibling pattern all paint `background: ink; color:
    // paper` — but `contrast_ratio` does not care which argument is lighter
    // (see its own doc comment and the test above that pins exactly that), so
    // it is the identical number `ink`-on-`paper` already asserts below. Cited
    // here rather than given its own assertion, so nobody reading this table
    // wonders where the fourth Chip state went.
    struct NeutralPair {
        text: &'static str,
        text_light: &'static str,
        text_dark: &'static str,
        background: &'static str,
        background_light: &'static str,
        background_dark: &'static str,
        /// Where this pairing was found — one real call site, not every one.
        drawn: &'static str,
    }

    const NEUTRAL_TEXT_PAIRS: &[NeutralPair] = &[
        NeutralPair {
            text: "ink",
            text_light: LIGHT_INK,
            text_dark: DARK_INK,
            background: "paper",
            background_light: LIGHT_PAPER,
            background_dark: DARK_PAPER,
            drawn: "the default body colour set on the app root (crate::theme::tokens); \
                    every screen title and every unstyled line of text",
        },
        NeutralPair {
            text: "ink",
            text_light: LIGHT_INK,
            text_dark: DARK_INK,
            background: "card",
            background_light: LIGHT_CARD,
            background_dark: DARK_CARD,
            drawn: "chart_editor.rs's FIELD — the chart text area itself, typed onto \
                    `background: var(--sla-card)`",
        },
        NeutralPair {
            text: "ink",
            text_light: LIGHT_INK,
            text_dark: DARK_INK,
            background: "fill",
            background_light: LIGHT_FILL,
            background_dark: DARK_FILL,
            drawn: "search.rs's search field input, typed onto the `var(--sla-fill)` \
                    pill behind it (library.rs's search door and capture.rs's URL_FIELD \
                    inside PANEL are the same pairing)",
        },
        NeutralPair {
            text: "ink-2",
            text_light: LIGHT_INK_2,
            text_dark: DARK_INK_2,
            background: "paper",
            background_light: LIGHT_PAPER,
            background_dark: DARK_PAPER,
            drawn: "song_detail.rs's confidence label (\"Solid\"/\"Rusty\"/…) sitting \
                    directly on the screen, no card or fill under it",
        },
        NeutralPair {
            text: "ink-2",
            text_light: LIGHT_INK_2,
            text_dark: DARK_INK_2,
            background: "card",
            background_light: LIGHT_CARD,
            background_dark: DARK_CARD,
            drawn: "menu.rs's ITEM/SUBITEM — every ordinary row of every dropdown, \
                    which is drawn on MENU_SURFACE's `var(--sla-card)`",
        },
        NeutralPair {
            text: "ink-2",
            text_light: LIGHT_INK_2,
            text_dark: DARK_INK_2,
            background: "fill",
            background_light: LIGHT_FILL,
            background_dark: DARK_FILL,
            drawn: "ui::IconButton's glyph on its own `var(--sla-fill)` disc — the back \
                    chevron and gear appear on nearly every screen",
        },
        NeutralPair {
            text: "muted",
            text_light: LIGHT_MUTED,
            text_dark: DARK_MUTED,
            background: "paper",
            background_light: LIGHT_PAPER,
            background_dark: DARK_PAPER,
            drawn: "T_META/T_META_SMALL's own default — every meta line and caption \
                    that is not sitting on a card or a fill",
        },
        NeutralPair {
            text: "muted",
            text_light: LIGHT_MUTED,
            text_dark: DARK_MUTED,
            background: "card",
            background_light: LIGHT_CARD,
            background_dark: DARK_CARD,
            drawn: "menu.rs's LABEL — a dropdown's small-caps section heading, on \
                    MENU_SURFACE's `var(--sla-card)`",
        },
        NeutralPair {
            text: "muted",
            text_light: LIGHT_MUTED,
            text_dark: DARK_MUTED,
            background: "fill",
            background_light: LIGHT_FILL,
            background_dark: DARK_FILL,
            drawn: "library.rs's and search.rs's search-door placeholder text, typed \
                    onto the `var(--sla-fill)` pill (ui::Chip's default state is the \
                    same pairing)",
        },
        NeutralPair {
            text: "danger",
            text_light: LIGHT_DANGER,
            text_dark: DARK_DANGER,
            background: "paper",
            background_light: LIGHT_PAPER,
            background_dark: DARK_PAPER,
            drawn: "song_form.rs's \"A song needs a title\" line, and the matching \
                    inline errors in song_detail.rs, chart_editor.rs and capture.rs",
        },
        NeutralPair {
            text: "danger",
            text_light: LIGHT_DANGER,
            text_dark: DARK_DANGER,
            background: "card",
            background_light: LIGHT_CARD,
            background_dark: DARK_CARD,
            drawn: "menu.rs's ITEM_DANGER — every menu's \"Delete\"/\"Remove\" row, on \
                    MENU_SURFACE's `var(--sla-card)`",
        },
        NeutralPair {
            text: "danger",
            text_light: LIGHT_DANGER,
            text_dark: DARK_DANGER,
            background: "fill",
            background_light: LIGHT_FILL,
            background_dark: DARK_FILL,
            drawn: "chart_editor.rs's and import_flow.rs's \"Discard\"/\"Choose backup \
                    file…\" confirm strips, and lib.rs's library-unavailable banner, all \
                    on `var(--sla-fill)`",
        },
    ];

    /// The twelve pairs above, in both modes — 24 checks over real screen
    /// pairings rather than the token table's shape in the abstract. The
    /// message on failure names the pair, the mode and exactly how far short
    /// it fell, because "assertion failed" would send the next person back to
    /// this whole table to find out which of the twelve broke.
    #[test]
    fn every_neutral_text_token_clears_4_5_to_1_on_every_background_it_is_actually_drawn_on() {
        for pair in NEUTRAL_TEXT_PAIRS {
            let light = contrast_ratio(pair.text_light, pair.background_light);
            assert!(
                light >= 4.5,
                "{} on {} (light) is {light:.2}:1, short of 4.5:1 by {:.2} — drawn at {}",
                pair.text,
                pair.background,
                4.5 - light,
                pair.drawn
            );

            let dark = contrast_ratio(pair.text_dark, pair.background_dark);
            assert!(
                dark >= 4.5,
                "{} on {} (dark) is {dark:.2}:1, short of 4.5:1 by {:.2} — drawn at {}",
                pair.text,
                pair.background,
                4.5 - dark,
                pair.drawn
            );
        }
    }

    /// A thirteenth pairing that is real but is not one of `Accent`'s three:
    /// accent-family text on a neutral `fill`, which is what
    /// `setlist_detail.rs`'s pending-undo strip draws its "Undo" link as.
    ///
    /// **This is the pair that made the audit worth writing.** The strip used
    /// to colour that link with the bare accent (`color: var(--sla-accent)`),
    /// and Rust's authored base (`#B54724`) on the authored `fill`
    /// (`#F1E9DC`) is **4.48:1** — short of the 4.5:1 the handoff commits to
    /// in writing, by two hundredths. Nobody was ever going to catch that by
    /// eye, and every other accent cleared it (Pine 4.97, Indigo 5.52, Plum
    /// 5.44 in light; all four above 6:1 in dark), so it was invisible in
    /// three quarters of the app's own configurations too.
    ///
    /// Neither hex was the thing to change. `base` is the accent's own
    /// published value — the handoff's "proof the base holds" — and `fill` is
    /// an authored neutral, so neither is one of the derived per-accent
    /// values H2 left open for adjustment. What was wrong was the *token the
    /// screen reached for*: `accent-on-tint` is defined by the handoff as
    /// "readable accent-family text on the tint", which is precisely this
    /// situation, and the screen was using the base because it was the
    /// nearest thing to hand. So the fix is in `setlist_detail.rs` and this
    /// assertion now holds the token actually drawn there.
    ///
    /// It is asserted for all four accents rather than just the one that
    /// failed, because the point is that the four are the same control.
    #[test]
    fn accent_text_on_a_neutral_fill_clears_4_5_to_1_in_both_modes() {
        for accent in ACCENTS {
            let light = contrast_ratio(accent.on_tint, LIGHT_FILL);
            assert!(
                light >= 4.5,
                "{}: on_tint vs light fill is only {light:.2}:1 (short by {:.2}) — drawn by \
                 setlist_detail.rs's \"Undo\" link on its pending-undo strip",
                accent.name,
                4.5 - light
            );

            let dark = contrast_ratio(accent.on_tint_dark, DARK_FILL);
            assert!(
                dark >= 4.5,
                "{}: on_tint_dark vs dark fill is only {dark:.2}:1 (short by {:.2}) — drawn by \
                 setlist_detail.rs's \"Undo\" link on its pending-undo strip",
                accent.name,
                4.5 - dark
            );
        }
    }

    /// The phone fault this stands in for: a live capture of hymnal.net drew
    /// `A♭ Major` as `A□ Major` on the moto g stylus 5G (card K34), because
    /// the flat sign U+266D has no glyph in Newsreader or Karla and Android
    /// carries no fontconfig behind the stack to hand Parley a substitute —
    /// the laptop never sees this because it does. `FONT_DISPLAY`/`FONT_UI`
    /// each carry `'DejaVu Sans Mono'` as a coverage tail for exactly this, so
    /// the assertion below is the laptop-side proof that the tail is still
    /// there and the bundled files still cover ♭/♮/♯ — the same check a phone
    /// would otherwise have to make by displaying tofu.
    ///
    /// It reads the bundled font files' own `cmap` and `name` tables with
    /// `skrifa` (already in the tree via `parley`/`swash` — see this crate's
    /// `[dev-dependencies]`) rather than asking fontconfig, on purpose: the
    /// whole failure mode is a system font answering on the laptop and
    /// nothing answering on the phone, so a check that can be satisfied by a
    /// system font would pass on exactly the machine that cannot see the bug.
    #[test]
    fn bundled_fonts_cover_the_music_accidentals_named_in_every_stack() {
        use skrifa::{FontRef, MetadataProvider, string::StringId};
        use std::collections::HashMap;

        // The four faces the binary actually carries — see `crate::FONTS`.
        // Every name each file's own `name` table answers to (family, id 1,
        // and typographic family, id 16 — Newsreader's variable-font instance
        // name is "Newsreader 16pt", but its typographic family, the one a
        // CSS stack actually says, is "Newsreader") maps to that file's
        // bytes, so a stack entry is checked against real glyph coverage
        // instead of an assumption about which of the two ids is "the" name.
        let files: &[&[u8]] = &[
            include_bytes!("../assets/fonts/Newsreader[opsz,wght].ttf"),
            include_bytes!("../assets/fonts/Newsreader-Italic[opsz,wght].ttf"),
            include_bytes!("../assets/fonts/Karla[wght].ttf"),
            include_bytes!("../assets/fonts/DejaVuSansMono.ttf"),
        ];

        let mut by_name: HashMap<String, &[u8]> = HashMap::new();
        for bytes in files {
            let font = FontRef::new(bytes).expect("a bundled font file failed to parse");
            for id in [StringId::FAMILY_NAME, StringId::TYPOGRAPHIC_FAMILY_NAME] {
                for entry in font.localized_strings(id) {
                    by_name.entry(entry.to_string().to_lowercase()).or_insert(bytes);
                }
            }
        }

        // U+266D FLAT, U+266E NATURAL, U+266F SHARP: the three signs a key
        // signature or a chord name actually uses. Covered by DejaVu Sans
        // Mono; not by Newsreader or Karla (checked with `fc-query
        // --format '%{charset}'` against the files in `assets/fonts/`).
        let signs = ['\u{266D}', '\u{266E}', '\u{266F}'];

        // Parsed the same way the constant declares the list: comma
        // separated, optionally single-quoted. A generic keyword or a system
        // face name (`Georgia`, `'Helvetica Neue'`) is simply absent from
        // `by_name` and is silently skipped — which is the point: a system
        // font must never be the thing that saves this assertion.
        fn stack_names(css: &str) -> Vec<String> {
            css.split(',').map(|s| s.trim().trim_matches('\'').to_lowercase()).collect()
        }

        for (stack_name, css) in [
            ("FONT_DISPLAY", FONT_DISPLAY),
            ("FONT_UI", FONT_UI),
            ("FONT_MONO", FONT_MONO),
        ] {
            for sign in signs {
                let covered = stack_names(css).iter().any(|name| {
                    by_name
                        .get(name)
                        .map(|bytes| {
                            let font = FontRef::new(bytes).expect("parsed above");
                            font.charmap().map(sign).is_some()
                        })
                        .unwrap_or(false)
                });
                assert!(
                    covered,
                    "{stack_name} = \"{css}\" has no *bundled* family that draws U+{:04X} — on \
                     a phone with no fontconfig behind it, that cluster is tofu, the way `A♭ \
                     Major` was on hymnal.net with no DejaVu Sans Mono tail (card K34)",
                    sign as u32
                );
            }
        }
    }
}
