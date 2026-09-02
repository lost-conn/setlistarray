//! The import flow shared by two doors — card I2's Settings → Backup row and
//! the first-run screen's secondary line, both of which end up asking for the
//! same file and doing the same thing with it once they have it.
//!
//! Not a screen, and has no route — the same shape `captured_page` and
//! `chart_surface` already use for a component two screens need to agree
//! about, applied here to a *behaviour* two screens need to agree about
//! rather than to a picture.
//!
//! ## Where "say replace, before it happens" lives
//!
//! The card's rule is not negotiable: a person has to be told, in the UI,
//! that this replaces their library, before anything does. [`confirm_strip`]
//! is where that sentence lives, and it runs **before the file picker ever
//! opens** — the confirmation names the operation ("this replaces
//! everything on this device") rather than the specific backup about to
//! land, because at the point someone reads it no file has been chosen yet.
//! That is the more conservative of the two shapes this could have taken:
//! the alternative — pick the file first, validate it, *then* show counts
//! and ask "replace with this?" — would say more (the exact number of
//! songs a specific backup holds) at the cost of a filesystem round trip
//! nobody asked for if the answer was going to be "no" anyway. Naming the
//! operation up front lets anyone who was not planning to replace their
//! library back out without touching a file dialog at all.
//!
//! There is no modal anywhere in this app (`chart_editor`'s own
//! leave-without-saving strip is the precedent this one's markup is copied
//! from almost verbatim) — an inline strip that pushes the row's own content
//! down rather than covering the screen, with the same two-word buttons in
//! the same two colours.
//!
//! ## What happens once a file is chosen
//!
//! [`start`] is the whole pipeline, run inside `crate::picker::pick`'s
//! callback: [`crate::import::stage`] validates and extracts without
//! touching the real library (see that module's own header for the five
//! checks); on success, `Storage::close` releases this process's hold on the
//! *current* library's `db/` — required before `Staged::install` can rename
//! it out of the way — `Staged::install` performs the swap, and
//! `crate::store::reload_all` runs whichever way `install` returned, because
//! a swap that failed and rolled itself back has still left `Storage` closed
//! and every store has still not been told the library it is showing might
//! have changed underneath it. Reopening is not conditional on success; only
//! the [`ImportStatus`] shown afterwards is.

use rinch::prelude::*;

use crate::db::DataDir;
use crate::export::Manifest;
use crate::picker::{PickRequest, Picked};
use crate::store::{AttachmentsStore, LibraryViewStore, SettingsStore, SetlistsStore, SongsStore, Storage};
use crate::theme::T_META;

/// What to ask the platform picker for. Extensions are honoured by the
/// desktop dialog and ignored by Android (`crate::picker`'s own header) —
/// which is why `crate::import::stage` checks the bytes rather than trusting
/// the name, exactly as `crate::pdf::PICK_REQUEST` already does for a chart.
pub const PICK_REQUEST: PickRequest = PickRequest {
    title: "Import backup",
    kind: "Zip",
    extensions: &["zip"],
    max_bytes: crate::import::MAX_BYTES,
};

/// What the last import attempt came back with. Mirrors
/// `crate::screens::settings::ExportStatus` on purpose — three outcomes, the
/// same reasons to keep them apart — with `Done` carrying the manifest
/// instead of a byte count, because "this worked" is a count of songs and
/// setlists to a person restoring their book, not a size on disk.
#[derive(Clone, Debug, PartialEq)]
pub enum ImportStatus {
    Done(Manifest),
    Cancelled,
    Failed(String),
}

/// Same rule `crate::screens::settings::export_status_color` follows:
/// muted for nothing-yet and for a cancel, danger for a failure, accent for
/// the one time this is good news.
pub fn status_color(status: Option<ImportStatus>) -> &'static str {
    match status {
        None | Some(ImportStatus::Cancelled) => "var(--sla-muted)",
        Some(ImportStatus::Failed(_)) => "var(--sla-danger)",
        Some(ImportStatus::Done(_)) => "var(--sla-accent)",
    }
}

