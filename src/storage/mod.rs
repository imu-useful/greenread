//! Persistent state: reading progress (later bookmarks and the library index).
//!
//! Everything lives in one SQLite database at
//! `$XDG_DATA_HOME/greenread/greenread.db`. Schema changes go through the
//! numbered migrations in [`progress`].

pub mod progress;

use std::io;
use std::path::PathBuf;

use gtk::glib;

pub use progress::{ProgressStore, ReadingPosition};

const APP_DIR: &str = "greenread";
const DATABASE_FILE: &str = "greenread.db";

/// Default database location (`$XDG_DATA_HOME/greenread/greenread.db`,
/// falling back to `~/.local/share/greenread/greenread.db`).
pub fn default_database_path() -> PathBuf {
    glib::user_data_dir().join(APP_DIR).join(DATABASE_FILE)
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("cannot create the data directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("database schema version {found} is newer than this build supports ({supported})")]
    SchemaTooNew { found: i64, supported: i64 },
    #[error("stored value {value} for {column} is out of range")]
    InvalidValue { column: &'static str, value: i64 },
}
