//! Book format parsers. Each format lives in its own module and implements
//! [`Parser`]. Parsers open files read-only and never modify them.

pub mod plain_text;

use std::io;
use std::path::{Path, PathBuf};

pub use plain_text::PlainTextParser;

/// A parsed book: a title and its chapters in reading order.
///
/// A successfully parsed book always has at least one chapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    pub title: String,
    pub chapters: Vec<Chapter>,
}

/// One chapter of a book, as plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub title: String,
    pub text: String,
}

/// Errors a parser can return.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{path} is too large ({size} bytes; the limit is {limit} bytes)")]
    TooLarge {
        path: PathBuf,
        size: u64,
        limit: u64,
    },
}

/// Common interface for all book formats.
pub trait Parser {
    /// Reads the file at `path` (read-only) and returns its chapters.
    fn parse(&self, path: &Path) -> Result<Book, ParseError>;
}
