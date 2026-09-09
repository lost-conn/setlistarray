use rinch::prelude::*;

use crate::capture::CaptureMode;
use crate::store::{Storage, SystemStore};
use crate::theme::{ACCENTS, RUST, ResolvedAccent, Rgb, WallpaperAccent};

/// Resolution order for the accent: a user pick wins; otherwise the Material
/// You primary extracted from the wallpaper, darkened until it clears 4.5:1
/// against paper; otherwise Rust.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccentChoice {
    FromSystem,
    Named(usize),
}

impl AccentChoice {
    /// The accent this choice actually paints, given whatever the platform
    /// last said the wallpaper's primary colour was
    /// (`SystemStore::wallpaper`).
    ///
    /// **This arm stopped lying in card K8.** What used to be here was a TODO
    /// and a `=> RUST`, with a long note explaining that wallpaper extraction
    /// was a platform call Rinch did not expose, that card H2 had therefore
    /// kept "follow system" off the Settings accent picker rather than ship a
    /// control that silently meant Rust — and that the day the platform call
    /// arrived, whoever had tapped Rust/Pine/Indigo/Plum would be on
    /// `Named(_)` forever with no control anywhere that put them back on
    /// `FromSystem`. All three of those are settled now: the call exists
    /// (`platform::wallpaper_primary`), the derivation exists
    /// (`theme::WallpaperAccent`, which is where the 4.5:1 arithmetic lives),
    /// and the fifth chip H2 withheld is on the picker — see
    /// `screens::settings`'s `accent_row`, which also had to stop deciding
    /// which chip is selected by comparing *resolved* accents, because on a
    /// device with no wallpaper colour `FromSystem` and `Named(0)` resolve to
    /// the same Rust and would both have lit up.
    ///
    /// **Rust is still the answer when there is no wallpaper colour**, and
    /// that is the ordinary path rather than the error path: `None` is what
    /// most live wallpapers, most OEM wallpaper stacks and every desktop build
    /// report, and it is what the phone this card was verified on reports.
    /// `platform::wallpaper_primary`'s own doc comment has the list.
    pub fn resolve(self, wallpaper: Option<Rgb>) -> ResolvedAccent {
        match self {
            AccentChoice::FromSystem => match wallpaper {
                Some(seed) => ResolvedAccent::Wallpaper(WallpaperAccent::from_seed(seed)),
                None => ResolvedAccent::Authored(RUST),
            },
            AccentChoice::Named(i) => {
                ResolvedAccent::Authored(ACCENTS[i.min(ACCENTS.len() - 1)])
            }
        }
    }

    /// The five options the Settings picker draws, in the order it draws them:
    /// the four authored accents, then "follow the system".
    ///
    /// System **last**, not first. It is the default a fresh install starts on
    /// (see `Preferences::default`), so first would put the resting state at
    /// the head of the row — but the four named ones are the four the note on
    /// the right of the row can name, and a person scanning the chips is
    /// looking for a colour. "System" is the one that is not a colour, so it
    /// sits at the end where an "other" belongs.
    pub fn all() -> [AccentChoice; 5] {
        [
            AccentChoice::Named(0),
            AccentChoice::Named(1),
            AccentChoice::Named(2),
            AccentChoice::Named(3),
            AccentChoice::FromSystem,
        ]
    }

    /// What the chip for this choice says. The *choice*, not the colour it
    /// resolves to — "System" keeps saying System on a device with no
    /// wallpaper to read, because the chip is the control and the control is
    /// still doing what it says. What colour that turned out to be is the
    /// job of the note on the right of the row (`derive::accent_note`).
    pub fn label(self) -> &'static str {
        match self {
            AccentChoice::FromSystem => "System",
            AccentChoice::Named(i) => ACCENTS[i.min(ACCENTS.len() - 1)].name,
        }
    }
}

/// Light, Dark, or whatever the system is set to — card K8.
///
/// **Why a third state rather than a second switch.** `1q` draws two rows,
/// "Dark mode" and "Dark mode follows system", and that is two controls for
/// one answer: with the follow switch on, the dark switch shows something that
/// is not what it does, and every combination of the two has to mean
/// something. Three exclusive options mean exactly three things and cannot be
/// set to a contradiction, which is also the shape `PerformanceTheme` and
/// `Density` already use on this screen.
///
/// The `name`/`from_name` pair is the established idiom for a value that is
/// persisted — see [`PerformanceTheme`] and [`DefaultTuning`] — and is
/// deliberately not the on-screen label, so that rewording a chip is not a
/// migration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChoice {
    Light,
    Dark,
    FollowSystem,
}

