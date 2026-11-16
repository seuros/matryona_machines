//! Why a reader refused, handed to the message closure.
use std::cell::Cell;

use magnus::value::ReprValue;
use magnus::{Error, RClass, Ruby, TryConvert, Value};
use matryona_machines::ruby::{
    Choice, Refused, exact_array_n_why, exact_fields_why, explain, optional_fields_why,
    optional_which_fields_why, symbol_or_fields_why, symbol_or_struct_why, which_fields_why,
    which_struct_why,
};

use crate::{eval, raises};

const SETUP: &str = r"
module Fixture
  class PointSub < Point; end
  Fixed = Data.define(:level)
  Span = Data.define(:from, :to, :step)
end
";

/// A mixed enum: two unit variants, one data variant.
#[derive(Debug, PartialEq)]
enum Mode {
    Off,
    Auto,
    Fixed(u64),
}

matryona_machines::symbols! {
    /// Modes: `:off`, `:auto`, or a `Fixed`.
    Modes for Mode as "a mode" => names_only {
        Off => "off",
        Auto => "auto",
        Fixed(..) => "fixed",
    }
}

/// Each reason as a short tag a test can look for in the message.
pub fn tag(why: Refused<'_>) -> String {
    match why {
        Refused::Subclass => "subclass".into(),
        Refused::MissingReader(reader) => format!("missing #{reader}"),
        Refused::FieldCount { expected, found } => format!("count {expected}/{found}"),
        Refused::UnknownSymbol(symbol) => format!("symbol :{}", symbol.name().unwrap_or_default()),
        Refused::NotFinite(real) => format!("not finite {real}"),
        Refused::WrongClass => "wrong".into(),
    }
}

fn class(ruby: &Ruby, code: &str) -> Result<RClass, Error> {
    RClass::try_convert(eval(ruby, code)?)
}

fn int(value: Value) -> Result<u64, Error> {
    u64::try_convert(value)
}

pub fn exact(ruby: &Ruby) -> Result<(), Error> {
    eval(ruby, SETUP)?;
    let type_error = ruby.exception_type_error();
    let point = class(ruby, "Fixture::Point")?;
    let read = |code: &str| exact_fields_why::<2>(ruby, eval(ruby, code)?, point, tag);
    let [x, y] = read("Fixture::Point.new(1, 2).tap(&:singleton_class)")?;
    assert_eq!((int(x)?, int(y)?), (1, 2));
    for code in ["Fixture::PointSub.new(1, 2)", "Fixture::Forged.new(1, 2)"] {
        raises(read(code), type_error, "subclass");
    }
    for code in ["Fixture::Pair.new(1, 2)", "1", "nil", ":point", "[1, 2]"] {
        raises(read(code), type_error, "wrong");
    }
    let three = exact_fields_why::<3>(ruby, eval(ruby, "Fixture::Point.new(1, 2)")?, point, tag);
    raises(three, type_error, "count 3/2");

    let maybe = |code: &str| optional_fields_why::<2>(ruby, eval(ruby, code)?, point, tag);
    assert!(maybe("nil")?.is_none());
    assert!(maybe("Fixture::Point.new(1, 2)")?.is_some());
    raises(maybe("Fixture::PointSub.new(1, 2)"), type_error, "subclass");

    // Arrays: a subclass, another length, anything else.
    let pair = |code: &str| exact_array_n_why::<2>(ruby, eval(ruby, code)?, tag);
    let [a, b] = pair("[3, 4]")?;
    assert_eq!((int(a)?, int(b)?), (3, 4));
    raises(pair("Fixture::List[1, 2]"), type_error, "subclass");
    raises(pair("[1]"), type_error, "count 2/1");
    raises(pair("[1, 2, 3]"), type_error, "count 2/3");
    raises(pair("{1 => 2}"), type_error, "wrong");

    // Built only on refusal.
    let built = Cell::new(0_u8);
    let count = |why: Refused<'_>| {
        built.set(built.get() + 1);
        tag(why)
    };
    exact_fields_why::<2>(ruby, eval(ruby, "Fixture::Point.new(1, 2)")?, point, count)?;
    exact_array_n_why::<2>(ruby, eval(ruby, "[1, 2]")?, count)?;
    assert_eq!(built.get(), 0);
    let refused = exact_array_n_why::<2>(ruby, eval(ruby, "[]")?, count);
    raises(refused, type_error, "count 2/0");
    assert_eq!(built.get(), 1);
    Ok(())
}

