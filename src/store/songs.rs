use rinch::prelude::*;

use crate::model::{Confidence, Day, Song, SongId};
use crate::store::Storage;

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
}

impl SongsStore {
    /// In memory only — no database behind it.
    pub fn new(seed: Vec<Song>) -> Self {
        Self::restored(Storage::in_memory(), seed)
    }

    /// The library as it was left, behind the storage that will keep it.
    pub fn restored(storage: Storage, songs: Vec<Song>) -> Self {
        let next = songs.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        Self {
            songs: Signal::new(songs),
            next_id: Signal::new(next),
            storage,
        }
    }

    pub fn get(self, id: SongId) -> Option<Song> {
        self.songs.get().into_iter().find(|s| s.id == id)
    }

    /// Adding a song has to be possible in a few seconds: a title is enough.
    pub fn add(self, title: impl Into<String>, artist: impl Into<String>) -> SongId {
        let mut song = Song::new(0, title, artist);
        song.created_at = now_millis();

        let minted = self.storage.create(
            "adding a song",
            || self.next_id.get(),
            |repo| repo.create_song(&song),
        );
        let Some(id) = minted else {
            // The write did not land. Nothing was added, and the caller gets an
            // id that matches nothing — every lookup on it returns None, which
            // is exactly what "there is no such song" looks like everywhere
            // else in this store.
            return 0;
        };

        song.id = id;
        self.songs.update(|list| list.push(song));
        self.bump_next_id(id);
        id
    }

    pub fn edit(self, id: SongId, f: impl FnOnce(&mut Song)) {
        let Some(mut song) = self.get(id) else { return };
        f(&mut song);
        if !self
            .storage
            .write("saving a song", |repo| repo.save_song(&song))
        {
            return;
        }
        self.songs.update(|list| {
            if let Some(slot) = list.iter_mut().find(|s| s.id == id) {
                *slot = song;
            }
        });
    }

    /// The schema takes the song's attachments with it and drops it from every
    /// setlist. In-memory setlists keep the id until the next launch; nothing
    /// renders for a song that is not there, so it shows as already gone.
    pub fn delete(self, id: SongId) {
        if !self
            .storage
            .write("deleting a song", |repo| repo.delete_song(id))
        {
            return;
        }
        self.songs.update(|list| list.retain(|s| s.id != id));
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

    fn store() -> SongsStore {
        SongsStore::new(vec![Song::new(1, "Carolina", "M. Ward")])
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
}
