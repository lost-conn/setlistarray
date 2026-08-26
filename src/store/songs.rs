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

    /// A copy of the song's data under a new id, from the overflow menu.
    /// Attachments are kept by reference — the copy doesn't need its own
    /// chart to start. Play history does not carry over: this entry hasn't
    /// been played yet, whatever the original's last-played date said.
    pub fn duplicate(self, id: SongId) -> Option<SongId> {
        let mut song = self.get(id)?;
        let new_id = self.next_id.get();
        song.id = new_id;
        song.title = format!("{} (copy)", song.title);
        song.created_at = new_id as u64;
        song.last_played = None;
        self.songs.update(|list| list.push(song));
        self.next_id.set(new_id + 1);
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
