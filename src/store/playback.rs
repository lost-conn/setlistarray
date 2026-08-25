use rinch::prelude::*;

use crate::model::SetlistId;

/// Performance mode: which set is playing and where in it we are.
#[derive(Clone, Copy)]
pub struct PlaybackStore {
    pub setlist: Signal<Option<SetlistId>>,
    pub index: Signal<usize>,
    pub keep_awake: Signal<bool>,
}

impl PlaybackStore {
    pub fn new() -> Self {
        Self {
            setlist: Signal::new(None),
            index: Signal::new(0),
            keep_awake: Signal::new(true),
        }
    }

    pub fn start(self, setlist: SetlistId) {
        self.setlist.set(Some(setlist));
        self.index.set(0);
    }

    pub fn stop(self) {
        self.setlist.set(None);
        self.index.set(0);
    }

    pub fn next(self, len: usize) {
        let i = self.index.get();
        if i + 1 < len {
            self.index.set(i + 1);
        }
    }

    pub fn prev(self) {
        let i = self.index.get();
        if i > 0 {
            self.index.set(i - 1);
        }
    }
}

impl Default for PlaybackStore {
    fn default() -> Self {
        Self::new()
    }
}
