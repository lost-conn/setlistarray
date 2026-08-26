use rinch::prelude::*;

use crate::model::{Setlist, SetlistId, SongId};
use crate::store::Storage;

/// Setlists, in memory, written through on every change.
///
/// Membership is one statement — "this is the running order now" — because add,
/// remove and reorder are the same statement with a different list. The
/// `position` edge field is not a derived value: it is the order the user
/// chose, and it is the only place that order exists.
#[derive(Clone, Copy)]
pub struct SetlistsStore {
    pub setlists: Signal<Vec<Setlist>>,
    next_id: Signal<SetlistId>,
    storage: Storage,
}

impl SetlistsStore {
    pub fn new(seed: Vec<Setlist>) -> Self {
        Self::restored(Storage::in_memory(), seed)
    }

    pub fn restored(storage: Storage, setlists: Vec<Setlist>) -> Self {
        let next = setlists.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        Self {
            setlists: Signal::new(setlists),
            next_id: Signal::new(next),
            storage,
        }
    }

    pub fn get(self, id: SetlistId) -> Option<Setlist> {
        self.setlists.get().into_iter().find(|s| s.id == id)
    }

    pub fn add(self, name: impl Into<String>) -> SetlistId {
        let mut setlist = Setlist {
            id: 0,
            name: name.into(),
            song_ids: Vec::new(),
            last_played: None,
        };

        let minted = self.storage.create(
            "adding a setlist",
            || self.next_id.get(),
            |repo| repo.create_setlist(&setlist),
        );
        let Some(id) = minted else { return 0 };

        setlist.id = id;
        self.setlists.update(|list| list.push(setlist));
        self.bump_next_id(id);
        id
    }

    pub fn rename(self, id: SetlistId, name: impl Into<String>) {
        let Some(mut setlist) = self.get(id) else { return };
        setlist.name = name.into();
        if !self
            .storage
            .write("renaming a setlist", |repo| repo.save_setlist(&setlist))
        {
            return;
        }
        self.setlists.update(|list| {
            if let Some(slot) = list.iter_mut().find(|s| s.id == id) {
                *slot = setlist;
            }
        });
    }

    /// Songs join a setlist by reference, at the end.
    pub fn add_song(self, setlist: SetlistId, song: SongId) {
        let Some(current) = self.get(setlist) else {
            return;
        };
        if current.song_ids.contains(&song) {
            return;
        }
        let mut order = current.song_ids.clone();
        order.push(song);
        self.commit_order("adding a song to a setlist", setlist, order);
    }

    /// Removing here never deletes the song itself — `@on_delete(remove)` on the
    /// membership makes that the schema's promise, not ours to keep by hand.
    pub fn remove_song(self, setlist: SetlistId, song: SongId) {
        let Some(current) = self.get(setlist) else {
            return;
        };
        if !current.song_ids.contains(&song) {
            return;
        }
        let order = current
            .song_ids
            .iter()
            .copied()
            .filter(|id| *id != song)
            .collect();
        self.commit_order("removing a song from a setlist", setlist, order);
    }

    pub fn reorder(self, setlist: SetlistId, from: usize, to: usize) {
        let Some(current) = self.get(setlist) else {
            return;
        };
        if from >= current.song_ids.len() || to >= current.song_ids.len() {
            return;
        }
        let mut order = current.song_ids.clone();
        let id = order.remove(from);
        order.insert(to, id);
        self.commit_order("reordering a setlist", setlist, order);
    }

    pub fn delete(self, id: SetlistId) {
        if !self
            .storage
            .write("deleting a setlist", |repo| repo.delete_setlist(id))
        {
            return;
        }
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

    fn commit_order(self, doing: &'static str, setlist: SetlistId, order: Vec<SongId>) {
        if !self
            .storage
            .write(doing, |repo| repo.set_members(setlist, &order))
        {
            return;
        }
        self.setlists.update(|list| {
            if let Some(slot) = list.iter_mut().find(|s| s.id == setlist) {
                slot.song_ids = order;
            }
        });
    }

    fn bump_next_id(self, used: SetlistId) {
        if used >= self.next_id.get() {
            self.next_id.set(used + 1);
        }
    }
}
