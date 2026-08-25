use rinch::prelude::*;

use crate::model::{Setlist, SetlistId, SongId};

#[derive(Clone, Copy)]
pub struct SetlistsStore {
    pub setlists: Signal<Vec<Setlist>>,
    next_id: Signal<SetlistId>,
}

impl SetlistsStore {
    pub fn new(seed: Vec<Setlist>) -> Self {
        let next = seed.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        Self {
            setlists: Signal::new(seed),
            next_id: Signal::new(next),
        }
    }

    pub fn get(self, id: SetlistId) -> Option<Setlist> {
        self.setlists.get().into_iter().find(|s| s.id == id)
    }

    pub fn add(self, name: impl Into<String>) -> SetlistId {
        let id = self.next_id.get();
        self.setlists.update(|list| {
            list.push(Setlist {
                id,
                name: name.into(),
                song_ids: Vec::new(),
                last_played: None,
            })
        });
        self.next_id.set(id + 1);
        id
    }

    /// Songs join a setlist by reference, at the end.
    pub fn add_song(self, setlist: SetlistId, song: SongId) {
        self.setlists.update(|list| {
            if let Some(sl) = list.iter_mut().find(|s| s.id == setlist)
                && !sl.song_ids.contains(&song)
            {
                sl.song_ids.push(song);
            }
        });
    }

    /// Removing here never deletes the song itself.
    pub fn remove_song(self, setlist: SetlistId, song: SongId) {
        self.setlists.update(|list| {
            if let Some(sl) = list.iter_mut().find(|s| s.id == setlist) {
                sl.song_ids.retain(|id| *id != song);
            }
        });
    }

    pub fn reorder(self, setlist: SetlistId, from: usize, to: usize) {
        self.setlists.update(|list| {
            if let Some(sl) = list.iter_mut().find(|s| s.id == setlist)
                && from < sl.song_ids.len()
                && to < sl.song_ids.len()
            {
                let id = sl.song_ids.remove(from);
                sl.song_ids.insert(to, id);
            }
        });
    }

    pub fn delete(self, id: SetlistId) {
        self.setlists.update(|list| list.retain(|s| s.id != id));
    }

    /// How many setlists a song appears in — the "2 setlists" link on detail.
    pub fn containing(self, song: SongId) -> Vec<Setlist> {
        self.setlists
            .get()
            .into_iter()
            .filter(|sl| sl.song_ids.contains(&song))
            .collect()
    }
}
