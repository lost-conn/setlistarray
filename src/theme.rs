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

pub const FONT_DISPLAY: &str = "Newsreader, Georgia, serif";
pub const FONT_UI: &str = "Karla, 'Helvetica Neue', sans-serif";
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

/// `--sla-paper`, on its own. The two neutrals blocks above are one CSS-ready
/// string each, so pulling the same hex out of them at runtime is a parse for
/// no reason — these exist so `contrast_ratio`'s tests (and J3, which widens
/// them) have something to check `accent`-as-text against directly, at the
/// cost of the hex appearing twice in the file. Change one, change both.
const LIGHT_PAPER: &str = "#FBF7F0";
const DARK_PAPER: &str = "#181512";

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
}
