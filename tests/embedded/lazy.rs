//! Lazy messages on the String and Array readers.
use std::cell::Cell;

use magnus::{Error, Ruby, TryConvert, Value, prelude::*};
use matryona_machines::ruby::{
    bytes_with, exact_array_n_with, exact_array_with, exact_string_with, utf8_with,
};

use crate::{eval, raises};

/// Values that are not exactly a String; each is named in the message.
const NOT_STRINGS: &[(&str, &str)] = &[
    (":abc", "Symbol"),
    ("97", "Integer"),
    ("nil", "NilClass"),
    (
        "Object.new.tap { |o| def o.to_str = 'abc' }",
        "#<Class:#<Object",
    ),
];

pub fn strings(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let built = Cell::new(0_u8);
    let never = || {
        built.set(built.get() + 1);
        "never"
    };

    // Accepted: no message is built.
    let text = eval(ruby, r#""caf\u00e9""#)?;
    assert_eq!(
        exact_string_with(ruby, text, never)?.to_string()?,
        "caf\u{e9}"
    );
    assert_eq!(utf8_with(text, "key", never)?, "caf\u{e9}");
    assert_eq!(bytes_with(text, never)?, "caf\u{e9}".as_bytes());
    assert_eq!(built.get(), 0);

    // Refused: the message names the class that arrived, built once each.
    for (code, class) in NOT_STRINGS {
        let got = eval(ruby, code)?;
        let named = || {
            built.set(built.get() + 1);
            format!("a key is a String, not {}", got.class())
        };
        raises(exact_string_with(ruby, got, named), type_error, class);
        raises(utf8_with(got, "key", named), type_error, class);
        raises(bytes_with(got, named), type_error, class);
    }
    assert_eq!(usize::from(built.get()), NOT_STRINGS.len() * 3);

    // A String subclass is refused like any other class.
    let subclass = eval(ruby, "Class.new(String).new('abc')")?;
    raises(
        bytes_with(subclass, || "exactly a String"),
        type_error,
        "exactly a String",
    );

    // Invalid UTF-8 is still about the bytes: the name, not the closure.
    let invalid = eval(ruby, r#""caf\xFF".force_encoding('UTF-8')"#)?;
    let refused = utf8_with(invalid, "key", || {
        built.set(built.get() + 1);
        "not a String"
    });
    raises(refused, range_error, "key must be valid UTF-8");
    assert_eq!(usize::from(built.get()), NOT_STRINGS.len() * 3);
    Ok(())
}

pub fn arrays(ruby: &Ruby) -> Result<(), Error> {
    let type_error = ruby.exception_type_error();
    let built = Cell::new(0_u8);
    let never = || {
        built.set(built.get() + 1);
        "never"
    };

    let list = exact_array_with(ruby, eval(ruby, "[1, 2].tap(&:singleton_class)")?, never)?;
    assert_eq!(list.len(), 2);
    let [a, b] = exact_array_n_with::<2>(ruby, eval(ruby, "[1, 2]")?, never)?;
    assert_eq!((i64::try_convert(a)?, i64::try_convert(b)?), (1, 2));
    let [] = exact_array_n_with::<0>(ruby, eval(ruby, "[]")?, never)?;
    assert_eq!(built.get(), 0);

    let name = "weights";
    for code in [
        "Fixture::List.new([1, 2])",
        "{}",
        "nil",
        "Fixture::Pair.new(1, 2)",
    ] {
        let got: Value = eval(ruby, code)?;
        let refused = exact_array_with(ruby, got, || {
            built.set(built.get() + 1);
            format!("{name} must be an Array, not {}", got.class())
        });
        raises(refused, type_error, "weights must be an Array, not ");
    }
    assert_eq!(built.get(), 4);

    // Either refusal of the fixed-length read builds the message, once.
    for code in ["[1]", "[1, 2, 3]", "Fixture::List.new([1, 2])", ":ab"] {
        let got = eval(ruby, code)?;
        let refused = exact_array_n_with::<2>(ruby, got, || {
            built.set(built.get() + 1);
            format!("a pair must be an Array of two, not {}", got.inspect())
        });
        raises(refused, type_error, "a pair must be an Array of two, not ");
    }
    assert_eq!(built.get(), 8);
    let one = eval(ruby, "[1]")?;
    let named = exact_array_n_with::<2>(ruby, one, || format!("not {}", one.inspect()));
    raises(named, type_error, "not [1]");
    Ok(())
}
