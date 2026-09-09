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

/// The five values [`tokens`] writes for **one** mode, as numbers rather than
/// as the `#RRGGBB` strings the four authored accents are written in.
///
/// Numbers because of card K8. Until it, every colour in this file was a
/// literal somebody typed, and `&'static str` was the honest type for one: a
/// hex the handoff authored is a hex, for the life of the process. K8 adds a
/// sixth accent that nobody types — the wallpaper's own primary, read off the
/// device at runtime and then darkened until it clears the same 4.5:1 the
/// other five already do — and a colour that is *computed* cannot be a
/// `&'static str` without leaking one per computation. So the seam between
/// "the accent" and "the CSS" moved down to a triple of channels, which is
/// what the arithmetic in this file wanted anyway: [`contrast_ratio`] has
/// always had to parse those strings straight back into channels before it
/// could say anything about them.
///
/// [`Accent`] keeps its authored strings — they are the handoff's own
/// published values and the file reads better with them written out — and
/// converts on the way through [`Accent::colours`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccentColours {
    pub base: Rgb,
    pub tint: Rgb,
    pub on_tint: Rgb,
    pub on_accent: Rgb,
    pub dim: Rgb,
}

impl Accent {
    /// This accent's five values for the mode asked for. The `if dark` that
    /// used to sit inside [`tokens`] and pick between two halves of the
    /// struct, moved here so that the wallpaper accent below can answer the
    /// same question in its own way and `tokens` stops caring which kind it
    /// was handed.
    pub fn colours(self, dark: bool) -> AccentColours {
        let hex = |light: &str, night: &str| Rgb::from_hex(if dark { night } else { light });
        AccentColours {
            base: hex(self.base, self.base_dark),
            tint: hex(self.tint, self.tint_dark),
            on_tint: hex(self.on_tint, self.on_tint_dark),
            on_accent: hex(self.on_accent, self.on_accent_dark),
            dim: hex(self.dim, self.dim_dark),
        }
    }
}

/// Where a derived accent's seed came from — card K57.
///
/// It exists because the name is the *only* thing a user ever sees of this
/// distinction, and until this card the name was wrong. K8 built one derived
/// accent, from the wallpaper, and called the variant `Wallpaper`; K57 put a
/// better source in front of it (`android.R.color.system_accent1_500`, the
/// palette the system is itself themed with) and the old name immediately
/// became a lie on the most common configuration there is — a device whose
/// palette came from a *preset* the user picked in Wallpaper & style, where
/// the wallpaper's own colour is the thing they explicitly overrode.
///
/// This app's comments call out controls that lie about what they do in
/// several places, `screens::settings`'s accent row most loudly of all, and
/// the row's whole contract (`derive::accent_note`) is to name the colour the
/// pixels actually are. So the provenance is carried on the value rather than
/// inferred by whoever prints it, and there is exactly one place — [`name`] —
/// that turns it into a word.
///
/// [`name`]: AccentSource::name
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccentSource {
    /// `system_accent1_500`, off the Material You palette Android 12+ publishes
    /// as framework colour resources. What the device is themed in, whatever
    /// the user's theme was *derived* from.
    System,
    /// `WallpaperColors.getPrimaryColor()`. The fallback, for API 28-30, which
    /// have a wallpaper to read and no published palette to read it from.
    Wallpaper,
}

impl AccentSource {
    /// The word the Settings row prints. Short, because it is set in
    /// `T_META_SMALL` next to the word "Accent" and shares its line with
    /// nothing else.
    pub fn name(self) -> &'static str {
        match self {
            AccentSource::System => "System",
            AccentSource::Wallpaper => "Wallpaper",
        }
    }
}

