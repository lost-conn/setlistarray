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

    /// Several songs onto the end of a setlist, in the order given, in one
    /// write.
    ///
    /// The picker (`1i`) hands over everything the user ticked at once, and
    /// membership is one statement — so this is one `set_members` rather than
    /// N of them. Existing positions are untouched: the new songs land after
    /// the last one already there, in tick order. Anything already in the set
    /// is skipped rather than moved, because a set holds a song once and
    /// "add" is not a reorder.
    pub fn add_songs(self, setlist: SetlistId, songs: &[SongId]) {
        let Some(current) = self.get(setlist) else {
            return;
        };
        let mut order = current.song_ids.clone();
        for song in songs {
            if !order.contains(song) {
                order.push(*song);
            }
        }
        if order.len() == current.song_ids.len() {
            return;
        }
        self.commit_order("adding songs to a setlist", setlist, order);
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

    /// A copy of the setlist's running order under a new id, from the
    /// long-press card menu. Songs are still shared by reference — nothing
    /// about the songs themselves is touched. The copy hasn't been played,
    /// whatever the original's `last_played` said.
    pub fn duplicate(self, id: SetlistId) -> Option<SetlistId> {
        let mut sl = self.get(id)?;
        sl.id = 0;
        sl.name = format!("{} (copy)", sl.name);
        sl.last_played = None;

        let minted = self.storage.create(
            "duplicating a setlist",
            || self.next_id.get(),
            |repo| repo.create_setlist(&sl),
        );
        let new_id = minted?;
        sl.id = new_id;

        // The running order is a relationship, so it is written separately —
        // and by reference: the copy holds the same songs, it does not clone
        // them.
        let members = sl.song_ids.clone();
        if !members.is_empty()
            && !self.storage.write("copying a setlist's running order", |repo| {
                repo.set_members(new_id, &members)
            })
        {
            return None;
        }

        self.setlists.update(|list| list.push(sl));
        self.bump_next_id(new_id);
        Some(new_id)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Day;

    fn store() -> SetlistsStore {
        let setlists = SetlistsStore::new(Vec::new());
        let id = setlists.add("Friday set");
        setlists.add_song(id, 1);
        setlists.add_song(id, 2);
        setlists
    }

    #[test]
    fn rename_replaces_the_name_and_nothing_else() {
        let setlists = store();
        let id = setlists.setlists.get()[0].id;
        setlists.rename(id, "Saturday set");
        let sl = setlists.get(id).unwrap();
        assert_eq!(sl.name, "Saturday set");
        assert_eq!(sl.song_ids, vec![1, 2]);
    }

    #[test]
    fn added_songs_land_after_the_existing_ones_in_the_order_they_were_picked() {
        let setlists = store();
        let id = setlists.setlists.get()[0].id;
        setlists.add_songs(id, &[7, 5, 9]);
        assert_eq!(setlists.get(id).unwrap().song_ids, vec![1, 2, 7, 5, 9]);
    }

    #[test]
    fn adding_a_song_the_set_already_holds_does_not_move_it() {
        let setlists = store();
        let id = setlists.setlists.get()[0].id;
        setlists.add_songs(id, &[1, 3]);
        // 1 keeps its position; only 3 joins, at the end.
        assert_eq!(setlists.get(id).unwrap().song_ids, vec![1, 2, 3]);
    }

    #[test]
    fn adding_nothing_new_leaves_the_running_order_alone() {
        let setlists = store();
        let id = setlists.setlists.get()[0].id;
        setlists.add_songs(id, &[2, 1]);
        setlists.add_songs(id, &[]);
        assert_eq!(setlists.get(id).unwrap().song_ids, vec![1, 2]);
    }

    #[test]
    fn adding_songs_to_a_missing_setlist_is_a_no_op() {
        let setlists = store();
        setlists.add_songs(999, &[1, 2]);
        assert_eq!(setlists.setlists.get().len(), 1);
    }

    #[test]
    fn renaming_a_missing_setlist_is_a_no_op() {
        let setlists = store();
        setlists.rename(999, "Nothing");
        assert_eq!(setlists.setlists.get().len(), 1);
    }

    #[test]
    fn duplicate_copies_the_running_order_under_a_new_id() {
        let setlists = store();
        let id = setlists.setlists.get()[0].id;
        let copy_id = setlists.duplicate(id).expect("setlist exists");
        assert_ne!(copy_id, id);

        let original = setlists.get(id).unwrap();
        let copy = setlists.get(copy_id).unwrap();
        assert_eq!(copy.name, format!("{} (copy)", original.name));
        assert_eq!(copy.song_ids, original.song_ids);
        assert_eq!(setlists.setlists.get().len(), 2);
    }

    #[test]
    fn duplicate_does_not_inherit_last_played() {
        let setlists = store();
        let id = setlists.setlists.get()[0].id;
        setlists.setlists.update(|list| {
            list[0].last_played = Some(Day::new(2026, 8, 14));
        });
        let copy_id = setlists.duplicate(id).unwrap();
        assert_eq!(setlists.get(copy_id).unwrap().last_played, None);
    }

    #[test]
    fn duplicating_a_missing_setlist_is_a_no_op() {
        let setlists = store();
        assert_eq!(setlists.duplicate(999), None);
        assert_eq!(setlists.setlists.get().len(), 1);
    }

    #[test]
    fn removing_a_song_from_a_duplicate_leaves_the_original_alone() {
        let setlists = store();
        let id = setlists.setlists.get()[0].id;
        let copy_id = setlists.duplicate(id).unwrap();
        setlists.remove_song(copy_id, 1);
        assert_eq!(setlists.get(copy_id).unwrap().song_ids, vec![2]);
        assert_eq!(setlists.get(id).unwrap().song_ids, vec![1, 2]);
    }
}
