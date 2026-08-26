use rinch::prelude::*;

use crate::model::{SetlistId, SongId};

/// Two bottom-nav tabs only. Settings is a gear in each tab's header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Songs,
    Setlists,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Library,
    Setlists,
    SongDetail(SongId),
    SetlistDetail(SetlistId),
    Settings,
    /// Wireframe screens still to be built out.
    AddSong,
    Performance(SetlistId),
}

#[derive(Clone, Copy)]
pub struct NavStore {
    pub route: Signal<Route>,
    /// Each tab keeps its own scroll position and filter state; the route
    /// stack is per-tab for the same reason.
    pub tab: Signal<Tab>,
    /// The add-to-setlist bottom sheet, when open for a song.
    pub add_to_setlist_for: Signal<Option<SongId>>,
    /// The sort & group bottom sheet.
    pub sort_sheet_open: Signal<bool>,
    /// The setlist card currently in inline rename mode on the Setlists tab
    /// (opened from its long-press/right-click menu), and the text field's
    /// live draft. Screen-transient UI state, same as the two fields above —
    /// not worth a store of its own for one small pair of signals.
    pub renaming_setlist: Signal<Option<SetlistId>>,
    pub rename_draft: Signal<String>,
}

impl NavStore {
    pub fn new() -> Self {
        Self {
            route: Signal::new(Route::Library),
            tab: Signal::new(Tab::Songs),
            add_to_setlist_for: Signal::new(None),
            sort_sheet_open: Signal::new(false),
            renaming_setlist: Signal::new(None),
            rename_draft: Signal::new(String::new()),
        }
    }

    pub fn go(self, route: Route) {
        self.route.set(route);
    }

    pub fn select_tab(self, tab: Tab) {
        self.tab.set(tab);
        self.route.set(match tab {
            Tab::Songs => Route::Library,
            Tab::Setlists => Route::Setlists,
        });
    }

    /// Back always lands on the tab's root for now — deep stacks come with
    /// the screens that need them.
    pub fn back(self) {
        let tab = self.tab.get();
        self.route.set(match tab {
            Tab::Songs => Route::Library,
            Tab::Setlists => Route::Setlists,
        });
    }
}

impl Default for NavStore {
    fn default() -> Self {
        Self::new()
    }
}
