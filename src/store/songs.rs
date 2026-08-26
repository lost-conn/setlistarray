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