impl ThemeChoice {
    pub fn name(self) -> &'static str {
        match self {
            ThemeChoice::Light => "Light",
            ThemeChoice::Dark => "Dark",
            ThemeChoice::FollowSystem => "FollowSystem",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.name() == name)
    }

    /// What the chip says. Two words for the third one because "System" alone
    /// would be a colour on the accent row two rows above it and a theme here.
    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::Light => "Light",
            ThemeChoice::Dark => "Dark",
            ThemeChoice::FollowSystem => "Follow system",
        }
    }

    pub const ALL: [ThemeChoice; 3] = [
        ThemeChoice::Light,
        ThemeChoice::Dark,
        ThemeChoice::FollowSystem,
    ];
}

impl Default for ThemeChoice {
    /// **Follow the system, on a fresh install.**
    ///
    /// The old default was `dark_mode: false` — light, always, until somebody
    /// found the switch — and it was not chosen so much as inherited: a `bool`
    /// has to start somewhere and `false` is where a `bool` starts. It is
    /// worth arguing rather than inheriting again, because this is the value
    /// every new install lands on and most installs never change.
    ///
    /// Following the system is the better default for one reason that
    /// outweighs the rest: a person who has set their phone to dark has
    /// already answered this question, once, for every app on the device, and
    /// an app that opens light anyway is asking them to answer it a second
    /// time. The handoff asks for exactly this row ("Dark mode follows
    /// system") and it was only ever absent because nothing could read the
    /// setting.
    ///
    /// It also costs nothing on a device that has no answer:
    /// `platform::night_mode()` returns `None` on the desktop and on any
    /// Android build that leaves `uiMode` undefined, and `derive::dark_active`
    /// resolves `None` to light — so the fresh-install default degrades
    /// exactly onto the old one wherever the new reading is unavailable.
    fn default() -> Self {
        ThemeChoice::FollowSystem
    }
}

#[cfg(test)]
mod theme_choice_tests {
    use super::*;

    /// `name` is the stored form and has to survive a trip through it for all
    /// three — the round trip in `src/db/prefs.rs` only exercises whichever
    /// one its fixture happens to pick.
    #[test]
    fn every_theme_choice_survives_its_own_stored_name() {
        for choice in ThemeChoice::ALL {
            assert_eq!(ThemeChoice::from_name(choice.name()), Some(choice));
        }
    }

    #[test]
    fn an_unrecognised_stored_name_is_none_rather_than_a_guess() {
        assert_eq!(ThemeChoice::from_name("Sepia"), None);
        // And in particular, the *old* field's values are not accidentally
        // readable as the new field's names. A `Preferences` row written
        // before this card has a `dark_mode: Bool`, not a string, and the
        // migration in `src/db/prefs.rs` is where that is handled — this is
        // the assertion that stops anybody "helpfully" making "true" parse.
        assert_eq!(ThemeChoice::from_name("true"), None);
        assert_eq!(ThemeChoice::from_name("false"), None);
    }

    #[test]
    fn a_fresh_install_follows_the_system() {
        assert_eq!(ThemeChoice::default(), ThemeChoice::FollowSystem);
    }

    /// The stored name and the on-screen label are separate on purpose, and
    /// for `FollowSystem` they genuinely differ — which is the case that would
    /// catch anybody collapsing the two methods back into one.
    #[test]
    fn the_label_is_not_the_stored_name_where_the_two_have_no_reason_to_agree() {
        assert_eq!(ThemeChoice::FollowSystem.name(), "FollowSystem");
        assert_eq!(ThemeChoice::FollowSystem.label(), "Follow system");
    }
}

#[cfg(test)]
mod accent_choice_tests {
    use super::*;
    use crate::theme::{MIN_CONTRAST, contrast};

    /// The device this card was verified on, and every desktop build: no
    /// wallpaper colour to be had, so `FromSystem` lands on Rust. This test
    /// replaces the one H2 left behind
    /// (`accent_choice_from_system_resolves_to_rust_until_wallpaper_extraction_lands`),
    /// which pinned the same answer for the opposite reason — that there was
    /// no wallpaper *call*. There is one now; it just says `None` here.
    #[test]
    fn from_system_with_no_wallpaper_colour_falls_back_to_rust() {
        assert_eq!(
            AccentChoice::FromSystem.resolve(None),
            ResolvedAccent::Authored(RUST)
        );
    }

    /// And when there *is* one, it is used — which is the whole of what K8
    /// changed, and the thing that cannot be seen on the development phone.
    #[test]
    fn from_system_with_a_wallpaper_colour_uses_it_rather_than_rust() {
        let seed = Rgb::new(0x3F, 0x51, 0xB5);
        let resolved = AccentChoice::FromSystem.resolve(Some(seed));
        assert_ne!(resolved, ResolvedAccent::Authored(RUST));
        assert_eq!(resolved.name(), "Wallpaper");
    }

