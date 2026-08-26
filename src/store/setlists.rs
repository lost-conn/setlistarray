use rinch::prelude::*;

use crate::model::{Setlist, SetlistId, SongId};
use crate::store::Storage;

/// A membership that was just taken out of a set, kept only long enough to
/// offer it back.
///
/// The song itself is never at risk — that is `@on_delete(remove)`'s promise
/// and J5's rule — so what a removal actually destroys is the *place in the
/// running order*, and that is the part re-adding cannot give back: the picker
/// puts songs on the end. In a twenty-song set, "put it back where it was" is
/// otherwise a dozen taps of Move up. Hence the index rides along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Removal {
    pub setlist: SetlistId,
    pub song: SongId,
    /// Where in the running order it sat, before it was taken out.
    pub index: usize,
}

/// Setlists, in memory, written through on every change.
///
/// Membership is one statement — "this is the running order now" — because add,
/// remove and reorder are the same statement with a different list. The
/// `position` edge field is not a derived value: it is the order the user
/// chose, and it is the only place that order exists.
#[derive(Clone, Copy)]
pub struct SetlistsStore {
    pub setlists: Signal<Vec<Setlist>>,
    /// The one removal that can still be undone, or `None`. Any other write to
    /// any running order clears it — see `commit_order`.
    pub last_removal: Signal<Option<Removal>>,
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
            last_removal: Signal::new(None),
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
    ///
    /// The place it came out of is remembered so `undo_removal` can put it
    /// back there rather than on the end.
    pub fn remove_song(self, setlist: SetlistId, song: SongId) {
        let Some(current) = self.get(setlist) else {
            return;
        };
        let Some(index) = current.song_ids.iter().position(|id| *id == song) else {
            return;
        };
        let mut order = current.song_ids.clone();
        order.remove(index);
        self.commit_order("removing a song from a setlist", setlist, order);
        // After the commit, not before: `commit_order` clears the pending undo
        // (any write invalidates it), and a failed write must leave nothing to
        // undo at all.
        if !self.get(setlist).is_some_and(|s| s.song_ids.contains(&song)) {
            self.last_removal.set(Some(Removal {
                setlist,
                song,
                index,
            }));
        }
    }

    /// Put the last removed song back where it was. Returns whether it went.
    ///
    /// Refuses if the set is gone, or if the song has since found its own way
    /// back in — an undo that duplicated a membership would be worse than one
    /// that quietly declines.
    pub fn undo_removal(self) -> bool {
        let Some(removal) = self.last_removal.get() else {
            return false;
        };
        let Some(current) = self.get(removal.setlist) else {
            self.last_removal.set(None);
            return false;
        };
        if current.song_ids.contains(&removal.song) {
            self.last_removal.set(None);
            return false;
        }
        let mut order = current.song_ids.clone();
        // The set may have been shortened since; clamp rather than panic.
        let at = removal.index.min(order.len());
        order.insert(at, removal.song);
        self.commit_order("putting a removed song back", removal.setlist, order);
        self.get(removal.setlist)
            .is_some_and(|s| s.song_ids.contains(&removal.song))
    }

    /// Move the song at `from` so that it ends up at index `to`, the rest
    /// closing up behind it.
    ///
    /// Take-then-insert, not swap: the two differ the moment a move spans more
    /// than one place, and "put this song third" is the statement the screen
    /// makes. Positions stay dense and unique for free — they are the indices
    /// of this vector, renumbered 0..n by `Repo::set_members` on the way to
    /// disk.
    pub fn reorder(self, setlist: SetlistId, from: usize, to: usize) {
        let Some(current) = self.get(setlist) else {
            return;
        };
        if from >= current.song_ids.len() || to >= current.song_ids.len() {
            return;
        }
        // A move to where it already is is not a change, and must not cost a
        // write — or throw away a pending undo.
        if from == to {
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
        // One undo, and only for the most recent removal. Any other write to
        // any running order moves the ground the remembered index stood on, so
        // it stops being an offer this store can honour.
        self.last_removal.set(None);
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

    // ── reorder (C6) ────────────────────────────────────────────────────
    //
    // The running order *is* the position column: `Repo::set_members` writes
    // the `position` edge field as 0..n over this vector, so a vector that
    // stays a permutation of itself is a position column that stays dense and
    // unique. Every test below therefore checks the whole vector, not just the
    // one song that moved.

    /// A five-song set, so a move can span more than one place in either
    /// direction and still have somewhere to go.
    fn five() -> (SetlistsStore, SetlistId) {
        let setlists = SetlistsStore::new(Vec::new());
        let id = setlists.add("Porch, Saturday");
        setlists.add_songs(id, &[10, 20, 30, 40, 50]);
        (setlists, id)
    }

    fn order(setlists: SetlistsStore, id: SetlistId) -> Vec<SongId> {
        setlists.get(id).unwrap().song_ids
    }

    /// What the `position` edge field will be written as: dense 0..n, unique,
    /// and a permutation of what went in.
    fn positions_are_dense_and_unique(before: &[SongId], after: &[SongId]) {
        assert_eq!(
            after.len(),
            before.len(),
            "a reorder must not add or drop a member"
        );
        let mut sorted = after.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), after.len(), "a song appears twice");
        let mut expected = before.to_vec();
        expected.sort_unstable();
        assert_eq!(sorted, expected, "the membership changed, not just the order");
    }

    #[test]
    fn moving_a_song_up_one_place_swaps_it_with_the_one_above() {
        let (setlists, id) = five();
        setlists.reorder(id, 2, 1);
        assert_eq!(order(setlists, id), vec![10, 30, 20, 40, 50]);
        positions_are_dense_and_unique(&[10, 20, 30, 40, 50], &order(setlists, id));
    }

