//! Restoring the library from one of card I1's zips — card I2.
//!
//! ## The decision that is not this module's to make
//!
//! A restore replaces the library; it does not merge with it. That was
//! settled before a line of this file was written, for a reason worth
//! restating because it is the reason this module is as small as it is: I1's
//! zip is `Database::backup_to` of the LSM plus the attachments directory, a
//! whole-library snapshot with no notion of "since when." Merging two of
//! those has no honest answer for the song edited in both, and answering it
//! would mean reading rows out of a second database and reconciling ids
//! against the live one — a different card, not a bigger version of this one.
//! So this module's entire job is: prove the zip is a real backup, and then
//! make it the only library on disk.
//!
//! ## Validate before touching anything
//!
//! [`stage`] runs four checks against the bytes the picker handed back,
//! entirely in memory, before a single byte reaches disk:
//!
//! 1. **The file opens as a zip** ([`ImportError::NotAZip`]) — `zip::ZipArchive::new`
//!    reads the central directory without touching a file system at all.
//! 2. **`setlistarray.json` is present** ([`ImportError::NoManifest`]) and
//! 3. **parses** ([`ImportError::ManifestUnreadable`]) — [`crate::export::Manifest::parse`],
//!    written by I1 for exactly this call.
//! 4. **`format_version` is one this build understands**
//!    ([`ImportError::FutureFormatVersion`]) — refused rather than guessed at,
//!    because a build that pressed on regardless would be promising to read a
//!    shape it has never seen.
//!
//! A fifth check — **the expected `db/` entries are present**
//! ([`ImportError::MissingDbEntries`]) — is the last one still payable without
//! writing anything: every zip I1 ever produces has `db/wal.log` at its root
//! (`crate::export`'s own module header, "always copies `wal.log`, never
//! links it" — the one file a live rhypedb directory is guaranteed to have,
//! empty library or not), so its absence is proof this zip was never one of
//! ours, cheaply, without extracting a byte.
//!
//! Only once all five hold does [`stage`] write anything, and even then not
//! to the real library: it extracts into a directory beside it
//! ([`staging_dir`]) and, as one last check the four above cannot buy —
//! **the extracted copy actually opens** — calls `crate::db::repo::Repo::open`
//! on it and drops the result immediately. A zip that named every right file
//! but whose WAL was truncated by a bad transfer fails right here, in
//! rhypedb's own replay, rather than after the real library has already been
//! moved aside for it. This is what [`ImportError::Corrupt`] means: everything
//! about the zip's *shape* was right and its *contents* were not.
//!
//! [`Staged`] is what a caller holds once every one of those has passed. It
//! is the type this module uses to say "I have proven this, and only this,
//! is safe to install" — there is no way to reach [`Staged::install`] without
//! going through [`stage`] first, and no field on it a caller could use to
//! skip a check.
//!
//! ## The swap, and what a crash leaves behind at each step
//!
//! [`Staged::install`] never touches `dir.path()` itself — only its two
//! children, `db/` and `attachments/` — because on Android `dir.path()` *is*
//! `AndroidApp::internal_data_path()` (`src/android.rs`), a path the OS hands
//! back verbatim on every launch. Renaming that root away would not relocate
//! it: the next launch would ask the OS for the same fixed path and find
//! nothing there. `crate::db::DataDir`'s own two children are what this
//! module is allowed to move, on both platforms alike.
//!
//! Each child is swapped by [`swap_component`]: rename the real directory to
//! an `outgoing` name, then rename the staged directory into the real one's
//! place. **The old library is never deleted until the new one already
//! occupies its path** — `outgoing` sits on disk, complete, for the whole of
//! the second rename, and is only reclaimed (best-effort, see below) after
//! both children have landed. Tracing what a crash — the process dying, not
//! an `Err` coming back, which [`swap_component`] already recovers from on
//! its own — leaves behind at each point:
//!
//! * **Before either rename.** Nothing has moved. `dir`'s `db/` and
//!   `attachments/` are exactly what they were; `staging_dir(dir)` sits beside
//!   them, complete and validated, orphaned garbage the next launch never
//!   reads and a person could delete by hand. The library on the next launch
//!   is the one from before the import, intact.
//! * **Between the two renames of one component** (`db/` renamed to
//!   `outgoing`, the staged copy not yet renamed into `db/`'s place) — the
//!   one gap this module cannot close with two separate `rename()` calls,
//!   because nothing in POSIX renames two directories at once. `dir`'s `db/`
//!   does not exist for the width of one syscall. If the process dies in
//!   exactly that window, the next launch's `crate::db::open` calls
//!   `create_dir_all(dir.database())` and quietly opens a **fresh, empty**
//!   database at that path, because that is what `open` has always done for
//!   a first run and it has no way to tell this apart from one. Nothing is
//!   destroyed by this — the pre-import library sits complete at the
//!   `outgoing` name and the imported one sits complete at whatever
//!   [`staging_dir`] or its remnants are named — but neither is *found*
//!   automatically, and recovering either means renaming it back into place
//!   by hand. This window is two back-to-back `rename()` syscalls with no
//!   computation between them; it is not zero-risk, and it is the one
//!   accepted trade in this design rather than an oversight. A `renameat2`
//!   atomic exchange would close it on Linux, at the cost of an `unsafe`
//!   syscall this crate does not otherwise need and a mechanism Android's
//!   filesystem support is not something this app has tested; that is worth
//!   revisiting the day this risk stops being theoretical, not before.
//! * **`db/` swapped, `attachments/` swap failed.** [`Staged::install`] does
//!   not leave this half-done: it calls [`unswap_component`] on `db/` before
//!   returning, putting the pre-import `db/` back and the staged one back
//!   where it came from, so an ordinary failure (permissions, a full disk on
//!   the second rename) reports [`ImportError::SwapFailed`] and leaves the
//!   library exactly as it was — this is the failure path a plain `Err`
//!   return takes, not a crash, and it is fully recovered rather than merely
//!   documented.
//! * **Both renames of both components succeed.** The import has happened.
//!   What remains is reclaiming `outgoing`'s two directories and the now-empty
//!   staging root, which [`Staged::install`] does with `remove_dir_all` and
//!   ignores the result of: failing to reclaim disk space is not a reason to
//!   tell someone their restored library did not restore.
//!
//! ## The one thing this module cannot do by itself
//!
//! [`Staged::install`] assumes nothing in this process still has `dir`'s
//! `db/` open. It cannot enforce that — closing the live `Arc<Repo>` lives on
//! `crate::store::Storage`, a type this module does not depend on so that it
//! stays testable without a window (`export.rs`'s own module header makes the
//! same choice for the same reason). The caller in
//! `crate::screens::import_flow` is what closes `Storage` before calling
//! `install` and reopens it (through `crate::store::reload_all`) after,
//! whichever way `install` returns.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::db::repo::Repo;
use crate::db::DataDir;
use crate::export::{Manifest, MANIFEST_FORMAT, MANIFEST_NAME};

