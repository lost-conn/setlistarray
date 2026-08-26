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
const ATTACHMENT_META: [&str; 6] = [
    "kind",
    "title",
    "bytes_on_disk",
    "page_count",
    "source_url",
    "captured_at",
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
