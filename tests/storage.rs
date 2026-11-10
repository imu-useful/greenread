//! Integration tests for reading-progress storage.

use std::fs;
use std::path::{Path, PathBuf};

use greenread::storage::{ProgressStore, ReadingPosition, StorageError};

/// A fresh, empty directory under Cargo's per-test temp dir.
fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("storage")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn position(chapter: usize, char_offset: usize) -> ReadingPosition {
    ReadingPosition {
        chapter,
        char_offset,
    }
}

#[test]
fn save_then_load_round_trips() {
    let store = ProgressStore::open_in_memory().unwrap();
    let book = Path::new("/books/a.txt");
    store.save(book, position(3, 1200)).unwrap();
    assert_eq!(store.load(book).unwrap(), Some(position(3, 1200)));
}

#[test]
fn unknown_file_has_no_position() {
    let store = ProgressStore::open_in_memory().unwrap();
    assert_eq!(
        store.load(Path::new("/books/never-opened.txt")).unwrap(),
        None
    );
}

#[test]
fn saving_again_replaces_previous_position() {
    let store = ProgressStore::open_in_memory().unwrap();
    let book = Path::new("/books/a.txt");
    store.save(book, position(1, 10)).unwrap();
    store.save(book, position(2, 20)).unwrap();
    assert_eq!(store.load(book).unwrap(), Some(position(2, 20)));
}

#[test]
fn positions_are_kept_per_file() {
    let store = ProgressStore::open_in_memory().unwrap();
    store
        .save(Path::new("/books/a.txt"), position(1, 1))
        .unwrap();
    store
        .save(Path::new("/books/b.txt"), position(2, 2))
        .unwrap();
    assert_eq!(
        store.load(Path::new("/books/a.txt")).unwrap(),
        Some(position(1, 1))
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_paths_are_supported() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let store = ProgressStore::open_in_memory().unwrap();
    let book = Path::new(OsStr::from_bytes(b"/books/caf\xe9.txt"));
    store.save(book, position(4, 5)).unwrap();
    assert_eq!(store.load(book).unwrap(), Some(position(4, 5)));
}

#[test]
fn position_survives_reopening_database_file() {
    let db = temp_dir("reopen").join("nested/dir/greenread.db");
    let book = Path::new("/books/a.txt");
    ProgressStore::open(&db)
        .unwrap()
        .save(book, position(7, 77))
        .unwrap();

    let reopened = ProgressStore::open(&db).unwrap();
    assert_eq!(reopened.load(book).unwrap(), Some(position(7, 77)));
}

#[test]
fn migrations_set_schema_version_and_are_idempotent() {
    let db = temp_dir("migrations").join("greenread.db");
    let first = ProgressStore::open(&db).unwrap().schema_version().unwrap();
    let second = ProgressStore::open(&db).unwrap().schema_version().unwrap();
    assert!(first >= 1);
    assert_eq!(first, second);
}

#[test]
fn newer_schema_version_is_rejected() {
    let db = temp_dir("too-new").join("greenread.db");
    drop(ProgressStore::open(&db).unwrap());
    rusqlite::Connection::open(&db)
        .unwrap()
        .pragma_update(None, "user_version", 9999)
        .unwrap();
    let err = ProgressStore::open(&db).err().expect("open should fail");
    assert!(
        matches!(err, StorageError::SchemaTooNew { found: 9999, .. }),
        "got {err:?}"
    );
}

#[test]
fn corrupt_database_file_is_an_error() {
    let db = temp_dir("corrupt").join("greenread.db");
    fs::write(
        &db,
        b"this is not an sqlite database, just some bytes ...........",
    )
    .unwrap();
    assert!(ProgressStore::open(&db).is_err());
}
