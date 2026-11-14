//! A gem's own value read from the struct, a sibling's by its readers.
use std::cell::Cell;

use magnus::{Error, RClass, Ruby, TryConvert, Value};
use matryona_machines::ruby::{own_or_sibling, own_or_sibling_with};

use crate::{eval, raises};

const SETUP: &str = r"
module Fixture
  # A Struct, so a genuine instance can redefine a reader on itself.
  Token = Struct.new(:scope, :raw)
  # Another gem's token: same readers, its own class.
  SiblingToken = Data.define(:scope, :raw)
  $sibling_reads = 0
  class CountingToken
    def scope = ($sibling_reads += 1; 3)
    def raw = ($sibling_reads += 1; 4)
  end
  class HalfToken
    def scope = ($sibling_reads += 1; 3)
  end
  class PrivateRaw
    def scope = ($sibling_reads += 1; 3)
    private def raw = ($sibling_reads += 1; 4)
  end
  class Raising
    def scope = raise(IOError, 'reader raised')
    def raw = 4
  end
end
";

fn pair(fields: [Value; 2]) -> Result<(u64, u64), Error> {
    Ok((u64::try_convert(fields[0])?, u64::try_convert(fields[1])?))
}

fn reads(ruby: &Ruby) -> Result<i64, Error> {
    i64::try_convert(eval(ruby, "$sibling_reads")?)
}

pub fn siblings(ruby: &Ruby) -> Result<(), Error> {
    eval(ruby, SETUP)?;
    let type_error = ruby.exception_type_error();
    let own = RClass::try_convert(eval(ruby, "Fixture::Token")?)?;
    let message = "expected a Fixture::Token or a value with its readers";
    let read = |code: &str| own_or_sibling(ruby, eval(ruby, code)?, own, ["scope", "raw"], message);

    // Own class: the struct, never a reader, even one redefined on it.
    let opened = "Fixture::Token.new(1, 2).tap { |t| def t.raw = ($sibling_reads += 1; 99) }";
    assert_eq!(pair(read(opened)?)?, (1, 2));
    assert_eq!(pair(read("Fixture::Token.new(1, 2)")?)?, (1, 2));
    assert_eq!(reads(ruby)?, 0);

    // A sibling's value: its public readers, in the order named.
    assert_eq!(pair(read("Fixture::SiblingToken.new(5, 6)")?)?, (5, 6));
    assert_eq!(pair(read("Fixture::CountingToken.new")?)?, (3, 4));
    assert_eq!(reads(ruby)?, 2);
    let swapped = own_or_sibling(
        ruby,
        eval(ruby, "Fixture::SiblingToken.new(5, 6)")?,
        own,
        ["raw", "scope"],
        message,
    )?;
    assert_eq!(pair(swapped)?, (6, 5));

    // A subclass of the own class has every reader, and is still refused:
    // the duck-typed path would otherwise read a forgery.
    for code in [
        "Class.new(Fixture::Token).new(1, 2)",
        "Class.new(Fixture::Token).new(1, 2).tap(&:singleton_class)",
        "Class.new(Class.new(Fixture::Token)).new(1, 2)",
        "Class.new(Fixture::Token) { def is_a?(_) = false; def kind_of?(_) = false }.new(1, 2)",
    ] {
        raises(read(code), type_error, message);
    }
    // Missing or private readers: refused before any reader runs.
    let before = reads(ruby)?;
    for code in [
        "Fixture::HalfToken.new",
        "Fixture::PrivateRaw.new",
        "nil",
        "7",
        ":raw",
    ] {
        raises(read(code), type_error, message);
    }
    assert_eq!(reads(ruby)?, before);
    // A reader that raises: its own error, as it raised.
    raises(
        read("Fixture::Raising.new"),
        ruby.exception_io_error(),
        "reader raised",
    );

    // An own instance of another length: the binding and the class disagree.
    let short = own_or_sibling(
        ruby,
        eval(ruby, "Fixture::Token.new(1, 2)")?,
        own,
        ["raw"],
        "one",
    );
    raises(short, type_error, "one");

    // Lazy: built only on refusal.
    let built = Cell::new(0_u8);
    let count = || {
        built.set(built.get() + 1);
        "counted"
    };
    for code in [
        "Fixture::Token.new(1, 2)",
        "Fixture::SiblingToken.new(1, 2)",
    ] {
        own_or_sibling_with(ruby, eval(ruby, code)?, own, ["scope", "raw"], count)?;
    }
    assert_eq!(built.get(), 0);
    for code in [
        "Class.new(Fixture::Token).new(1, 2)",
        "Fixture::HalfToken.new",
        "1",
    ] {
        let refused = own_or_sibling_with(ruby, eval(ruby, code)?, own, ["scope", "raw"], count);
        raises(refused, type_error, "counted");
    }
    assert_eq!(built.get(), 3);
    Ok(())
}
