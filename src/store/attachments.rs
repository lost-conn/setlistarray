use rinch::prelude::*;

use crate::model::{Attachment, AttachmentId};

#[derive(Clone, Copy)]
pub struct AttachmentsStore {
    pub items: Signal<Vec<Attachment>>,
}

impl AttachmentsStore {
    pub fn new(seed: Vec<Attachment>) -> Self {
        Self {
            items: Signal::new(seed),
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
}
