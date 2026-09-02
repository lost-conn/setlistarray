use std::path::PathBuf;

use rinch::prelude::*;

use crate::model::{Attachment, AttachmentId, SongId};
use crate::store::Storage;

/// Attachment *metadata*. The bytes live in `<data>/attachments/<id>/` and are
/// never loaded to render a row — the startup scan does not even ask the
/// database for the extracted `body`, so a list row cannot reach one by
/// accident. [`body`](Self::body) fetches it, once, for something about to show
/// it.
///
/// ## This store does not attach anything
///
/// A chart belongs to exactly one song — that is what `Attachment.song` and its
/// cascade say in the schema — and three of the song's own fields
/// (`attachments`, `primary_attachment`, and by extension "does this song have
/// a chart") move whenever a chart is added or removed. Doing half of that here
/// and half of it in [`SongsStore`](crate::store::SongsStore) would put the
/// "first chart added becomes primary" rule in two places and leave a way to
/// skip it.
///
/// So the mutating half of this store is `pub(super)` and its only callers are
/// `SongsStore::attach` and `SongsStore::detach`. Everything a screen, a
/// capture or a PDF import needs to *read* — the row, the directory, the body,
/// the size — is public and stays here. The dependency runs one way, songs to
/// attachments, and there is nothing an attachment needs to know about a song.
#[derive(Clone, Copy)]
pub struct AttachmentsStore {
    pub items: Signal<Vec<Attachment>>,
    next_id: Signal<AttachmentId>,
    storage: Storage,
}

impl AttachmentsStore {
    pub fn new(seed: Vec<Attachment>) -> Self {
        Self::restored(Storage::in_memory(), seed)
    }

    pub fn restored(storage: Storage, items: Vec<Attachment>) -> Self {
        let next = items.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        Self {
            items: Signal::new(items),
            next_id: Signal::new(next),
            storage,
        }
    }

    pub fn get(self, id: AttachmentId) -> Option<Attachment> {
        self.items.get().into_iter().find(|a| a.id == id)
    }

    /// Replace every attachment's metadata in memory — card I2's import,
    /// through `crate::store::reload_all`. `next_id` is re-derived the way
    /// [`restored`](Self::restored) derives it at startup, for the same
    /// reason [`SongsStore::reload`](crate::store::SongsStore::reload) gives.
    /// The bytes on disk this list points at have already moved by the time
    /// this runs — the swap that replaced `<data>/attachments/` wholesale is
    /// `crate::import::Staged::install`'s job, not this one's.
    pub fn reload(self, items: Vec<Attachment>) {
        let next = items.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        self.items.set(items);
        self.next_id.set(next);
    }

    pub fn many(self, ids: &[AttachmentId]) -> Vec<Attachment> {
        let all = self.items.get();
        ids.iter()
            .filter_map(|id| all.iter().find(|a| a.id == *id).cloned())
            .collect()
    }

    pub fn total_bytes(self) -> u64 {
        self.items.get().iter().map(|a| a.bytes_on_disk).sum()
    }

    /// Where this attachment's file and anything derived from it live.
    pub fn directory(self, id: AttachmentId) -> Option<PathBuf> {
        Some(self.storage.repo()?.dir().attachment(id as u64))
    }

    /// The extracted text, read on demand. Deliberately not held in the list:
    /// a captured page's text is measured in megabytes and no row shows it.
    ///
    /// The card on song detail is not a row, and this is what feeds it. It
    /// costs one object read, so a caller inside a reactive closure should
    /// call it once per redraw and not once per line.
    pub fn body(self, id: AttachmentId) -> Option<String> {
        match self.storage.repo() {
            Some(repo) => repo.attachment_body(id).ok().flatten(),
            // In memory there was never anywhere else for it to be.
            None => self.get(id).and_then(|a| a.body),
        }
    }

    /// Change an attachment's metadata: the page count a rasteriser worked out
    /// (D4), the real size on disk once a capture has finished writing (E2).
    ///
    /// Public because those are producers rather than owners — neither changes
    /// which song holds the chart or which chart is primary, so neither needs
    /// to go through `SongsStore`.
    pub fn update(self, id: AttachmentId, f: impl FnOnce(&mut Attachment)) -> bool {
        let Some(mut attachment) = self.get(id) else {
            return false;
        };
        f(&mut attachment);
        attachment.id = id;
        if !self
            .storage
            .write("saving a chart", |repo| repo.save_attachment(&attachment))
        {
            return false;
        }
        // Projected *before* the update, not inside it. `Signal::update` holds
        // the signal store's `RefCell` borrowed for as long as its closure
        // runs, and `strip_body` asks `Storage` whether there is a repository —
        // which is another signal read. Doing that from inside here panics
        // with "RefCell already mutably borrowed", and it took until card D2
        // for anything to call this on a persistent library and find out.
        // `insert` above already had the order right.
        let listed = strip_body(attachment, self.storage);
        self.items.update(move |list| {
            if let Some(slot) = list.iter_mut().find(|a| a.id == id) {
                *slot = listed;
            }
        });
        true
    }