    /// The handoff's rule, measured rather than pinned to a hex: whatever the
    /// wallpaper turns out to be, what the app paints with clears 4.5:1
    /// against the paper it is painted on, in both modes.
    #[test]
    fn a_wallpaper_accent_is_legible_on_paper_whatever_the_wallpaper_was() {
        for seed in [
            Rgb::new(0, 0, 0),
            Rgb::new(255, 255, 255),
            Rgb::new(255, 214, 0),
            Rgb::new(0x3F, 0x51, 0xB5),
        ] {
            let resolved = AccentChoice::FromSystem.resolve(Some(seed));
            for dark in [false, true] {
                let colours = resolved.colours(dark);
                let paper = Rgb::from_hex(crate::theme::paper(dark));
                let ratio = contrast(colours.base, paper);
                assert!(
                    ratio >= MIN_CONTRAST,
                    "{seed} in {} mode is only {ratio:.2}:1 on paper",
                    if dark { "dark" } else { "light" }
                );
            }
        }
    }

    /// A user pick is a user pick: the wallpaper is read, and then ignored.
    #[test]
    fn a_named_accent_ignores_the_wallpaper_entirely() {
        let seed = Some(Rgb::new(0x3F, 0x51, 0xB5));
        assert_eq!(
            AccentChoice::Named(1).resolve(seed),
            ResolvedAccent::Authored(ACCENTS[1])
        );
    }

    /// A stored index from a future version with more accents than this build
    /// has clamps rather than panicking — the property `Named(i.min(len - 1))`
    /// has always had and nothing has ever asserted.
    #[test]
    fn an_index_past_the_end_of_the_table_clamps_to_the_last_accent() {
        assert_eq!(
            AccentChoice::Named(99).resolve(None),
            ResolvedAccent::Authored(ACCENTS[ACCENTS.len() - 1])
        );
    }

    /// The picker draws all five and every one of them is reachable, which is
    /// the trap H2's comment described: whoever is on `Named(_)` today has to
    /// have a way back to `FromSystem`.
    #[test]
    fn the_picker_offers_every_choice_including_the_way_back_to_the_system() {
        let all = AccentChoice::all();
        assert_eq!(all.len(), ACCENTS.len() + 1);
        assert!(all.contains(&AccentChoice::FromSystem), "no way back to the system");
        for index in 0..ACCENTS.len() {
            assert!(all.contains(&AccentChoice::Named(index)), "accent {index} is unreachable");
        }
    }

    #[test]
    fn a_chip_is_labelled_with_the_choice_rather_than_the_colour_it_resolved_to() {
        assert_eq!(AccentChoice::Named(0).label(), "Rust");
        assert_eq!(AccentChoice::FromSystem.label(), "System");
    }
}

/// Performance mode follows the app theme by default; forcing dark is a
/// setting, not a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerformanceTheme {
    FollowApp,
    AlwaysDark,
}

impl PerformanceTheme {
    pub fn name(self) -> &'static str {
        match self {
            PerformanceTheme::FollowApp => "FollowApp",
            PerformanceTheme::AlwaysDark => "AlwaysDark",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "FollowApp" => Some(PerformanceTheme::FollowApp),
            "AlwaysDark" => Some(PerformanceTheme::AlwaysDark),
            _ => None,
        }
    }
}

/// The tuning a **new** song's Tuning field starts on — card H5.
///
/// A fixed list, and a short one, even though `Song::tuning` itself is free
/// text. That split is the card's whole decision, not an oversight: the
/// *default* only has to name a common starting point somebody can recognise
/// in a `▾`, while the *song* keeps free text so the one player in Nashville
/// tuning or DGDGBD still has a place to type it. A free-text box here would
/// be honest about the model but would make "default" mean nothing more than
/// "last thing typed", and a picker built from *this book's own* tunings
/// (the way the filter sheet's Tuning facet is) would make the default
/// depend on what has already been entered — hardly a default for the first
/// song in an empty library, which is exactly when this setting matters
/// most. Seven named tunings, closed, is the smallest option of the card's
/// three that does not lie about one thing or the other.
///
/// The seven are the ones a working guitarist actually reaches for: standard,
/// the two common down-tunings, and the four open/modal tunings a chord chart
/// is likely to call out by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefaultTuning {
    Standard,
    DropD,
    HalfStepDown,
    DropCSharp,
    Dadgad,
    OpenG,
    OpenD,
}

