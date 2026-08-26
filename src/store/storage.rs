//! The seam between fallible writes and the infallible calls the screens make.
//!
//! A screen calls `songs.delete(id)` and gets nothing back. Underneath, that
//! delete can fail — a full disk, a directory that went read-only, a database
//! another process has open. Two things are not acceptable: swallowing it, and
//! panicking. So:
//!
//! * **The write happens first; memory follows only if it landed.** The
//!   alternative — change the signal, then try the disk — leaves the screen
//!   showing a song that does not exist, and no amount of error reporting makes
//!   that honest. This way a failed delete simply does not happen: the row is
//!   still there, which is the truth.
//! * **The fault is recorded, every time.** [`Storage::fault`] holds the most
//!   recent one and is a signal like any other, so the error surface card J4
//!   builds is a read away. Until that screen exists it also goes to stderr,
//!   which is where a developer running the binary will see it.
//!
//! `Storage` is `Copy` like every other store handle: the repository sits
//! behind a signal, not behind a field, so nothing here needs cloning before a
//! closure.

use std::sync::Arc;

use rinch::prelude::*;

use crate::db::prefs::Preferences;
use crate::db::repo::{Loaded, Repo};
use crate::db::{DataDir, DbResult};

/// A write that did not reach the disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    /// What the app was doing, in words a person could be shown.
    pub doing: &'static str,
    pub message: String,
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.doing, self.message)
    }
}

#[derive(Clone, Copy)]
pub struct Storage {
    repo: Signal<Option<Arc<Repo>>>,
    fault: Signal<Option<Fault>>,
    /// The one Preferences row, mirrored. View state and settings live in two
    /// stores but in a single row, so neither store can write its own slice
    /// without knowing the other's — this is where they meet.
    preferences: Signal<Preferences>,
}

impl Storage {
    /// Open the library. A library that cannot be opened is not a reason to
    /// refuse to start: the app runs on memory alone, the fault is recorded,
    /// and nothing is written where it might land on a half-open database.
    pub fn open(dir: &DataDir) -> Self {
        let storage = Self {
            repo: Signal::new(None),
            fault: Signal::new(None),
            preferences: Signal::new(Preferences::default()),
        };
        match Repo::open(dir) {
            Ok(repo) => storage.repo.set(Some(Arc::new(repo))),
            Err(e) => storage.record("opening the library", e.to_string()),
        }
        storage
    }

    /// No database at all — the shape tests and `--seed` screenshots use, and
    /// what [`open`](Self::open) degrades to.
    pub fn in_memory() -> Self {
        Self {
            repo: Signal::new(None),
            fault: Signal::new(None),
            preferences: Signal::new(Preferences::default()),
        }
    }

    pub fn repo(self) -> Option<Arc<Repo>> {
        self.repo.get()
    }

    pub fn is_persistent(self) -> bool {
        self.repo.get().is_some()
    }

    /// The most recent write that did not land, or `None` if every write so far
    /// has. A signal, so a screen can render it the moment one exists.
    pub fn fault(self) -> Signal<Option<Fault>> {
        self.fault
    }

    pub fn clear_fault(self) {
        self.fault.set(None);
    }

    /// Let go of the library, releasing the single-writer lock on its
    /// directory.
    ///
    /// The app never needs this — the process ending does it — but a signal
    /// outlives the handle that made it, so anything that opens two libraries
    /// in one thread (a test) has to say when it is done with the first.
    pub fn close(self) {
        self.repo.set(None);
    }

    /// Everything the stores start from. A failed read is recorded and comes
    /// back empty rather than taking the app down — an unreadable library still
    /// shows its empty state, and the fault says why.
    pub fn load(self) -> Loaded {
        let Some(repo) = self.repo.get() else {
            return Loaded::default();
        };
        let loaded = match repo.load() {
            Ok(loaded) => loaded,
            Err(e) => {
                self.record("reading the library", e.to_string());
                Loaded::default()
            }
        };
        if let Some(preferences) = &loaded.preferences {
            self.preferences.set(preferences.clone());
        }
        loaded
    }

    /// Preferences as they stand. A first run has never written a row, so this
    /// is the defaults until something changes.
    pub fn preferences(self) -> Preferences {
        self.preferences.get()
    }