/// The most this app will read for a backup. Generous on purpose: a real
/// library's PDFs and saved pages can run to hundreds of megabytes, and this
/// is the one file a user picks rarely and on purpose, unlike
/// `crate::pdf::MAX_BYTES`'s single chart. The Android tax
/// `crate::picker`'s own header describes — three copies in memory on the way
/// in — is the honest reason this is not simply "as large as the zip gets":
/// 300 MB here is very nearly a gigabyte of peak heap on a phone, which is
/// already a real cost for a real restore rather than a safety margin with
/// nothing behind it.
pub const MAX_BYTES: u64 = 300_000_000;

/// The four (five, counting [`stage`]'s own deep-open check) ways a restore
/// can be refused, each with its own [`message`](Self::message) — see the
/// module header for what each one means and where it is caught.
#[derive(Debug)]
pub enum ImportError {
    /// Not readable as a zip at all — `ZipArchive::new` refused before this
    /// module asked it for anything by name.
    NotAZip(String),
    /// A zip, but with no `setlistarray.json` at its root.
    NoManifest,
    /// A manifest entry existed but would not parse.
    ManifestUnreadable(String),
    /// A manifest this build understands the *shape* of less than the file
    /// promises — see [`crate::export::MANIFEST_FORMAT`].
    FutureFormatVersion(u32),
    /// No `db/wal.log` — the one entry a real export always has, from an
    /// empty library or a full one alike.
    MissingDbEntries,
    /// The zip passed every check above and still would not become a working
    /// library — a truncated `wal.log`, a corrupted SST, or any other way the
    /// *contents* disagreed with the *shape*.
    Corrupt(String),
    /// A directory could not be created, removed or read while staging or
    /// swapping — a full disk, a permissions problem, nothing this module
    /// caused.
    Io(std::io::Error),
    /// The rename-based swap in [`Staged::install`] failed. See the module
    /// header's "the swap, and what a crash leaves behind" — an `Err` here
    /// (as opposed to the process dying) means [`Staged::install`] already
    /// rolled back what it could, and the library on disk is the one from
    /// before the import.
    SwapFailed(String),
}

