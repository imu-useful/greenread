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

/// Parses the file on a GIO worker thread. Await this from the main context
/// (e.g. inside `glib::spawn_future_local`) so the result lands on the UI thread.
pub async fn load_book(path: PathBuf) -> Result<Book, LoadError> {
    gio::spawn_blocking(move || PlainTextParser.parse(&path))
        .await
        .map_err(|_panic| LoadError::Panicked)?
        .map_err(LoadError::from)
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
        let book = glib::MainContext::new()
            .block_on(load_book(fixture("sample.txt")))
            .unwrap();
        assert_eq!(book.chapters.len(), 4);
    }

    #[test]
    fn missing_file_becomes_parse_error() {
        let err = glib::MainContext::new()
            .block_on(load_book(fixture("does_not_exist.txt")))
            .unwrap_err();
        assert!(matches!(err, LoadError::Parse(ParseError::Io { .. })));
    }
}