/// An accent built from a colour the device reported rather than one anybody
/// authored — card K8, and the thing `AccentChoice::FromSystem` was waiting
/// for. Card K57 gave it a second, better [`source`](Self::source) and took
/// the word "wallpaper" out of its name.
///
/// The seed is whatever `AccentChoice::resolve` found first — the system
/// palette's tone 500, or, on a device too old to publish one, the wallpaper's
/// primary colour. Either way it can be *anything a photograph can be*:
/// near-black, near-white, or a saturated yellow that vanishes on cream. Both
/// framework calls deliberately hand back the raw triple and no more — a
/// contrast ratio is a fact about a **pair** of colours and they only know one
/// of them — so every question about legibility is answered here, against this
/// app's own paper, with the same [`contrast_ratio`] arithmetic the four
/// authored accents are already audited by.
///
/// ## Why one seed, and not the thirteen tones the palette actually publishes
///
/// `system_accent1_*` is a full ramp — `0, 10, 50, 100, 200 … 900, 1000` —
/// and the obvious thing to do with a ramp is to take a light tone for light
/// mode and a dark one for dark mode, the way Material's own components do.
/// K57 deliberately does not, and this is the note for whoever wonders why a
/// thirteen-tone gift went unused.
///
/// Those tones are tuned against **Material's** surfaces. This app's papers
/// are `#FBF7F0`, a warm cream, and `#181512`, a near-black with a brown cast
/// — neither is `md.sys.color.surface`, and a tone chosen to sit at a
/// particular contrast against a colour we do not paint on is a number with
/// no relationship to the one thing that has to be true here. Worse, it would
/// *look* principled: a table mapping tone to mode, sourced from a real
/// specification, quietly missing 4.5:1 on the surface it is actually drawn
/// on. Whereas [`derive_family`] already takes any seed at all and walks it
/// until it clears 4.5:1 against the papers we really have, and is tested on
/// seeds far nastier than tone 500 will ever be. One tone as a seed is less
/// code and more correct, and if the ramp ever becomes useful it is one call
/// away.
///
/// ## What is derived, and from what
///
/// The handoff states the rule for the base in one line — *"Material You
/// primary … darkened until it clears 4.5:1 against paper"* — and says
/// nothing about the other four values, because it was describing a colour it
/// expected to hand-tune the rest of. There is nobody to hand-tune this one,
/// so each of the remaining four is derived on the **same relationship the
/// four authored accents already hold**, and then checked rather than assumed:
///
/// * `base` — the seed, darkened (light) or lightened (dark) in small steps
///   until it clears 4.5:1 against that mode's paper. A seed that already
///   clears it is kept exactly as it came, which matters: a user whose
///   wallpaper is a deep blue should get *their* blue, not a version of it
///   this app decided to darken for no reason.
/// * `tint` — the base at 12% over paper, which is what `Accent`'s own doc
///   comment says the authored tints are.
/// * `on_tint` — the base pushed further from paper until it clears 4.5:1
///   against **both** its own tint and the neutral `fill`. Both, because card
///   J3 found the thirteenth pairing the hard way: `accent-on-tint` is also
///   what `setlist_detail`'s "Undo" link is drawn in, on `fill` rather than
///   on the tint, and Rust's base missed 4.5:1 there by two hundredths.
/// * `on_accent` — paper, in either mode. It is a free result rather than a
///   derivation: `base` has just been forced to clear 4.5:1 against that
///   mode's paper, and contrast is symmetric, so paper on the accent is the
///   identical ratio the base was measured at.
/// * `dim` — the base mixed most of the way back to paper. The only value
///   here with no contrast rule over it, because the only thing it colours is
///   `ConfidenceDots`' rusty dots: WCAG's 4.5:1 is a text rule and a dot is
///   not text (J3's audit says exactly this, at more length).
///
/// Every one of those claims is asserted in this file's tests, for a table of
/// seeds chosen to be nastier than a real wallpaper or palette: pure black,
/// pure white, the paper colour itself, and a saturated yellow. Note that the
/// [`source`](Self::source) plays no part in any of the arithmetic — it is
/// carried for the name and nothing else, which is why the same
/// [`AWKWARD_SEEDS`] table is asserted for both of them from
/// `src/store/settings.rs` rather than only for the one K8 happened to build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DerivedAccent {
    /// Which of the two device readings this was built from. Used by
    /// [`ResolvedAccent::name`] and by nothing else in this file.
    pub source: AccentSource,
    /// The colour the platform actually reported, before any of the maths.
    /// Kept so that a future card can show it, and so the tests can say what
    /// went in beside what came out.
    pub seed: Rgb,
    light: AccentColours,
    dark: AccentColours,
}

impl DerivedAccent {
    pub fn from_seed(source: AccentSource, seed: Rgb) -> Self {
        Self {
            source,
            seed,
            light: derive_family(seed, Rgb::from_hex(LIGHT_PAPER), Rgb::from_hex(LIGHT_FILL)),
            dark: derive_family(seed, Rgb::from_hex(DARK_PAPER), Rgb::from_hex(DARK_FILL)),
        }
    }

    pub fn colours(self, dark: bool) -> AccentColours {
        if dark { self.dark } else { self.light }
    }
}

/// The accent the app is actually **painted in**, which is not the same thing
/// as the accent that was **chosen** — see `AccentChoice::resolve`, which is
/// the only thing that builds one of these.
///
/// Two variants rather than one type with runtime colours throughout, because
/// the four authored accents really are different in kind from the derived
/// one: theirs are published values with a name a person recognises, and the
/// device's is a number off a platform reading with no name at all — only a
/// [`source`](DerivedAccent::source) to say where it was found. Flattening the
/// two would mean either giving Rust a computed palette it does not need or
/// giving the derived one a name it does not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedAccent {
    Authored(Accent),
    Derived(DerivedAccent),
}

impl ResolvedAccent {
    /// What to call the colour on screen. For the derived one that is its
    /// source — `"System"` or `"Wallpaper"` — which is the honest answer, and
    /// it is what `derive::accent_note` prints on the Settings row, whose
    /// whole contract is to name the colour the pixels actually are rather
    /// than the control that was tapped.
    ///
    /// Until K57 this returned `"Wallpaper"` for every derived accent, which
    /// was true while the wallpaper was the only reading there was. It is
    /// [`AccentSource`]'s doc comment that explains why keeping it would have
    /// been a control lying about what it does.
    pub fn name(self) -> &'static str {
        match self {
            ResolvedAccent::Authored(accent) => accent.name,
            ResolvedAccent::Derived(derived) => derived.source.name(),
        }
    }

    pub fn colours(self, dark: bool) -> AccentColours {
        match self {
            ResolvedAccent::Authored(accent) => accent.colours(dark),
            ResolvedAccent::Derived(derived) => derived.colours(dark),
        }
    }
}

