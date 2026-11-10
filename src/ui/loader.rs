//! Loads books off the GTK main thread.

use std::path::PathBuf;

use greenread::parsers::{Book, ParseError, Parser, PlainTextParser};
use gtk::gio;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error("an internal error occurred while reading the file")]
    Panicked,
}

/// A parsed book together with its canonical file path (the storage key).
#[derive(Debug)]
pub struct LoadedBook {
    pub path: PathBuf,
    pub book: Book,
}

/// Resolves the canonical path and parses the file on a GIO worker thread.
/// Await this from the main context (e.g. inside `glib::spawn_future_local`)
/// so the result lands on the UI thread.
pub async fn load_book(path: PathBuf) -> Result<LoadedBook, LoadError> {
    gio::spawn_blocking(move || {
        // If canonicalising fails, parsing the original path reports the real error.
        let path = path.canonicalize().unwrap_or(path);
        let book = PlainTextParser.parse(&path)?;
        Ok(LoadedBook { path, book })
    })
    .await
    .map_err(|_panic| LoadError::Panicked)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::glib;
    use std::path::Path;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn loads_book_through_worker_pool() {
        let loaded = glib::MainContext::new()
            .block_on(load_book(fixture("sample.txt")))
            .unwrap();
        assert_eq!(loaded.book.chapters.len(), 4);
        assert!(loaded.path.is_absolute());
    }

    #[test]
    fn missing_file_becomes_parse_error() {
        let err = glib::MainContext::new()
            .block_on(load_book(fixture("does_not_exist.txt")))
            .unwrap_err();
        assert!(matches!(err, LoadError::Parse(ParseError::Io { .. })));
    }
}
