use rinch::prelude::*;

use crate::model::SetlistId;

/// Performance mode: which set is playing and where in it we are.
#[derive(Clone, Copy)]
pub struct PlaybackStore {
    pub setlist: Signal<Option<SetlistId>>,
    pub index: Signal<usize>,
    /// Whether *this* set is holding the screen on.
    ///
    /// Session state and not a setting, which is the distinction card H1 had to
    /// draw when it built the Settings row beside it. `SettingsStore::keep_awake`
    /// is the answer the user gave once and expects to keep; this is the answer
    /// for the gig currently on, flipped from `1o`'s own bottom bar by somebody
    /// whose set is running long and whose battery is not. Turning it off
    /// mid-song must not silently rewrite the preference, and the preference
    /// changing between sets must not be ignored — so the two are separate
    /// signals and [`start`](Self::start) is the one place they meet.
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

    /// Begin a set, with the keep-awake preference the user has actually set.
    ///
    /// The preference arrives as an argument rather than being read from a
    /// `SettingsStore` held on this struct, and that is the point: the compiler
    /// then asks the question at every entry into performance mode, and a third
    /// "Play set" button added a year from now cannot quietly inherit whatever
    /// the last gig was left on. There are two today — the play glyph on a
    /// setlist card and the button at the foot of setlist detail — and before
    /// H1 both left this signal at the `true` it was constructed with, so the
    /// stored preference was written, read back at launch and then never
    /// consulted by anything.
    pub fn start(self, setlist: SetlistId, keep_awake: bool) {
        self.setlist.set(Some(setlist));
        self.index.set(0);
        self.keep_awake.set(keep_awake);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The Settings row is only worth having if starting a set actually asks
    /// it. Before card H1 nothing read `SettingsStore::keep_awake` at all — it
    /// was written, restored on launch, and then this signal sat on the `true`
    /// it was constructed with for the life of the app.
    #[test]
    fn starting_a_set_takes_the_keep_awake_preference_it_is_given() {
        let playback = PlaybackStore::new();
        playback.start(7, false);
        assert!(!playback.keep_awake.get());

        // And the other way, so the test is not passing on the constructor.
        playback.start(7, true);
        assert!(playback.keep_awake.get());
    }

    /// Flipping the bar's toggle mid-song changes this set and nothing else;
    /// the next set starts from the preference again. That is the whole reason
    /// the session value and the stored one are two signals.
    #[test]
    fn a_toggle_during_one_set_does_not_survive_into_the_next() {
        let playback = PlaybackStore::new();
        playback.start(1, true);
        playback.keep_awake.set(false);
        playback.stop();

        playback.start(2, true);
        assert!(playback.keep_awake.get());
    }
}
