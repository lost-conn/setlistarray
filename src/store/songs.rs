use rinch::prelude::*;

use crate::model::{Attachment, AttachmentId, Confidence, Day, Song, SongId};
use crate::store::{AttachmentsStore, SetlistsStore, Storage};

/// The library, in memory, with every mutation written through to disk first.
///
/// The API is unchanged from the in-memory version the screens were written
/// against; what changed is that a mutation now only reaches the signal once it
/// has reached the database. See [`Storage`] for why that order and not the
/// other one.
#[derive(Clone, Copy)]
pub struct SongsStore {
    pub songs: Signal<Vec<Song>>,
    /// The id source when there is no database to mint one. With a database,
    /// object ids are the domain ids and this only tracks behind them so that a
    /// library which loses its database mid-session does not reuse an id.
    next_id: Signal<SongId>,
    storage: Storage,
    /// The charts.
    ///
    /// A song *owns* its attachments — the schema says so with a cascade on
    /// `Attachment.song` — and the three rules on [`Song`] about which chart is
    /// primary are rules about a song. So attaching and removing live here,
    /// where `Song.attachments` and `Song.primary_attachment` can move in the
    /// same operation as the row and the directory, and
    /// [`AttachmentsStore`]'s mutating half is `pub(super)` so there is no
    /// second door into it.
    ///
    /// The dependency points this way and only this way. A leaf store that had
    /// to reach back into the library would be a cycle, and there is nothing an
    /// attachment needs to know about a song.
    attachments: AttachmentsStore,
    /// The sets a song can belong to.
    ///
    /// A song does not own its memberships the way it owns its attachments —
    /// `Setlist.songs` is the setlist's own edge, not this store's — but
    /// deleting a song is still this store's job, and `@on_delete(remove)` on
    /// that edge means the database has already dropped the song from every
    /// set by the time [`delete`](Self::delete) returns. `SetlistsStore` has
    /// no way to hear that happen on its own — nothing about a delete touches
    /// its signal — so this handle exists for exactly one call:
    /// [`SetlistsStore::forget_song`](crate::store::SetlistsStore::forget_song),
    /// the setlist half of the same seam `attachments` above is the chart
    /// half of. Same one-way rule: `SetlistsStore` does not hold a
    /// `SongsStore` back.
    setlists: SetlistsStore,
}

impl SongsStore {
    /// In memory only — no database behind it. The attachments and setlists
    /// stores it gets are its own, and equally in memory: the three have to
    /// share a `Storage` or the library would be part persistent and part not.
    pub fn new(seed: Vec<Song>) -> Self {
        let storage = Storage::in_memory();
        Self::restored(
            storage,
            AttachmentsStore::restored(storage, Vec::new()),
            SetlistsStore::restored(storage, Vec::new()),
            seed,
        )
    }

    /// The library as it was left, behind the storage that will keep it.
    ///
    /// `setlists` is handed in rather than built here because it has to be
    /// the *same* store the rest of the app reads — `app()` constructs it
    /// first, with no dependency of its own, and passes it down for exactly
    /// that reason.
    pub fn restored(
        storage: Storage,
        attachments: AttachmentsStore,
        setlists: SetlistsStore,
        songs: Vec<Song>,
    ) -> Self {
        let next = songs.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        Self {
            songs: Signal::new(songs),
            next_id: Signal::new(next),
            storage,
            attachments,
            setlists,
        }
    }

    /// The charts, for a screen that has a `SongsStore` and would otherwise
    /// have to pull a second store out of the context to read one.
    pub fn attachments(self) -> AttachmentsStore {
        self.attachments
    }

    /// Replace every song in memory, after `crate::store::reload_all` has
    /// pointed [`Storage`] at a library that is not the one these signals
    /// were filled from — card I2's import.
    ///
    /// `next_id` is re-derived exactly the way [`restored`](Self::restored)
    /// derives it at startup, and for the same reason: this *is* a second
    /// first load, of a library with its own highest id, and a counter left
    /// over from the one being replaced could hand out an id the import just
    /// brought in. `attachments` and `setlists` are untouched here — they are
    /// reloaded independently by the same caller, and reaching into them from
    /// here would be the one-way dependency this store already avoids reaching
    /// the other way (see the field doc comments above).
    pub fn reload(self, songs: Vec<Song>) {
        let next = songs.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        self.songs.set(songs);
        self.next_id.set(next);
    }

    pub fn get(self, id: SongId) -> Option<Song> {
        self.songs.get().into_iter().find(|s| s.id == id)
    }

