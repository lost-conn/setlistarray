//! Exporting the library as one `.zip` — card I1.
//!
//! Three things go in, preserving the relative paths [`crate::db::DataDir`]
//! already gives them: `db/` (the database), `attachments/` (their bytes),
//! and a manifest at the zip's root, [`MANIFEST_NAME`]. Restoring is meant to
//! be the mirror of that shape — unzip onto a fresh [`DataDir`]'s root and
//! `db/` and `attachments/` land exactly where [`DataDir::database`] and
//! [`DataDir::attachments_root`] already expect them, with nothing to
//! rename. [`the_zip_this_module_builds_restores_into_a_working_library`]
//! is the test that proves that end to end, and it is the one this card
//! calls out as mattering most.
//!
//! ## What rhypedb guarantees about a *live* database, established by reading it
//!
//! The database is open and in use for the whole time this module is
//! building a zip — there is no "pause writes" step anywhere in this app,
//! and card I1 does not ask for one. So the question this module had to
//! answer before writing a byte was: what, exactly, can be copied out of a
//! live rhypedb directory and reopened somewhere else as a working library?
//!
//! **Not a raw copy of `db/`.** A completed write is durable the moment its
//! transaction commits, but "durable" here means "recoverable by replaying
//! the WAL," not "sitting in an SST file" — rhypedb's memtable is an
//! in-memory structure and the WAL is its only durable form until the next
//! flush (`rhypedb-storage/src/lsm.rs:1052-1053`, `flush_locked`'s own
//! comment at `lsm.rs:1062-1069`). A `std::fs::read_dir` walk over `db/`
//! while the WAL is being appended to by the next write has no isolation
//! from that append at all — nothing stops the walk from reading a WAL file
//! mid-write, or reading it, then having a concurrent flush truncate it out
//! from under the copy that is still in flight for a different file in the
//! same walk.
//!
//! **`Database::backup_to`, instead** (`rhypedb-engine/src/database.rs:598`,
//! and see this crate's own `src/db/mod.rs` for why there is exactly one
//! writer to worry about). It delegates the load-bearing part to
//! `LsmTree::snapshot_to` (`database.rs:600`, `rhypedb-storage/src/lsm.rs:1150`),
//! which:
//!
//! * **Flushes the active memtable first** (`lsm.rs:1155`, calling
//!   `flush_locked` — the non-reentrant twin of `flush()`, used here because
//!   `snapshot_to` already holds the lock `flush()` would try to take again).
//!   Every committed write not yet in an SST becomes one before anything is
//!   copied, so nothing this module captures depends on WAL replay to be
//!   readable — though replay still works if it ever needed to (see below).
//! * **Holds `flush_lock.write()` for the whole pass** (`lsm.rs:1152`,
//!   `1189`), which excludes every writer and any concurrent flush for as
//!   long as the snapshot takes. In this app that means: the one thread that
//!   ever calls `Storage::write` cannot land a row while a backup is being
//!   read out from under it, because both go through the same lock.
//! * **Holds `compaction_mutex` across the SST enumeration** (`lsm.rs:1157`),
//!   so rhypedb's own background compaction worker — the one thing in this
//!   process besides the app's writer that touches the SST set — cannot
//!   splice a merge into the list mid-copy.
//! * **Hard-links each immutable SST** rather than copying its bytes
//!   (`lsm.rs:1178-1196`) — safe because an SST is written once and only ever
//!   unlinked, never mutated, so a hard link pins the same bytes a later
//!   compaction might otherwise remove the only other reference to.
//! * **Always copies `wal.log`, never links it** (`lsm.rs:1198-1204`): the
//!   WAL *is* mutable — the next flush truncates and recreates it — so a
//!   hard link here would alias the live file and let a write made after the
//!   snapshot corrupt a backup that claims to predate it.
//!
//! The directory `backup_to` produces is not a special format: it is exactly
//! the `sst/` + `wal.log` shape `LsmTree::open` already knows how to read
//! (`lsm.rs:189-193`, which creates `sst/` and discovers whatever `.sst`
//! files are already there — nothing else is required). That is confirmed
//! rather than assumed by [`the_zip_this_module_builds_restores_into_a_working_library`]
//! below, which unzips into a bare directory and opens a [`Database`] from
//! it with no special-cased restore path — the same [`crate::db::open`] the
//! app's normal launch calls.
//!
//! One thing `backup_to` returns and this module deliberately ignores: its
//! own [`rhypedb_engine::database::BackupManifest`]. Reading `backup_to`'s
//! doc comment (`database.rs:589-597`) says writing that manifest to disk is
//! "deliberately the CALLER's job," as a completeness sentinel for an
//! operator's own backup tooling — but nothing in `LsmTree::open` ever reads
//! it back (confirmed by reading the whole of `open`, which discovers SSTs
//! from `sst/`'s own directory listing and replays whatever `wal.log`
//! contains), so it buys this app no correctness it does not already have
//! from [`Manifest`], which is this card's own answer to "how does I2 check
//! a zip before touching anything."
//!
//! ## The manifest, and why it exists at all
//!
//! Card I2 has to "validate before touching anything," and there is nothing
//! in `db/`'s SSTs a caller can honestly validate without opening the whole
//! database first — which is the one thing I2 is not supposed to do to a zip
//! it has not yet decided to trust. [`Manifest`] is designed to be the
//! smallest thing that answers "should I2 go any further with this file":
//! a format version I2 can refuse outright if it is higher than this build
//! understands, and the three counts I2 can hold against what it is about
//! to overwrite or merge before it does either. **I2 is this manifest's
//! consumer** — nothing in this crate reads it back yet, and [`Manifest::parse`]
//! exists for I2 to call rather than for this module's own use.
//!
//! Attachment bytes are not walked for their own manifest entry the way `db/`
//! and `attachments/` are walked for the zip itself: the count is a number
//! from the database (`Repo::export_counts`), which is the same source the
//! library's own screens already trust for "how many attachments," rather
//! than a second count taken by listing directories that could disagree with
//! it if something in `<data>/attachments/` were ever orphaned.

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::db::repo::Repo;

