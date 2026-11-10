//! Plain-text (`.txt`) parser.
//!
//! Chapters are detected from heading lines such as `Chapter 1`,
//! `CHAPTER XII` or `Chapter 2. The Title`. A heading must follow a blank
//! line (or the start of the file), and the chapter number must be followed
//! by the end of the line or by punctuation, so prose like
//! "Chapter 2 was the best part" is not mistaken for a heading.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::{Book, Chapter, ParseError, Parser};

/// Files larger than this are rejected instead of being loaded into memory.
const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;
/// Lines longer than this are never treated as headings.
const MAX_HEADING_LEN: usize = 80;
/// Title for text that comes before the first heading.
const PREFACE_TITLE: &str = "Preface";
/// Title used when the file name gives none.
const UNTITLED: &str = "Untitled";
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";
const HEADING_WORD: &str = "chapter";
const HEADING_PUNCTUATION: [char; 5] = ['.', ':', '-', '\u{2013}', '\u{2014}'];
const ROMAN_DIGITS: &str = "ivxlcdm";

/// Parses plain-text files. Invalid UTF-8 sequences are replaced with U+FFFD.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlainTextParser;

impl Parser for PlainTextParser {
    fn parse(&self, path: &Path) -> Result<Book, ParseError> {
        let bytes = read_limited(path, MAX_FILE_SIZE)?;
        let body = bytes.strip_prefix(UTF8_BOM).unwrap_or(&bytes);
        let text = String::from_utf8_lossy(body);
        Ok(split_into_book(&text, &book_title(path)))
    }
}

/// Reads the whole file read-only, refusing files over `limit` bytes.
fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, ParseError> {
    let io_err = |source| ParseError::Io {
        path: path.to_path_buf(),
        source,
    };
    let file = File::open(path).map_err(io_err)?;
    let size = file.metadata().map_err(io_err)?.len();
    if size > limit {
        return Err(too_large(path, size, limit));
    }
    let mut bytes = Vec::new();
    // `take` guards against a file that grows between the size check and the read.
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(io_err)?;
    let read = bytes.len() as u64;
    if read > limit {
        return Err(too_large(path, read, limit));
    }
    Ok(bytes)
}

fn too_large(path: &Path, size: u64, limit: u64) -> ParseError {
    ParseError::TooLarge {
        path: path.to_path_buf(),
        size,
        limit,
    }
}

fn book_title(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| UNTITLED.to_owned())
}

/// A run of lines, optionally opened by a heading.
struct Section<'a> {
    heading: Option<&'a str>,
    lines: Vec<&'a str>,
}

/// Splits already-decoded text into chapters. Always returns at least one chapter.
fn split_into_book(text: &str, title: &str) -> Book {
    let normalised = text.replace("\r\n", "\n");
    let sections = split_sections(&normalised);
    let has_headings = sections.iter().any(|s| s.heading.is_some());

    let mut chapters: Vec<Chapter> = sections
        .into_iter()
        .filter_map(|section| {
            let body = join_trimmed(&section.lines);
            match section.heading {
                Some(heading) => Some(chapter(heading, body)),
                None if body.is_empty() => None,
                None if has_headings => Some(chapter(PREFACE_TITLE, body)),
                None => Some(chapter(title, body)),
            }
        })
        .collect();

    if chapters.is_empty() {
        chapters.push(chapter(title, String::new()));
    }
    Book {
        title: title.to_owned(),
        chapters,
    }
}

fn split_sections(text: &str) -> Vec<Section<'_>> {
    let mut sections = vec![Section {
        heading: None,
        lines: Vec::new(),
    }];
    let mut previous_blank = true;
    for line in text.lines() {
        if previous_blank && is_heading(line) {
            sections.push(Section {
                heading: Some(line.trim()),
                lines: Vec::new(),
            });
        } else if let Some(current) = sections.last_mut() {
            current.lines.push(line);
        }
        previous_blank = line.trim().is_empty();
    }
    sections
}

