use rinch::prelude::*;

use crate::capture::CaptureMode;
use crate::store::Storage;
use crate::theme::{ACCENTS, Accent, RUST};

/// Resolution order for the accent: a user pick wins; otherwise the Material
/// You primary extracted from the wallpaper, darkened until it clears 4.5:1
/// against paper; otherwise Rust.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccentChoice {
    FromSystem,
    Named(usize),
}

impl AccentChoice {
    pub fn resolve(self) -> Accent {
        match self {
            // TODO: wallpaper extraction is a platform call Rinch does not
            // expose yet — falls through to the default until it does.
            //
            // Card H2 kept `FromSystem` off the Settings accent picker for
            // exactly this reason: offering it as a fifth option would mean
            // Rust today regardless of what the user picked, which is a
            // control that lies about what it does. That is fine while
            // `FromSystem` is the *only* value `Preferences` can hold before
            // a user ever touches the picker — nobody has "chosen" Rust, the
            // app just hasn't been told otherwise. It stops being fine the
            // day K8 gives this arm a real wallpaper colour: at that point
            // whoever has tapped Rust/Pine/Indigo/Plum in Settings is on
            // `Named(_)` forever, with no control anywhere that sets them
            // back to `FromSystem`, because H2 built no such control. Adding
            // that fifth "follow the system" option to the picker is K8's
            // work, not a gap left here — but it is a real piece of that
            // card's scope, not a footnote, so it is written down here where
            // whoever picks up K8 will be reading this match arm anyway.
            AccentChoice::FromSystem => RUST,
            AccentChoice::Named(i) => ACCENTS[i.min(ACCENTS.len() - 1)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins today's answer so K8 changing it — the day wallpaper extraction
    /// exists — is a deliberate edit to this test rather than a silent
    /// behaviour change nobody noticed. See the long comment on `resolve`
    /// above for why this arm is temporary and what has to accompany the day
    /// it changes.
    #[test]
    fn accent_choice_from_system_resolves_to_rust_until_wallpaper_extraction_lands() {
        assert_eq!(AccentChoice::FromSystem.resolve(), RUST);
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
    /// Mode follows the Android system setting; this mirrors it.
    pub dark_mode: Signal<bool>,
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
}

impl SettingsStore {
    /// The defaults, remembering nothing.
    pub fn new() -> Self {
        Self::restored(Storage::in_memory())
    }

    /// Settings as they were left. They share the Preferences row with the
    /// library view — see `src/db/prefs.rs`.
    pub fn restored(storage: Storage) -> Self {
        let preferences = storage.preferences();
        Self {
            dark_mode: Signal::new(preferences.dark_mode),
            accent: Signal::new(preferences.accent),
            performance_theme: Signal::new(preferences.performance_theme),
            keep_awake: Signal::new(preferences.keep_awake),
            recheck_saved_pages: Signal::new(preferences.recheck_saved_pages),
            default_tuning: Signal::new(preferences.default_tuning),
            capture_mode: Signal::new(preferences.capture_mode),
            storage,
        }
    }

    pub fn accent_resolved(self) -> Accent {
        self.accent.get().resolve()
    }

    pub fn toggle_dark(self) {
        self.set_dark(!self.dark_mode.get());
    }

    pub fn set_dark(self, dark: bool) {
        self.dark_mode.set(dark);
        self.storage.remember(|p| p.dark_mode = dark);
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