/// The zip's root file. I2's first read, before it opens anything else in
/// the archive.
pub const MANIFEST_NAME: &str = "setlistarray.json";

/// Bumped when the *shape* of what this manifest promises changes in a way
/// I2 must react to differently — a renamed field, a count whose meaning
/// changed — not on every release of the app. `app_version` on [`Manifest`]
/// carries the release; this carries the contract.
pub const MANIFEST_FORMAT: u32 = 1;

/// What a restorer can check without opening `db/`. See the module header
/// for the full argument; this struct is deliberately the smallest thing
/// that argument needs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    pub app_version: String,
    /// Wall-clock export time, milliseconds since the epoch. `0` if the
    /// clock is unavailable — the same fallback
    /// `rhypedb_engine::database::BackupManifest::created_at_ms` uses, for
    /// the same reason: a backup a clockless machine cannot date is still
    /// worth taking.
    pub created_at_ms: u64,
    pub song_count: u64,
    pub setlist_count: u64,
    pub attachment_count: u64,
}

impl Manifest {
    fn to_bytes(&self) -> Vec<u8> {
        // A `Manifest` is five plain fields with no map, no recursive type
        // and no `f64` NaN/Infinity to choke on — serialising it cannot fail
        // in practice, and a caller that had to handle an `Err` here for a
        // struct this shaped would be handling a lie.
        serde_json::to_vec_pretty(self).expect("a Manifest always serialises")
    }

    /// I2's entry point. Not called anywhere in this crate today — see the
    /// module header's note on why this manifest exists for the *next*
    /// card, not this one.
    pub fn parse(bytes: &[u8]) -> Result<Manifest, String> {
        serde_json::from_slice(bytes).map_err(|e| e.to_string())
    }
}