    /// Adding a song has to be possible in a few seconds: a title is enough.
    pub fn add(self, title: impl Into<String>, artist: impl Into<String>) -> SongId {
        // The write did not land. Nothing was added, and the caller gets an id
        // that matches nothing — every lookup on it returns None, which is
        // exactly what "there is no such song" looks like everywhere else in
        // this store.
        self.create(Song::new(0, title, artist)).unwrap_or(0)
    }

    /// A song filled in before it exists: the add form's Save, where the
    /// optional fields behind **More details** are already typed in.
    ///
    /// One write, not an insert followed by an edit — a form that saved twice
    /// would be two chances to half-fail, and the second would leave a song in
    /// the book missing everything the user had just entered. The id and the
    /// added-at time are the store's to set, whatever the draft says.
    pub fn create(self, mut song: Song) -> Option<SongId> {
        song.id = 0;
        song.created_at = now_millis();

        let id = self.storage.create(
            "adding a song",
            || self.next_id.get(),
            |repo| repo.create_song(&song),
        )?;

        song.id = id;
        self.songs.update(|list| list.push(song));
        self.bump_next_id(id);
        Some(id)
    }

    pub fn edit(self, id: SongId, f: impl FnOnce(&mut Song)) {
        let _ = self.try_edit(id, f);
    }

    /// [`edit`](Self::edit), for callers that have to know whether it landed.
    ///
    /// A screen does not: a failed write leaves the row exactly as it was,
    /// which is the truth, and `Storage` has already recorded why. The
    /// attachment operations below do, because each is two writes and the
    /// second failing has to undo the first.
    fn try_edit(self, id: SongId, f: impl FnOnce(&mut Song)) -> bool {
        let Some(mut song) = self.get(id) else {
            return false;
        };
        f(&mut song);
        if !self
            .storage
            .write("saving a song", |repo| repo.save_song(&song))
        {
            return false;
        }
        self.songs.update(|list| {
            if let Some(slot) = list.iter_mut().find(|s| s.id == id) {
                *slot = song;
            }
        });
        true
    }

    // ── Attachments (card D1) ───────────────────────────────────────────────

    /// Attach a chart: the row, the link to the song, the directory its bytes
    /// go in, and the song's own list and primary pointer. The first chart a
    /// song gets becomes its primary one.
    ///
    /// `None` means nothing was attached, and in that case the song is exactly
    /// as it was — including the case where the chart was written and the
    /// song's own row then refused the pointer, which takes the chart back out
    /// again. There is no state where a chart exists and no song lists it: an
    /// unlisted chart is invisible in the UI, immortal on disk, and a size in
    /// the Storage screen nobody can account for.
    ///
    /// The bytes are not written here. A producer — a typed editor (D2), a PDF
    /// import (D3), a capture (E2) — calls this to mint an id and a directory,
    /// writes into [`AttachmentsStore::directory`], and calls
    /// [`detach`](Self::detach) if that write fails.
    pub fn attach(self, song: SongId, attachment: Attachment) -> Option<AttachmentId> {
        // `Attachment.song` is what the cascade rides on, so a chart with no
        // song would be an orphan from the moment it was written.
        self.get(song)?;
        let id = self.attachments.insert(song, attachment)?;
        if !self.try_edit(song, |s| s.attach(id)) {
            self.attachments.forget(id);
            return None;
        }
        Some(id)
    }

    /// Remove a chart from a song and take its bytes with it. `false` if the
    /// song never had it, or if the write did not land.
    ///
    /// If it was the primary chart, the oldest chart still attached takes its
    /// place — [`Song::detach`] is where that rule lives and why.
    ///
    /// The bytes go first, for the same reason
    /// [`AttachmentsStore::forget`](AttachmentsStore) deletes the row before
    /// the directory. If the song's save then fails, the song lists an id that
    /// no longer resolves — which every read here already tolerates, and which
    /// the next launch does not even see, because the link went with the row.
    /// The other order risks the opposite: a song that has let go of a chart
    /// still sitting on disk, which nothing will ever clean up.
    pub fn detach(self, song: SongId, attachment: AttachmentId) -> bool {
        let Some(current) = self.get(song) else {
            return false;
        };
        if !current.attachments.contains(&attachment) {
            return false;
        }
        if !self.attachments.forget(attachment) {
            return false;
        }
        self.try_edit(song, |s| {
            s.detach(attachment);
        })
    }

