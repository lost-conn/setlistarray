use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::model::Song;
use crate::store::Storage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupBy {
    Confidence,
    FirstLetter,
    Artist,
    Tag,
    Tuning,
    None,
}

impl GroupBy {
    pub fn label(self) -> &'static str {
        match self {
            GroupBy::Confidence => "Confidence",
            GroupBy::FirstLetter => "First letter",
            GroupBy::Artist => "Artist",
            GroupBy::Tag => "Tag",
            GroupBy::Tuning => "Tuning",
            GroupBy::None => "None",
        }
    }

    /// The name it is stored under. Deliberately not [`label`](Self::label):
    /// a label is UI copy and may be reworded, a stored name may not.
    pub fn name(self) -> &'static str {
        match self {
            GroupBy::Confidence => "Confidence",
            GroupBy::FirstLetter => "FirstLetter",
            GroupBy::Artist => "Artist",
            GroupBy::Tag => "Tag",
            GroupBy::Tuning => "Tuning",
            GroupBy::None => "None",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.name() == name)
    }

    pub const ALL: [GroupBy; 6] = [
        GroupBy::Confidence,
        GroupBy::FirstLetter,
        GroupBy::Artist,
        GroupBy::Tag,
        GroupBy::Tuning,
        GroupBy::None,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortField {
    Artist,
    Title,
    Confidence,
    LastPlayed,
    DateAdded,
    Key,
    Tempo,
    Duration,
    Capo,
    Tuning,
}

impl SortField {
    pub fn label(self) -> &'static str {
        match self {
            SortField::Artist => "Artist",
            SortField::Title => "Title",
            SortField::Confidence => "Confidence",
            SortField::LastPlayed => "Last played",
            SortField::DateAdded => "Date added",
            SortField::Key => "Key",
            SortField::Tempo => "Tempo",
            SortField::Duration => "Duration",
            SortField::Capo => "Capo",
            SortField::Tuning => "Tuning",
        }
    }

    /// The human-readable direction shown beside each row in the sort sheet.
    pub fn direction_label(self, dir: SortDir) -> &'static str {
        let asc = dir == SortDir::Asc;
        match self {
            SortField::Artist | SortField::Title | SortField::Key | SortField::Tuning => {
                if asc { "A to Z" } else { "Z to A" }
            }
            SortField::Confidence => {
                if asc {
                    "solid first"
                } else {
                    "shakiest first"
                }
            }
            SortField::LastPlayed | SortField::DateAdded => {
                if asc {
                    "newest first"
                } else {
                    "oldest first"
                }
            }
            SortField::Tempo => {
                if asc { "slow to fast" } else { "fast to slow" }
            }
            SortField::Duration => {
                if asc { "short to long" } else { "long to short" }
            }
            SortField::Capo => {
                if asc { "low to high" } else { "high to low" }
            }
        }
    }

    /// Whether a song has this field filled. Songs missing the active sort
    /// field sort to the bottom of their group rather than disappearing.
    pub fn is_present(self, song: &Song) -> bool {
        match self {
            SortField::Artist | SortField::Title | SortField::DateAdded => true,
            SortField::Confidence => song.confidence.is_some(),
            SortField::LastPlayed => song.last_played.is_some(),
            SortField::Key => song.key.is_some(),
            SortField::Tempo => song.tempo.is_some(),
            SortField::Duration => song.duration.is_some(),
            SortField::Capo => song.capo.is_some(),
            SortField::Tuning => song.tuning.is_some(),
        }
    }

    /// The name it is stored under — see [`GroupBy::name`].
    pub fn name(self) -> &'static str {
        match self {
            SortField::Artist => "Artist",
            SortField::Title => "Title",
            SortField::Confidence => "Confidence",
            SortField::LastPlayed => "LastPlayed",
            SortField::DateAdded => "DateAdded",
            SortField::Key => "Key",
            SortField::Tempo => "Tempo",
            SortField::Duration => "Duration",
            SortField::Capo => "Capo",
            SortField::Tuning => "Tuning",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }

    pub const ALL: [SortField; 10] = [
        SortField::Artist,
        SortField::Title,
        SortField::Confidence,
        SortField::LastPlayed,
        SortField::DateAdded,
        SortField::Key,
        SortField::Tempo,
        SortField::Duration,
        SortField::Capo,
        SortField::Tuning,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn arrow(self) -> TablerIcon {
        match self {
            SortDir::Asc => TablerIcon::ArrowNarrowUp,
            SortDir::Desc => TablerIcon::ArrowNarrowDown,
        }
    }

    pub fn flip(self) -> Self {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SortDir::Asc => "Asc",
            SortDir::Desc => "Desc",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Asc" => Some(SortDir::Asc),
            "Desc" => Some(SortDir::Desc),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Comfortable,
    Compact,
}

impl Density {
    pub fn name(self) -> &'static str {
        match self {
            Density::Comfortable => "Comfortable",
            Density::Compact => "Compact",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Comfortable" => Some(Density::Comfortable),
            "Compact" => Some(Density::Compact),
            _ => None,
        }
    }
}

/// One rendered group: its header label and the songs under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub label: String,
    pub songs: Vec<Song>,
}

