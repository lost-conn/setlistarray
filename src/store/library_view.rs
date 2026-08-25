use rinch::prelude::*;

use crate::model::{Confidence, Song};

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

/// How many rows a group shows before "Show N more".
pub const GROUP_PREVIEW: usize = 6;

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

    /// The library list, grouped and sorted. Derived on read — nothing here
    /// is stored.
    pub fn grouped(self, all: Vec<Song>) -> Vec<Group> {
        let query = self.query.get().trim().to_lowercase();
        let matching: Vec<Song> = all
            .into_iter()
            .filter(|s| {
                query.is_empty()
                    || s.title.to_lowercase().contains(&query)
                    || s.artist.to_lowercase().contains(&query)
                    || s.tags.iter().any(|t| t.to_lowercase().contains(&query))
            })
            .collect();

        let mut groups = match self.group_by.get() {
            GroupBy::Confidence => bucket_by_confidence(matching),
            GroupBy::FirstLetter => bucket_by(matching, |s| {
                s.title
                    .chars()
                    .next()
                    .map(|c| c.to_ascii_uppercase().to_string())
                    .unwrap_or_else(|| "#".into())
            }),
            GroupBy::Artist => bucket_by(matching, |s| s.artist.clone()),
            GroupBy::Tuning => bucket_by(matching, |s| {
                s.tuning.clone().unwrap_or_else(|| "No tuning set".into())
            }),
            GroupBy::Tag => bucket_by(matching, |s| {
                s.tags.first().cloned().unwrap_or_else(|| "Untagged".into())
            }),
            GroupBy::None => vec![Group {
                label: "All songs".into(),
                songs: matching,
            }],
        };

        let field = self.sort_field.get();
        let dir = self.sort_dir.get();
        for group in &mut groups {
            sort_songs(&mut group.songs, field, dir);
        }
        groups
    }
}

impl Default for LibraryViewStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Groups in the fixed order Solid · Rusty · Learning · Unrated. Empty groups
/// are dropped.
fn bucket_by_confidence(songs: Vec<Song>) -> Vec<Group> {
    let order = [
        (Some(Confidence::Solid), "Solid"),
        (Some(Confidence::Rusty), "Rusty"),
        (Some(Confidence::Learning), "Learning"),
        (None, "Unrated"),
    ];
    order
        .into_iter()
        .filter_map(|(confidence, label)| {
            let songs: Vec<Song> = songs
                .iter()
                .filter(|s| s.confidence == confidence)
                .cloned()
                .collect();
            (!songs.is_empty()).then(|| Group {
                label: label.into(),
                songs,
            })
        })
        .collect()
}

fn bucket_by(songs: Vec<Song>, key: impl Fn(&Song) -> String) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for song in songs {
        let label = key(&song);
        match groups.iter_mut().find(|g| g.label == label) {
            Some(group) => group.songs.push(song),
            None => groups.push(Group {
                label,
                songs: vec![song],
            }),
        }
    }
    groups.sort_by(|a, b| a.label.cmp(&b.label));
    groups
}

fn sort_songs(songs: &mut [Song], field: SortField, dir: SortDir) {
    songs.sort_by(|a, b| {
        // Missing-field songs go last regardless of direction.
        match (field.is_present(a), field.is_present(b)) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        let ordering = match field {
            SortField::Artist => a.artist.to_lowercase().cmp(&b.artist.to_lowercase()),
            SortField::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            SortField::Confidence => confidence_rank(a).cmp(&confidence_rank(b)),
            // "newest first" is the ascending reading of a date field.
            SortField::LastPlayed => b.last_played.cmp(&a.last_played),
            SortField::DateAdded => b.created_at.cmp(&a.created_at),
            SortField::Key => a.key.cmp(&b.key),
            SortField::Tempo => a.tempo.cmp(&b.tempo),
            SortField::Duration => a.duration.cmp(&b.duration),
            SortField::Capo => a.capo.cmp(&b.capo),
            SortField::Tuning => a.tuning.cmp(&b.tuning),
        };
        match dir {
            SortDir::Asc => ordering,
            SortDir::Desc => ordering.reverse(),
        }
    });
}

fn confidence_rank(song: &Song) -> u8 {
    match song.confidence {
        Some(Confidence::Solid) => 0,
        Some(Confidence::Rusty) => 1,
        Some(Confidence::Learning) => 2,
        None => 3,
    }
}