impl ImportError {
    /// Words a person could be shown. See `crate::export::ExportError::message`
    /// for the sibling this mirrors and the same rule it follows: never the
    /// `Debug` form, and never a path under `<data>/`.
    pub fn message(&self) -> String {
        match self {
            ImportError::NotAZip(m) => {
                format!("That file is not a zip this app can read: {m}")
            }
            ImportError::NoManifest => {
                "That zip has no setlistarray.json at its root — it is not a backup this app made.".to_string()
            }
            ImportError::ManifestUnreadable(m) => {
                format!("The backup's manifest could not be read: {m}")
            }
            ImportError::FutureFormatVersion(found) => format!(
                "This backup was made by a newer version of the app (format {found}); \
                 this build only understands format {MANIFEST_FORMAT}."
            ),
            ImportError::MissingDbEntries => {
                "That zip is missing the database files a backup always has — \
                 it is not a whole library."
                    .to_string()
            }
            ImportError::Corrupt(m) => format!("The backup could not be restored: {m}"),
            ImportError::Io(e) => format!("The import could not be prepared: {e}"),
            ImportError::SwapFailed(m) => format!(
                "The library could not be replaced, and nothing changed: {m}"
            ),
        }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for ImportError {}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::Io(e)
    }
}

/// A backup that has passed every check [`stage`] can run without touching
/// the real library, extracted into a directory beside it and waiting to be
/// installed.
///
/// The only way to build one is [`stage`]; the only thing to do with one is
/// [`Staged::install`] it or let it drop. There is no field a caller could
/// read to skip a check this type does not already guarantee was run.
#[derive(Debug)]
pub struct Staged {
    root: PathBuf,
    manifest: Manifest,
}

impl Staged {
    /// What the backup says about itself — for a caller that wants to show
    /// counts or a date, or, once card I3 exists, to compare against
    /// `Preferences::last_export_at`.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn db(&self) -> PathBuf {
        self.root.join("db")
    }

    fn attachments(&self) -> PathBuf {
        self.root.join("attachments")
    }

    /// Make this backup the library at `dir`. See the module header's "the
    /// swap, and what a crash leaves behind at each step" for the full
    /// argument; in short, `db/` and `attachments/` are swapped one at a
    /// time, the directory each swap displaces is kept until both have
    /// succeeded, and a failure on the second undoes the first rather than
    /// leaving a new database beside old attachments.
    ///
    /// **The caller must have already released any open handle onto `dir`'s
    /// `db/`** — this function has no way to check that, and renaming a
    /// directory a live `rhypedb` tree still has open underneath it is not a
    /// state this module is able to reason about. `crate::screens::
    /// import_flow::start` is where that happens: `Storage::close` runs
    /// immediately before this call and `crate::store::reload_all` — which
    /// reopens it — immediately after, on both the success and the failure
    /// path.
    pub fn install(self, dir: &DataDir) -> Result<(), ImportError> {
        let staged_db = self.db();
        let staged_attachments = self.attachments();
        let real_db = dir.database();
        let real_attachments = dir.attachments_root();
        let outgoing_db = outgoing_path(dir, "db");
        let outgoing_attachments = outgoing_path(dir, "attachments");

        swap_component(&real_db, &staged_db, &outgoing_db)
            .map_err(|e| ImportError::SwapFailed(e.to_string()))?;

        if let Err(e) = swap_component(&real_attachments, &staged_attachments, &outgoing_attachments) {
            unswap_component(&real_db, &staged_db, &outgoing_db);
            return Err(ImportError::SwapFailed(e.to_string()));
        }

        // Both children landed. Reclaiming what they displaced — and the now
        // (mostly) empty staging root, which this value's own `Drop` below
        // also tries — is bookkeeping, not part of whether the import
        // happened, so its result is not this function's to report.
        let _ = std::fs::remove_dir_all(&outgoing_db);
        let _ = std::fs::remove_dir_all(&outgoing_attachments);
        Ok(())
    }
}

