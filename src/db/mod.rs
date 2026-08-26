//! The on-device database.
//!
//! [rhypedb](https://github.com/joeleaver/rhypedb) run in-process — no server,
//! no network. Songs, setlists and attachments are objects; setlist membership
//! is a relationship carrying its running order, and the schema's delete
//! policies do the work that would otherwise be careful bookkeeping in ours:
//! deleting a song removes it from every set, and removing it from a set
//! leaves the song alone.
//!
//! Attachment *bytes* live on disk beside the database, not in it. Nothing
//! that renders a list row ever reads them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rhypedb_engine::database::Database;

pub mod convert;

/// Schema v1. Kept as its own file so it reads as a schema rather than as a
/// Rust string literal.
pub const SCHEMA: &str = include_str!("schema.rhype");

/// Where the library lives.
///
/// Android hands us a directory at `android_main`
/// (`AndroidApp::internal_data_path()`); desktop derives one. Nothing else in
/// the app knows a path.
#[derive(Clone, Debug)]
pub struct DataDir(PathBuf);

impl DataDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self(root.into())
    }

    /// The desktop default: `$XDG_DATA_HOME/setlistarray`, or
    /// `~/.local/share/setlistarray`.
    pub fn desktop_default() -> Self {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        Self(base.join("setlistarray"))
    }

    pub fn database(&self) -> PathBuf {
        self.0.join("db")
    }

    /// `<data>/attachments/<id>/` holds an attachment's file and anything
    /// derived from it.
    pub fn attachment(&self, id: u64) -> PathBuf {
        self.0.join("attachments").join(id.to_string())
    }

    pub fn attachments_root(&self) -> PathBuf {
        self.0.join("attachments")
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

#[derive(Debug)]
pub enum DbError {
    Schema(String),
    Engine(String),
    Io(std::io::Error),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Schema(m) => write!(f, "the schema did not parse: {m}"),
            DbError::Engine(m) => write!(f, "the database refused: {m}"),
            DbError::Io(e) => write!(f, "the library directory is unusable: {e}"),
        }
    }
}

impl std::error::Error for DbError {}

impl From<std::io::Error> for DbError {
    fn from(e: std::io::Error) -> Self {
        DbError::Io(e)
    }
}

pub type DbResult<T> = Result<T, DbError>;

/// Open the library, creating it on first run.
///
/// rhypedb reconciles the stored schema with the one passed here, so a field
/// added in a later version arrives as a schema edit rather than a migration
/// script. Anything it cannot reconcile by itself — a rename, a type change —
/// goes through the engine's migration API, which is why the schema is
/// versioned in the file rather than by a counter here.
pub fn open(dir: &DataDir) -> DbResult<Arc<Database>> {
    std::fs::create_dir_all(dir.database())?;
    std::fs::create_dir_all(dir.attachments_root())?;

    let schema =
        rhypedb_schema::parser::parse_schema(SCHEMA).map_err(|e| DbError::Schema(e.to_string()))?;

    Database::open(schema, dir.database()).map_err(|e| DbError::Engine(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> DataDir {
        let dir = std::env::temp_dir().join(format!("sla-db-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        DataDir::new(dir)
    }

    #[test]
    fn the_schema_parses() {
        rhypedb_schema::parser::parse_schema(SCHEMA).expect("schema v1 parses");
    }

    #[test]
    fn a_first_run_creates_the_library_and_a_second_opens_it() {
        let dir = scratch("first-run");
        {
            let db = open(&dir).expect("first open");
            assert_eq!(db.scan_type("Song").unwrap().len(), 0, "starts empty");
        }
        assert!(dir.database().exists());
        assert!(dir.attachments_root().exists());

        let db = open(&dir).expect("second open");
        assert_eq!(db.scan_type("Song").unwrap().len(), 0);
    }

    #[test]
    fn an_attachment_directory_is_per_id() {
        let dir = DataDir::new("/tmp/example");
        assert_eq!(
            dir.attachment(42),
            PathBuf::from("/tmp/example/attachments/42")
        );
    }

    #[test]
    fn the_desktop_default_sits_under_the_data_home() {
        // SAFETY: single-threaded test, and the variable is read once below.
        unsafe { std::env::set_var("XDG_DATA_HOME", "/tmp/xdg-example") };
        assert_eq!(
            DataDir::desktop_default().path(),
            Path::new("/tmp/xdg-example/setlistarray")
        );
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
    }
}