    /// Promote one of the song's charts to primary — the ⋮ menu and the
    /// long-press on a collapsed row. `false`, and nothing written, for a
    /// chart this song does not have.
    ///
    /// Expanding a collapsed row does *not* come through here. That is view
    /// state and it stays on the screen that owns it.
    pub fn set_primary(self, song: SongId, attachment: AttachmentId) -> bool {
        let Some(current) = self.get(song) else {
            return false;
        };
        if !current.attachments.contains(&attachment) {
            return false;
        }
        if current.primary_attachment == Some(attachment) {
            return true;
        }
        self.try_edit(song, |s| {
            s.set_primary(attachment);
        })
    }

    /// The schema takes the song's attachments with it and drops it from every
    /// setlist — `@on_delete(cascade)` on `Attachment.song` for the first,
    /// `@on_delete(remove)` on `Setlist.songs` for the second — and neither
    /// signal has a way to learn what a delete policy did on its own. So both
    /// halves are handled the same way: collect what is about to be doomed
    /// before the write, and once it lands, tell the signal directly rather
    /// than waiting for the next launch to notice.
    ///
    /// Charts: `Repo::delete_song` takes the rows *and* the directories, so
    /// the attachment ids are collected up front and
    /// [`forget_all`](AttachmentsStore::forget_all) drops them from
    /// `attachments` afterwards.
    ///
    /// Setlists: nothing needs collecting — every running order is already in
    /// memory — so [`SetlistsStore::forget_song`] just walks `setlists` and
    /// removes this id from each one directly. This used to be the part that
    /// did not happen: a deleted song lingered as an id in `song_ids` until
    /// the next launch, invisible because every read already filters through
    /// [`get`](Self::get), but real underneath — `rows`' positions on
    /// `setlist_detail` index into that raw list, so a stale id there could
    /// point a reorder tap at the wrong song. `forget_song` closes that,
    /// including taking a pending removal (`SetlistsStore::last_removal`) with it
    /// if it named this song.
    pub fn delete(self, id: SongId) {
        let doomed = self.get(id).map(|s| s.attachments).unwrap_or_default();
        if !self
            .storage
            .write("deleting a song", |repo| repo.delete_song(id))
        {
            return;
        }
        self.songs.update(|list| list.retain(|s| s.id != id));
        self.attachments.forget_all(&doomed);
        self.setlists.forget_song(id);
    }

    /// A copy of the song's data under a new id, from the overflow menu.
    /// Attachments are kept by reference — the copy doesn't need its own
    /// chart to start. Play history does not carry over: this entry hasn't
    /// been played yet, whatever the original's last-played date said.
    pub fn duplicate(self, id: SongId) -> Option<SongId> {
        let mut song = self.get(id)?;
        song.id = 0;
        song.title = format!("{} (copy)", song.title);
        // A copy is a new song in the book: added now, never played, and
        // carrying none of the original's attachments — those belong to the
        // song they were filed under.
        song.created_at = now_millis();
        song.last_played = None;
        song.attachments.clear();
        song.primary_attachment = None;

        let minted = self.storage.create(
            "duplicating a song",
            || self.next_id.get(),
            |repo| repo.create_song(&song),
        );
        let new_id = minted?;

        song.id = new_id;
        self.songs.update(|list| list.push(song));
        self.bump_next_id(new_id);
        Some(new_id)
    }

    pub fn set_confidence(self, id: SongId, confidence: Option<Confidence>) {
        self.edit(id, |s| s.confidence = confidence);
    }

    pub fn mark_played(self, id: SongId, day: Day) {
        self.edit(id, |s| s.last_played = Some(day));
    }

    pub fn count(self) -> usize {
        self.songs.get().len()
    }

    pub fn solid_count(self) -> usize {
        self.songs
            .get()
            .iter()
            .filter(|s| s.confidence == Some(Confidence::Solid))
            .count()
    }

    fn bump_next_id(self, used: SongId) {
        if used >= self.next_id.get() {
            self.next_id.set(used + 1);
        }
    }
}