impl Drop for Staged {
    /// Whether this was installed, refused, or abandoned mid-decision, the
    /// staging directory is this value's own scratch space and nothing else
    /// reads it — the same `RemoveOnDrop` idea `crate::export` uses for its
    /// snapshot, applied to the other end of the same trip. Once
    /// [`install`](Self::install) has run, `root` holds at most empty
    /// directories (its two children were renamed out of it, not copied), so
    /// this is a formality on success and the actual cleanup on every other
    /// path.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Validate a backup entirely from `bytes` — the four checks the module
/// header numbers — then extract it into a fresh directory beside `dir` and
/// prove the extracted copy actually opens. Nothing at `dir` itself is read
/// or written; every failure here leaves the real library exactly as it was
/// found.
pub fn stage(bytes: &[u8], dir: &DataDir) -> Result<Staged, ImportError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| ImportError::NotAZip(e.to_string()))?;

    let manifest_bytes = {
        let mut entry = archive.by_name(MANIFEST_NAME).map_err(|_| ImportError::NoManifest)?;
        let mut buf = Vec::new();
        std::io::copy(&mut entry, &mut buf).map_err(ImportError::Io)?;
        buf
    };
    let manifest = Manifest::parse(&manifest_bytes).map_err(ImportError::ManifestUnreadable)?;
    if manifest.format_version > MANIFEST_FORMAT {
        return Err(ImportError::FutureFormatVersion(manifest.format_version));
    }
    if !archive.file_names().any(|name| name == "db/wal.log") {
        return Err(ImportError::MissingDbEntries);
    }

    let root = staging_dir(dir);
    // A directory of this exact name has never existed before — the counter
    // in `staging_dir` is only ever incremented, never reused — but a prior
    // run of this same process crashing mid-extract into it is not something
    // this line can rule out, so clearing it first costs nothing and buys
    // `create_dir_all` an empty destination either way.
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)?;
    let staged = Staged { root: root.clone(), manifest };

    archive
        .extract(&root)
        .map_err(|e| ImportError::Corrupt(format!("the archive could not be extracted: {e}")))?;

    // The deepest check available without moving a single real file: open
    // the extracted copy as an ordinary rhypedb directory and let it go
    // immediately. Proving it opens is all this needs `Repo::open` for —
    // holding it open would leave a lock on `staged/db` that `install`'s
    // renames below would have to contend with.
    Repo::open(&DataDir::new(root))
        .map_err(|e| ImportError::Corrupt(format!("the extracted library will not open: {e}")))?;

    Ok(staged)
}