fn chapter(title: &str, text: String) -> Chapter {
    Chapter {
        title: title.to_owned(),
        text,
    }
}

/// Joins lines with `\n`, dropping blank lines at the start and end.
fn join_trimmed(lines: &[&str]) -> String {
    let is_content = |line: &&str| !line.trim().is_empty();
    let Some(first) = lines.iter().position(is_content) else {
        return String::new();
    };
    let last = lines.iter().rposition(is_content).unwrap_or(first);
    lines[first..=last].join("\n")
}

fn is_heading(line: &str) -> bool {
    let line = line.trim();
    if line.len() > MAX_HEADING_LEN {
        return false;
    }
    let Some((word, rest)) = line.split_once(char::is_whitespace) else {
        return false;
    };
    if !word.eq_ignore_ascii_case(HEADING_WORD) {
        return false;
    }
    let rest = rest.trim_start();
    let number_len = rest
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    let (number, after) = rest.split_at(number_len);
    is_chapter_number(number) && (after.is_empty() || after.starts_with(HEADING_PUNCTUATION))
}

fn is_chapter_number(token: &str) -> bool {
    !token.is_empty()
        && (token.chars().all(|c| c.is_ascii_digit())
            || token
                .chars()
                .all(|c| ROMAN_DIGITS.contains(c.to_ascii_lowercase())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(book: &Book) -> Vec<&str> {
        book.chapters.iter().map(|c| c.title.as_str()).collect()
    }

    #[test]
    fn splits_on_chapter_headings() {
        let text = "Chapter 1\n\nFirst body.\n\nChapter 2. The Next One\n\nSecond body.\n";
        let book = split_into_book(text, "t");
        assert_eq!(titles(&book), ["Chapter 1", "Chapter 2. The Next One"]);
        assert_eq!(book.chapters[0].text, "First body.");
        assert_eq!(book.chapters[1].text, "Second body.");
    }

    #[test]
    fn accepts_roman_numerals_in_any_case() {
        let text = "CHAPTER IV\n\nA.\n\nchapter xii: End\n\nB.";
        assert_eq!(
            titles(&split_into_book(text, "t")),
            ["CHAPTER IV", "chapter xii: End"]
        );
    }

    #[test]
    fn text_without_headings_is_one_chapter_named_after_book() {
        let book = split_into_book("Just some prose.\n\nMore prose.\n", "My Book");
        assert_eq!(titles(&book), ["My Book"]);
        assert_eq!(book.chapters[0].text, "Just some prose.\n\nMore prose.");
    }

    #[test]
    fn text_before_first_heading_becomes_preface() {
        let book = split_into_book("Foreword words.\n\nChapter 1\n\nBody.", "t");
        assert_eq!(titles(&book), ["Preface", "Chapter 1"]);
        assert_eq!(book.chapters[0].text, "Foreword words.");
    }

    #[test]
    fn heading_requires_blank_line_before_it() {
        let text = "Chapter 1\n\nShe said:\nChapter 2. was her favourite.\n";
        assert_eq!(titles(&split_into_book(text, "t")), ["Chapter 1"]);
    }

    #[test]
    fn prose_starting_with_chapter_is_not_a_heading() {
        let text = "Chapter 1\n\nBody.\n\nChapter 2 was the best part of the book.\n";
        assert_eq!(titles(&split_into_book(text, "t")), ["Chapter 1"]);
    }

    #[test]
    fn crlf_line_endings_are_normalised() {
        let book = split_into_book("Chapter 1\r\n\r\nLine a\r\nLine b\r\n", "t");
        assert_eq!(book.chapters[0].text, "Line a\nLine b");
    }

    #[test]
    fn file_over_size_limit_is_rejected() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.txt");
        let err = read_limited(&path, 10).unwrap_err();
        assert!(
            matches!(err, ParseError::TooLarge { limit: 10, .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn empty_text_gives_one_empty_chapter() {
        let book = split_into_book("", "Empty");
        assert_eq!(titles(&book), ["Empty"]);
        assert_eq!(book.chapters[0].text, "");
    }
}
