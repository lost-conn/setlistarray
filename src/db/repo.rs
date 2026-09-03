//! The repository: every read and write the stores make, in one place.
//!
//! Nothing above this file knows a type name, a field name or a link. Nothing
//! in this file knows a Signal — it is plain, synchronous, fallible code, which
//! is what makes it testable without a window. The seam that turns a fallible
//! write into the infallible store call a screen makes is
//! [`crate::store::Storage`].
//!
//! Two rules run through it:
//!
//! * **Metadata only.** An attachment's bytes live in
//!   `<data>/attachments/<id>/`; the database holds what a row needs to render.
//!   The startup load projects the `body` field away entirely, so building the
//!   library list cannot touch an attachment body even by accident.
//! * **Derived values are not stored.** Group buckets, cumulative clocks and
//!   counts are computed on read. The one thing that looks derived but is not
//!   is a setlist's `position`: that is the running order the user chose.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rhypedb_engine::database::Database;
use rhypedb_engine::object::{FieldMap, Value};

use crate::db::convert::{
    attachment_fields, attachment_from, setlist_fields, setlist_from, setlist_updates, song_fields, song_from, song_updates,
};
use crate::db::prefs::{self, Preferences};
use crate::db::{DataDir, DbError, DbResult, open};
use crate::model::{Attachment, AttachmentId, Setlist, SetlistId, Song, SongId};

pub const SONG: &str = "Song";
pub const SETLIST: &str = "Setlist";
pub const ATTACHMENT: &str = "Attachment";
pub const PREFERENCES: &str = "Preferences";

/// Every attachment field except `body`. The startup scan asks for exactly
/// these, so a library row physically cannot pull an extracted chart into
/// memory — the performance budget in the plan, enforced rather than promised.
const ATTACHMENT_META: [&str; 7] = [
    "kind",
    "title",
    "bytes_on_disk",
    "page_count",
    "source_url",
    "captured_at",
    // Card E6: small enough to sit beside the rest of the metadata rather
    // than behind the same on-demand door as `body` — it is one `Day`, not
    // megabytes of extracted text — and a list row needs it in memory to
    // decide, without a second read, whether opening this page is even
    // allowed to fire a check today. See `Attachment::rechecked_at`.
    "rechecked_at",
];

/// What a fresh launch reads.
#[derive(Debug, Default)]
pub struct Loaded {
    pub songs: Vec<Song>,
    pub setlists: Vec<Setlist>,
    pub attachments: Vec<Attachment>,
    pub preferences: Option<Preferences>,
}

pub struct Repo {
    db: Arc<Database>,
    dir: DataDir,
    /// The single Preferences row, once we have seen it. `0` means "not yet".
    preferences_id: AtomicU64,
}

fn engine(e: impl std::fmt::Display) -> DbError {
    DbError::Engine(e.to_string())
}

impl Repo {
    pub fn open(dir: &DataDir) -> DbResult<Self> {
        Ok(Self {
            db: open(dir)?,
            dir: dir.clone(),
            preferences_id: AtomicU64::new(0),
        })
    }

    pub fn dir(&self) -> &DataDir {
        &self.dir
    }

    pub fn database(&self) -> &Arc<Database> {
        &self.db
    }

    /// Counts for the export manifest (card I1) — exactly the numbers a
    /// person restoring a zip later can check `setlistarray.json` against
    /// without opening `db/` at all. See `crate::export`'s module header for
    /// why the manifest exists and what it promises.
    ///
    /// `count_type` rather than `.len()` on `self.songs()`/etc.: those load
    /// every field of every row to build the app's own structs, and a count
    /// the export takes on its way out has no use for any of that.
    pub fn export_counts(&self) -> DbResult<(u64, u64, u64)> {
        Ok((
            self.db.count_type(SONG).map_err(engine)?,
            self.db.count_type(SETLIST).map_err(engine)?,
            self.db.count_type(ATTACHMENT).map_err(engine)?,
        ))
    }

    // ── loading ─────────────────────────────────────────────────────────────

    /// One pass over the library at startup. Everything the screens read lives
    /// in memory afterwards; every mutation writes back through here.
    pub fn load(&self) -> DbResult<Loaded> {
        Ok(Loaded {
            songs: self.songs()?,
            setlists: self.setlists()?,
            attachments: self.attachments()?,
            preferences: self.preferences()?,
        })
    }