/// Move `real` aside to `outgoing`, then move `staged` into `real`'s place.
/// See the module header's "the swap, and what a crash leaves behind" for
/// the gap between these two calls and why it cannot be closed with a plain
/// `rename`.
fn swap_component(real: &Path, staged: &Path, outgoing: &Path) -> std::io::Result<()> {
    std::fs::rename(real, outgoing)?;
    if let Err(e) = std::fs::rename(staged, real) {
        // The second rename failed; put the first one back rather than leave
        // `real` missing. Best-effort: if this also fails, `install`'s own
        // caller sees `SwapFailed` and the un-renamed `outgoing` directory is
        // still on disk, findable by name, holding the original data.
        let _ = std::fs::rename(outgoing, real);
        return Err(e);
    }
    Ok(())
}

/// The inverse of a [`swap_component`] that already completed, used only when
/// swapping the *second* component fails after the first already succeeded —
/// putting the library back exactly as it was rather than leaving a
/// mismatched new-database-with-old-attachments (or the reverse) sitting on
/// disk because one write happened and the other did not.
fn unswap_component(real: &Path, staged: &Path, outgoing: &Path) {
    let _ = std::fs::rename(real, staged);
    let _ = std::fs::rename(outgoing, real);
}

/// A directory beside `dir`'s own children, named so two imports racing in
/// one process (a test running more than one) can never collide — the same
/// pid-plus-counter idiom `crate::export::scratch_snapshot_dir` uses, and
/// `crate::db::scratch` before it.
///
/// Lives *inside* `dir.path()` rather than beside it: on Android `dir.path()`
/// is `AndroidApp::internal_data_path()` itself (see the module header), and
/// nothing about this app's permissions says its *parent* is writable, let
/// alone safe to litter with a second app's scratch files. Everything this
/// module ever creates stays inside the one directory this app already owns
/// outright.
fn staging_dir(dir: &DataDir) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.path().join(format!("import-staged-{}-{n}", std::process::id()))
}