/// One mode's worth of [`DerivedAccent`] — see that type for what each of
/// the five is and why. `paper` is the mode's background and `fill` is the
/// neutral the `on_tint` value has to survive as text on as well.
fn derive_family(seed: Rgb, paper: Rgb, fill: Rgb) -> AccentColours {
    // Which way "away from the background" points. In light mode the accent
    // has to get darker to become readable and in dark mode lighter, and that
    // is the only thing the two modes differ by in here.
    let away = |from: Rgb, target: f64, backgrounds: [Rgb; 2]| {
        pushed_until(from, backgrounds, target, paper.is_light())
    };

    let base = away(seed, MIN_CONTRAST, [paper, paper]);
    let tint = base.mixed(paper, TINT_OVER_PAPER);
    let on_tint = away(base, MIN_CONTRAST, [tint, fill]);
    AccentColours {
        base,
        tint,
        on_tint,
        // Paper, and it is a free result rather than a choice — see the type's
        // doc comment. `base` was just pushed until it cleared `MIN_CONTRAST`
        // against exactly this colour, and `contrast_ratio` does not care
        // which of its two arguments is the text.
        on_accent: paper,
        dim: base.mixed(paper, DIM_TOWARDS_PAPER),
    }
}

/// The 4.5:1 the handoff commits to in writing, in one place rather than as a
/// literal in each of the places this file now checks it.
///
/// `pub` because the accent resolution's own tests, over in
/// `src/store/settings.rs`, assert the same bar on the same colours from the
/// other side of the seam — and a second `4.5` written down over there would
/// be a second place to forget if the handoff ever moved it.
pub const MIN_CONTRAST: f64 = 4.5;

/// The background a mode is painted on, by name, for the two places outside
/// this file that have to measure something against it. Everything *inside*
/// the file reads `LIGHT_PAPER`/`DARK_PAPER` directly; this exists so that
/// nothing outside has to know there are two constants, or which is which.
pub const fn paper(dark: bool) -> &'static str {
    if dark { DARK_PAPER } else { LIGHT_PAPER }
}

/// "Accent at ~12% over paper", which is [`Accent`]'s own description of what
/// the authored tints are.
const TINT_OVER_PAPER: f64 = 0.88;

/// How far the dim variant is mixed back towards paper. Measured off the
/// authored pairs rather than picked: Rust's `#B54724` → `#D8B4A2` is a little
/// over half the way, and the other three sit in the same place.
const DIM_TOWARDS_PAPER: f64 = 0.55;

/// Push `colour` away from `backgrounds` — darker if `darker` is true,
/// lighter if not — in small steps, until it clears `target` against **both**
/// of them, and hand back the first value that does.
///
/// **A colour that already clears is returned untouched**, which is the whole
/// reason this is a loop with the test at the top rather than a fixed
/// adjustment: a wallpaper that is already a readable deep blue must come out
/// as that blue.
///
/// The step is multiplicative (8% of the remaining distance to black, or to
/// white) so the walk is roughly perceptually even rather than crawling
/// through the dark end and leaping through the light one. It is bounded at
/// [`PUSH_STEPS`] and cannot fail to find an answer in practice: black clears
/// 19:1 against this app's light paper and white clears 16:1 against its dark
/// one, and both ends are reachable inside the bound. The bound is there so
/// that a background this function was never designed for — a mid grey that
/// nothing clears 4.5:1 against in either direction — terminates with the
/// most-contrasting colour it managed rather than spinning.
fn pushed_until(colour: Rgb, backgrounds: [Rgb; 2], target: f64, darker: bool) -> Rgb {
    let clears = |c: Rgb| backgrounds.iter().all(|bg| contrast(c, *bg) >= target);
    let mut c = colour;
    for _ in 0..PUSH_STEPS {
        if clears(c) {
            return c;
        }
        let next = if darker { c.darker() } else { c.lighter() };
        if next == c {
            // Black cannot get darker and white cannot get lighter. Stop
            // rather than spend the rest of the bound proving it again.
            break;
        }
        c = next;
    }
    c
}