/// The three ways building a zip can fail. Every variant carries words a
/// screen can show — see [`ExportError::message`] — because
/// [`crate::screens::settings`]'s export row has nothing else to tell
/// someone whose backup did not happen.
#[derive(Debug)]
pub enum ExportError {
    /// `Database::backup_to` refused. In this single-writer app that is not
    /// a race with anything else in the process — the module header is
    /// where that is argued — so in practice this means a full disk or a
    /// permissions problem on the scratch directory this module writes the
    /// snapshot into before zipping it.
    Snapshot(String),
    /// Counting songs, setlists or attachments for the manifest failed.
    Count(String),
    /// A file could not be read while its bytes were being copied into the
    /// archive — the card's "unreadable file mid-walk." This can only be an
    /// attachment: `db/` is a private scratch copy nothing else can touch
    /// between the snapshot and the read, but `<data>/attachments/` is the
    /// live directory, and something else in this single-threaded app would
    /// have to be running concurrently to lose a race with it — which
    /// nothing does today, but the walk does not assume that will always be
    /// true.
    Unreadable(PathBuf, std::io::Error),
    /// Some other I/O failure — creating the scratch directory, or removing
    /// it again afterwards.
    Io(std::io::Error),
    /// The zip writer itself refused a name or a write.
    Zip(String),
}

impl ExportError {
    /// Words a person could be shown. Never the `Debug` form: a `PathBuf`
    /// under `<data>/attachments/` names an object id, not anything a
    /// musician chose, and printing it verbatim would read as a stack trace
    /// wearing a UI's clothes.
    pub fn message(&self) -> String {
        match self {
            ExportError::Snapshot(m) => format!("The library would not hold still to be copied: {m}"),
            ExportError::Count(m) => format!("Counting the library failed: {m}"),
            ExportError::Unreadable(_, e) => format!("A file could not be read while building the zip: {e}"),
            ExportError::Io(e) => format!("The export could not be prepared: {e}"),
            ExportError::Zip(m) => format!("The zip could not be built: {m}"),
        }
    }
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for ExportError {}

impl From<std::io::Error> for ExportError {
    fn from(e: std::io::Error) -> Self {
        ExportError::Io(e)
    }
}

/// Build the export zip for `repo`'s library, entirely in memory.
///
/// "Entirely in memory" is [`crate::picker::save`]'s own currency — bytes
/// plus a name — and the reason nothing here takes a destination path: the
/// caller hands the result to the picker seam, which is the only thing in
/// this app that knows how to put bytes somewhere a user chose. This
/// function never touches a save dialog and cannot: see `crate::screens::settings`
/// for where the two meet.
pub fn build_zip(repo: &Repo) -> Result<Vec<u8>, ExportError> {
    let scratch = scratch_snapshot_dir();
    let _cleanup = RemoveOnDrop(&scratch);

    // The consistent copy — see the module header for what `backup_to`
    // guarantees and why a raw walk of the live `db/` would not.
    repo.database()
        .backup_to(&scratch)
        .map_err(|e| ExportError::Snapshot(e.to_string()))?;

    let (song_count, setlist_count, attachment_count) =
        repo.export_counts().map_err(|e| ExportError::Count(e.to_string()))?;
    let manifest = Manifest {
        format_version: MANIFEST_FORMAT,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at_ms: now_ms(),
        song_count,
        setlist_count,
        attachment_count,
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        write_file(&mut zip, MANIFEST_NAME, &manifest.to_bytes(), options)?;
        add_tree(&mut zip, &scratch, "db", options)?;
        add_tree(&mut zip, &repo.dir().attachments_root(), "attachments", options)?;

        zip.finish().map_err(|e| ExportError::Zip(e.to_string()))?;
    }
    Ok(cursor.into_inner())
}

/// Milliseconds since the epoch, or `0` if the clock is unavailable — see
/// [`Manifest::created_at_ms`]'s own doc comment for why that fallback is the
/// same one `rhypedb_engine::database::BackupManifest` uses. Public because
/// `crate::screens::settings` stamps `Preferences::last_export_at` with the
/// same clock the manifest inside that same zip was stamped with, rather
/// than reading the system clock a second time a few milliseconds later.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The name suggested to the save dialog: `setlistarray-2026-09-01.zip`.
/// Dated, not timestamped to the second — a musician exporting twice in one
/// day is choosing to keep both, and the platform's own collision handling
/// (`crate::picker`'s module header, "the name that lands can differ from
/// the one asked for") is what a second export that day falls back to,
/// exactly the way it would for any other repeated file name.
pub fn suggested_file_name() -> String {
    let day = crate::model::Day::today();
    format!("setlistarray-{:04}-{:02}-{:02}.zip", day.year, day.month, day.day)
}

/// A directory nothing else in this process knows about, for `backup_to`'s
/// destination. Named like [`crate::db::scratch`] — pid plus a counter, so
/// two exports racing in the same process (a test running more than one) can
/// never collide — but under the system temp directory rather than beside
/// the real library, because this one is deleted the moment the zip is
/// built rather than kept for the next launch.
fn scratch_snapshot_dir() -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("sla-export-{}-{n}", std::process::id()))
}