/// Where a real child directory is parked while its replacement is being
/// swapped in — see [`swap_component`]. `which` is `"db"` or `"attachments"`,
/// so the two components' outgoing directories cannot collide with each
/// other even though they share a counter source and a process id.
fn outgoing_path(dir: &DataDir, which: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.path()
        .join(format!("import-outgoing-{which}-{}-{n}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use crate::db::scratch;
    use crate::model::{Attachment, AttachmentKind, Setlist, Song};

    fn chart(title: &str) -> Attachment {
        Attachment {
            id: 0,
            kind: AttachmentKind::Pdf,
            title: title.into(),
            bytes_on_disk: 11,
            page_count: Some(1),
            source_url: None,
            captured_at: None,
            body: None,
            rechecked_at: None,
        }
    }

    /// Builds a small, real backup zip the way `crate::export::build_zip`
    /// does — used by every test below that needs a *valid* zip to mutate or
    /// to prove round-trips through, rather than re-deriving I1's own logic.
    ///
    /// `name` has to be distinct per call site rather than a shared constant:
    /// the test harness runs these in parallel threads of one process, so two
    /// calls sharing a `scratch` name would open the *same* on-disk directory
    /// from two threads at once — not the race this card is about, just two
    /// tests fighting over one scratch library and failing with rhypedb's own
    /// "another process stamped a different owner token" refusal.
    fn a_real_backup(name: &str) -> (Vec<u8>, crate::model::SongId, crate::model::AttachmentId) {
        let dir = scratch(&format!("import-fixture-source-{name}"));
        let repo = Repo::open(&dir).unwrap();
        let song = repo.create_song(&Song::new(0, "Carolina", "M. Ward")).unwrap();
        repo.create_setlist(&Setlist {
            id: 0,
            name: "Porch, Saturday".into(),
            song_ids: Vec::new(),
            last_played: None,
        })
        .unwrap();
        let chart_id = repo
            .create_attachment(song as crate::model::SongId, &chart("carolina.pdf"))
            .unwrap();
        std::fs::write(
            dir.attachment(chart_id).join("carolina.pdf"),
            b"%PDF-1.4 not a real pdf, just bytes to carry across",
        )
        .unwrap();
        let bytes = crate::export::build_zip(&repo).unwrap();
        (bytes, song as crate::model::SongId, chart_id as crate::model::AttachmentId)
    }

    // ── the four checks that never touch a disk ─────────────────────────────

    #[test]
    fn something_that_is_not_a_zip_at_all_is_refused_by_name() {
        let dir = scratch("import-not-a-zip");
        let err = stage(b"this is not a zip file", &dir).unwrap_err();
        assert!(matches!(err, ImportError::NotAZip(_)), "{err:?}");
    }

    #[test]
    fn a_zip_with_no_manifest_is_refused_by_name() {
        let dir = scratch("import-no-manifest");
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            zip.start_file("db/wal.log", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.finish().unwrap();
        }
        let err = stage(&cursor.into_inner(), &dir).unwrap_err();
        assert!(matches!(err, ImportError::NoManifest), "{err:?}");
    }

    #[test]
    fn a_manifest_that_will_not_parse_is_refused_by_name() {
        let dir = scratch("import-bad-manifest");
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file(MANIFEST_NAME, options).unwrap();
            zip.write_all(b"not json").unwrap();
            zip.start_file("db/wal.log", options).unwrap();
            zip.finish().unwrap();
        }
        let err = stage(&cursor.into_inner(), &dir).unwrap_err();
        assert!(matches!(err, ImportError::ManifestUnreadable(_)), "{err:?}");
    }

    #[test]
    fn a_manifest_from_a_newer_format_is_refused_by_name_and_says_which_version() {
        let dir = scratch("import-future-format");
        let manifest = Manifest {
            format_version: MANIFEST_FORMAT + 1,
            app_version: "9.9.9".into(),
            created_at_ms: 0,
            song_count: 0,
            setlist_count: 0,
            attachment_count: 0,
        };
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file(MANIFEST_NAME, options).unwrap();
            zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
            zip.start_file("db/wal.log", options).unwrap();
            zip.finish().unwrap();
        }
        let err = stage(&cursor.into_inner(), &dir).unwrap_err();
        let ImportError::FutureFormatVersion(found) = err else {
            panic!("{err:?}");
        };
        assert_eq!(found, MANIFEST_FORMAT + 1);
        let message = ImportError::FutureFormatVersion(found).message();
        assert!(message.contains(&(MANIFEST_FORMAT + 1).to_string()), "{message}");
        assert!(message.contains(&MANIFEST_FORMAT.to_string()), "{message}");
    }

    #[test]
    fn a_manifest_with_no_database_files_beside_it_is_refused_by_name() {
        let dir = scratch("import-missing-db");
        let manifest = Manifest {
            format_version: MANIFEST_FORMAT,
            app_version: "0.1.0".into(),
            created_at_ms: 0,
            song_count: 0,
            setlist_count: 0,
            attachment_count: 0,
        };
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file(MANIFEST_NAME, options).unwrap();
            zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
            // No `db/wal.log` at all — a zip of something else entirely, or
            // one somebody hand-edited down to just the manifest.
            zip.finish().unwrap();
        }
        let err = stage(&cursor.into_inner(), &dir).unwrap_err();
        assert!(matches!(err, ImportError::MissingDbEntries), "{err:?}");
    }

    /// A zip that got cut off partway through the download it arrived by —
    /// the truncation the card names by name. Cutting bytes off the *end* of
    /// a real zip removes its central directory (`zip::ZipWriter` writes that
    /// last), which is the realistic shape an interrupted transfer takes and
    /// the one `ZipArchive::new` itself refuses, before this module ever asks
    /// it for an entry by name.
    #[test]
    fn a_zip_truncated_by_an_interrupted_transfer_is_refused_not_panicked_on() {
        let dir = scratch("import-truncated");
        let (bytes, _, _) = a_real_backup("truncated");
        let cut = &bytes[..bytes.len() / 2];
        let err = stage(cut, &dir).unwrap_err();
        assert!(matches!(err, ImportError::NotAZip(_)), "{err:?}");
        assert!(!err.message().is_empty());
    }

    /// A zip whose shape is entirely right — a real manifest, a real
    /// `db/wal.log` name in the central directory — but whose file *contents*
    /// were corrupted after the fact. Exactly which of this module's checks
    /// catches it depends on which entry the flipped bytes landed in — the
    /// manifest read, the extraction's own checksum, or `Repo::open`'s replay
    /// — and that is not this test's business to pin down: what matters is
    /// that every one of those paths returns a specific, sayable
    /// [`ImportError`] rather than a panic or a silently-broken "success."
    #[test]
    fn a_zip_whose_bytes_were_corrupted_after_the_fact_is_refused_not_panicked_on() {
        let dir = scratch("import-corrupt-wal");
        let (mut bytes, _, _) = a_real_backup("corrupted-bytes");
        // Flip a run of bytes somewhere past the local file headers, where a
        // real archive's file data lives, so at least one entry's CRC-32 no
        // longer matches what its header claims.
        let start = bytes.len() / 3;
        for byte in bytes.iter_mut().skip(start).take(64) {
            *byte ^= 0xFF;
        }
        let err = stage(&bytes, &dir).unwrap_err();
        assert!(!err.message().is_empty(), "{err:?}");
    }

    // ── the file-level round trip ────────────────────────────────────────────

    /// The test the card calls out as the one that matters most, at the level
    /// this module can prove it without a running app: stage a real backup,
    /// install it over a *different* library, and open what is left with the
    /// same `Repo::open` any launch uses — no special-cased restore reader.
    #[test]
    fn a_staged_backup_installs_and_the_result_opens_as_a_working_library() {
        let (bytes, song_id, chart_id) = a_real_backup("install-target");

        let dir = scratch("import-install-target");
        {
            // A different library already sitting at `dir`, so `install` has
            // something real to replace rather than an empty first run.
            let repo = Repo::open(&dir).unwrap();
            repo.create_song(&Song::new(0, "Should not survive", "Nobody")).unwrap();
        }

        let staged = stage(&bytes, &dir).expect("a real backup stages cleanly");
        assert_eq!(staged.manifest().song_count, 1);
        assert_eq!(staged.manifest().setlist_count, 1);
        assert_eq!(staged.manifest().attachment_count, 1);
        staged.install(&dir).expect("installing a validated backup succeeds");

        let repo = crate::db::restart(|| Repo::open(&dir).ok());
        let songs = repo.songs().unwrap();
        assert_eq!(songs.len(), 1, "replaced, not merged — the other library's song is gone");
        assert_eq!(songs[0].id, song_id);
        assert_eq!(songs[0].title, "Carolina");

        let setlists = repo.setlists().unwrap();
        assert_eq!(setlists.len(), 1);
        assert_eq!(setlists[0].name, "Porch, Saturday");

        let attachments = repo.attachments().unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].title, "carolina.pdf");

        let restored_file = dir.attachment(chart_id as u64).join("carolina.pdf");
        assert_eq!(
            std::fs::read(&restored_file).unwrap(),
            b"%PDF-1.4 not a real pdf, just bytes to carry across"
        );
    }

    /// No trace of the staging directory survives an install, success or
    /// failure — the same "no litter" guarantee `crate::export` holds itself
    /// to for its own scratch snapshot.
    #[test]
    fn installing_a_staged_backup_leaves_no_staging_directory_behind() {
        let (bytes, _, _) = a_real_backup("no-litter");
        let dir = scratch("import-no-litter");
        Repo::open(&dir).unwrap();

        let staged = stage(&bytes, &dir).unwrap();
        let root = staged.root.clone();
        staged.install(&dir).unwrap();

        assert!(!root.exists(), "{root:?} should have been cleaned up");
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("import-"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    /// Refusing to stage cleans up after itself too — a corrupt archive must
    /// not leave a half-extracted copy sitting beside the real library
    /// forever, even though a leftover one is harmless (nothing ever reads
    /// it back).
    #[test]
    fn a_refused_stage_leaves_no_staging_directory_behind() {
        let dir = scratch("import-refused-no-litter");
        let _ = stage(b"not a zip", &dir);
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|name| name.starts_with("import-"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}
