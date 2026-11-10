//! Integration tests for the plain-text parser using files in tests/fixtures.

use std::path::{Path, PathBuf};

use greenread::parsers::{ParseError, Parser, PlainTextParser};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn parses_sample_into_preface_and_chapters() {
    let book = PlainTextParser.parse(&fixture("sample.txt")).unwrap();

    assert_eq!(book.title, "sample");
    let titles: Vec<_> = book.chapters.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Preface",
            "Chapter 1. The Station",
            "Chapter 2. The Carriage",
            "CHAPTER III"
        ]
    );
    assert_eq!(
        book.chapters[1].text,
        "The train was late, as it always was on Tuesdays.\n\
         Mara waited on the platform with a paper cup of tea."
    );
}

#[test]
fn file_without_headings_is_single_chapter() {
    let book = PlainTextParser.parse(&fixture("no_headings.txt")).unwrap();
    assert_eq!(book.chapters.len(), 1);
    assert_eq!(book.chapters[0].title, "no_headings");
}

#[test]
fn invalid_utf8_is_replaced_not_rejected() {
    let book = PlainTextParser.parse(&fixture("invalid_utf8.txt")).unwrap();
    assert_eq!(book.chapters[0].text, "Caf\u{FFFD} au lait.");
}

#[test]
fn utf8_bom_is_stripped() {
    let book = PlainTextParser.parse(&fixture("bom.txt")).unwrap();
    assert_eq!(book.chapters[0].title, "Chapter 1");
}

#[test]
fn missing_file_returns_io_error() {
    let err = PlainTextParser
        .parse(&fixture("does_not_exist.txt"))
        .unwrap_err();
    assert!(matches!(err, ParseError::Io { .. }), "got {err:?}");
}

#[test]
fn parsing_does_not_modify_the_file() {
    let path = fixture("sample.txt");
    let before = std::fs::read(&path).unwrap();
    PlainTextParser.parse(&path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
