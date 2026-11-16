//! Exact Strings, their bytes and their UTF-8 text.
use magnus::{Error, Ruby};
use matryona_machines::ruby::{bytes, exact_string, utf8};

use crate::{eval, raises};

/// Values that are not exactly a String, each answering every String
/// method a reader might be tempted to call.
const FORGERIES: &[&str] = &[
    "Class.new(String).new('abc')",
    "Class.new(String) { def bytes = [1]; def to_s = 'x'; def to_str = 'x' }.new('abc')",
    "Object.new.tap { |o| def o.to_str = 'abc'; def o.to_s = 'abc'; def o.bytes = [97] }",
    ":abc",
    "nil",
    "97",
    "['a']",
];

pub fn strings(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let value = |code: &str| eval(ruby, code);

    let held = exact_string(ruby, value("'abc'")?, "name")?;
    assert_eq!(held.to_string()?, "abc");
    // An opened singleton class with redefined readers: still a String, and
    // the readers never run.
    let opened =
        r#""caf\u00e9".dup.tap { |s| def s.bytes = [0]; def s.to_s = 'x'; def s.to_str = 'x' }"#;
    assert!(exact_string(ruby, value(opened)?, "name").is_ok());
    assert_eq!(utf8(value(opened)?, "name")?, "caf\u{e9}");
    assert_eq!(bytes(value(opened)?, "name")?, "caf\u{e9}".as_bytes());
    for code in FORGERIES {
        raises(
            exact_string(ruby, value(code)?, "name"),
            type_error,
            "name must be a String",
        );
        raises(
            utf8(value(code)?, "name"),
            type_error,
            "name must be a String",
        );
        raises(
            bytes(value(code)?, "name"),
            type_error,
            "name must be a String",
        );
    }

    // Bytes in any encoding, NUL and high bytes included.
    assert_eq!(
        bytes(value("\"\\x00\\xFF\\x80a\".b")?, "frame")?,
        [0, 0xFF, 0x80, b'a']
    );
    assert_eq!(bytes(value("''")?, "frame")?, [0_u8; 0]);
    assert_eq!(
        bytes(value("\"a\\x00b\\x00\".b")?, "frame")?,
        [b'a', 0, b'b', 0]
    );
    assert_eq!(bytes(value("\"\\xFF\"")?, "frame")?, [0xFF]);
    let big = bytes(value("'xy' * 100_000")?, "frame")?;
    assert_eq!(big.len(), 200_000);
    assert!(big.chunks(2).all(|pair| pair == b"xy"));

    // UTF-8 text: valid UTF-8, or ASCII in any ASCII-compatible encoding;
    // never transcoded. (This VM evaluates source as ASCII-8BIT, so the
    // encodings are spelled out.)
    let text = |code: &str| utf8(value(code)?, "key");
    assert_eq!(text(r#""\u{1F98A} key""#)?, "\u{1F98A} key");
    assert_eq!(
        text(r#""caf\xC3\xA9".force_encoding('UTF-8')"#)?,
        "caf\u{e9}"
    );
    assert_eq!(text("'ascii'.encode('US-ASCII')")?, "ascii");
    assert_eq!(text("'abc'.b")?, "abc");
    assert_eq!(text("''.b")?, "");
    assert_eq!(text("\"a\\x00b\".b")?, "a\0b");
    // ASCII in another ASCII-compatible encoding is text too, and past
    // ASCII it is never transcoded. A static libruby has no encoding
    // database, so EUC-JP (built in) stands for "ASCII-compatible, not
    // UTF-8"; its UTF-16 and dummy entries are not the real ones either, so
    // those cases (UTF-16LE, ISO-2022-JP) are proven in a real process:
    // `probabilistic_machines/test/heavy_hitters_test.rb`.
    assert_eq!(text("'abc'.force_encoding('EUC-JP')")?, "abc");
    raises(
        text(r#""\xA4\xA2".force_encoding('EUC-JP')"#),
        range_error,
        "key must be valid UTF-8",
    );
    for code in [
        r#""caf\xFF".force_encoding('UTF-8')"#,
        r#""\xC3".force_encoding('UTF-8')"#,
        r#""caf\u00e9".b"#,
        r#""caf\xC3\xA9".force_encoding('US-ASCII')"#,
        r#""caf\xE9".b"#,
    ] {
        raises(text(code), range_error, "key must be valid UTF-8");
    }
    Ok(())
}