    pub fn songs(&self) -> DbResult<Vec<Song>> {
        let objects = self.db.scan_type(SONG).map_err(engine)?;
        let mut songs = Vec::with_capacity(objects.len());
        for object in &objects {
            let mut ids: Vec<AttachmentId> = self
                .db
                .get_links(SONG, object.id, "attachments")
                .map_err(engine)?
                .into_iter()
                .map(|(id, _)| id as AttachmentId)
                .collect();
            ids.sort_unstable();
            songs.push(song_from(object, ids));
        }
        songs.sort_by_key(|s| s.id);
        Ok(songs)
    }

    pub fn setlists(&self) -> DbResult<Vec<Setlist>> {
        let objects = self.db.scan_type(SETLIST).map_err(engine)?;
        let mut setlists = Vec::with_capacity(objects.len());
        for object in &objects {
            let mut members = self
                .db
                .get_links(SETLIST, object.id, "songs")
                .map_err(engine)?;
            // The edge scan comes back in target-id order; the running order is
            // the `position` edge field, which is the whole reason it exists.
            members.sort_by_key(|(id, fields)| (position_of(fields), *id));
            let song_ids = members.into_iter().map(|(id, _)| id as SongId).collect();
            setlists.push(setlist_from(object, song_ids));
        }
        setlists.sort_by_key(|s| s.id);
        Ok(setlists)
    }

    /// Metadata only — see [`ATTACHMENT_META`].
    pub fn attachments(&self) -> DbResult<Vec<Attachment>> {
        let objects = self
            .db
            .scan_type_projected(ATTACHMENT, &ATTACHMENT_META)
            .map_err(engine)?;
        let mut list: Vec<Attachment> = objects.iter().map(attachment_from).collect();
        list.sort_by_key(|a| a.id);
        Ok(list)
    }

    /// The extracted text, fetched only when something is about to render it.
    pub fn attachment_body(&self, id: AttachmentId) -> DbResult<Option<String>> {
        let object = self.db.get(ATTACHMENT, id as u64).map_err(engine)?;
        Ok(match object.fields.get("body") {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        })
    }

    // ── songs ───────────────────────────────────────────────────────────────

    /// Returns the id the database minted. The caller adopts it: object ids are
    /// the domain ids, so there is no second numbering to keep in step.
    pub fn create_song(&self, song: &Song) -> DbResult<u64> {
        let object = self.db.create(SONG, song_fields(song)).map_err(engine)?;
        Ok(object.id)
    }

    /// A full write of the song's scalars. Fields the song no longer has are
    /// written as `Null` rather than left behind — clearing a key has to clear
    /// it on disk too, or the next launch resurrects it.
    pub fn save_song(&self, song: &Song) -> DbResult<()> {
        self.db
            .update(SONG, song.id as u64, song_updates(song))
            .map(|_| ())
            .map_err(engine)
    }

    /// The schema's delete policies do the rest: the song's attachments go with
    /// it (`@on_delete(cascade)`) and it leaves every setlist it was in
    /// (`@on_delete(remove)`). Their directories go too.
    pub fn delete_song(&self, id: SongId) -> DbResult<()> {
        let doomed: Vec<AttachmentId> = self
            .db
            .get_links(SONG, id as u64, "attachments")
            .map_err(engine)?
            .into_iter()
            .map(|(id, _)| id as AttachmentId)
            .collect();
        self.db.delete(SONG, id as u64).map_err(engine)?;
        for attachment in doomed {
            self.remove_attachment_dir(attachment)?;
        }
        Ok(())
    }

    // ── setlists ────────────────────────────────────────────────────────────

    pub fn create_setlist(&self, setlist: &Setlist) -> DbResult<u64> {
        let object = self
            .db
            .create(SETLIST, setlist_fields(setlist))
            .map_err(engine)?;
        Ok(object.id)
    }

    pub fn save_setlist(&self, setlist: &Setlist) -> DbResult<()> {
        self.db
            .update(SETLIST, setlist.id as u64, setlist_updates(setlist))
            .map(|_| ())
            .map_err(engine)
    }

    pub fn delete_setlist(&self, id: SetlistId) -> DbResult<()> {
        self.db.delete(SETLIST, id as u64).map_err(engine)
    }

