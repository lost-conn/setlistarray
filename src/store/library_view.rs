use rinch::prelude::*;

use crate::model::Song;

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
                if asc { "A → Z" } else { "Z → A" }
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
                if asc { "slow → fast" } else { "fast → slow" }
            }
            SortField::Duration => {
                if asc { "short → long" } else { "long → short" }
            }
            SortField::Capo => {
                if asc { "low → high" } else { "high → low" }
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
    pub fn arrow(self) -> &'static str {
        match self {
            SortDir::Asc => "↑",
            SortDir::Desc => "↓",
        }
    }

    pub fn flip(self) -> Self {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Comfortable,
    Compact,
}

/// One rendered group: its header label and the songs under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub label: String,
    pub songs: Vec<Song>,
}

/// Search query, grouping, sort, filters, density and collapse state. All of
/// it persisted; each tab keeps its own.
#[derive(Clone, Copy)]
pub struct LibraryViewStore {
    pub query: Signal<String>,
    pub group_by: Signal<GroupBy>,
    pub sort_field: Signal<SortField>,
    pub sort_dir: Signal<SortDir>,
    pub density: Signal<Density>,
    /// Group labels the user has collapsed.
    pub collapsed: Signal<Vec<String>>,
    /// Groups the user has expanded past the truncation limit.
    pub expanded: Signal<Vec<String>>,
}

impl LibraryViewStore {
    pub fn new() -> Self {
        Self {
            query: Signal::new(String::new()),
            // Confidence grouping is the default library view, not A–Z.
            group_by: Signal::new(GroupBy::Confidence),
            sort_field: Signal::new(SortField::Artist),
            sort_dir: Signal::new(SortDir::Asc),
            density: Signal::new(Density::Comfortable),
            collapsed: Signal::new(Vec::new()),
            expanded: Signal::new(Vec::new()),
        }
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
    }

    /// Tapping the active sort row reverses it; tapping another selects it.
    pub fn choose_sort(self, field: SortField) {
        if self.sort_field.get() == field {
            self.sort_dir.set(self.sort_dir.get().flip());
        } else {
            self.sort_field.set(field);
            self.sort_dir.set(SortDir::Asc);
        }
    }

    pub fn toggle_density(self) {
        self.density.set(match self.density.get() {
            Density::Comfortable => Density::Compact,
            Density::Compact => Density::Comfortable,
        });
    }

    /// The library list, grouped and sorted. Derived on read — the rules live
    /// in [`crate::derive`] as plain functions so they can be tested without a
    /// window.
    pub fn grouped(self, all: Vec<Song>) -> Vec<Group> {
        crate::derive::grouped(
            all,
            &self.query.get(),
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
