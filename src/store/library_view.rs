use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::model::{Confidence, Song};
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

/// The library's filters — card G3, and no wireframe drew this: the handoff's
/// `Filter` chip (`design_handoff_setlistarray/README.md:144`) is
/// `background: fill, color: muted` and nothing more, never wired to a sheet.
/// See `src/screens/filter_sheet.rs` for the sheet this drives and the
/// decisions it made in the wireframe's absence.
///
/// Four facets — Confidence, Tag, Tuning, has-a-chart — each a set of values
/// to accept. **A song passes a facet with nothing picked in it**, which is
/// what lets all four start wide open with no "any" chip to tap first, and
/// [`matches`](Self::matches) is the one place that rule is written down.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    /// `None` in this list means the `Unrated` chip — `confidence: None` on
    /// the song, not "no confidence chip selected". That second meaning is
    /// [`is_active`](Self::is_active): an *empty* `Vec` here, not a `Vec`
    /// containing `None`.
    pub confidences: Vec<Option<Confidence>>,
    pub tags: Vec<String>,
    pub tunings: Vec<String>,
    pub has_chart: bool,
}

impl Filters {
    /// How many chips are lit, across every facet — what the `Filter` chip's
    /// `· N` counts and what [`is_active`](Self::is_active) is asking about.
    pub fn count(&self) -> usize {
        self.confidences.len() + self.tags.len() + self.tunings.len() + usize::from(self.has_chart)
    }

    /// Whether any facet constrains anything at all — the difference between
    /// the muted `Filter` chip and its active, counted treatment.
    pub fn is_active(&self) -> bool {
        self.count() > 0
    }

    pub fn is_confidence_selected(&self, confidence: Option<Confidence>) -> bool {
        self.confidences.contains(&confidence)
    }

    pub fn toggle_confidence(&mut self, confidence: Option<Confidence>) {
        match self.confidences.iter().position(|c| *c == confidence) {
            Some(i) => {
                self.confidences.remove(i);
            }
            None => self.confidences.push(confidence),
        }
    }

    /// Tags are compared case-insensitively, the same rule [`parse_tags`]
    /// dedupes by and [`crate::derive::library_tags`] lists by — otherwise a
    /// chip for `Campfire` could sit selected beside a song's own `campfire`
    /// and the two would never agree that they name the same tag.
    ///
    /// [`parse_tags`]: crate::derive::parse_tags
    pub fn is_tag_selected(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }

    pub fn toggle_tag(&mut self, tag: String) {
        match self.tags.iter().position(|t| t.eq_ignore_ascii_case(&tag)) {
            Some(i) => {
                self.tags.remove(i);
            }
            None => self.tags.push(tag),
        }
    }

    pub fn is_tuning_selected(&self, tuning: &str) -> bool {
        self.tunings.iter().any(|t| t.eq_ignore_ascii_case(tuning))
    }

    pub fn toggle_tuning(&mut self, tuning: String) {
        match self.tunings.iter().position(|t| t.eq_ignore_ascii_case(&tuning)) {
            Some(i) => {
                self.tunings.remove(i);
            }
            None => self.tunings.push(tuning),
        }
    }

    pub fn toggle_has_chart(&mut self) {
        self.has_chart = !self.has_chart;
    }