    /// Change one preference and write the row. Both `LibraryViewStore` and
    /// `SettingsStore` come through here so the row is always whole.
    pub fn remember(self, f: impl FnOnce(&mut Preferences)) {
        self.preferences.update(f);
        let preferences = self.preferences.get();
        self.save_preferences(&preferences);
    }

    /// A write that changes state that already exists.
    ///
    /// `true` means memory may follow — either the write landed, or there is no
    /// database for it to land in. `false` means it did not, and the caller
    /// must leave the signal alone.
    #[must_use]
    pub fn write(self, doing: &'static str, f: impl FnOnce(&Repo) -> DbResult<()>) -> bool {
        let Some(repo) = self.repo.get() else {
            return true;
        };
        match f(&repo) {
            Ok(()) => true,
            Err(e) => {
                self.record(doing, e.to_string());
                false
            }
        }
    }

    /// A write that mints an id.
    ///
    /// Object ids *are* the domain ids, so with a database the database
    /// numbers them. `without_database` is the fallback counter for the
    /// degraded case. `None` means nothing was created.
    #[must_use]
    pub fn create<Id: TryFrom<u64>>(
        self,
        doing: &'static str,
        without_database: impl FnOnce() -> Id,
        f: impl FnOnce(&Repo) -> DbResult<u64>,
    ) -> Option<Id> {
        let Some(repo) = self.repo.get() else {
            return Some(without_database());
        };
        match f(&repo) {
            Ok(id) => match Id::try_from(id) {
                Ok(id) => Some(id),
                Err(_) => {
                    // 4 billion songs is not a case worth a panic, but it is
                    // worth saying out loud rather than wrapping around.
                    self.record(doing, format!("the library ran out of ids at {id}"));
                    None
                }
            },
            Err(e) => {
                self.record(doing, e.to_string());
                None
            }
        }
    }

    /// View state and settings, written on every change. Failures are recorded
    /// like any other, but a lost preference never blocks the change itself —
    /// the toggle the user just pressed still moves.
    fn save_preferences(self, preferences: &Preferences) {
        let Some(repo) = self.repo.get() else { return };
        if let Err(e) = repo.save_preferences(preferences) {
            self.record("remembering a setting", e.to_string());
        }
    }

    fn record(self, doing: &'static str, message: String) {
        let fault = Fault { doing, message };
        eprintln!("setlistarray: {fault}");
        self.fault.set(Some(fault));
    }
}