pub fn one_of(ruby: &Ruby) -> Result<(), Error> {
    let type_error = ruby.exception_type_error();
    let classes = [
        class(ruby, "Fixture::Point")?,
        class(ruby, "Fixture::Span")?,
    ];
    let read = |code: &str| which_fields_why::<2>(ruby, eval(ruby, code)?, &classes, tag);
    let (index, [x, _]) = read("Fixture::Point.new(5, 6)")?;
    assert_eq!((index, int(x)?), (0, 5));
    raises(read("Fixture::Span.new(1, 2, 3)"), type_error, "count 2/3");
    raises(read("Fixture::PointSub.new(1, 2)"), type_error, "subclass");
    raises(
        read("Class.new(Fixture::Span).new(1, 2, 3)"),
        type_error,
        "subclass",
    );
    raises(read("Fixture::Pair.new(1, 2)"), type_error, "wrong");

    let maybe = |code: &str| optional_which_fields_why::<2>(ruby, eval(ruby, code)?, &classes, tag);
    assert!(maybe("nil")?.is_none());
    raises(maybe("false"), type_error, "wrong");

    let held = |code: &str| which_struct_why(ruby, eval(ruby, code)?, &classes, tag);
    let (index, span_held) = held("Fixture::Span.new(1, 2, 3)")?;
    assert_eq!((index, span_held.get().size()), (1, 3));
    raises(
        held("Fixture::Forged.new(1, 2)").map(|_| ()),
        type_error,
        "subclass",
    );
    raises(held("'span'").map(|_| ()), type_error, "wrong");
    Ok(())
}

pub fn symbol_or_class(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let fixed = class(ruby, "Fixture::Fixed")?;
    let read = |code: &str| -> Result<Mode, Error> {
        match symbol_or_fields_why(ruby, eval(ruby, code)?, Modes, &[fixed], tag)? {
            Choice::Symbol(mode) => Ok(mode),
            Choice::Struct(_, [level]) => Ok(Mode::Fixed(int(level)?)),
        }
    };
    assert_eq!(read(":off")?, Mode::Off);
    assert_eq!(read(":auto")?, Mode::Auto);
    assert_eq!(read("Fixture::Fixed.new(3)")?, Mode::Fixed(3));
    // The Symbol refused is handed over: a data variant's name, or none.
    raises(read(":fixed"), range_error, "symbol :fixed");
    raises(read(":nope"), range_error, "symbol :nope");
    raises(
        read("Class.new(Fixture::Fixed).new(3)"),
        type_error,
        "subclass",
    );
    raises(read("Fixture::Point.new(1, 2)"), type_error, "wrong");
    raises(read("'off'"), type_error, "wrong");
    let wide = symbol_or_fields_why::<_, 2>(
        ruby,
        eval(ruby, "Fixture::Fixed.new(3)")?,
        Modes,
        &[fixed],
        tag,
    );
    raises(wide.map(|_| ()), type_error, "count 2/1");

    let held = |code: &str| symbol_or_struct_why(ruby, eval(ruby, code)?, Modes, &[fixed], tag);
    let Choice::Symbol(auto) = held(":auto")? else {
        panic!("a Symbol");
    };
    assert_eq!(auto, Mode::Auto);
    let Choice::Struct(0, level) = held("Fixture::Fixed.new(9)")? else {
        panic!("a Fixed");
    };
    assert_eq!(int(level.fields::<1>(ruby, "one")?[0])?, 9);
    raises(held(":fixed").map(|_| ()), range_error, "symbol :fixed");
    raises(
        held("Class.new(Fixture::Fixed).new(1)").map(|_| ()),
        type_error,
        "subclass",
    );
    raises(held("nil").map(|_| ()), type_error, "wrong");

    // The reason picks the text; the class still follows the reason.
    let message = |why: Refused<'_>| match why {
        Refused::UnknownSymbol(_) => "unknown mode".into(),
        _ => format!("not a mode: {}", tag(why)),
    };
    let refused = symbol_or_fields_why::<_, 1>(ruby, eval(ruby, ":x")?, Modes, &[fixed], message);
    raises(refused.map(|_| ()), range_error, "unknown mode");
    let refused = symbol_or_fields_why::<_, 1>(ruby, eval(ruby, "1")?, Modes, &[fixed], message);
    raises(refused.map(|_| ()), type_error, "not a mode: wrong");

    // Bound to a `let` through `explain`, with no annotation, and still
    // `Copy`: one message serves both reads.
    let message = explain(|why| match why {
        Refused::UnknownSymbol(symbol) => format!("unknown mode {}", symbol.inspect()),
        _ => format!("not a mode: {}", tag(why)),
    });
    let refused = symbol_or_fields_why::<_, 1>(ruby, eval(ruby, ":x")?, Modes, &[fixed], message);
    raises(refused.map(|_| ()), range_error, "unknown mode :x");
    let refused = symbol_or_fields_why::<_, 1>(ruby, eval(ruby, "1")?, Modes, &[fixed], message);
    raises(refused.map(|_| ()), type_error, "not a mode: wrong");
    Ok(())
}