    /// Whether `song` survives every facet that has a selection — OR within a
    /// facet, AND across them. `crate::derive::grouped` calls this before
    /// bucketing, so a group header counts what it actually contains rather
    /// than the unfiltered book underneath it.
    ///
    /// **A stale tag or tuning is read exactly as this function reads
    /// anything else the book no longer has: nothing carries it, so nothing
    /// passes that facet.** The alternative was pruning the value out of the
    /// stored filter the moment its last song is deleted, the way card J5's
    /// `SetlistsStore::forget_song` prunes a dead id out of `song_ids` the
    /// instant `SongsStore::delete` runs. That pattern only works there
    /// because `SongsStore` already holds the `SetlistsStore` it reaches
    /// into; doing the same here would mean handing `SongsStore` a
    /// `LibraryViewStore` too, a second cross-store dependency, to keep one
    /// filter facet tidy. `matches` alone already gives the same *visible*
    /// answer — a facet nothing in the book satisfies excludes everything,
    /// same as it would for a tag never typed — and Clear is one tap away.
    /// So the stored selection lingers, unreachable through the sheet once
    /// its chip stops being offered, until Clear or a fresh pick replaces it.
    pub fn matches(&self, song: &Song) -> bool {
        let confidence_ok =
            self.confidences.is_empty() || self.confidences.contains(&song.confidence);
        let tag_ok = self.tags.is_empty()
            || self
                .tags
                .iter()
                .any(|t| song.tags.iter().any(|s| s.eq_ignore_ascii_case(t)));
        let tuning_ok = self.tunings.is_empty()
            || song
                .tuning
                .as_deref()
                .is_some_and(|t| self.tunings.iter().any(|f| f.eq_ignore_ascii_case(t)));
        let chart_ok = !self.has_chart || song.has_chart();
        confidence_ok && tag_ok && tuning_ok && chart_ok
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
    /// The library's active filters (card G3) — see [`Filters`].
    pub filters: Signal<Filters>,
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
            filters: Signal::new(preferences.filters),
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

    /// Every filter mutator below follows [`toggle_collapsed`](Self::toggle_collapsed)'s
    /// own shape: mutate the signal, then write the whole `Filters` back to
    /// the one `Preferences` row it lives in.
    pub fn toggle_confidence(self, confidence: Option<Confidence>) {
        self.filters.update(|f| f.toggle_confidence(confidence));
        let filters = self.filters.get();
        self.storage.remember(|p| p.filters = filters);
    }

    pub fn toggle_tag_filter(self, tag: String) {
        self.filters.update(|f| f.toggle_tag(tag));
        let filters = self.filters.get();
        self.storage.remember(|p| p.filters = filters);
    }

    pub fn toggle_tuning_filter(self, tuning: String) {
        self.filters.update(|f| f.toggle_tuning(tuning));
        let filters = self.filters.get();
        self.storage.remember(|p| p.filters = filters);
    }

    pub fn toggle_has_chart_filter(self) {
        self.filters.update(|f| f.toggle_has_chart());
        let filters = self.filters.get();
        self.storage.remember(|p| p.filters = filters);
    }

    /// The filter sheet's footer — empties every facet in one write.
    pub fn clear_filters(self) {
        self.filters.set(Filters::default());
        self.storage.remember(|p| p.filters = Filters::default());
    }

    pub fn toggle_density(self) {
        let density = match self.density.get() {
            Density::Comfortable => Density::Compact,
            Density::Compact => Density::Comfortable,
        };
        self.density.set(density);
        self.storage.remember(|p| p.density = density);
    }

    /// The library list, filtered, grouped and sorted. Derived on read — the
    /// rules live in [`crate::derive`] as plain functions so they can be
    /// tested without a window.
    ///
    /// [`query`](Self::query) is deliberately not one of the inputs any more.
    /// Card G1 moved the typing to the search screen (`1p`) and left the
    /// library showing the whole book; `crate::derive::grouped`'s own header
    /// carries the argument for why a library that stays filtered behind a
    /// field that no longer types is the worse of the two.
    ///
    /// [`filters`](Self::filters) *is* one of the inputs, and deliberately the
    /// first thing `crate::derive::grouped` does with the book — card G3's
    /// Filter chip narrows what is actually in your book, which `query` no
    /// longer does, so the two are not the contradiction they look like side
    /// by side.
    pub fn grouped(self, all: Vec<Song>) -> Vec<Group> {
        crate::derive::grouped(
            all,
            self.group_by.get(),
            self.sort_field.get(),
            self.sort_dir.get(),
            &self.filters.get(),
        )
    }
}

impl Default for LibraryViewStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: u32) -> Song {
        Song::new(id, "Title", "Artist")
    }

    #[test]
    fn nothing_selected_matches_every_song() {
        let filters = Filters::default();
        assert!(filters.matches(&song(1)));
        let mut solid = song(2);
        solid.confidence = Some(Confidence::Solid);
        assert!(filters.matches(&solid));
    }

    #[test]
    fn confidence_is_or_within_the_facet() {
        let filters = Filters {
            confidences: vec![Some(Confidence::Solid), Some(Confidence::Rusty)],
            ..Default::default()
        };
        let mut solid = song(1);
        solid.confidence = Some(Confidence::Solid);
        let mut rusty = song(2);
        rusty.confidence = Some(Confidence::Rusty);
        let mut learning = song(3);
        learning.confidence = Some(Confidence::Learning);

        assert!(filters.matches(&solid));
        assert!(filters.matches(&rusty));
        assert!(!filters.matches(&learning));
    }

    #[test]
    fn unrated_matches_a_song_with_no_confidence_set() {
        let filters = Filters {
            confidences: vec![None],
            ..Default::default()
        };
        assert!(filters.matches(&song(1)));
        let mut rated = song(2);
        rated.confidence = Some(Confidence::Solid);
        assert!(!filters.matches(&rated));
    }

    #[test]
    fn confidence_and_tag_are_anded_across_facets() {
        let filters = Filters {
            confidences: vec![Some(Confidence::Solid), Some(Confidence::Rusty)],
            tags: vec!["campfire".into()],
            ..Default::default()
        };
        // Solid, but not tagged — fails the tag facet even though it clears
        // the confidence one.
        let mut solid_untagged = song(1);
        solid_untagged.confidence = Some(Confidence::Solid);
        assert!(!filters.matches(&solid_untagged));

        // Rusty and tagged campfire — clears both.
        let mut rusty_tagged = song(2);
        rusty_tagged.confidence = Some(Confidence::Rusty);
        rusty_tagged.tags = vec!["Campfire".into()];
        assert!(filters.matches(&rusty_tagged));

        // Learning and tagged — clears the tag facet, fails confidence.
        let mut learning_tagged = song(3);
        learning_tagged.confidence = Some(Confidence::Learning);
        learning_tagged.tags = vec!["campfire".into()];
        assert!(!filters.matches(&learning_tagged));
    }

    #[test]
    fn tag_matching_is_case_insensitive() {
        let filters = Filters {
            tags: vec!["Campfire".into()],
            ..Default::default()
        };
        let mut song = song(1);
        song.tags = vec!["campfire".into()];
        assert!(filters.matches(&song));
    }

    #[test]
    fn tuning_matching_is_case_insensitive() {
        let filters = Filters {
            tunings: vec!["drop d".into()],
            ..Default::default()
        };
        let mut song = song(1);
        song.tuning = Some("Drop D".into());
        assert!(filters.matches(&song));
    }

    #[test]
    fn has_chart_excludes_a_song_with_no_attachments() {
        let filters = Filters {
            has_chart: true,
            ..Default::default()
        };
        assert!(!filters.matches(&song(1)));

        let mut charted = song(2);
        charted.attachments.push(7);
        assert!(filters.matches(&charted));
    }

    #[test]
    fn toggling_a_confidence_twice_leaves_it_unselected() {
        let mut filters = Filters::default();
        filters.toggle_confidence(Some(Confidence::Solid));
        assert!(filters.is_confidence_selected(Some(Confidence::Solid)));
        filters.toggle_confidence(Some(Confidence::Solid));
        assert!(!filters.is_confidence_selected(Some(Confidence::Solid)));
        assert!(filters.confidences.is_empty());
    }

    #[test]
    fn an_untouched_filter_set_is_not_active() {
        assert!(!Filters::default().is_active());
        assert_eq!(Filters::default().count(), 0);
    }

    #[test]
    fn count_adds_across_every_facet() {
        let filters = Filters {
            confidences: vec![Some(Confidence::Solid), None],
            tags: vec!["campfire".into()],
            tunings: vec!["drop d".into(), "standard".into()],
            has_chart: true,
        };
        assert_eq!(filters.count(), 6);
        assert!(filters.is_active());
    }
}
