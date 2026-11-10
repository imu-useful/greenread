//! Reading position per book file.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

use super::StorageError;

/// Schema migrations, applied in order. `PRAGMA user_version` records how many
/// have run. Never edit a released migration; append a new one instead.
const MIGRATIONS: &[&str] = &[
    // 1: reading position per file. `file_path` is the platform-encoded bytes
    // of the canonical path, so non-UTF-8 paths work.
    "CREATE TABLE reading_position (
        file_path   BLOB PRIMARY KEY NOT NULL,
        chapter     INTEGER NOT NULL CHECK (chapter >= 0),
        char_offset INTEGER NOT NULL CHECK (char_offset >= 0),
        updated_at  INTEGER NOT NULL
    ) STRICT;",
];

/// Where the reader was in a book: chapter index and the character offset
/// (within that chapter's text) of the first visible line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReadingPosition {
    pub chapter: usize,
    pub char_offset: usize,
}

/// SQLite-backed store of reading positions, keyed by the book's file path.
pub struct ProgressStore {
    conn: Connection,
}

impl ProgressStore {
    /// Opens (creating if needed) the database at `path` and applies migrations.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|source| StorageError::CreateDir {
                path: dir.to_path_buf(),
                source,
            })?;
        }
        Self::with_connection(Connection::open(path)?)
    }

    /// Opens a private in-memory database (for tests and fallback use).
    pub fn open_in_memory() -> Result<Self, StorageError> {
        Self::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(mut conn: Connection) -> Result<Self, StorageError> {
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Records the position for `book_path`, replacing any earlier one.
    pub fn save(&self, book_path: &Path, position: ReadingPosition) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO reading_position (file_path, chapter, char_offset, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (file_path) DO UPDATE SET
                 chapter = excluded.chapter,
                 char_offset = excluded.char_offset,
                 updated_at = excluded.updated_at",
            params![
                path_key(book_path),
                to_sql_int(position.chapter),
                to_sql_int(position.char_offset),
                unix_now(),
            ],
        )?;
        Ok(())
    }

    /// Returns the saved position for `book_path`, if any.
    pub fn load(&self, book_path: &Path) -> Result<Option<ReadingPosition>, StorageError> {
        let row: Option<(i64, i64)> = self
            .conn
            .query_row(
                "SELECT chapter, char_offset FROM reading_position WHERE file_path = ?1",
                params![path_key(book_path)],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        row.map(|(chapter, char_offset)| {
            Ok(ReadingPosition {
                chapter: from_sql_int("chapter", chapter)?,
                char_offset: from_sql_int("char_offset", char_offset)?,
            })
        })
        .transpose()
    }

    /// Current schema version (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<i64, StorageError> {
        schema_version(&self.conn)
    }
}

fn schema_version(conn: &Connection) -> Result<i64, StorageError> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

/// Applies every migration newer than the database's `user_version`,
/// all in one transaction.
fn migrate(conn: &mut Connection) -> Result<(), StorageError> {
    let supported = i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX);
    let current = schema_version(conn)?;
    if current > supported {
        return Err(StorageError::SchemaTooNew {
            found: current,
            supported,
        });
    }
    let pending = usize::try_from(current).unwrap_or(0);
    if pending == MIGRATIONS.len() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    for migration in &MIGRATIONS[pending..] {
        tx.execute_batch(migration)?;
    }
    tx.pragma_update(None, "user_version", supported)?;
    tx.commit()?;
    Ok(())
}

fn path_key(path: &Path) -> &[u8] {
    path.as_os_str().as_encoded_bytes()
}

fn to_sql_int(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn from_sql_int(column: &'static str, value: i64) -> Result<usize, StorageError> {
    usize::try_from(value).map_err(|_| StorageError::InvalidValue { column, value })
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