    /// Make the stored membership match `song_ids`, in that order.
    ///
    /// One method covers add, remove and reorder because all three are the same
    /// statement: "this is the running order now". `link` upserts, so a member
    /// that has not moved costs an identical write rather than a special case.
    ///
    /// A song id that no longer exists is skipped rather than failing the whole
    /// write: deleting a song already removed it from every set in the database,
    /// and an in-memory list that has not caught up must not block the reorder
    /// the user just asked for.
    pub fn set_members(&self, setlist: SetlistId, song_ids: &[SongId]) -> DbResult<()> {
        let existing: Vec<u64> = self
            .db
            .get_links(SETLIST, setlist as u64, "songs")
            .map_err(engine)?
            .into_iter()
            .map(|(id, _)| id)
            .collect();

        for id in &existing {
            if !song_ids.contains(&(*id as SongId)) {
                self.db
                    .unlink(SETLIST, setlist as u64, "songs", *id)
                    .map_err(engine)?;
            }
        }

        let mut position = 0u32;
        for song in song_ids {
            if self.db.get(SONG, *song as u64).is_err() {
                continue;
            }
            let mut edge = FieldMap::new();
            edge.insert("position".to_string(), Value::U32(position));
            self.db
                .link(SETLIST, setlist as u64, "songs", *song as u64, Some(edge))
                .map_err(engine)?;
            position += 1;
        }
        Ok(())
    }

    // ── attachments ─────────────────────────────────────────────────────────

    /// Creates the row, links it to its song, and makes the directory its bytes
    /// will live in. If the directory cannot be made the row is taken back out
    /// again — an attachment with nowhere to put its file is not a half-success.
    pub fn create_attachment(&self, song: SongId, attachment: &Attachment) -> DbResult<u64> {
        let object = self
            .db
            .create(ATTACHMENT, attachment_fields(attachment))
            .map_err(engine)?;

        let placed = self
            .db
            .link(ATTACHMENT, object.id, "song", song as u64, None)
            .map_err(engine)
            .and_then(|()| {
                std::fs::create_dir_all(self.dir.attachment(object.id)).map_err(DbError::Io)
            });

        if let Err(e) = placed {
            let _ = self.db.delete(ATTACHMENT, object.id);
            let _ = std::fs::remove_dir_all(self.dir.attachment(object.id));
            return Err(e);
        }
        Ok(object.id)
    }

    pub fn save_attachment(&self, attachment: &Attachment) -> DbResult<()> {
        self.db
            .update(
                ATTACHMENT,
                attachment.id as u64,
                attachment_fields(attachment),
            )
            .map(|_| ())
            .map_err(engine)
    }

    /// The row goes first: a directory left behind is recoverable, a row
    /// pointing at bytes that are gone is not.
    pub fn delete_attachment(&self, id: AttachmentId) -> DbResult<()> {
        self.db.delete(ATTACHMENT, id as u64).map_err(engine)?;
        self.remove_attachment_dir(id)
    }