/// Deletes the path it holds when it goes out of scope, success or failure
/// alike — the scratch snapshot is only ever a means to the zip already
/// built by the time this drops, and a build that failed partway through has
/// even less reason to leave a half-written copy of the database sitting in
/// the system temp directory.
struct RemoveOnDrop<'a>(&'a Path);

impl Drop for RemoveOnDrop<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0);
    }
}

fn write_file<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    options: zip::write::SimpleFileOptions,
) -> Result<(), ExportError> {
    zip.start_file(name, options)
        .map_err(|e| ExportError::Zip(e.to_string()))?;
    zip.write_all(bytes)?;
    Ok(())
}

/// Walk `src` and add every file under it to `zip`, named `<prefix>/<relative
/// path>` with `/` separators regardless of this platform's own — a zip
/// entry name is not a filesystem path and Android's unzip has no interest
/// in Windows' separators either, though this app never runs there.
///
/// A missing `src` is not an error: `attachments_root()` exists on every
/// library `crate::db::open` has ever created (it calls `create_dir_all` on
/// it), but a scratch directory a test builds by hand may not have bothered,
/// and an export with zero attachments should not need one either.
fn add_tree<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    src: &Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<(), ExportError> {
    if !src.exists() {
        return Ok(());
    }
    let mut pending = vec![src.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(src)
                .expect("walked from src, so src is always a prefix")
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            let bytes = std::fs::read(&path).map_err(|e| ExportError::Unreadable(path.clone(), e))?;
            write_file(zip, &format!("{prefix}/{relative}"), &bytes, options)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::scratch;
    use crate::model::{Attachment, AttachmentKind};

    fn chart(title: &str) -> Attachment {
        Attachment {
            id: 0,
            kind: AttachmentKind::Pdf,
            title: title.into(),
            bytes_on_disk: 12,
            page_count: Some(1),
            source_url: None,
            captured_at: None,
            body: None,
        }
    }

    #[test]
    fn a_manifest_survives_the_json_round_trip() {
        let manifest = Manifest {
            format_version: MANIFEST_FORMAT,
            app_version: "0.1.0".into(),
            created_at_ms: 1_756_770_000_000,
            song_count: 41,
            setlist_count: 3,
            attachment_count: 12,
        };
        let bytes = manifest.to_bytes();
        assert_eq!(Manifest::parse(&bytes).unwrap(), manifest);
    }

    #[test]
    fn an_unreadable_manifest_is_an_error_not_a_panic() {
        assert!(Manifest::parse(b"not json").is_err());
    }

    /// The test the card calls out as the one that matters most: build a
    /// zip, restore it into a directory that has never heard of this
    /// library, and open a database from the restored copy — no
    /// special-cased restore path, the same `crate::db::open` a normal
    /// launch calls.
    #[test]
    fn the_zip_this_module_builds_restores_into_a_working_library() {
        let source_dir = scratch("export-source");
        let (song_id, chart_id) = {
            let repo = Repo::open(&source_dir).expect("source library opens");
            let song = repo
                .create_song(&crate::model::Song::new(0, "Carolina", "M. Ward"))
                .expect("song created");
            repo.create_setlist(&crate::model::Setlist {
                id: 0,
                name: "Porch, Saturday".into(),
                song_ids: Vec::new(),
                last_played: None,
            })
            .expect("setlist created");
            let chart_id = repo
                .create_attachment(song as crate::model::SongId, &chart("carolina-chords.pdf"))
                .expect("attachment created");
            // A real file where the attachment's bytes would live — the
            // directory itself is `create_attachment`'s job, the file inside
            // it is whatever imported the chart's job (`crate::pdf::import`,
            // the capture module). This test stands in for both.
            std::fs::write(
                source_dir.attachment(chart_id).join("carolina-chords.pdf"),
                b"%PDF-1.4 not a real pdf, just bytes to carry across",
            )
            .unwrap();
            (song, chart_id)
        };

        let repo = Repo::open(&source_dir).expect("source library reopens for export");
        let bytes = build_zip(&repo).expect("the zip builds");

        // The manifest at the root, readable without touching `db/` at all —
        // I2's first move, proven here rather than only asserted about.
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes.clone())).expect("a valid zip");
        let manifest_bytes = {
            let mut file = archive.by_name(MANIFEST_NAME).expect("the manifest is at the root");
            let mut buf = Vec::new();
            std::io::copy(&mut file, &mut buf).unwrap();
            buf
        };
        let manifest = Manifest::parse(&manifest_bytes).expect("the manifest parses");
        assert_eq!(manifest.format_version, MANIFEST_FORMAT);
        assert_eq!(manifest.song_count, 1);
        assert_eq!(manifest.setlist_count, 1);
        assert_eq!(manifest.attachment_count, 1);

        // Restore: unzip onto a bare directory's root. `db/` and
        // `attachments/` land exactly where `DataDir` already expects them —
        // no renaming, no special restore code, which is the whole point of
        // matching the zip's layout to `DataDir`'s own.
        let restored_root = scratch("export-restored");
        std::fs::create_dir_all(restored_root.path()).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("a valid zip, twice");
        archive.extract(restored_root.path()).expect("the archive extracts");

        let restored = Repo::open(&restored_root).expect("the restored library opens");
        let songs = restored.songs().expect("songs read back");
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].title, "Carolina");
        assert_eq!(songs[0].id, song_id as crate::model::SongId);

        let setlists = restored.setlists().expect("setlists read back");
        assert_eq!(setlists.len(), 1);
        assert_eq!(setlists[0].name, "Porch, Saturday");

        let attachments = restored.attachments().expect("attachments read back");
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].title, "carolina-chords.pdf");

        // And the bytes beside the database — not the database's job to
        // carry, which is exactly why this test writes them by hand above
        // and checks for them here rather than trusting `db/` to have done
        // it.
        let restored_file = restored_root.attachment(chart_id).join("carolina-chords.pdf");
        assert_eq!(
            std::fs::read(&restored_file).unwrap(),
            b"%PDF-1.4 not a real pdf, just bytes to carry across"
        );
        let _ = chart_id;
    }

    #[test]
    fn an_export_leaves_no_scratch_directory_behind() {
        let source_dir = scratch("export-no-litter");
        let repo = Repo::open(&source_dir).expect("library opens");
        let before: std::collections::HashSet<_> = std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name()))
            .collect();

        build_zip(&repo).expect("the zip builds");

        let after: std::collections::HashSet<_> = std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name()))
            .collect();
        let leftover: Vec<_> = after
            .difference(&before)
            .filter(|name| name.to_string_lossy().starts_with("sla-export-"))
            .collect();
        assert!(leftover.is_empty(), "{leftover:?}");
    }
}
