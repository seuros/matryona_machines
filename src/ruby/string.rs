//! Exact Strings and their contents, read without running Ruby.
//!
//! magnus's `RString` conversion takes a String subclass and calls
//! `to_str` on anything else; reading the bytes through `#bytes` or
//! `#to_s` runs whatever a forgery redefined; and `RString::as_slice`,
//! the borrow, is `unsafe`. These readers take exactly a String (an opened
//! singleton class allowed, by [`instance_of`]'s rule) and copy its bytes
//! out with `RString::to_bytes` (magnus's `bytes` feature, which this
//! crate enables): one native copy, no Ruby method, no `unsafe` here.
//!
//! # Encoding
//!
//! [`bytes`] takes any encoding: bytes are bytes (an id, a frame, a key).
//!
//! [`utf8`] copies the bytes as they are, never transcoding, and takes a
//! String whose bytes mean the same text in UTF-8 as in its own encoding:
//!
//! - a UTF-8 String with valid bytes;
//! - a String of ASCII bytes only, in any ASCII-compatible encoding
//!   (US-ASCII, ASCII-8BIT, Latin-1, ...): Ruby's own "7-bit" coderange,
//!   the rule under which it lets such a String meet a UTF-8 one. A
//!   socket's `read` or an `Array#pack` answers ASCII-8BIT, and `"abc".b`
//!   is still the text `abc`.
//!
//! Anything else is `RangeError`, even when its bytes happen to be valid
//! UTF-8: `"caf\xC3\xA9".b` (binary, 8-bit), a Latin-1 `"caf\xE9"`, a
//! UTF-16 String (`"ab"` is `a\0b\0`, valid UTF-8 meaning something else)
//! and a dummy encoding such as ISO-2022-JP, whose 7-bit bytes are escape
//! sequences, are none of them UTF-8 text. Pass `bytes` for binary input,
//! or `encode` it in Ruby first.
use std::{borrow::Cow, format, string::String, vec::Vec};

use magnus::{
    Error, RString, Ruby, Value,
    encoding::{Coderange, EncodingCapable},
    value::ReprValue,
};

use super::{exact::instance_of, message::refusal};

/// `value` as an `RString` that is exactly a String: no subclass, no
/// `to_str` conversion.
///
/// `TypeError` `"<name> must be a String"` for anything else, a Symbol and
/// an object answering `to_str` included.
///
/// ```ignore
/// let name = exact_string(ruby, value, "name")?;
/// ```
///
/// # Errors
///
/// `TypeError` as above.
pub fn exact_string(ruby: &Ruby, value: impl ReprValue, name: &str) -> Result<RString, Error> {
    RString::from_value(value.as_value())
        .filter(|held| instance_of(*held, ruby.class_string()))
        .ok_or_else(|| refusal(ruby, || Cow::Owned(format!("{name} must be a String"))))
}

/// The bytes of exactly a String, in whatever encoding it has, copied out
/// natively (see the module docs).
///
/// ```ignore
/// let id = bytes(value, "id")?;
/// ```
///
/// # Errors
///
/// `TypeError` `"<name> must be a String"`, as [`exact_string`].
pub fn bytes(value: Value, name: &str) -> Result<Vec<u8>, Error> {
    let ruby = Ruby::get_with(value);
    Ok(exact_string(&ruby, value, name)?.to_bytes().into())
}

/// The text of exactly a String that is UTF-8 text, copied out natively.
///
/// A UTF-8 String with valid bytes, or ASCII bytes in any ASCII-compatible
/// encoding (see the module docs). `TypeError` `"<name> must be a String"`
/// as [`exact_string`]; `RangeError` `"<name> must be valid UTF-8"` for
/// invalid bytes or non-ASCII bytes in another encoding, which is never
/// transcoded.
///
/// ```ignore
/// let key = utf8(value, "key")?; // "caf\xFF": "key must be valid UTF-8"
/// ```
///
/// # Errors
///
/// `TypeError` or `RangeError` as above.
pub fn utf8(value: Value, name: &str) -> Result<String, Error> {
    let ruby = Ruby::get_with(value);
    let string = exact_string(&ruby, value, name)?;
    let text = string.enc_get() == ruby.utf8_encindex()
        || string.enc_coderange_scan() == Coderange::SevenBit;
    if text && let Ok(text) = String::from_utf8(string.to_bytes().into()) {
        return Ok(text);
    }
    let message = format!("{name} must be valid UTF-8");
    Err(Error::new(ruby.exception_range_error(), message))
}
