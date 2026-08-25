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

/// Horizontal padding for every screen.
pub const SCREEN_PAD: &str = "22px";

/// The full token block, as an inline `style` value for the app root.
///
/// Everything downstream reads `var(--sla-*)`; nothing hard-codes a hex.
pub fn tokens(dark: bool, accent: Accent) -> String {
    let neutrals = if dark {
        // fill-2 is not authored for dark in the handoff — derived one step
        // below `fill` so the empty-thumb still reads as a hole, not a chip.
        "--sla-paper: #181512;\
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
         --sla-card-shadow: 0 0 0 1px rgba(255,255,255,.05);"
    } else {
        "--sla-paper: #FBF7F0;\
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
         --sla-card-shadow: 0 2px 10px -4px rgba(28,25,23,.14), 0 0 0 1px rgba(28,25,23,.06);"
    };

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
pub const T_NAV_LABEL: &str = "font-size: 11px; letter-spacing: 0.04em;";