    fn remove_attachment_dir(&self, id: AttachmentId) -> DbResult<()> {
        let path = self.dir.attachment(id as u64);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(DbError::Io(e)),
        }
    }

    /// Card J6's sweep: a directory under `attachments/` whose name no row in
    /// `known` names is gone the moment this returns.
    ///
    /// The audit for this card found the crash window is narrower than it
    /// first looked. `create_attachment` above makes the row and the
    /// directory in the same call, and every place that later takes a row
    /// back out — `delete_attachment`, and therefore
    /// `AttachmentsStore::forget`, which is every ordinary failure exit
    /// `SongsStore::attach`/`detach`, `capture::attach_captured` and
    /// `pdf::import` have — deletes the directory right along with it. None
    /// of those leak; an orphan needs an actual crash between `attach`
    /// minting the row+directory and a producer (`capture::write_into`,
    /// `pdf::import`'s own write, the typed editor's save) finishing the
    /// bytes inside it, or a `remove_attachment_dir` that failed partway
    /// (the row already gone, the directory refusing to follow — a full
    /// disk or a permissions error, not a code path this app chooses).
    /// Nothing here can tell those two apart, and it does not need to: both
    /// leave the same thing behind, a directory nothing points at.
    ///
    /// `known` is handed in rather than re-read here because the caller —
    /// `crate::lib::app`, right after its own `Storage::load` — already paid
    /// for that scan, and this walk has to run against the *same* answer the
    /// caller is trusting, not a second one that could disagree with it by
    /// the time this runs.
    ///
    /// Two refusals inside the walk itself, both about not touching what this
    /// app did not put there:
    /// * A directory whose name does not parse as a bare id is left alone.
    ///   `DataDir::attachment` only ever names one after an id, so anything
    ///   else under `attachments/` was not written by this app.
    /// * A file sitting directly in `attachments/` (there should never be
    ///   one) is left alone for the same reason — this only ever removes
    ///   directories.
    ///
    /// A missing `attachments/` altogether is not a fault, it is nothing to
    /// sweep, and comes back as an empty list rather than an error.
    pub fn sweep_orphaned_attachments(&self, known: &[AttachmentId]) -> DbResult<Vec<AttachmentId>> {
        let entries = match std::fs::read_dir(self.dir.attachments_root()) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(DbError::Io(e)),
        };

        let mut removed = Vec::new();
        for entry in entries {
            let entry = entry.map_err(DbError::Io)?;
            if !entry.file_type().map_err(DbError::Io)?.is_dir() {
                continue;
            }
            let Some(id) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<AttachmentId>().ok())
            else {
                continue;
            };
            if known.contains(&id) {
                continue;
            }
            std::fs::remove_dir_all(entry.path()).map_err(DbError::Io)?;
            removed.push(id);
        }
        removed.sort_unstable();
        Ok(removed)
    }

    // ── preferences ─────────────────────────────────────────────────────────

    pub fn preferences(&self) -> DbResult<Option<Preferences>> {
        let objects = self.db.scan_type(PREFERENCES).map_err(engine)?;
        let Some(object) = objects.iter().min_by_key(|o| o.id) else {
            return Ok(None);
        };
        self.preferences_id.store(object.id, Ordering::Relaxed);
        Ok(Some(prefs::from_fields(&object.fields)))
    }

    /// One row, rewritten in place. There is nothing to query, relate or index
    /// here, so it is a row rather than a library of objects.
    pub fn save_preferences(&self, preferences: &Preferences) -> DbResult<()> {
        let fields = prefs::to_fields(preferences);
        let known = self.preferences_id.load(Ordering::Relaxed);
        if known != 0
            && self
                .db
                .update(PREFERENCES, known, fields.clone())
                .map_err(engine)
                .is_ok()
        {
            return Ok(());
        }
        // Either we have never seen the row, or it went away under us.
        let existing = self.db.scan_type(PREFERENCES).map_err(engine)?;
        let id = match existing.iter().min_by_key(|o| o.id) {
            Some(object) => {
                self.db
                    .update(PREFERENCES, object.id, fields)
                    .map_err(engine)?;
                object.id
            }
            None => self.db.create(PREFERENCES, fields).map_err(engine)?.id,
        };
        self.preferences_id.store(id, Ordering::Relaxed);
        Ok(())
    }
}

