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
        self.items.update(|list| {
            if let Some(slot) = list.iter_mut().find(|a| a.id == id) {
                *slot = strip_body(attachment, self.storage);
            }
        });
        true
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