impl DefaultTuning {
    /// What this is called, on screen and — because every one of these names
    /// is itself a string a player would happily type into `Song::tuning` —
    /// what a new song's Tuning field is prefilled with. There is no second
    /// mapping to keep in sync: the label *is* the text.
    pub fn label(self) -> &'static str {
        match self {
            DefaultTuning::Standard => "Standard",
            DefaultTuning::DropD => "Drop D",
            DefaultTuning::HalfStepDown => "Half-step down",
            DefaultTuning::DropCSharp => "Drop C#",
            DefaultTuning::Dadgad => "DADGAD",
            DefaultTuning::OpenG => "Open G",
            DefaultTuning::OpenD => "Open D",
        }
    }

    /// The name it is stored under. Deliberately not [`label`](Self::label):
    /// a label is UI copy and may be reworded (or, for this one, have its
    /// punctuation changed — `label`'s `#` is not something a rename should
    /// have to migrate), a stored name may not.
    pub fn name(self) -> &'static str {
        match self {
            DefaultTuning::Standard => "Standard",
            DefaultTuning::DropD => "DropD",
            DefaultTuning::HalfStepDown => "HalfStepDown",
            DefaultTuning::DropCSharp => "DropCSharp",
            DefaultTuning::Dadgad => "Dadgad",
            DefaultTuning::OpenG => "OpenG",
            DefaultTuning::OpenD => "OpenD",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    pub const ALL: [DefaultTuning; 7] = [
        DefaultTuning::Standard,
        DefaultTuning::DropD,
        DefaultTuning::HalfStepDown,
        DefaultTuning::DropCSharp,
        DefaultTuning::Dadgad,
        DefaultTuning::OpenG,
        DefaultTuning::OpenD,
    ];
}

impl Default for DefaultTuning {
    /// E standard, because that is what most guitars are strung in and most
    /// songs in an empty book will turn out to be.
    fn default() -> Self {
        DefaultTuning::Standard
    }
}

#[cfg(test)]
mod default_tuning_tests {
    use super::*;

    /// `name` is the stored form and has to survive a trip through it for
    /// every one of the seven — the round trip in `src/db/prefs.rs` only
    /// exercises whichever one its fixture happens to pick.
    #[test]
    fn every_default_tuning_survives_its_own_stored_name() {
        for tuning in DefaultTuning::ALL {
            assert_eq!(DefaultTuning::from_name(tuning.name()), Some(tuning));
        }
    }

    #[test]
    fn an_unrecognised_stored_name_is_none_rather_than_a_guess() {
        assert_eq!(DefaultTuning::from_name("Nashville"), None);
    }

    /// The card's own reasoning for why `label` and the prefill text are one
    /// method rather than two: every label is, verbatim, a string a player
    /// would type into `Song::tuning` by hand.
    #[test]
    fn no_label_is_empty() {
        for tuning in DefaultTuning::ALL {
            assert!(!tuning.label().is_empty());
        }
    }

    #[test]
    fn the_default_is_standard() {
        assert_eq!(DefaultTuning::default(), DefaultTuning::Standard);
    }
}

#[derive(Clone, Copy)]
pub struct SettingsStore {
    /// Light, Dark, or the system's own answer — see [`ThemeChoice`]. What is
    /// actually painted is [`dark_active`](Self::dark_active), because the
    /// third option is not a colour, it is a question asked of
    /// [`SystemStore::night`].
    ///
    /// This used to be a `Signal<bool>` whose doc comment read "Mode follows
    /// the Android system setting; this mirrors it" — which was aspirational
    /// rather than true, and stayed that way for as long as nothing could read
    /// the system setting. Card K8 made it true and had to widen the type to
    /// do it.
    pub theme: Signal<ThemeChoice>,
    pub accent: Signal<AccentChoice>,
    pub performance_theme: Signal<PerformanceTheme>,
    pub keep_awake: Signal<bool>,
    pub recheck_saved_pages: Signal<bool>,
    /// The tuning `song_form` prefills a **new** song's Tuning field with.
    /// Changing it is a change of mind about what to suggest next, not a
    /// rewrite of history — see [`DefaultTuning`] and `song_form::blank_draft`,
    /// the one place this is ever read.
    pub default_tuning: Signal<DefaultTuning>,
    /// Card E3's `Save as:`, remembered between captures.
    ///
    /// It is remembered because the choice is about the *user*, not about the
    /// page: somebody whose chord site is CifraClub pastes CifraClub URLs, and
    /// re-picking the same answer on every capture is a tax on the common case.
    /// It is not remembered per site, which would be the next thing to reach
    /// for and is worse — the app would then be silently deciding for a site
    /// the user has captured once, on the strength of one decision that may
    /// have been about that one page.
    pub capture_mode: Signal<CaptureMode>,
    storage: Storage,
    /// What the platform last said about itself — the night mode two of this
    /// store's answers depend on, and the wallpaper colour a third does.
    ///
    /// Held as a handle rather than passed to `dark_active`/`accent_resolved`
    /// at each of their call sites, on the same reasoning `SongsStore` is
    /// handed the `SetlistsStore` it has to reach into: the dependency runs
    /// one way, `SystemStore` is built first and depends on nothing, and the
    /// alternative is every screen that wants a colour having to know that the
    /// answer is assembled from two stores.
    system: SystemStore,
}

