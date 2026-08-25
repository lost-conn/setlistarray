use rinch::prelude::*;

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
            AccentChoice::FromSystem => RUST,
            AccentChoice::Named(i) => ACCENTS[i.min(ACCENTS.len() - 1)],
        }
    }
}

/// Performance mode follows the app theme by default; forcing dark is a
/// setting, not a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerformanceTheme {
    FollowApp,
    AlwaysDark,
}

#[derive(Clone, Copy)]
pub struct SettingsStore {
    /// Mode follows the Android system setting; this mirrors it.
    pub dark_mode: Signal<bool>,
    pub accent: Signal<AccentChoice>,
    pub performance_theme: Signal<PerformanceTheme>,
    pub keep_awake: Signal<bool>,
    pub recheck_saved_pages: Signal<bool>,
}

impl SettingsStore {
    pub fn new() -> Self {
        Self {
            dark_mode: Signal::new(false),
            accent: Signal::new(AccentChoice::FromSystem),
            performance_theme: Signal::new(PerformanceTheme::FollowApp),
            keep_awake: Signal::new(true),
            recheck_saved_pages: Signal::new(false),
        }
    }

    pub fn accent_resolved(self) -> Accent {
        self.accent.get().resolve()
    }

    pub fn toggle_dark(self) {
        self.dark_mode.set(!self.dark_mode.get());
    }
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new()
    }
}