/// Epoch milliseconds. `Song::new` seeds `created_at` from the id, which was
/// fine while ids were the only ordering there was; a song added for real gets
/// the time it was actually added, so Date-added sorting means something.
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::model::AttachmentKind;

    fn store() -> SongsStore {
        SongsStore::new(vec![Song::new(1, "Carolina", "M. Ward")])
    }

    fn chart(title: &str) -> Attachment {
        Attachment {
            id: 0,
            kind: AttachmentKind::Text,
            title: title.into(),
            bytes_on_disk: 900,
            page_count: None,
            source_url: None,
            captured_at: None,
            body: Some("Capo 3.".into()),
            rechecked_at: None,
        }
    }

    /// **Card K54 — this passes, and that is the point of it.**
    ///
    /// On the moto g stylus 5G, tapping `Set confidence › Rusty` in a song's ⋮
    /// menu ran the handler (logged), wrote the row (it survived a relaunch)
    /// and left the header on screen still reading `Solid` — through a forced
    /// repaint, and until the screen was navigated away from and back.
    ///
    /// The obvious suspicion was this layer: that a mutation reached the
    /// database and left the signal, or its subscribers, behind. It did not.
    /// A reader that was already watching when the write happened sees the new
    /// value and re-runs exactly once, which is what this test and its
    /// `duplicate` twin below pin. **The store is not where K54 lives**, and
    /// these exist so that the next person to read that card does not spend an
    /// afternoon here.
    ///
    /// Where it does live is `screens::song_detail`: `SongDetail` binds
    /// `songs.get(id)` in the component body, which runs once per mount, so
    /// the header renders from a snapshot no later write can move. Asserting
    /// *that* needs a mounted component, which the screen tests have no
    /// harness for.
    ///
    /// The assertion is on the value the watcher last *saw*, not on what the
    /// store returns when asked afterwards — the second is what already passed
    /// while the app was visibly wrong.
    #[test]
    fn a_confidence_change_reaches_a_reader_that_was_already_watching() {
        use rinch::reactive::Effect;
        use std::cell::Cell;
        use std::rc::Rc;

        let songs = store();
        let seen: Rc<Cell<Option<Confidence>>> = Rc::new(Cell::new(None));
        let runs = Rc::new(Cell::new(0usize));

        let (s, r) = (seen.clone(), runs.clone());
        let _watcher = Effect::new(move || {
            r.set(r.get() + 1);
            s.set(songs.get(1).and_then(|song| song.confidence));
        });

        assert_eq!(runs.get(), 1, "an effect runs once when it is created");
        assert_eq!(seen.get(), None, "and the seeded song is unrated");

        songs.set_confidence(1, Some(Confidence::Rusty));

        assert_eq!(
            seen.get(),
            Some(Confidence::Rusty),
            "a reader watching the library must see a confidence change without \
             being re-created — this is the header the phone left reading Solid"
        );
        assert_eq!(runs.get(), 2, "and it must re-run exactly once for one write");
    }

    /// The same question for a mutation that adds a row rather than editing
    /// one, because they take different paths into the signal — `duplicate`
    /// pushes, `set_confidence` goes through `try_edit` — and the library
    /// header ("3 in your book") is a reader of the count.
    #[test]
    fn a_duplicate_reaches_a_reader_that_was_already_watching() {
        use rinch::reactive::Effect;
        use std::cell::Cell;
        use std::rc::Rc;

        let songs = store();
        let seen = Rc::new(Cell::new(0usize));

        let s = seen.clone();
        let _watcher = Effect::new(move || s.set(songs.count()));

        assert_eq!(seen.get(), 1, "one seeded song");
        songs.duplicate(1).expect("song 1 exists");
        assert_eq!(
            seen.get(),
            2,
            "the library header counts through this read, and a duplicate has \
             to reach it without the screen being rebuilt"
        );
    }

    #[test]
    fn duplicate_gets_a_fresh_id_and_a_copy_suffix() {
        let songs = store();
        let new_id = songs.duplicate(1).expect("song 1 exists");
        assert_ne!(new_id, 1);

        let original = songs.get(1).unwrap();
        let copy = songs.get(new_id).unwrap();
        assert_eq!(original.title, "Carolina");
        assert_eq!(copy.title, "Carolina (copy)");
        assert_eq!(copy.artist, "M. Ward");
        assert_eq!(songs.count(), 2);
    }

    #[test]
    fn duplicate_does_not_inherit_play_history() {
        let songs = store();
        songs.mark_played(1, Day::new(2026, 3, 14));
        let new_id = songs.duplicate(1).unwrap();
        assert_eq!(songs.get(new_id).unwrap().last_played, None);
        // The original is untouched.
        assert_eq!(songs.get(1).unwrap().last_played, Some(Day::new(2026, 3, 14)));
    }

    #[test]
    fn duplicating_a_missing_song_is_a_no_op() {
        let songs = store();
        assert_eq!(songs.duplicate(999), None);
        assert_eq!(songs.count(), 1);
    }

    #[test]
    fn a_duplicate_can_itself_be_duplicated_without_id_collision() {
        let songs = store();
        let first_copy = songs.duplicate(1).unwrap();
        let second_copy = songs.duplicate(first_copy).unwrap();
        assert_ne!(first_copy, second_copy);
        assert_eq!(songs.count(), 3);
    }

    #[test]
    fn set_confidence_and_mark_played_wire_straight_through() {
        let songs = store();
        songs.set_confidence(1, Some(Confidence::Solid));
        assert_eq!(songs.get(1).unwrap().confidence, Some(Confidence::Solid));

        songs.mark_played(1, Day::new(2026, 6, 2));
        assert_eq!(songs.get(1).unwrap().last_played, Some(Day::new(2026, 6, 2)));
    }

    // ── D1: attaching, removing and promoting ───────────────────────────────

    #[test]
    fn the_first_chart_a_song_gets_becomes_its_primary() {
        let songs = store();
        let first = songs.attach(1, chart("chords.txt")).expect("attached");
        assert_eq!(songs.get(1).unwrap().primary_attachment, Some(first));

        let second = songs.attach(1, chart("lyrics.txt")).expect("attached");
        assert_eq!(
            songs.get(1).unwrap().primary_attachment,
            Some(first),
            "the second one joins the rows under the card"
        );
        assert_eq!(songs.get(1).unwrap().attachments, vec![first, second]);
        assert!(songs.get(1).unwrap().has_chart());
    }

    #[test]
    fn attaching_records_the_chart_where_the_screens_look_for_it() {
        let songs = store();
        let id = songs.attach(1, chart("chords.txt")).unwrap();
        // The song's list and the attachment list have to agree the moment the
        // call returns — song detail reads both, and a card that appears only
        // after a restart is the bug this seam exists to prevent.
        assert_eq!(songs.get(1).unwrap().attachments, vec![id]);
        assert_eq!(songs.attachments().get(id).unwrap().title, "chords.txt");
    }

    #[test]
    fn a_chart_cannot_be_attached_to_a_song_that_is_not_there() {
        let songs = store();
        assert_eq!(songs.attach(999, chart("chords.txt")), None);
        assert!(
            songs.attachments().items.get().is_empty(),
            "and no orphan row was written"
        );
    }

    #[test]
    fn removing_the_primary_chart_promotes_the_oldest_one_left() {
        let songs = store();
        let first = songs.attach(1, chart("chords.txt")).unwrap();
        let second = songs.attach(1, chart("lyrics.txt")).unwrap();
        let third = songs.attach(1, chart("tab.txt")).unwrap();

        assert!(songs.detach(1, first));

        let song = songs.get(1).unwrap();
        assert_eq!(song.attachments, vec![second, third]);
        assert_eq!(song.primary_attachment, Some(second));
        assert!(songs.attachments().get(first).is_none(), "and the row went");
    }

    #[test]
    fn removing_the_last_chart_leaves_the_song_with_no_primary() {
        let songs = store();
        let only = songs.attach(1, chart("chords.txt")).unwrap();
        assert!(songs.detach(1, only));

        let song = songs.get(1).unwrap();
        assert!(song.attachments.is_empty());
        assert_eq!(song.primary_attachment, None);
        assert!(!song.has_chart());
    }

    #[test]
    fn removing_a_chart_leaves_the_song_itself_alone() {
        let songs = store();
        songs.edit(1, |s| s.key = Some("G".into()));
        let id = songs.attach(1, chart("chords.txt")).unwrap();

        songs.detach(1, id);

        let song = songs.get(1).unwrap();
        assert_eq!(song.title, "Carolina");
        assert_eq!(song.key.as_deref(), Some("G"), "removing a chart is not an edit");
        assert_eq!(songs.count(), 1);
    }

    #[test]
    fn deleting_a_song_takes_its_charts_and_leaves_another_songs_alone() {
        let songs = store();
        let other = songs.add("Ripple", "Grateful Dead");
        let doomed = songs.attach(1, chart("chords.txt")).unwrap();
        let kept = songs.attach(other, chart("ripple.txt")).unwrap();

        songs.delete(1);

        assert!(songs.get(1).is_none());
        assert!(
            songs.attachments().get(doomed).is_none(),
            "the chart went with the song, in memory as well as on disk"
        );
        assert_eq!(songs.attachments().get(kept).unwrap().title, "ripple.txt");
        assert_eq!(songs.get(other).unwrap().attachments, vec![kept]);
    }

    // ── delete and the setlist half of it (J5) ──────────────────────────────
    //
    // `store()` above builds a `SongsStore` through `new`, which mints its own
    // private `SetlistsStore` nobody else can see — fine for the attachment
    // tests, useless for these, which need the *same* `SetlistsStore` a
    // setlist screen would be holding. So these build the trio by hand, the
    // way `crate::app` and `Session::open` (`store/storage.rs`) do.

    fn linked() -> (SongsStore, SetlistsStore) {
        let storage = Storage::in_memory();
        let attachments = AttachmentsStore::restored(storage, Vec::new());
        let setlists = SetlistsStore::restored(storage, Vec::new());
        let songs = SongsStore::restored(
            storage,
            attachments,
            setlists,
            vec![Song::new(1, "Carolina", "M. Ward"), Song::new(2, "Ripple", "Grateful Dead")],
        );
        (songs, setlists)
    }

    #[test]
    fn deleting_a_song_drops_it_from_every_setlist_immediately() {
        let (songs, setlists) = linked();
        let friday = setlists.add("Friday set");
        let saturday = setlists.add("Saturday set");
        setlists.add_songs(friday, &[1, 2]);
        setlists.add_songs(saturday, &[2]);

        songs.delete(1);

        assert_eq!(
            setlists.get(friday).unwrap().song_ids,
            vec![2],
            "gone the moment the delete lands, not at the next launch"
        );
        assert_eq!(setlists.get(saturday).unwrap().song_ids, vec![2], "and a set it was never in stays that way");
    }

    /// The bug this seam replaces: a stale id sitting in `song_ids` is
    /// invisible on screen — every read already filters through
    /// [`SongsStore::get`] — but a reorder still indexes into the *raw* list,
    /// so a set holding one dead id could send a reorder tap to the wrong
    /// song. Deleting the song has to remove the id, not just stop it from
    /// rendering.
    #[test]
    fn a_deleted_songs_id_does_not_linger_where_a_reorder_could_still_find_it() {
        let (songs, setlists) = linked();
        let set = setlists.add("Friday set");
        setlists.add_songs(set, &[1, 2]);

        songs.delete(1);

        assert!(
            !setlists.get(set).unwrap().song_ids.contains(&1),
            "not merely unrenderable — actually gone from the running order"
        );
    }

    /// A song can come out of a set (arming the undo offer) and then be
    /// deleted from the library outright before anyone taps Undo. See
    /// `SetlistsStore::forget_song`'s own doc comment for what `undo_removal`
    /// would otherwise do with the dead id.
    #[test]
    fn deleting_a_song_that_is_the_pending_undo_spends_the_offer_too() {
        let (songs, setlists) = linked();
        let set = setlists.add("Friday set");
        setlists.add_songs(set, &[1, 2]);
        setlists.remove_song(set, 1);
        assert!(setlists.last_removal.get().is_some(), "the setup is an offer standing");

        songs.delete(1);

        assert_eq!(setlists.last_removal.get(), None, "nothing left to put back");
        assert!(!setlists.undo_removal());
    }

    #[test]
    fn set_primary_promotes_a_chart_the_song_already_has() {
        let songs = store();
        let first = songs.attach(1, chart("chords.txt")).unwrap();
        let second = songs.attach(1, chart("lyrics.txt")).unwrap();

        assert!(songs.set_primary(1, second));
        assert_eq!(songs.get(1).unwrap().primary_attachment, Some(second));
        assert_eq!(
            songs.get(1).unwrap().attachments,
            vec![first, second],
            "and the rows do not reshuffle"
        );
    }

    #[test]
    fn set_primary_refuses_a_chart_filed_under_a_different_song() {
        let songs = store();
        let other = songs.add("Ripple", "Grateful Dead");
        let mine = songs.attach(1, chart("chords.txt")).unwrap();
        let theirs = songs.attach(other, chart("ripple.txt")).unwrap();

        assert!(!songs.set_primary(1, theirs));
        assert_eq!(songs.get(1).unwrap().primary_attachment, Some(mine));
    }

    #[test]
    fn a_duplicate_starts_with_no_charts_of_its_own() {
        let songs = store();
        let id = songs.attach(1, chart("chords.txt")).unwrap();
        let copy = songs.duplicate(1).unwrap();

        let copy = songs.get(copy).unwrap();
        assert!(copy.attachments.is_empty());
        assert_eq!(copy.primary_attachment, None);
        assert_eq!(
            songs.get(1).unwrap().primary_attachment,
            Some(id),
            "and the original keeps its own"
        );
    }
}