pub fn status_note(status: Option<ImportStatus>) -> String {
    match status {
        None => String::new(),
        Some(ImportStatus::Cancelled) => "Cancelled".to_string(),
        Some(ImportStatus::Failed(why)) => why,
        Some(ImportStatus::Done(manifest)) => format!(
            "Replaced — {} song{}, {} setlist{}",
            manifest.song_count,
            if manifest.song_count == 1 { "" } else { "s" },
            manifest.setlist_count,
            if manifest.setlist_count == 1 { "" } else { "s" },
        ),
    }
}

/// The one question this flow asks, and the only thing that has to run
/// before [`start`] does. `confirming` is the screen's own signal — both
/// buttons reset it to `false` the moment either is tapped, so the strip
/// never lingers once its question has been answered either way.
///
/// Always mounted, `display: none` when `confirming` is false — the same
/// shape `crate::app`'s own bottom sheets use, rather than an `if` that
/// removes the element from the tree, so that a caller can drop this into a
/// column of other rows without it being the one child whose presence
/// changes the layout around it on every tap.
pub fn confirm_strip(
    scope: &mut RenderScope,
    confirming: Signal<bool>,
    proceed: impl Fn() + Copy + 'static,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            style: {move || format!(
                "display: {}; padding: 12px 14px; background: var(--sla-fill); \
                 border-radius: 10px; margin: 4px 0 10px; \
                 align-items: center; gap: 10px; flex-wrap: wrap;",
                if confirming.get() { "flex" } else { "none" },
            )},
            span { style: {format!("{T_META} flex: 1; min-width: 180px;")},
                "Replace this device's whole library with a backup file? \
                 Nothing is merged — everything here now is gone, and this cannot be undone."
            }
            div {
                onclick: move || confirming.set(false),
                style: "font-weight: 600; font-size: 15px; color: var(--sla-ink-2); padding: 8px 10px;",
                "Cancel"
            }
            div {
                onclick: move || { confirming.set(false); proceed(); },
                style: "font-weight: 600; font-size: 15px; color: var(--sla-danger); padding: 8px 10px;",
                "Choose backup file…"
            }
        }
    }
}

/// Pick a file, and if one is chosen, validate it, replace the library with
/// it, and refill every store from what is now on disk. See the module
/// header for the sequencing argument; `status` is written exactly once per
/// attempt, at the end of whichever branch it took.
pub fn start(
    storage: Storage,
    songs: SongsStore,
    setlists: SetlistsStore,
    attachments: AttachmentsStore,
    view: LibraryViewStore,
    settings: SettingsStore,
    status: Signal<Option<ImportStatus>>,
) {
    status.set(None);
    crate::picker::pick(PICK_REQUEST, move |picked| match picked {
        Picked::Cancelled => status.set(Some(ImportStatus::Cancelled)),
        Picked::Failed(why) => status.set(Some(ImportStatus::Failed(why))),
        Picked::Chose(file) => {
            let dir = DataDir::current();
            match crate::import::stage(&file.bytes, &dir) {
                Err(e) => status.set(Some(ImportStatus::Failed(e.message()))),
                Ok(staged) => {
                    let manifest = staged.manifest().clone();
                    // Releasing the lock on the *current* library has to
                    // happen before `install` can rename `db/` out of the
                    // way — see `crate::import::Staged::install`'s own doc
                    // comment, which names this exact call as the precondition
                    // it cannot check for itself.
                    storage.close();
                    let result = staged.install(&dir);
                    // Reopen unconditionally: whether the swap landed or
                    // `install` rolled itself back, `Storage` is closed right
                    // now and every store above it is still showing whatever
                    // was in memory before this line ran. `reload_all` is
                    // correct either way — it opens whatever is actually at
                    // `dir`, which is the import on success and the original
                    // library, restored, on a rolled-back failure.
                    crate::store::reload_all(storage, songs, setlists, attachments, view, settings, &dir);
                    match result {
                        Ok(()) => status.set(Some(ImportStatus::Done(manifest))),
                        Err(e) => status.set(Some(ImportStatus::Failed(e.message()))),
                    }
                }
            }
        }
    });
}
