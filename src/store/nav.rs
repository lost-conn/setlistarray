use rinch::prelude::*;

use crate::model::{AttachmentId, SetlistId, SongId};

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
    /// The add/edit form (`1j`). One screen serves both: adding starts empty,
    /// editing arrives carrying the song it is about to overwrite.
    AddSong,
    EditSong(SongId),
    /// The typed lyrics/chords editor (D2) — the screen behind `1j`'s
    /// `TXT · Type lyrics / chords · ›` row, which the handoff never drew.
    /// One screen serves both jobs here too: `chart: None` writes a new
    /// `Text` attachment on Save, `Some(id)` reopens that one. The song is
    /// carried because a chart cannot exist without one — `attach` needs it,
    /// and ✕ goes back to it.
    TypeChart { song: SongId, chart: Option<AttachmentId> },
    /// Save a webpage offline (`1l`, card E2). Carries the song for the same
    /// reason `TypeChart` does: a capture cannot exist without one — it is
    /// `SongsStore::attach` that mints the row and the directory the page is
    /// written into — and ← has to land back on the song that opened it.
    ///
    /// It does *not* carry an attachment. A capture screen only ever produces
    /// a new chart; there is no "re-capture this one" way in, and if E6's
    /// re-check grows one it will be a different operation with a different
    /// question to answer (replace, or keep both?) rather than this route with
    /// an `Option` on it.
    CaptureWebpage { song: SongId },
    /// The full-screen attachment viewer (`1k`, card D5). Carries the song as
    /// well as the chart for two reasons, and neither is decoration: the top
    /// bar prints the song's name under the file's, and ← has to land back on
    /// the screen the viewer was opened from. A chart knows neither — the
    /// ownership arrow runs songs → attachments and never back (see
    /// `AttachmentsStore`'s header), so the only place that pairing exists is
    /// the route.
    ViewAttachment { song: SongId, attachment: AttachmentId },
    /// Wireframe screens still to be built out.
    Performance(SetlistId),
}

impl Route {
    /// Whether this screen takes the whole window, bottom nav included.
    ///
    /// One route answers yes today. The viewer (`1k`) is drawn edge to edge in
    /// dark chrome of its own, and a cream tab bar under it would be both the
    /// wrong colour and 60 px of a chart somebody is reading off a music stand.
    /// It is a method on `Route` rather than a flag the viewer sets, because
    /// the two things that have to obey it — the bottom nav and the strip
    /// behind the status bar — are drawn by `crate::app`, above the screen, and
    /// the route is the only thing they and the viewer share.
    pub fn full_screen(self) -> bool {
        matches!(self, Route::ViewAttachment { .. })
    }
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
    /// The setlist song picker (`1i`), when open over a setlist. The setlist
    /// detail screen stays mounted and visible behind it — that is the whole
    /// reason `1i` was chosen over the two-step wizard.
    pub picking_songs_for: Signal<Option<SetlistId>>,
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
            picking_songs_for: Signal::new(None),
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