fn position_of(fields: &FieldMap) -> u32 {
    match fields.get("position") {
        Some(Value::U32(p)) => *p,
        Some(Value::U64(p)) => *p as u32,
        _ => u32::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{restart, scratch};
    use crate::model::{AttachmentKind, Confidence, Day};

    fn song(title: &str, artist: &str) -> Song {
        Song::new(0, title, artist)
    }

    fn chart(title: &str) -> Attachment {
        Attachment {
            id: 0,
            kind: AttachmentKind::Text,
            title: title.into(),
            bytes_on_disk: 2_100,
            page_count: None,
            source_url: None,
            captured_at: None,
            body: Some("Capo 3. Eb shapes played as C.".into()),
            rechecked_at: None,
        }
    }

    fn named(name: &str) -> Setlist {
        Setlist {
            id: 0,
            name: name.into(),
            song_ids: Vec::new(),
            last_played: None,
        }
    }

    #[test]
    fn a_song_survives_a_close_and_a_reopen() {
        let dir = scratch("repo-song-restart");
        let id = {
            let repo = Repo::open(&dir).unwrap();
            let mut carolina = song("Carolina", "M. Ward");
            carolina.key = Some("G".into());
            carolina.tempo = Some(96);
            carolina.confidence = Some(Confidence::Solid);
            carolina.tags = vec!["campfire".into()];
            carolina.last_played = Some(Day::new(2026, 6, 2));
            repo.create_song(&carolina).unwrap()
        };

        let repo = restart(|| Repo::open(&dir).ok());
        let songs = repo.songs().unwrap();
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].id as u64, id);
        assert_eq!(songs[0].title, "Carolina");
        assert_eq!(songs[0].key.as_deref(), Some("G"));
        assert_eq!(songs[0].confidence, Some(Confidence::Solid));
        assert_eq!(songs[0].tags, vec!["campfire"]);
        assert_eq!(songs[0].last_played, Some(Day::new(2026, 6, 2)));
    }

    #[test]
    fn a_field_cleared_on_screen_is_cleared_on_disk() {
        let dir = scratch("repo-clear-field");
        let repo = Repo::open(&dir).unwrap();

        let mut landslide = song("Landslide", "Fleetwood Mac");
        landslide.key = Some("Eb".into());
        landslide.capo = Some(3);
        landslide.id = repo.create_song(&landslide).unwrap() as SongId;

        landslide.key = None;
        repo.save_song(&landslide).unwrap();

        let back = &repo.songs().unwrap()[0];
        assert_eq!(back.key, None, "an update must clear, not merge");
        assert_eq!(back.capo, Some(3), "and must not touch what it did not say");
    }

    #[test]
    fn a_setlist_keeps_its_running_order_across_a_reopen() {
        let dir = scratch("repo-order");
        let (setlist, ids) = {
            let repo = Repo::open(&dir).unwrap();
            let ids: Vec<SongId> = ["Carolina", "Blackbird", "Ripple"]
                .iter()
                .map(|t| repo.create_song(&song(t, "someone")).unwrap() as SongId)
                .collect();
            let setlist = repo.create_setlist(&named("Porch, Saturday")).unwrap() as SetlistId;
            // Deliberately not id order: the order is the point.
            repo.set_members(setlist, &[ids[2], ids[0], ids[1]]).unwrap();
            (setlist, ids)
        };

        let repo = restart(|| Repo::open(&dir).ok());
        let setlists = repo.setlists().unwrap();
        assert_eq!(setlists.len(), 1);
        assert_eq!(setlists[0].id, setlist);
        assert_eq!(setlists[0].song_ids, vec![ids[2], ids[0], ids[1]]);
    }

    #[test]
    fn reordering_moves_a_song_without_changing_who_is_in_the_set() {
        let dir = scratch("repo-reorder");
        let repo = Repo::open(&dir).unwrap();
        let ids: Vec<SongId> = ["a", "b", "c"]
            .iter()
            .map(|t| repo.create_song(&song(t, "x")).unwrap() as SongId)
            .collect();
        let setlist = repo.create_setlist(&named("set")).unwrap() as SetlistId;
        repo.set_members(setlist, &ids).unwrap();
        repo.set_members(setlist, &[ids[1], ids[2], ids[0]]).unwrap();

        assert_eq!(repo.setlists().unwrap()[0].song_ids, vec![ids[1], ids[2], ids[0]]);
        assert_eq!(repo.songs().unwrap().len(), 3, "nobody was deleted");
    }

    /// The store's reorder tests assert the running-order *vector* stays a
    /// permutation of itself. This is the other half: that the vector reaches
    /// the `position` edge field as a dense, unique 0..n — which is what the
    /// column actually is on disk, and the only thing `setlists()` can sort by
    /// when the library is reopened.
    #[test]
    fn a_reorder_renumbers_positions_densely_from_zero() {
        let dir = scratch("repo-positions-dense");
        let repo = Repo::open(&dir).unwrap();
        let ids: Vec<SongId> = ["a", "b", "c", "d", "e"]
            .iter()
            .map(|t| repo.create_song(&song(t, "x")).unwrap() as SongId)
            .collect();
        let setlist = repo.create_setlist(&named("set")).unwrap() as SetlistId;
        repo.set_members(setlist, &ids).unwrap();
        // Move the fourth song to the top, the way Move-up-repeatedly would.
        repo.set_members(setlist, &[ids[3], ids[0], ids[1], ids[2], ids[4]])
            .unwrap();

        let mut positions: Vec<u32> = repo
            .db
            .get_links(SETLIST, setlist as u64, "songs")
            .unwrap()
            .iter()
            .map(|(_, fields)| position_of(fields))
            .collect();
        positions.sort_unstable();
        assert_eq!(
            positions,
            vec![0, 1, 2, 3, 4],
            "positions must be dense and unique, not sparse or repeated"
        );
    }


    #[test]
    fn taking_a_song_out_of_a_set_leaves_the_song_alone() {
        let dir = scratch("repo-remove-member");
        let repo = Repo::open(&dir).unwrap();
        let a = repo.create_song(&song("a", "x")).unwrap() as SongId;
        let b = repo.create_song(&song("b", "x")).unwrap() as SongId;
        let setlist = repo.create_setlist(&named("set")).unwrap() as SetlistId;
        repo.set_members(setlist, &[a, b]).unwrap();

        repo.set_members(setlist, &[a]).unwrap();

        assert_eq!(repo.setlists().unwrap()[0].song_ids, vec![a]);
        assert_eq!(repo.songs().unwrap().len(), 2);
    }

    #[test]
    fn deleting_a_song_takes_it_out_of_every_set_and_takes_its_charts_with_it() {
        let dir = scratch("repo-cascade");
        let repo = Repo::open(&dir).unwrap();
        let doomed = repo.create_song(&song("Carolina", "M. Ward")).unwrap() as SongId;
        let kept = repo.create_song(&song("Ripple", "Grateful Dead")).unwrap() as SongId;
        let attachment = repo.create_attachment(doomed, &chart("carolina.txt")).unwrap();
        let here = repo.create_setlist(&named("here")).unwrap() as SetlistId;
        let there = repo.create_setlist(&named("there")).unwrap() as SetlistId;
        repo.set_members(here, &[doomed, kept]).unwrap();
        repo.set_members(there, &[doomed]).unwrap();
        let directory = dir.attachment(attachment);
        assert!(directory.exists());

        repo.delete_song(doomed).unwrap();

        assert_eq!(repo.songs().unwrap().len(), 1);
        assert_eq!(repo.attachments().unwrap().len(), 0, "the chart went with it");
        assert!(!directory.exists(), "and so did its directory");
        let setlists = repo.setlists().unwrap();
        assert_eq!(setlists[0].song_ids, vec![kept]);
        assert!(setlists[1].song_ids.is_empty());
    }

    #[test]
    fn an_attachment_gets_a_directory_and_loses_it_again() {
        let dir = scratch("repo-attachment-dir");
        let repo = Repo::open(&dir).unwrap();
        let owner = repo.create_song(&song("Landslide", "Fleetwood Mac")).unwrap() as SongId;

        let id = repo.create_attachment(owner, &chart("landslide.txt")).unwrap();
        assert_eq!(dir.attachment(id), dir.attachments_root().join(id.to_string()));
        assert!(dir.attachment(id).exists());
        assert_eq!(repo.songs().unwrap()[0].attachments, vec![id as AttachmentId]);

        repo.delete_attachment(id as AttachmentId).unwrap();
        assert!(!dir.attachment(id).exists());
        assert!(repo.songs().unwrap()[0].attachments.is_empty());
    }

    // ── sweep_orphaned_attachments (J6) ──────────────────────────────────────

    #[test]
    fn the_sweep_removes_a_directory_no_row_names() {
        let dir = scratch("repo-sweep-orphan");
        let repo = Repo::open(&dir).unwrap();
        // Nobody's row: a plausible orphan, the shape a crash between
        // `create_attachment` and a producer finishing its write would leave.
        std::fs::create_dir_all(dir.attachment(999)).unwrap();
        std::fs::write(dir.attachment(999).join("page.html"), b"orphaned").unwrap();

        let removed = repo.sweep_orphaned_attachments(&[]).unwrap();

        assert_eq!(removed, vec![999]);
        assert!(!dir.attachment(999).exists());
    }

    #[test]
    fn the_sweep_leaves_a_directory_a_row_names_alone() {
        let dir = scratch("repo-sweep-referenced");
        let repo = Repo::open(&dir).unwrap();
        let owner = repo.create_song(&song("Landslide", "Fleetwood Mac")).unwrap() as SongId;
        let id = repo.create_attachment(owner, &chart("landslide.txt")).unwrap() as AttachmentId;

        let removed = repo.sweep_orphaned_attachments(&[id]).unwrap();

        assert!(removed.is_empty());
        assert!(dir.attachment(id as u64).exists());
    }

    /// The row-with-no-files state `captured_page.rs` already renders — a
    /// capture that minted its row but has not written a byte into it yet, or
    /// hasn't since a crash. The directory `create_attachment` makes exists,
    /// it is empty, and a row still names it: the sweep has to survive on the
    /// same evidence a live session would, not assume a directory with
    /// nothing in it is fair game.
    #[test]
    fn the_sweep_leaves_a_named_directory_alone_even_with_no_files_in_it() {
        let dir = scratch("repo-sweep-empty-but-named");
        let repo = Repo::open(&dir).unwrap();
        let owner = repo.create_song(&song("Landslide", "Fleetwood Mac")).unwrap() as SongId;
        let id = repo.create_attachment(owner, &chart("landslide.txt")).unwrap() as AttachmentId;
        // `create_attachment` already made the directory; empty it out to
        // stand in for a producer that never got to write anything.
        assert!(std::fs::read_dir(dir.attachment(id as u64)).unwrap().next().is_none());

        let removed = repo.sweep_orphaned_attachments(&[id]).unwrap();

        assert!(removed.is_empty());
        assert!(dir.attachment(id as u64).exists());
    }

    #[test]
    fn the_sweep_leaves_a_directory_whose_name_is_not_an_attachment_id_alone() {
        let dir = scratch("repo-sweep-not-ours");
        let repo = Repo::open(&dir).unwrap();
        std::fs::create_dir_all(dir.attachments_root().join(".DS_Store")).unwrap();
        std::fs::create_dir_all(dir.attachments_root().join("thumbnails")).unwrap();

        let removed = repo.sweep_orphaned_attachments(&[]).unwrap();

        assert!(removed.is_empty());
        assert!(dir.attachments_root().join(".DS_Store").exists());
        assert!(dir.attachments_root().join("thumbnails").exists());
    }

    #[test]
    fn the_sweep_does_nothing_when_attachments_root_does_not_exist_yet() {
        // `DataDir::new` alone, never opened — `attachments_root()` was never
        // created, which is not the same thing as "everything in it is an
        // orphan."
        let dir = DataDir::new(std::env::temp_dir().join("sla-sweep-never-opened"));
        let _ = std::fs::remove_dir_all(dir.path());
        let repo = Repo::open(&dir).unwrap();
        std::fs::remove_dir_all(dir.attachments_root()).unwrap();

        let removed = repo.sweep_orphaned_attachments(&[]).unwrap();

        assert!(removed.is_empty());
    }

    /// The failure path this card asked to be checked directly: `attach`
    /// minting a row and then having the song's own write refused. Traced
    /// through `SongsStore::attach` → `AttachmentsStore::forget` →
    /// `delete_attachment`, which is exercised here at the `Repo` layer the
    /// same way `an_attachment_gets_a_directory_and_loses_it_again` proves the
    /// ordinary path: nothing is left for the sweep to find because
    /// `delete_attachment` already took the directory with the row.
    #[test]
    fn the_attach_failure_path_leaves_nothing_for_the_sweep_to_find() {
        let dir = scratch("repo-sweep-attach-failure");
        let repo = Repo::open(&dir).unwrap();
        let owner = repo.create_song(&song("Landslide", "Fleetwood Mac")).unwrap() as SongId;
        let id = repo.create_attachment(owner, &chart("landslide.txt")).unwrap() as AttachmentId;

        // Stand in for the song's own save refusing the pointer — the branch
        // `SongsStore::attach` takes it back out on, whatever the reason.
        repo.delete_attachment(id).unwrap();

        let removed = repo.sweep_orphaned_attachments(&[]).unwrap();

        assert!(removed.is_empty(), "there was nothing left to sweep");
        assert!(!dir.attachment(id as u64).exists());
    }

    /// Attaching a chart writes the link *and* then updates the song, because
    /// the song now has a primary chart to point at. Deleting that song
    /// afterwards used to fail with `write conflict` about half the time: the
    /// update queued a cover refresh, and rhypedb's background worker committed
    /// on top of the same edge keys while the delete was in flight. The library
    /// looked corrupt and the failure never reproduced twice in a row.
    ///
    /// `db::options()` turns that worker off — the reasoning is there — and
    /// this is the shape that caught it. Forty rounds because one round passed
    /// perfectly well on the bad build.
    #[test]
    fn attaching_a_chart_and_deleting_its_song_is_repeatable() {
        for round in 0..40 {
            let dir = scratch(&format!("repo-attach-delete-{round}"));
            let repo = Repo::open(&dir).unwrap();
            let owner = repo.create_song(&song("Landslide", "Fleetwood Mac")).unwrap() as SongId;
            let id = repo.create_attachment(owner, &chart("landslide.txt")).unwrap() as AttachmentId;

            let mut stored = repo.songs().unwrap().into_iter().next().unwrap();
            stored.primary_attachment = Some(id);
            repo.save_song(&stored).unwrap();

            repo.delete_song(owner)
                .unwrap_or_else(|e| panic!("round {round} refused the delete: {e}"));
            assert!(repo.attachments().unwrap().is_empty());
            assert!(!dir.attachment(id as u64).exists());
        }
    }

    #[test]
    fn the_startup_scan_never_reads_an_attachment_body() {
        let dir = scratch("repo-no-body");
        let repo = Repo::open(&dir).unwrap();
        let owner = repo.create_song(&song("Landslide", "Fleetwood Mac")).unwrap() as SongId;
        let id = repo.create_attachment(owner, &chart("landslide.txt")).unwrap() as AttachmentId;

        let listed = repo.attachments().unwrap();
        assert_eq!(listed[0].title, "landslide.txt");
        assert_eq!(listed[0].bytes_on_disk, 2_100);
        assert_eq!(listed[0].body, None, "a list row must not carry a chart");

        assert_eq!(
            repo.attachment_body(id).unwrap().as_deref(),
            Some("Capo 3. Eb shapes played as C."),
            "and it is still there when something asks for it"
        );
    }

    #[test]
    fn a_membership_write_steps_over_a_song_that_is_already_gone() {
        let dir = scratch("repo-stale-member");
        let repo = Repo::open(&dir).unwrap();
        let kept = repo.create_song(&song("a", "x")).unwrap() as SongId;
        let setlist = repo.create_setlist(&named("set")).unwrap() as SetlistId;

        // 9_999 is nobody. An in-memory list that has not caught up with a
        // delete must not block the write the user just asked for.
        repo.set_members(setlist, &[kept, 9_999]).unwrap();

        assert_eq!(repo.setlists().unwrap()[0].song_ids, vec![kept]);
    }

    #[test]
    fn preferences_are_one_row_however_many_times_they_are_written() {
        let dir = scratch("repo-prefs");
        let repo = Repo::open(&dir).unwrap();
        assert_eq!(repo.preferences().unwrap(), None, "a first run has none");

        let mut preferences = Preferences {
            density: crate::store::Density::Compact,
            ..Default::default()
        };
        repo.save_preferences(&preferences).unwrap();
        preferences.collapsed = vec!["Learning".into()];
        repo.save_preferences(&preferences).unwrap();
        repo.save_preferences(&preferences).unwrap();

        assert_eq!(repo.database().scan_type(PREFERENCES).unwrap().len(), 1);
        drop(repo);

        let repo = restart(|| Repo::open(&dir).ok());
        assert_eq!(repo.preferences().unwrap(), Some(preferences));
    }

    #[test]
    fn the_demo_library_can_be_written_and_read_back_whole() {
        let dir = scratch("repo-seed");
        let repo = Repo::open(&dir).unwrap();
        crate::seed::install(&repo).unwrap();
        drop(repo);

        let repo = restart(|| Repo::open(&dir).ok());
        let loaded = repo.load().unwrap();
        assert_eq!(loaded.songs.len(), crate::seed::songs().len());
        assert_eq!(loaded.setlists.len(), crate::seed::setlists().len());
        assert_eq!(loaded.attachments.len(), crate::seed::attachments().len());

        // Ids were remapped, so compare on what the screens actually read.
        let porch = &loaded.setlists[0];
        assert_eq!(porch.name, "Porch, Saturday");
        assert_eq!(porch.song_ids.len(), 5);
        let titles: Vec<&str> = porch
            .song_ids
            .iter()
            .map(|id| {
                loaded
                    .songs
                    .iter()
                    .find(|s| s.id == *id)
                    .map(|s| s.title.as_str())
                    .unwrap()
            })
            .collect();
        assert_eq!(
            titles,
            vec![
                "Carolina",
                "Angel From Montgomery",
                "Blackbird",
                "Landslide",
                "Wagon Wheel"
            ]
        );

        let carolina = loaded.songs.iter().find(|s| s.title == "Carolina").unwrap();
        assert_eq!(carolina.attachments.len(), 1);
        assert_eq!(carolina.primary_attachment, Some(carolina.attachments[0]));
    }
}
