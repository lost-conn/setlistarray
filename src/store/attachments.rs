use std::path::PathBuf;

use rinch::prelude::*;

use crate::model::{Attachment, AttachmentId, SongId};
use crate::store::Storage;

/// Attachment *metadata*. The bytes live in `<data>/attachments/<id>/` and are
/// never loaded to render a row — the startup scan does not even ask the
/// database for the extracted `body`, so a list row cannot reach one by
/// accident. [`body`](Self::body) fetches it, once, for something about to show
/// it.
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

    /// Attach to a song. The row, the link and the directory the bytes go in
    /// are one operation: if the directory cannot be made, nothing is attached.
    pub fn add(self, song: SongId, attachment: Attachment) -> Option<AttachmentId> {
        let mut attachment = attachment;
        let minted = self.storage.create(
            "attaching a chart",
            || self.next_id.get(),
            |repo| repo.create_attachment(song, &attachment),
        )?;

        attachment.id = minted;
        self.items.update(|list| list.push(attachment));
        if minted >= self.next_id.get() {
            self.next_id.set(minted + 1);
        }
        Some(minted)
    }

    /// Takes the directory with it. A row pointing at bytes that are gone is
    /// worse than a directory nothing points at, so the row goes first.
    pub fn delete(self, id: AttachmentId) {
        if !self
            .storage
            .write("removing a chart", |repo| repo.delete_attachment(id))
        {
            return;
        }
        self.items.update(|list| list.retain(|a| a.id != id));
    }

    /// Where this attachment's file and anything derived from it live.
    pub fn directory(self, id: AttachmentId) -> Option<PathBuf> {
        Some(self.storage.repo()?.dir().attachment(id as u64))
    }

    /// The extracted text, read on demand. Deliberately not held in the list:
    /// a captured page's text is measured in megabytes and no row shows it.
    pub fn body(self, id: AttachmentId) -> Option<String> {
        match self.storage.repo() {
            Some(repo) => repo.attachment_body(id).ok().flatten(),
            // In memory there was never anywhere else for it to be.
            None => self.get(id).and_then(|a| a.body),
        }
    }
}