impl Default for Storage {
    fn default() -> Self {
        Self::in_memory()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{restart, scratch};
    use crate::model::{Attachment, AttachmentKind, Confidence, Day, Song};
    use crate::store::{
        AttachmentsStore, Density, GroupBy, LibraryViewStore, SettingsStore, SetlistsStore,
        SongsStore, SortDir, SortField,
    };

    /// One launch: open the library, read it, hand the stores what was there.
    /// The same three lines `app()` runs, minus the window.
    struct Session {
        storage: Storage,
        songs: SongsStore,
        setlists: SetlistsStore,
        attachments: AttachmentsStore,
        view: LibraryViewStore,
        settings: SettingsStore,
    }

    impl Drop for Session {
        /// A launch ends when the process does. Here it ends when the block
        /// does, and the library has to be let go of either way.
        fn drop(&mut self) {
            self.storage.close();
        }
    }

    impl Session {
        fn open(dir: &DataDir) -> Self {
            let storage = restart(|| {
                let storage = Storage::open(dir);
                if storage.is_persistent() {
                    Some(storage)
                } else {
                    storage.close();
                    None
                }
            });
            let loaded = storage.load();
            Self {
                storage,
                songs: SongsStore::restored(storage, loaded.songs),
                setlists: SetlistsStore::restored(storage, loaded.setlists),
                attachments: AttachmentsStore::restored(storage, loaded.attachments),
                view: LibraryViewStore::restored(storage),
                settings: SettingsStore::restored(storage),
            }
        }
    }

    fn chart(title: &str) -> Attachment {
        Attachment {
            id: 0,
            kind: AttachmentKind::Pdf,
            title: title.into(),
            bytes_on_disk: 412_000,
            page_count: Some(2),
            source_url: None,
            captured_at: None,
            body: None,
        }
    }

    // ── B2: songs and setlists survive a restart ────────────────────────────

    #[test]
    fn a_song_added_and_edited_is_still_there_next_launch() {
        let dir = scratch("store-song-restart");
        {
            let s = Session::open(&dir);
            let id = s.songs.add("Carolina", "M. Ward");
            s.songs.edit(id, |song| {
                song.key = Some("G".into());
                song.tempo = Some(96);
            });
            s.songs.set_confidence(id, Some(Confidence::Solid));
            s.songs.mark_played(id, Day::new(2026, 6, 2));
        }

        let s = Session::open(&dir);
        let songs = s.songs.songs.get();
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].title, "Carolina");
        assert_eq!(songs[0].key.as_deref(), Some("G"));
        assert_eq!(songs[0].tempo, Some(96));
        assert_eq!(songs[0].confidence, Some(Confidence::Solid));
        assert_eq!(songs[0].last_played, Some(Day::new(2026, 6, 2)));
    }

    #[test]
    fn a_deleted_song_stays_deleted() {
        let dir = scratch("store-song-delete");
        {
            let s = Session::open(&dir);
            let kept = s.songs.add("Ripple", "Grateful Dead");
            let gone = s.songs.add("Carolina", "M. Ward");
            s.songs.delete(gone);
            assert_eq!(s.songs.count(), 1);
            assert!(s.songs.get(kept).is_some());
        }

        let s = Session::open(&dir);
        assert_eq!(s.songs.count(), 1);
        assert_eq!(s.songs.songs.get()[0].title, "Ripple");
    }

    #[test]
    fn a_setlist_comes_back_in_the_order_it_was_left() {
        let dir = scratch("store-setlist-restart");
        let expected = {
            let s = Session::open(&dir);
            let ids: Vec<_> = ["Carolina", "Blackbird", "Ripple", "Landslide"]
                .iter()
                .map(|t| s.songs.add(*t, "someone"))
                .collect();
            let set = s.setlists.add("Porch, Saturday");
            for id in &ids {
                s.setlists.add_song(set, *id);
            }
            s.setlists.remove_song(set, ids[1]);
            // Carolina, Ripple, Landslide → Landslide, Carolina, Ripple.
            s.setlists.reorder(set, 2, 0);
            s.setlists.get(set).unwrap().song_ids
        };
        assert_eq!(expected.len(), 3);

        let s = Session::open(&dir);
        let setlists = s.setlists.setlists.get();
        assert_eq!(setlists.len(), 1);
        assert_eq!(setlists[0].name, "Porch, Saturday");
        assert_eq!(setlists[0].song_ids, expected);
        assert_eq!(s.songs.count(), 4, "removing from a set kept every song");
    }

    #[test]
    fn a_deleted_setlist_does_not_take_its_songs_with_it() {
        let dir = scratch("store-setlist-delete");
        {
            let s = Session::open(&dir);
            let song = s.songs.add("Carolina", "M. Ward");
            let set = s.setlists.add("Porch, Saturday");
            s.setlists.add_song(set, song);
            s.setlists.delete(set);
        }

        let s = Session::open(&dir);
        assert!(s.setlists.setlists.get().is_empty());
        assert_eq!(s.songs.count(), 1);
    }

    #[test]
    fn deleting_a_song_leaves_the_sets_it_was_in_without_it() {
        let dir = scratch("store-cascade");
        {
            let s = Session::open(&dir);
            let kept = s.songs.add("Ripple", "Grateful Dead");
            let gone = s.songs.add("Carolina", "M. Ward");
            let set = s.setlists.add("Porch, Saturday");
            s.setlists.add_song(set, kept);
            s.setlists.add_song(set, gone);
            s.songs.delete(gone);
        }

        let s = Session::open(&dir);
        assert_eq!(s.songs.count(), 1);
        assert_eq!(s.setlists.setlists.get()[0].song_ids.len(), 1);
    }

    // ── B3: view state and settings ─────────────────────────────────────────

    #[test]
    fn grouping_sort_direction_and_collapsed_groups_come_back_exactly_as_left() {
        let dir = scratch("store-view-restart");
        {
            let s = Session::open(&dir);
            s.view.choose_group(GroupBy::Tuning);
            s.view.choose_sort(SortField::LastPlayed);
            s.view.choose_sort(SortField::LastPlayed); // tap again to reverse
            s.view.toggle_density();
            s.view.toggle_collapsed("Learning".into());
            s.view.toggle_collapsed("Open D".into());
            s.view.expand("Solid".into());
            s.view.set_query("wheel");
        }

        let s = Session::open(&dir);
        assert_eq!(s.view.group_by.get(), GroupBy::Tuning);
        assert_eq!(s.view.sort_field.get(), SortField::LastPlayed);
        assert_eq!(s.view.sort_dir.get(), SortDir::Desc);
        assert_eq!(s.view.density.get(), Density::Compact);
        assert_eq!(s.view.collapsed.get(), vec!["Learning", "Open D"]);
        assert_eq!(s.view.expanded.get(), vec!["Solid"]);
        assert_eq!(s.view.query.get(), "wheel");
        assert!(s.view.is_collapsed("Open D"));
    }

    #[test]
    fn a_group_uncollapsed_before_closing_is_not_collapsed_on_opening() {
        let dir = scratch("store-view-uncollapse");
        {
            let s = Session::open(&dir);
            s.view.toggle_collapsed("Learning".into());
            s.view.toggle_collapsed("Learning".into());
        }

        let s = Session::open(&dir);
        assert!(s.view.collapsed.get().is_empty());
    }

    #[test]
    fn settings_come_back_as_left() {
        let dir = scratch("store-settings-restart");
        {
            let s = Session::open(&dir);
            s.settings.toggle_dark();
            s.settings.set_accent(crate::store::AccentChoice::Named(2));
            s.settings
                .set_performance_theme(crate::store::PerformanceTheme::AlwaysDark);
            s.settings.set_keep_awake(false);
            s.settings.set_recheck_saved_pages(true);
        }

        let s = Session::open(&dir);
        assert!(s.settings.dark_mode.get());
        assert_eq!(s.settings.accent.get(), crate::store::AccentChoice::Named(2));
        assert_eq!(
            s.settings.performance_theme.get(),
            crate::store::PerformanceTheme::AlwaysDark
        );
        assert!(!s.settings.keep_awake.get());
        assert!(s.settings.recheck_saved_pages.get());
    }

    #[test]
    fn a_first_run_gets_the_defaults_not_a_blank_view() {
        let dir = scratch("store-first-run");
        let s = Session::open(&dir);
        assert_eq!(s.view.group_by.get(), GroupBy::Confidence);
        assert_eq!(s.view.sort_field.get(), SortField::Artist);
        assert_eq!(s.view.density.get(), Density::Comfortable);
        assert!(s.settings.keep_awake.get());
        assert!(s.songs.songs.get().is_empty(), "and an empty library");
        assert_eq!(s.storage.fault().get(), None);
    }

    // ── B4: the attachments directory ───────────────────────────────────────

    #[test]
    fn attaching_a_chart_makes_its_directory_and_removing_it_takes_it_away() {
        let dir = scratch("store-attachment-dir");
        let s = Session::open(&dir);
        let song = s.songs.add("Carolina", "M. Ward");

        let id = s.attachments.add(song, chart("carolina-chords.pdf")).unwrap();
        let path = s.attachments.directory(id).unwrap();
        assert_eq!(path, dir.attachment(id as u64));
        assert!(path.is_dir());
        assert_eq!(s.attachments.total_bytes(), 412_000);

        s.attachments.delete(id);
        assert!(!path.exists());
        assert!(s.attachments.get(id).is_none());
    }

    #[test]
    fn an_attachment_and_its_link_to_a_song_survive_a_restart() {
        let dir = scratch("store-attachment-restart");
        let id = {
            let s = Session::open(&dir);
            let song = s.songs.add("Carolina", "M. Ward");
            let id = s.attachments.add(song, chart("carolina-chords.pdf")).unwrap();
            s.songs.edit(song, |song| song.primary_attachment = Some(id));
            id
        };

        let s = Session::open(&dir);
        let song = &s.songs.songs.get()[0];
        assert!(song.has_chart());
        assert_eq!(song.attachments, vec![id]);
        assert_eq!(song.primary_attachment, Some(id));
        assert_eq!(s.attachments.get(id).unwrap().title, "carolina-chords.pdf");
    }

    #[test]
    fn a_list_row_never_carries_an_attachment_body() {
        let dir = scratch("store-no-body");
        let id = {
            let s = Session::open(&dir);
            let song = s.songs.add("Landslide", "Fleetwood Mac");
            let mut typed = chart("Landslide — my version");
            typed.kind = AttachmentKind::Text;
            typed.body = Some("Capo 3. Eb shapes played as C.".into());
            s.attachments.add(song, typed).unwrap()
        };

        let s = Session::open(&dir);
        assert_eq!(s.attachments.get(id).unwrap().body, None);
        assert_eq!(
            s.attachments.body(id).as_deref(),
            Some("Capo 3. Eb shapes played as C."),
            "still readable when something means to show it"
        );
    }

    #[test]
    fn deleting_a_song_takes_its_attachment_directory_with_it() {
        let dir = scratch("store-attachment-cascade");
        let path = {
            let s = Session::open(&dir);
            let song = s.songs.add("Carolina", "M. Ward");
            let id = s.attachments.add(song, chart("carolina-chords.pdf")).unwrap();
            let path = s.attachments.directory(id).unwrap();
            assert!(path.is_dir());

            s.songs.delete(song);

            assert!(!path.exists());
            path
        };

        let s = Session::open(&dir);
        assert!(s.attachments.items.get().is_empty());
        assert!(!path.exists());
    }

    // ── failed writes ───────────────────────────────────────────────────────

    #[test]
    fn a_write_that_does_not_land_changes_nothing_and_says_so() {
        let dir = scratch("store-failed-write");
        let storage = Storage::open(&dir);
        // A song the screens believe in and the library has never heard of —
        // the shape every failed write takes from up here.
        let phantom = Song::new(9_999, "Carolina", "M. Ward");
        let songs = SongsStore::restored(storage, vec![phantom]);

        songs.edit(9_999, |song| song.key = Some("G".into()));

        assert_eq!(
            songs.get(9_999).unwrap().key,
            None,
            "memory must not run ahead of the disk"
        );
        let fault = storage.fault().get().expect("the failure was recorded");
        assert_eq!(fault.doing, "saving a song");
        assert!(!fault.message.is_empty());

        storage.clear_fault();
        assert_eq!(storage.fault().get(), None);
    }

    #[test]
    fn a_failed_delete_leaves_the_song_on_screen() {
        let dir = scratch("store-failed-delete");
        let storage = Storage::open(&dir);
        let songs = SongsStore::restored(storage, vec![Song::new(9_999, "Carolina", "M. Ward")]);

        songs.delete(9_999);

        assert_eq!(songs.count(), 1, "nothing was deleted, so nothing vanished");
        assert_eq!(storage.fault().get().unwrap().doing, "deleting a song");
    }

    #[test]
    fn a_library_that_will_not_open_still_gives_a_working_app() {
        // A file where the directory should be: nothing can be created under it.
        let dir = scratch("store-unopenable");
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(dir.path().join("db"), b"not a directory").unwrap();

        let storage = Storage::open(&dir);
        assert!(!storage.is_persistent());
        assert_eq!(
            storage.fault().get().unwrap().doing,
            "opening the library",
            "and it does not pretend everything is fine"
        );

        // The app still runs; it just remembers nothing.
        let songs = SongsStore::restored(storage, Vec::new());
        let id = songs.add("Carolina", "M. Ward");
        assert_eq!(songs.get(id).unwrap().title, "Carolina");
    }

    #[test]
    fn stores_with_no_database_behave_exactly_as_they_used_to() {
        let songs = SongsStore::new(vec![Song::new(1, "Ripple", "Grateful Dead")]);
        let id = songs.add("Carolina", "M. Ward");
        assert_eq!(id, 2, "the counter picks up after the seed");
        songs.edit(id, |s| s.key = Some("G".into()));
        assert_eq!(songs.get(id).unwrap().key.as_deref(), Some("G"));
        songs.delete(1);
        assert_eq!(songs.count(), 1);

        let setlists = SetlistsStore::new(Vec::new());
        let set = setlists.add("Porch, Saturday");
        setlists.add_song(set, id);
        setlists.add_song(set, id);
        assert_eq!(setlists.get(set).unwrap().song_ids, vec![id]);
        assert_eq!(setlists.containing(id).len(), 1);
        setlists.remove_song(set, id);
        assert!(setlists.get(set).unwrap().song_ids.is_empty());
    }
}