impl SettingsStore {
    /// The defaults, remembering nothing, on a platform read fresh.
    pub fn new() -> Self {
        Self::restored(Storage::in_memory(), SystemStore::read())
    }

    /// Settings as they were left. They share the Preferences row with the
    /// library view — see `src/db/prefs.rs`.
    pub fn restored(storage: Storage, system: SystemStore) -> Self {
        let preferences = storage.preferences();
        Self {
            theme: Signal::new(preferences.theme),
            accent: Signal::new(preferences.accent),
            performance_theme: Signal::new(preferences.performance_theme),
            keep_awake: Signal::new(preferences.keep_awake),
            recheck_saved_pages: Signal::new(preferences.recheck_saved_pages),
            default_tuning: Signal::new(preferences.default_tuning),
            capture_mode: Signal::new(preferences.capture_mode),
            storage,
            system,
        }
    }

    /// Re-derive every signal here from the Preferences row — card I2's
    /// import, through `crate::store::reload_all`. See
    /// [`LibraryViewStore::reload`](crate::store::LibraryViewStore::reload)
    /// for why this takes no argument and reads `storage.preferences()`
    /// again rather than being handed something.
    pub fn reload(self) {
        let preferences = self.storage.preferences();
        self.theme.set(preferences.theme);
        self.accent.set(preferences.accent);
        self.performance_theme.set(preferences.performance_theme);
        self.keep_awake.set(preferences.keep_awake);
        self.recheck_saved_pages.set(preferences.recheck_saved_pages);
        self.default_tuning.set(preferences.default_tuning);
        self.capture_mode.set(preferences.capture_mode);
    }

    /// The accent the app is painted in right now: the choice, resolved
    /// through the wallpaper colour the platform last reported.
    ///
    /// A signal read on both halves, so every style closure that calls this
    /// repaints both when the user taps a chip *and* when the wallpaper
    /// changes under a running app — which is the K53 half of this card and
    /// costs nothing extra here, because the reactivity was already how the
    /// first half worked.
    pub fn accent_resolved(self) -> ResolvedAccent {
        self.accent.get().resolve(self.system.wallpaper.get())
    }

    /// Whether the app is dark **right now**, which is the question every
    /// style closure actually has and is not the same as which option is
    /// selected in Settings.
    ///
    /// The rule itself is `crate::derive::dark_active`, a free function over
    /// two `Copy` values with no store in sight, so the whole resolution table
    /// is a `cargo test` rather than something only a phone can show you.
    pub fn dark_active(self) -> bool {
        crate::derive::dark_active(self.theme.get(), self.system.night.get())
    }

    pub fn set_theme(self, theme: ThemeChoice) {
        self.theme.set(theme);
        self.storage.remember(|p| p.theme = theme);
    }

    pub fn set_accent(self, accent: AccentChoice) {
        self.accent.set(accent);
        self.storage.remember(|p| p.accent = accent);
    }

    pub fn set_performance_theme(self, theme: PerformanceTheme) {
        self.performance_theme.set(theme);
        self.storage.remember(|p| p.performance_theme = theme);
    }

    pub fn set_keep_awake(self, keep_awake: bool) {
        self.keep_awake.set(keep_awake);
        self.storage.remember(|p| p.keep_awake = keep_awake);
    }

    pub fn set_recheck_saved_pages(self, recheck: bool) {
        self.recheck_saved_pages.set(recheck);
        self.storage.remember(|p| p.recheck_saved_pages = recheck);
    }

    pub fn set_default_tuning(self, tuning: DefaultTuning) {
        self.default_tuning.set(tuning);
        self.storage.remember(|p| p.default_tuning = tuning);
    }

    pub fn set_capture_mode(self, mode: CaptureMode) {
        self.capture_mode.set(mode);
        self.storage.remember(|p| p.capture_mode = mode);
    }
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new()
    }
}
