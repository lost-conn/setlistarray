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
pub use library_view::{Density, Group, GroupBy, LibraryViewStore, SortDir, SortField};
pub use nav::{NavStore, Route, Tab};
pub use playback::PlaybackStore;
pub use settings::{AccentChoice, PerformanceTheme, SettingsStore};
pub use setlists::SetlistsStore;
pub use songs::{SongsStore, now_millis};
pub use storage::{Fault, Storage};
