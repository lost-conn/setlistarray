use rinch::prelude::*;

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

#[derive(Clone, Copy)]
pub struct SettingsStore {
    /// Mode follows the Android system setting; this mirrors it.
    pub dark_mode: Signal<bool>,
    pub accent: Signal<AccentChoice>,
    pub performance_theme: Signal<PerformanceTheme>,
    pub keep_awake: Signal<bool>,
    pub recheck_saved_pages: Signal<bool>,
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
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new()
    }
}