/// Enough steps for either end of the range at 8% a step — 0.92^96 and
/// 1 - 0.92^96 both run out of `u8` long before this — with room to spare, so
/// that the bound is a guard against a pathological background rather than a
/// limit the ordinary case ever reaches.
const PUSH_STEPS: usize = 96;

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
pub fn tokens(dark: bool, accent: ResolvedAccent) -> String {
    let neutrals = if dark { DARK_NEUTRALS } else { LIGHT_NEUTRALS };

    // The `if dark` that used to be spelled out here moved onto the accent
    // itself (card K8), because there is now a second kind of accent that
    // answers it differently: the four authored ones pick between two halves
    // of a struct of literals, and the wallpaper's picks between two families
    // it derived. Neither is this function's business — all it ever wanted was
    // five colours for the mode it was told about.
    let AccentColours { base, tint, on_tint, on_accent, dim } = accent.colours(dark);

    // The FAB's drop shadow, as the accent's own three channels rather than as
    // a sixth colour anybody has to derive.
    //
    // It is a token at all because it could not be written as one any other
    // way: a shadow wants `rgba(r, g, b, .5)` and the accent tokens are
    // `#RRGGBB` strings, and CSS cannot take the alpha off one and put it on
    // the other — `rgba(var(--sla-accent), .5)` is not a thing. So the three
    // call sites that draw a FAB spelled the numbers out instead, and what
    // they spelled out was `rgba(181,71,36,.5)`, which is Rust. The button
    // went purple with the Material You palette behind it and kept a rust
    // shadow, in an app where the accent has been user-choosable since H2 and
    // system-derived since K8 — the shadow was the last thing still insisting
    // on the colour the app shipped with.
    let (shadow_r, shadow_g, shadow_b) = (base.r, base.g, base.b);

    format!(
        "{neutrals}\
         --sla-accent: {base};\
         --sla-accent-tint: {tint};\
         --sla-accent-on-tint: {on_tint};\
         --sla-on-accent: {on_accent};\
         --sla-accent-dim: {dim};\
         --sla-accent-shadow: rgba({shadow_r}, {shadow_g}, {shadow_b}, 0.5);\
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

/// One sRGB colour, as the three channels it actually is.
///
/// Added by card K8, which needed a colour this file could *compute* — see
/// [`AccentColours`] for why a computed one cannot be a `&'static str`. It is
/// deliberately the same shape `rinch_android::display::wallpaper_primary`
/// hands back, so the platform reading crosses into the app without being
/// reinterpreted on the way.
///
/// `Display` writes it as `#RRGGBB`, which is the only form anything
/// downstream of this file has ever seen, so a `format!` that used to
/// interpolate an authored hex still interpolates the same six digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses a `#RRGGBB` string into its three channels. Every hex in this
    /// file is authored in that exact shape, so a value that is not is a typo
    /// in the token table worth a panic rather than a silently-wrong contrast
    /// number.
    pub fn from_hex(hex: &str) -> Self {
        let digits = hex
            .strip_prefix('#')
            .unwrap_or_else(|| panic!("token hex must start with '#': {hex}"));
        assert_eq!(digits.len(), 6, "token hex must be #RRGGBB: {hex}");
        let channel = |range: std::ops::Range<usize>| {
            u8::from_str_radix(&digits[range], 16)
                .unwrap_or_else(|_| panic!("bad hex digit in {hex}"))
        };
        Self::new(channel(0..2), channel(2..4), channel(4..6))
    }

    /// Whether this colour is nearer white than black, which is the whole of
    /// what [`derive_family`] needs in order to know which way "away from the
    /// background" points. Measured in WCAG luminance rather than by averaging
    /// the channels, so a saturated yellow counts as light — which it is.
    fn is_light(self) -> bool {
        relative_luminance(self) > 0.18
    }

    /// This colour `amount` of the way towards `other`, per channel. `0.0` is
    /// unchanged and `1.0` is `other`.
    fn mixed(self, other: Rgb, amount: f64) -> Rgb {
        let channel = |from: u8, to: u8| {
            (f64::from(from) + (f64::from(to) - f64::from(from)) * amount).round() as u8
        };
        Rgb::new(
            channel(self.r, other.r),
            channel(self.g, other.g),
            channel(self.b, other.b),
        )
    }

    /// One step towards black, 8% of the distance. Floored rather than
    /// rounded so that the walk in [`pushed_until`] cannot stall one step
    /// above black on a channel that keeps rounding back to itself.
    fn darker(self) -> Rgb {
        let channel = |c: u8| (f64::from(c) * (1.0 - PUSH_STEP)).floor() as u8;
        Rgb::new(channel(self.r), channel(self.g), channel(self.b))
    }

    /// One step towards white, the same 8% of the remaining distance, ceiled
    /// for the mirror image of the reason [`darker`](Self::darker) floors.
    fn lighter(self) -> Rgb {
        let channel = |c: u8| {
            (f64::from(c) + (255.0 - f64::from(c)) * PUSH_STEP).ceil().min(255.0) as u8
        };
        Rgb::new(channel(self.r), channel(self.g), channel(self.b))
    }
}

/// How far one step of [`pushed_until`] moves. Small enough that a seed which
/// only just misses 4.5:1 is not thrown well past it — the point of that walk
/// is to change the user's own colour as little as it can get away with.
const PUSH_STEP: f64 = 0.08;

impl std::fmt::Display for Rgb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
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

/// WCAG 2.x relative luminance of a colour.
fn relative_luminance(colour: Rgb) -> f64 {
    0.2126 * channel_luminance(colour.r)
        + 0.7152 * channel_luminance(colour.g)
        + 0.0722 * channel_luminance(colour.b)
}

/// WCAG 2.x contrast ratio between two colours. Order of the two arguments
/// does not matter — the lighter one is always the numerator — so a call site
/// can read `contrast(text, background)` without having to know or care which
/// of the two is lighter.
///
/// This is the arithmetic; [`contrast_ratio`] below is the same thing spelled
/// for the authored hexes. Card K8 split them rather than adding a second
/// implementation, because the wallpaper accent has to ask this question of
/// colours that were never written down as strings, and "there are already
/// 4.5:1 checks in that file, reuse rather than duplicate" was the
/// instruction.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (lighter, darker) = if la >= lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

/// [`contrast`], for the `#RRGGBB` strings the authored token table is written
/// in. Every existing caller — H2's accent audit, J3's neutral audit — reads
/// this one, because what those tests hold up against each other are the
/// literals in this file.
pub fn contrast_ratio(a: &str, b: &str) -> f64 {
    contrast(Rgb::from_hex(a), Rgb::from_hex(b))
}

/// Shared machinery for this crate's two font-coverage tests: card K34's, in
/// `mod tests` below, and card K26's, in `src/glyph_coverage.rs`, which walks
/// every literal the crate ships looking for one no bundled face can draw.
/// Both need the same two steps — load the four bundled faces, and map every
/// name each one answers to (lowercased, from its own `name` table) back to
/// its own bytes — so it lives here once rather than being written twice at
/// the two call sites, which is exactly the shape a future third check would
/// otherwise copy a third time.
///
/// It reads the bundled font files' own `cmap` and `name` tables with
/// `skrifa` (already in the tree via `parley`/`swash` — see this crate's
/// `[dev-dependencies]`) rather than asking fontconfig, on purpose: the whole
/// failure mode both cards are guarding against is a system font answering on
/// the laptop and nothing answering on the phone, so a check that could be
/// satisfied by a system font would pass on exactly the machine that cannot
/// see the bug.
#[cfg(test)]
pub(crate) mod font_coverage {
    use skrifa::{FontRef, MetadataProvider, string::StringId};
    use std::collections::HashMap;

    /// The four faces the binary actually carries — see `crate::FONTS`.
    const FILES: &[&[u8]] = &[
        include_bytes!("../assets/fonts/Newsreader[opsz,wght].ttf"),
        include_bytes!("../assets/fonts/Newsreader-Italic[opsz,wght].ttf"),
        include_bytes!("../assets/fonts/Karla[wght].ttf"),
        include_bytes!("../assets/fonts/DejaVuSansMono.ttf"),
    ];

    /// The lowercased family name every stack in this file actually ends in
    /// (`FONT_DISPLAY`/`FONT_UI`/`FONT_MONO`, card K34) — the one bundled
    /// face whose coverage every one of them can rely on. A static string
    /// doesn't know which stack will draw it, so this is the only guarantee
    /// that holds regardless of which one does; card K26's scan checks every
    /// literal the crate ships against exactly this face and no other, for
    /// that reason.
    pub(crate) const DEJAVU_SANS_MONO: &str = "dejavu sans mono";

    /// Every name each bundled file's own `name` table answers to (family, id
    /// 1, and typographic family, id 16 — Newsreader's variable-font instance
    /// name is "Newsreader 16pt", but its typographic family, the one a CSS
    /// stack actually says, is "Newsreader") mapped to that file's bytes, so
    /// a stack entry is checked against real glyph coverage instead of an
    /// assumption about which of the two ids is "the" name.
    pub(crate) fn by_name() -> HashMap<String, &'static [u8]> {
        let mut by_name = HashMap::new();
        for bytes in FILES {
            let font = FontRef::new(bytes).expect("a bundled font file failed to parse");
            for id in [StringId::FAMILY_NAME, StringId::TYPOGRAPHIC_FAMILY_NAME] {
                for entry in font.localized_strings(id) {
                    by_name.entry(entry.to_string().to_lowercase()).or_insert(*bytes);
                }
            }
        }
        by_name
    }

    /// Does the bundled face named `name` (already lowercased — a generic
    /// keyword like `sans-serif` or a system face like `Georgia`/`'Helvetica
    /// Neue'` is simply absent from `by_name` and this returns `false`,
    /// which is the point: a system font must never be the thing that saves
    /// either of these tests) carry a glyph for `ch`?
    pub(crate) fn covers(by_name: &HashMap<String, &'static [u8]>, name: &str, ch: char) -> bool {
        by_name
            .get(name)
            .map(|bytes| {
                let font = FontRef::new(bytes).expect("parsed in by_name above");
                font.charmap().map(ch).is_some()
            })
            .unwrap_or(false)
    }
}