/// Search query, grouping, sort, filters, density and collapse state. All of
/// it persisted, in the one Preferences row — see `src/db/prefs.rs`.
#[derive(Clone, Copy)]
pub struct LibraryViewStore {
    /// What is typed into the search field — which since card G1 is the field
    /// on the search screen (`1p`) rather than the one on the library, and
    /// which no longer narrows [`grouped`](Self::grouped).
    ///
    /// **It stayed here, and it stayed persisted, and both halves of that were
    /// decisions.** The obvious home for a query the library no longer reads is
    /// a transient signal on `NavStore` beside `rename_draft` — a question you
    /// asked once, not a preference. What settles it the other way is what
    /// retiring this field would actually cost: it is a column of the one
    /// `Preferences` row (`src/db/prefs.rs`, `src/db/schema.rhype`), and rhypedb
    /// refuses to open a library whose schema has *lost* a field unless the app
    /// passes `OpenOptions::allow_schema_shrink(true)` — which its own error
    /// text calls one-way and irreversible. Measured, not assumed: dropping the
    /// line from `schema.rhype` and reopening a library made once with it fails
    /// with `schema would drop catalog entries (fields=["Preferences.query"])`.
    ///
    /// Turning a permanent destructive switch on in [`crate::db::open`], for
    /// every user's library forever, to retire one string that already has a
    /// perfectly good reader — is not a trade this card is entitled to make. So
    /// the query is still library view state, still restored with the rest of
    /// it, and the only screen that reads or writes it is `1p`. Reopening
    /// search finds the last thing you looked for still in the field, with the
    /// ✕ beside it that clears it.
    pub query: Signal<String>,
    pub group_by: Signal<GroupBy>,
    pub sort_field: Signal<SortField>,
    pub sort_dir: Signal<SortDir>,
    pub density: Signal<Density>,
    /// Group labels the user has collapsed.
    pub collapsed: Signal<Vec<String>>,
    /// Groups the user has expanded past the truncation limit.
    pub expanded: Signal<Vec<String>>,
    storage: Storage,
}

impl LibraryViewStore {
    /// The defaults, remembering nothing.
    pub fn new() -> Self {
        Self::restored(Storage::in_memory())
    }

    /// The view as it was left. Every default here also lives in
    /// [`Preferences::default`](crate::db::prefs::Preferences), which is where a
    /// first run gets them — Confidence grouping, not A–Z.
    pub fn restored(storage: Storage) -> Self {
        let preferences = storage.preferences();
        Self {
            query: Signal::new(preferences.query),
            group_by: Signal::new(preferences.group_by),
            sort_field: Signal::new(preferences.sort_field),
            sort_dir: Signal::new(preferences.sort_dir),
            density: Signal::new(preferences.density),
            collapsed: Signal::new(preferences.collapsed),
            expanded: Signal::new(preferences.expanded),
            storage,
        }
    }

    /// The query is remembered like everything else, but it changes on every
    /// keystroke — so it is set through here rather than written to directly.
    pub fn set_query(self, query: impl Into<String>) {
        let query = query.into();
        self.query.set(query.clone());
        self.storage.remember(|p| p.query = query);
    }

    pub fn is_collapsed(self, label: &str) -> bool {
        self.collapsed.get().iter().any(|l| l == label)
    }

    pub fn toggle_collapsed(self, label: String) {
        self.collapsed.update(|list| match list.iter().position(|l| *l == label) {
            Some(i) => {
                list.remove(i);
            }
            None => list.push(label),
        });
        let collapsed = self.collapsed.get();
        self.storage.remember(|p| p.collapsed = collapsed);
    }

    pub fn is_expanded(self, label: &str) -> bool {
        self.expanded.get().iter().any(|l| l == label)
    }

    pub fn expand(self, label: String) {
        self.expanded.update(|list| {
            if !list.contains(&label) {
                list.push(label);
            }
        });
        let expanded = self.expanded.get();
        self.storage.remember(|p| p.expanded = expanded);
    }

    /// Tapping the active sort row reverses it; tapping another selects it.
    pub fn choose_sort(self, field: SortField) {
        if self.sort_field.get() == field {
            self.sort_dir.set(self.sort_dir.get().flip());
        } else {
            self.sort_field.set(field);
            self.sort_dir.set(SortDir::Asc);
        }
        let (field, dir) = (self.sort_field.get(), self.sort_dir.get());
        self.storage.remember(|p| {
            p.sort_field = field;
            p.sort_dir = dir;
        });
    }

    pub fn choose_group(self, group_by: GroupBy) {
        self.group_by.set(group_by);
        self.storage.remember(|p| p.group_by = group_by);
    }

    pub fn toggle_density(self) {
        let density = match self.density.get() {
            Density::Comfortable => Density::Compact,
            Density::Compact => Density::Comfortable,
        };
        self.density.set(density);
        self.storage.remember(|p| p.density = density);
    }

    /// The library list, grouped and sorted. Derived on read — the rules live
    /// in [`crate::derive`] as plain functions so they can be tested without a
    /// window.
    ///
    /// [`query`](Self::query) is deliberately not one of the inputs any more.
    /// Card G1 moved the typing to the search screen (`1p`) and left the
    /// library showing the whole book; `crate::derive::grouped`'s own header
    /// carries the argument for why a library that stays filtered behind a
    /// field that no longer types is the worse of the two.
    pub fn grouped(self, all: Vec<Song>) -> Vec<Group> {
        crate::derive::grouped(
            all,
            self.group_by.get(),
            self.sort_field.get(),
            self.sort_dir.get(),
        )
    }
}

impl Default for LibraryViewStore {
    fn default() -> Self {
        Self::new()
    }
}
