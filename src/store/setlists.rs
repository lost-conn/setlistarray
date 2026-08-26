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

    pub fn rename(self, id: SetlistId, name: impl Into<String>) {
        let name = name.into();
        self.setlists.update(|list| {
            if let Some(sl) = list.iter_mut().find(|s| s.id == id) {
                sl.name = name;
            }
        });
    }

    /// A copy of the setlist's running order under a new id, from the
    /// long-press card menu. Songs are still shared by reference — nothing
    /// about the songs themselves is touched. The copy hasn't been played,
    /// whatever the original's `last_played` said.
    pub fn duplicate(self, id: SetlistId) -> Option<SetlistId> {
        let mut sl = self.get(id)?;
        let new_id = self.next_id.get();
        sl.id = new_id;
        sl.name = format!("{} (copy)", sl.name);
        sl.last_played = None;
        self.setlists.update(|list| list.push(sl));
        self.next_id.set(new_id + 1);
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