    #[test]
    fn moving_a_song_down_one_place_swaps_it_with_the_one_below() {
        let (setlists, id) = five();
        setlists.reorder(id, 2, 3);
        assert_eq!(order(setlists, id), vec![10, 20, 40, 30, 50]);
        positions_are_dense_and_unique(&[10, 20, 30, 40, 50], &order(setlists, id));
    }

    #[test]
    fn moving_a_song_to_the_top_closes_the_gap_behind_it() {
        let (setlists, id) = five();
        setlists.reorder(id, 3, 0);
        // Take-then-insert, not swap: everything it passed shuffles down one.
        assert_eq!(order(setlists, id), vec![40, 10, 20, 30, 50]);
        positions_are_dense_and_unique(&[10, 20, 30, 40, 50], &order(setlists, id));
    }

    #[test]
    fn moving_a_song_to_the_bottom_closes_the_gap_ahead_of_it() {
        let (setlists, id) = five();
        setlists.reorder(id, 1, 4);
        assert_eq!(order(setlists, id), vec![10, 30, 40, 50, 20]);
        positions_are_dense_and_unique(&[10, 20, 30, 40, 50], &order(setlists, id));
    }

    #[test]
    fn moving_a_song_to_where_it_already_is_changes_nothing() {
        let (setlists, id) = five();
        setlists.reorder(id, 2, 2);
        assert_eq!(order(setlists, id), vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn a_move_off_either_end_of_the_set_is_refused_rather_than_clamped() {
        // The screen never asks for one — the up arrow is dead on the first row
        // and the down arrow on the last — but a clamp would silently turn a
        // bug into a move, and a move is a persisted write.
        let (setlists, id) = five();
        setlists.reorder(id, 5, 0);
        setlists.reorder(id, 0, 5);
        setlists.reorder(id, 99, 99);
        assert_eq!(order(setlists, id), vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn reordering_a_missing_setlist_is_a_no_op() {
        let (setlists, id) = five();
        setlists.reorder(999, 0, 1);
        assert_eq!(order(setlists, id), vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn walking_a_song_from_the_bottom_to_the_top_one_tap_at_a_time_arrives() {
        // What Move up actually does when it is held down: four separate
        // writes, each of which has to leave a valid order behind it.
        let (setlists, id) = five();
        for from in (1..5).rev() {
            setlists.reorder(id, from, from - 1);
            positions_are_dense_and_unique(&[10, 20, 30, 40, 50], &order(setlists, id));
        }
        assert_eq!(order(setlists, id), vec![50, 10, 20, 30, 40]);
    }

    // ── removal and its undo (C6) ───────────────────────────────────────

    #[test]
    fn removing_a_song_remembers_where_it_was() {
        let (setlists, id) = five();
        setlists.remove_song(id, 30);
        assert_eq!(order(setlists, id), vec![10, 20, 40, 50]);
        assert_eq!(
            setlists.last_removal.get(),
            Some(Removal {
                setlist: id,
                song: 30,
                index: 2
            })
        );
    }

    #[test]
    fn undo_puts_the_song_back_in_its_old_place_not_on_the_end() {
        let (setlists, id) = five();
        setlists.remove_song(id, 20);
        assert!(setlists.undo_removal());
        assert_eq!(order(setlists, id), vec![10, 20, 30, 40, 50]);
        assert_eq!(setlists.last_removal.get(), None, "the offer is spent");
    }

    #[test]
    fn undo_restores_the_first_and_the_last_song_too() {
        let (setlists, id) = five();
        setlists.remove_song(id, 10);
        assert!(setlists.undo_removal());
        assert_eq!(order(setlists, id), vec![10, 20, 30, 40, 50]);

        setlists.remove_song(id, 50);
        assert!(setlists.undo_removal());
        assert_eq!(order(setlists, id), vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn any_other_write_to_a_running_order_spends_the_undo() {
        // The remembered index describes an order that no longer exists once
        // something else has moved, so the offer has to go with it.
        let (setlists, id) = five();
        setlists.remove_song(id, 30);
        setlists.reorder(id, 0, 3);
        assert_eq!(setlists.last_removal.get(), None);
        assert!(!setlists.undo_removal());
        assert_eq!(order(setlists, id), vec![20, 40, 50, 10]);
    }

    #[test]
    fn undo_declines_when_the_song_has_already_come_back_by_itself() {
        let (setlists, id) = five();
        setlists.remove_song(id, 30);
        // Re-added through the picker, which puts it on the end — and which
        // also clears the offer. Both belts: assert the store refuses even if
        // the removal is put back by hand.
        let removal = setlists.last_removal.get().unwrap();
        setlists.add_songs(id, &[30]);
        setlists.last_removal.set(Some(removal));
        assert!(!setlists.undo_removal());
        assert_eq!(
            order(setlists, id),
            vec![10, 20, 40, 50, 30],
            "no duplicate membership"
        );
    }

    #[test]
    fn undo_declines_when_the_setlist_has_since_been_deleted() {
        let (setlists, id) = five();
        setlists.remove_song(id, 30);
        setlists.delete(id);
        assert!(!setlists.undo_removal());
        assert_eq!(setlists.last_removal.get(), None);
    }

    #[test]
    fn there_is_nothing_to_undo_before_anything_has_been_removed() {
        let (setlists, _) = five();
        assert!(!setlists.undo_removal());
    }

    #[test]
    fn removing_a_song_that_is_not_in_the_set_leaves_no_undo_behind() {
        let (setlists, id) = five();
        setlists.remove_song(id, 999);
        assert_eq!(order(setlists, id), vec![10, 20, 30, 40, 50]);
        assert_eq!(setlists.last_removal.get(), None);
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