/// The seeds every derived-accent assertion in this crate is measured on —
/// here at file scope, rather than inside `mod tests` where K8 wrote it,
/// because `src/store/settings.rs` asserts the same promise from the other
/// side of the seam and card K57 was not willing to let it keep its own
/// shorter copy of the table. Two tables would be two things to remember to
/// widen, and the one that got forgotten would be the one guarding the arm
/// nobody was thinking about.
///
/// The four authored accents are checked as a table of literals somebody
/// chose; a derived one has to be checked as a *rule*, because its input is
/// whatever the device reports — a photograph's dominant colour, or a palette
/// tone generated from one. So these are deliberately worse than anything
/// Material You would hand over: the two ends of the range, the app's own
/// paper (a seed that is exactly the colour we are about to draw it on), and a
/// saturated yellow, which is the classic accent that looks fine in a swatch
/// and disappears the moment it is used as text.
///
/// Nothing measured against this table asserts a hex. A hex would pin the
/// arithmetic rather than the promise, and the promise is the handoff's: *the
/// accent clears 4.5:1 against paper*. Every assertion is on a measured ratio.
///
/// `pub(crate)` and `#[cfg(test)]` together, following [`font_coverage`] just
/// above — a fixture the whole crate's tests may read and no shipped code can.
#[cfg(test)]
pub(crate) const AWKWARD_SEEDS: &[(&str, Rgb)] = &[
    ("black", Rgb::new(0, 0, 0)),
    ("white", Rgb::new(255, 255, 255)),
    ("mid grey", Rgb::new(128, 128, 128)),
    ("the app's own light paper", Rgb::new(0xFB, 0xF7, 0xF0)),
    ("the app's own dark paper", Rgb::new(0x18, 0x15, 0x12)),
    ("saturated yellow", Rgb::new(255, 214, 0)),
    ("a plausible Material You blue", Rgb::new(0x3F, 0x51, 0xB5)),
];

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
    /// It reads the bundled font files' own `cmap` and `name` tables through
    /// [`font_coverage`], the helper card K26 lifted out of this test so that
    /// its own scan over every shipped literal could share it. Neither check
    /// asks fontconfig, on purpose: the whole failure mode is a system font
    /// answering on the laptop and nothing answering on the phone, so a check
    /// that can be satisfied by a system font would pass on exactly the
    /// machine that cannot see the bug.
    #[test]
    fn bundled_fonts_cover_the_music_accidentals_named_in_every_stack() {
        let by_name = font_coverage::by_name();

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
                let covered = stack_names(css)
                    .iter()
                    .any(|name| font_coverage::covers(&by_name, name, sign));
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

    // -----------------------------------------------------------------------
    // K8 — the derived accent. The one accent nobody authored.
    // -----------------------------------------------------------------------
    //
    // The table these walk is [`AWKWARD_SEEDS`], which moved out to file scope
    // in K57 so that `src/store/settings.rs` could measure the same promise on
    // the same seeds; its doc comment is where the reasoning for the table now
    // lives.

    #[test]
    fn a_derived_accent_clears_4_5_to_1_as_text_on_paper_in_both_modes() {
        for (what, seed) in AWKWARD_SEEDS {
            let accent = DerivedAccent::from_seed(AccentSource::Wallpaper, *seed);

            let light = contrast(accent.colours(false).base, Rgb::from_hex(LIGHT_PAPER));
            assert!(
                light >= MIN_CONTRAST,
                "{what} ({seed}): base vs light paper is only {light:.2}:1"
            );

            let dark = contrast(accent.colours(true).base, Rgb::from_hex(DARK_PAPER));
            assert!(
                dark >= MIN_CONTRAST,
                "{what} ({seed}): base_dark vs dark paper is only {dark:.2}:1"
            );
        }
    }

    /// The same three pairings H2 asserts for the authored four, asked of the
    /// derived one. `on_tint` is held against the neutral `fill` as well,
    /// because that is J3's thirteenth pairing and it is the one the authored
    /// table actually failed.
    #[test]
    fn a_derived_accents_text_pairs_clear_4_5_to_1_in_both_modes() {
        for (what, seed) in AWKWARD_SEEDS {
            let accent = DerivedAccent::from_seed(AccentSource::Wallpaper, *seed);
            for (mode, paper, fill) in [
                ("light", LIGHT_PAPER, LIGHT_FILL),
                ("dark", DARK_PAPER, DARK_FILL),
            ] {
                let dark = mode == "dark";
                let c = accent.colours(dark);

                let on_tint = contrast(c.on_tint, c.tint);
                assert!(
                    on_tint >= MIN_CONTRAST,
                    "{what} ({seed}, {mode}): on_tint vs tint is only {on_tint:.2}:1"
                );

                let on_fill = contrast(c.on_tint, Rgb::from_hex(fill));
                assert!(
                    on_fill >= MIN_CONTRAST,
                    "{what} ({seed}, {mode}): on_tint vs fill is only {on_fill:.2}:1 — this is \
                     the pairing setlist_detail.rs's \"Undo\" link is drawn in"
                );

                let on_accent = contrast(c.on_accent, c.base);
                assert!(
                    on_accent >= MIN_CONTRAST,
                    "{what} ({seed}, {mode}): on_accent vs base is only {on_accent:.2}:1"
                );

                // And the paper it is all sitting on is the paper we said it
                // was — `on_accent` is that colour by construction, and this
                // is the assertion that keeps the construction honest rather
                // than merely internally consistent.
                assert_eq!(c.on_accent, Rgb::from_hex(paper));
            }
        }
    }

    /// The card's own words: *a wallpaper colour that already clears 4.5:1 is
    /// kept*. A user whose wallpaper is a deep readable blue gets their blue,
    /// not this app's opinion of it.
    #[test]
    fn a_seed_that_already_clears_the_bar_is_used_exactly_as_it_came() {
        // #1B3A6B on #FBF7F0 is comfortably past 4.5:1 — asserted here rather
        // than assumed, so that a future change to `contrast` cannot make this
        // test vacuous by moving the seed below the bar.
        let seed = Rgb::new(0x1B, 0x3A, 0x6B);
        let measured = contrast(seed, Rgb::from_hex(LIGHT_PAPER));
        assert!(measured >= MIN_CONTRAST, "the fixture itself drifted: {measured:.2}:1");

        let base = DerivedAccent::from_seed(AccentSource::Wallpaper, seed).colours(false).base;
        assert_eq!(base, seed, "a seed that was already legible was darkened anyway");
    }

    /// And the other half of it: one that does *not* clear the bar comes back
    /// darker than it went in, and only just past the bar rather than crushed
    /// to black. The second half is what `PUSH_STEP` is small for.
    #[test]
    fn a_seed_that_misses_the_bar_is_darkened_until_it_clears_and_no_further() {
        // A mid-tone orange: plainly visible, and plainly not 4.5:1 on cream.
        let seed = Rgb::new(0xE8, 0x84, 0x5C);
        let before = contrast(seed, Rgb::from_hex(LIGHT_PAPER));
        assert!(before < MIN_CONTRAST, "the fixture itself drifted: {before:.2}:1");

        let base = DerivedAccent::from_seed(AccentSource::Wallpaper, seed).colours(false).base;
        let after = contrast(base, Rgb::from_hex(LIGHT_PAPER));
        assert!(after >= MIN_CONTRAST, "still only {after:.2}:1 after darkening");
        assert!(
            after < MIN_CONTRAST + 1.0,
            "darkened well past the bar ({after:.2}:1) — the walk is meant to stop at the \
             first step that clears, so that the user's own colour survives as far as it can"
        );
        assert!(
            relative_luminance(base) < relative_luminance(seed),
            "it cleared the bar by getting lighter, on light paper"
        );
    }

    /// The dark mode goes the other way, and this is the assertion that would
    /// catch a `derive_family` that darkened in both — which would look right
    /// in light mode and paint dark mode almost black on black.
    #[test]
    fn the_dark_mode_variant_is_lighter_than_the_seed_when_the_seed_is_too_dark() {
        let seed = Rgb::new(0x1B, 0x3A, 0x6B);
        let base_dark = DerivedAccent::from_seed(AccentSource::Wallpaper, seed).colours(true).base;
        assert!(
            relative_luminance(base_dark) > relative_luminance(seed),
            "a dark navy stayed dark on the dark theme's near-black paper"
        );
    }

    /// `Display` is what every `format!` in this file's `tokens` output goes
    /// through, so the six digits it writes have to be the six digits the
    /// authored table already wrote — uppercase, `#`-prefixed, zero-padded.
    /// Anything else would change every declaration in the token block for
    /// the four accents that are not supposed to be changing at all.
    #[test]
    fn an_authored_accent_still_writes_exactly_the_hexes_it_is_authored_as() {
        for accent in ACCENTS {
            let light = accent.colours(false);
            assert_eq!(light.base.to_string(), accent.base, "{}", accent.name);
            assert_eq!(light.tint.to_string(), accent.tint, "{}", accent.name);
            assert_eq!(light.on_tint.to_string(), accent.on_tint, "{}", accent.name);
            assert_eq!(light.on_accent.to_string(), accent.on_accent, "{}", accent.name);
            assert_eq!(light.dim.to_string(), accent.dim, "{}", accent.name);

            let dark = accent.colours(true);
            assert_eq!(dark.base.to_string(), accent.base_dark, "{}", accent.name);
            assert_eq!(dark.tint.to_string(), accent.tint_dark, "{}", accent.name);
            assert_eq!(dark.on_tint.to_string(), accent.on_tint_dark, "{}", accent.name);
            assert_eq!(dark.on_accent.to_string(), accent.on_accent_dark, "{}", accent.name);
            assert_eq!(dark.dim.to_string(), accent.dim_dark, "{}", accent.name);
        }
    }

    /// The FAB's shadow is the accent's own colour, in every mode and for both
    /// kinds of accent.
    ///
    /// It was `rgba(181,71,36,.5)` written out by hand at three call sites —
    /// Rust, forever, whatever the app was actually painted in. Nobody saw it
    /// while the shipped accent *was* Rust; a Material You palette put a purple
    /// button over an orange shadow and made it obvious. The assertion is that
    /// the three channels track `--sla-accent` rather than that they are any
    /// particular number, because the number is now whatever the device says.
    #[test]
    fn the_fab_shadow_follows_whatever_the_accent_is() {
        let cases = [
            (ResolvedAccent::Authored(RUST), false),
            (ResolvedAccent::Authored(RUST), true),
            (ResolvedAccent::Authored(ACCENTS[2]), false),
            (
                ResolvedAccent::Derived(DerivedAccent::from_seed(
                    AccentSource::System,
                    Rgb::new(177, 40, 255),
                )),
                true,
            ),
        ];

        for (accent, dark) in cases {
            let block = tokens(dark, accent);
            let base = accent.colours(dark).base;
            let wanted = format!(
                "--sla-accent-shadow: rgba({}, {}, {}, 0.5);",
                base.r, base.g, base.b
            );
            assert!(
                block.contains(&wanted),
                "{} in {dark:?} wanted {wanted}\n{block}",
                accent.name()
            );
        }

        // And the point of the whole change, stated as its own fact: an accent
        // that is not Rust does not get Rust's shadow. The loop above would
        // pass if `tokens` ignored its argument and every case happened to be
        // Rust, which is exactly the bug being fixed.
        let purple = ResolvedAccent::Derived(DerivedAccent::from_seed(
            AccentSource::System,
            Rgb::new(177, 40, 255),
        ));
        assert!(
            !tokens(true, purple).contains("rgba(181, 71, 36"),
            "a purple accent must not paint a rust shadow"
        );
    }

    /// The whole token block for an authored accent is byte-for-byte what it
    /// was before K8 rebuilt the middle of it. `tokens` is the one function
    /// every screen's colour comes out of, and the four authored accents are
    /// not what this card is changing.
    #[test]
    fn the_token_block_for_rust_is_unchanged_by_the_accent_rework() {
        let light = tokens(false, ResolvedAccent::Authored(RUST));
        assert!(light.contains("--sla-accent: #B54724;"), "{light}");
        assert!(light.contains("--sla-accent-tint: #F7E6DE;"), "{light}");
        assert!(light.contains("--sla-accent-on-tint: #8E3419;"), "{light}");
        assert!(light.contains("--sla-on-accent: #FBF7F0;"), "{light}");
        assert!(light.contains("--sla-accent-dim: #D8B4A2;"), "{light}");
        assert!(light.contains("--sla-paper: #FBF7F0;"), "{light}");

        let dark = tokens(true, ResolvedAccent::Authored(RUST));
        assert!(dark.contains("--sla-accent: #E8845C;"), "{dark}");
        assert!(dark.contains("--sla-paper: #181512;"), "{dark}");
    }

    /// The seed is kept verbatim alongside the family derived from it. It is
    /// the only record of what the platform actually reported — every other
    /// field has been through the contrast walk — so a future card that wants
    /// to show the user their wallpaper colour, or a bug report that needs to
    /// say what went in, has somewhere to read it.
    #[test]
    fn a_derived_accent_remembers_the_colour_it_was_built_from() {
        let seed = Rgb::new(255, 214, 0);
        let accent = DerivedAccent::from_seed(AccentSource::Wallpaper, seed);
        assert_eq!(accent.seed, seed);
        assert_ne!(
            accent.colours(false).base,
            seed,
            "this fixture is only interesting if the derivation had to move it"
        );
    }

    /// The derived accent has no name of its own, and the one it is given says
    /// where it came from rather than what colour it is — `derive::accent_note`
    /// prints this on the Settings row.
    ///
    /// Both provenances, and asserted to *differ*, because until K57 there was
    /// only one word here and the whole of that card's naming half is that the
    /// one word was wrong for the seed it had just started preferring. A
    /// `name()` that had been rewritten to return `"System"` unconditionally
    /// would pass an assertion on either line alone.
    #[test]
    fn a_derived_accent_is_named_after_where_it_came_from() {
        let seed = Rgb::new(9, 9, 9);
        let from_palette =
            ResolvedAccent::Derived(DerivedAccent::from_seed(AccentSource::System, seed));
        let from_wallpaper =
            ResolvedAccent::Derived(DerivedAccent::from_seed(AccentSource::Wallpaper, seed));

        assert_eq!(from_palette.name(), "System");
        assert_eq!(from_wallpaper.name(), "Wallpaper");
        assert_ne!(from_palette.name(), from_wallpaper.name());
        assert_eq!(ResolvedAccent::Authored(RUST).name(), "Rust");

        // Same seed, so the colours are identical and only the name is not —
        // which is the statement that the source is carried for the label and
        // takes no part in the arithmetic.
        for dark in [false, true] {
            assert_eq!(from_palette.colours(dark), from_wallpaper.colours(dark));
        }
    }

    /// `contrast_ratio` is now a wrapper over `contrast`, and the two have to
    /// stay the same number — every existing audit in this file goes through
    /// the string one and every K8 assertion goes through the other.
    #[test]
    fn the_string_and_the_channel_forms_of_the_contrast_check_agree() {
        assert_eq!(
            contrast_ratio(LIGHT_MUTED, LIGHT_PAPER),
            contrast(Rgb::from_hex(LIGHT_MUTED), Rgb::from_hex(LIGHT_PAPER))
        );
    }

}