    /// Say that the *files* behind an attachment have changed, without changing
    /// its row.
    ///
    /// D4 needs this and the ordering is the reason. `SongsStore::attach` mints
    /// the row and its directory in one step, and the row is in this signal —
    /// and therefore on screen — the instant it is minted, which is necessarily
    /// *before* the producer has written a single byte into that directory. So
    /// the first draw of a newly imported PDF happens against an empty
    /// directory: `song_detail` looks for `page-1.png`, finds nothing, and says
    /// "No page preview yet." Then `pdf::import` writes the file and rasterises
    /// page one — and nothing in this store has changed, so nothing redraws, and
    /// that sentence sits under a chart whose picture is on disk until the user
    /// happens to leave the screen and come back.
    ///
    /// A signal write with no value change is exactly the right size for that:
    /// it costs one redraw and no database round trip, because the row on disk
    /// is already correct and only the pixels beside it are new. Callers that
    /// changed the row itself want [`update`](Self::update) instead.
    pub fn files_changed(self, id: AttachmentId) {
        // Checked before the write, not inside it: `Signal::update` notifies
        // whatever its closure did or did not do, so an `if let` in there would
        // still have redrawn every card on the screen for an id this store has
        // never heard of. The read is also a subscription-free `get` on the same
        // signal, which is safe here for the reason `update` records below — the
        // borrow is released before the write begins.
        if !self.items.get().iter().any(|a| a.id == id) {
            return;
        }
        self.items.update(|_| {});
    }

    /// Create the row, link it to its song, and make the directory the bytes go
    /// in. All three or none: if the directory cannot be made, nothing is
    /// attached.
    ///
    /// Only [`SongsStore::attach`](crate::store::SongsStore::attach) calls this,
    /// because on its own it leaves the song's `attachments` list one behind.
    pub(super) fn insert(self, song: SongId, attachment: Attachment) -> Option<AttachmentId> {
        let mut attachment = attachment;
        let minted = self.storage.create(
            "attaching a chart",
            || self.next_id.get(),
            |repo| repo.create_attachment(song, &attachment),
        )?;

        attachment.id = minted;
        let listed = strip_body(attachment, self.storage);
        self.items.update(|list| list.push(listed));
        if minted >= self.next_id.get() {
            self.next_id.set(minted + 1);
        }
        Some(minted)
    }

    /// Takes the directory with it. A row pointing at bytes that are gone is
    /// worse than a directory nothing points at, so the row goes first.
    ///
    /// `false` means the write did not land and nothing was removed.
    pub(super) fn forget(self, id: AttachmentId) -> bool {
        if !self
            .storage
            .write("removing a chart", |repo| repo.delete_attachment(id))
        {
            return false;
        }
        self.items.update(|list| list.retain(|a| a.id != id));
        true
    }

    /// Drop rows the database has already taken, without asking it to take
    /// them again.
    ///
    /// Deleting a song cascades to its charts and their directories inside
    /// `Repo::delete_song`; this list is the one place that cannot hear about
    /// it, because a signal has no way to learn what a delete policy did. In
    /// memory only, and deliberately so — calling `forget` here would ask the
    /// database to delete rows that no longer exist.
    pub(super) fn forget_all(self, ids: &[AttachmentId]) {
        if ids.is_empty() {
            return;
        }
        self.items
            .update(|list| list.retain(|a| !ids.contains(&a.id)));
    }
}

/// The list holds metadata; the body stays on disk behind
/// [`AttachmentsStore::body`]. A typed chart arrives here *carrying* its text —
/// that is how it gets written — and keeping it in the signal afterwards would
/// undo the projection the startup scan is careful about, one attachment at a
/// time, until the next launch quietly fixed it.
///
/// With no database there is nowhere else for it to live, so it stays.
fn strip_body(mut attachment: Attachment, storage: Storage) -> Attachment {
    if storage.is_persistent() {
        attachment.body = None;
    }
    attachment
}
