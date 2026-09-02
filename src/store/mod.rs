//! Stores. Each is a `Copy` struct of Signals, registered once in `app()` with
//! `create_store` and pulled anywhere with `use_store::<T>()`.
//!
//! Anything derivable is derived on read — group buckets, cumulative setlist
//! times, "in N setlists", the prep facts — rather than stored.
//!
//! Part of this API is used only by screens that are still wireframes.
#![allow(unused_imports)]

mod attachments;
mod library_view;
mod nav;
mod playback;
mod settings;
mod setlists;
mod songs;
mod storage;

pub use attachments::AttachmentsStore;
pub use library_view::{Density, Filters, Group, GroupBy, LibraryViewStore, SortDir, SortField};
pub use nav::{NavStore, Route, Tab};
pub use playback::PlaybackStore;
pub use settings::{AccentChoice, DefaultTuning, PerformanceTheme, SettingsStore};
pub use setlists::SetlistsStore;
pub use songs::{SongsStore, now_millis};
pub use storage::{Fault, Storage};

/// Refill every store from a library that changed entirely underneath the
/// running app — card I2's import is the one caller, once
/// `crate::import::Staged::install` has finished swapping the imported files
/// onto disk. This is `app()`'s own startup sequence (`Storage::open`, then
/// `storage.load()`, then a `restored` call per store) run a second time in
/// place: `dir` names the same path the app has always used, so nothing about
/// routing, the window, or which stores exist has to change, only what the
/// signals inside them hold.
///
/// The order below does not matter the way it matters in `app()`. There, the
/// order is load-bearing because `SongsStore::restored` *constructs* itself
/// holding the `AttachmentsStore`/`SetlistsStore` handles it was given, so
/// those two have to exist first. Here nothing is being constructed — every
/// store above already exists and already holds the right handles to the
/// others, wired up once at startup and never rewired since — so `reload`
/// only has to overwrite the `Vec` each keeps in its own `Signal`, and doing
/// that to `songs` before `setlists` would work exactly as well. The order
/// here just mirrors `app()`'s anyway, so a reader who knows that sequence
/// recognises this one.
///
/// **`NavStore` and `PlaybackStore` are deliberately not reset here.** The
/// route currently on screen can only be a place import is reachable from —
/// `Route::Settings`, or `Route::Library` while `FirstRun` is showing over
/// it — and neither carries a song or setlist id that this could orphan. Any
/// route that *does* carry one (`SongDetail`, `SetlistDetail`, `Performance`,
/// …) is already covered by the orphaned-route effect `app()` installs, which
/// re-runs the instant `songs.songs` or `setlists.setlists` changes and sends
/// the app back to the current tab's root, stopping a gig on its way out — the
/// same effect that already handles a song menu's Delete. Forcing `nav` back
/// to `Route::Library` here regardless would be a second way of saying what
/// that effect already says, and the risk of the second way is it can drift
/// from the first (an orphaned-route case the effect will always catch could
/// be reached slightly differently through a reset written here by hand).
pub fn reload_all(
    storage: Storage,
    songs: SongsStore,
    setlists: SetlistsStore,
    attachments: AttachmentsStore,
    view: LibraryViewStore,
    settings: SettingsStore,
    dir: &crate::db::DataDir,
) {
    let loaded = storage.reload(dir);
    attachments.reload(loaded.attachments);
    setlists.reload(loaded.setlists);
    songs.reload(loaded.songs);
    view.reload();
    settings.reload();
}
