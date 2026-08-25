use rinch::prelude::*;

use crate::model::{Confidence, Day, Song, SongId};

#[derive(Clone, Copy)]
pub struct SongsStore {
    pub songs: Signal<Vec<Song>>,
    next_id: Signal<SongId>,
}

impl SongsStore {
    pub fn new(seed: Vec<Song>) -> Self {
        let next = seed.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        Self {
            songs: Signal::new(seed),
            next_id: Signal::new(next),
        }
    }

    pub fn get(self, id: SongId) -> Option<Song> {
        self.songs.get().into_iter().find(|s| s.id == id)
    }

    /// Adding a song has to be possible in a few seconds: a title is enough.
    pub fn add(self, title: impl Into<String>, artist: impl Into<String>) -> SongId {
        let id = self.next_id.get();
        self.songs
            .update(|list| list.push(Song::new(id, title, artist)));
        self.next_id.set(id + 1);
        id
    }

    pub fn edit(self, id: SongId, f: impl FnOnce(&mut Song)) {
        self.songs.update(|list| {
            if let Some(song) = list.iter_mut().find(|s| s.id == id) {
                f(song);
            }
        });
    }

    pub fn delete(self, id: SongId) {
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
}
