//! One of several classes, a Symbol or a class, and the unit variants of a
//! mixed enum read back.
use std::cell::Cell;

use magnus::{Error, RClass, Ruby, TryConvert, Value};
use matryona_machines::ruby::{
    Choice, SymbolNames, optional_which_fields, optional_which_fields_with, symbol_or_fields,
    symbol_or_fields_with, symbol_or_struct, symbol_or_struct_with, which_fields,
    which_fields_with, which_struct, which_struct_with,
};

use crate::{eval, raises};

const SETUP: &str = r"
module Fixture
  Delay = Data.define(:span)
  Window = Data.define(:from, :to, :step)
end
";

/// A mixed enum, as rogue's `Fault` and saga's `Progress` are.
#[derive(Debug, PartialEq)]
enum Fault {
    Raise,
    Garble,
    Delay(u64),
    Window { from: u64, to: u64, step: u64 },
}

matryona_machines::symbols! {
    /// Faults: two unit variants that come back from their Symbols.
    Faults for Fault as "a fault" => names_only {
        Raise => "raise",
        Delay(..) => "delay",
        Garble => "garble",
        Window { .. } => "window",
    }
}

fn class(ruby: &Ruby, code: &str) -> Result<RClass, Error> {
    RClass::try_convert(eval(ruby, code)?)
}

fn int(value: Value) -> Result<u64, Error> {
    u64::try_convert(value)
}

/// Values that are exactly none of `Delay` and `Point`.
const NEITHER: &[&str] = &[
    "Class.new(Fixture::Delay).new(1)",
    "Class.new(Fixture::Point).new(1, 2)",
    "Fixture::Forged.new(1, 2)",
    "Data.define(:span).new(1)",
    "[1]",
    "1",
    "'delay'",
];

pub fn which_readers(ruby: &Ruby) -> Result<(), Error> {
    eval(ruby, SETUP)?;
    let type_error = ruby.exception_type_error();
    let (delay, point) = (
        class(ruby, "Fixture::Delay")?,
        class(ruby, "Fixture::Point")?,
    );
    let window = class(ruby, "Fixture::Window")?;
    let one_field = [delay, point];
    let read = |code: &str| which_fields::<1>(ruby, eval(ruby, code)?, &[delay], "a delay");

    let (index, [span]) = read("Fixture::Delay.new(5).tap(&:singleton_class)")?;
    assert_eq!((index, int(span)?), (0, 5));
    let at = |code: &str| which_fields::<2>(ruby, eval(ruby, code)?, &[window, point], "two");
    let (index, [x, y]) = at("Fixture::Point.new(3, 4)")?;
    assert_eq!((index, int(x)?, int(y)?), (1, 3, 4));
    // Exactly the class, but of another length: the binding is wrong.
    raises(at("Fixture::Window.new(1, 2, 3)"), type_error, "two");
    for code in NEITHER {
        raises(read(code), type_error, "a delay");
        let both = which_fields::<1>(ruby, eval(ruby, code)?, &one_field, "a delay or a point");
        raises(both, type_error, "a delay or a point");
    }
    raises(read("nil"), type_error, "a delay");

    // Lazy: built only on refusal, once, either refusal.
    let built = Cell::new(0_u8);
    let count = || {
        built.set(built.get() + 1);
        "counted"
    };
    let five = eval(ruby, "Fixture::Delay.new(5)")?;
    which_fields_with::<1>(ruby, five, &[delay], count)?;
    assert_eq!(built.get(), 0);
    raises(
        which_fields_with::<2>(ruby, five, &[delay], count),
        type_error,
        "counted",
    );
    let nil = eval(ruby, "nil")?;
    raises(
        which_fields_with::<1>(ruby, nil, &[delay], count),
        type_error,
        "counted",
    );
    assert_eq!(built.get(), 2);

    // nil, or one of them.
    let maybe = |code: &str| optional_which_fields::<1>(ruby, eval(ruby, code)?, &[delay], "m");
    assert!(maybe("nil")?.is_none());
    let (index, [span]) = maybe("Fixture::Delay.new(7)")?.unwrap_or_else(|| panic!("a delay"));
    assert_eq!((index, int(span)?), (0, 7));
    raises(maybe("false"), type_error, "m");
    raises(maybe("Class.new(Fixture::Delay).new(1)"), type_error, "m");
    assert!(optional_which_fields_with::<1>(ruby, nil, &[delay], count)?.is_none());
    assert_eq!(built.get(), 2);

    // Different member counts: one check, then each class's own length.
    let classes = [delay, window];
    let (index, held) = which_struct(
        ruby,
        eval(ruby, "Fixture::Window.new(1, 9, 2)")?,
        &classes,
        "w",
    )?;
    assert_eq!(index, 1);
    let [from, to, step] = held.fields::<3>(ruby, "three")?;
    assert_eq!((int(from)?, int(to)?, int(step)?), (1, 9, 2));
    raises(held.fields::<1>(ruby, "not one"), type_error, "not one");
    raises(
        held.fields_with::<2>(ruby, || "not two"),
        type_error,
        "not two",
    );
    assert_eq!(held.get().size(), 3);
    let (index, held) = which_struct_with(ruby, five, &classes, count)?;
    assert_eq!(index, 0);
    assert_eq!(int(held.fields::<1>(ruby, "one")?[0])?, 5);
    for code in NEITHER {
        let refused = which_struct(ruby, eval(ruby, code)?, &classes, "neither");
        raises(refused.map(|(index, _)| index), type_error, "neither");
    }
    assert_eq!(built.get(), 2);
    Ok(())
}

pub fn units(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    assert_eq!(Faults::UNIT_NAMES, ["raise", "garble"]);
    assert_eq!(Faults::NAMES, ["raise", "delay", "garble", "window"]);
    assert_eq!(Faults::variant("garble"), Some(Fault::Garble));
    assert_eq!(Faults::variant("delay"), None);
    assert_eq!(Faults::variant("window"), None);
    assert_eq!(Faults::variant("Raise"), None);
    let parse = |code: &str| Faults::parse_unit(ruby, eval(ruby, code)?);
    assert_eq!(parse(":raise")?, Fault::Raise);
    assert_eq!(parse("'garble'.to_sym")?, Fault::Garble);
    for code in [":delay", ":window", ":nope"] {
        raises(
            parse(code),
            range_error,
            "a fault must be one of :raise, :garble",
        );
    }
    for code in ["'raise'", "nil", "Fixture::Delay.new(1)"] {
        raises(parse(code), type_error, "a fault must be a Symbol");
    }
    assert_eq!(
        Faults::name(&Fault::Window {
            from: 1,
            to: 2,
            step: 1
        }),
        "window"
    );
    assert_eq!(Faults::name(&Fault::Delay(1)), "delay");
    Ok(())
}

/// A table whose enum has no unit variant reads no Symbol back.
enum Bound {
    Deadline(u64),
}

matryona_machines::symbols! {
    /// Bounds: all data.
    Bounds for Bound as "a bound" => names_only { Deadline(..) => "deadline" }
}

pub fn no_units(ruby: &Ruby) -> Result<(), Error> {
    assert_eq!(Bounds::UNIT_NAMES, [""; 0]);
    assert!(Bounds::variant("deadline").is_none());
    let refused = Bounds::parse_unit(ruby, eval(ruby, ":deadline")?).map(|_| ());
    raises(
        refused,
        ruby.exception_range_error(),
        "a bound is never read from a Symbol",
    );
    let deadline = Bound::Deadline(1);
    assert_eq!(Bounds::name(&deadline), "deadline");
    let Bound::Deadline(at) = deadline;
    assert_eq!(at, 1);
    Ok(())
}

pub fn symbol_or_class(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let (delay, window) = (
        class(ruby, "Fixture::Delay")?,
        class(ruby, "Fixture::Window")?,
    );
    let message = "a fault is :raise, :garble or a Delay";
    let fault = |code: &str| -> Result<Fault, Error> {
        Ok(
            match symbol_or_fields(ruby, eval(ruby, code)?, Faults, &[delay], message)? {
                Choice::Symbol(fault) => fault,
                Choice::Struct(_, [span]) => Fault::Delay(int(span)?),
            },
        )
    };
    assert_eq!(fault(":raise")?, Fault::Raise);
    assert_eq!(fault(":garble")?, Fault::Garble);
    assert_eq!(fault("Fixture::Delay.new(9)")?, Fault::Delay(9));
    assert_eq!(
        fault("Fixture::Delay.new(9).tap(&:singleton_class)")?,
        Fault::Delay(9)
    );
    // A data variant's name, or any other Symbol, is out of range.
    for code in [":delay", ":window", ":Raise"] {
        raises(fault(code), range_error, message);
    }
    for code in NEITHER {
        raises(fault(code), type_error, message);
    }
    raises(fault("nil"), type_error, message);
    // Exactly the class but another length.
    let three = symbol_or_fields::<Faults, 1>(
        ruby,
        eval(ruby, "Fixture::Window.new(1, 2, 3)")?,
        Faults,
        &[window],
        message,
    );
    raises(three.map(|_| ()), type_error, message);

    // Lazy, either half.
    let built = Cell::new(0_u8);
    let count = || {
        built.set(built.get() + 1);
        "counted"
    };
    for code in [":raise", "Fixture::Delay.new(1)"] {
        symbol_or_fields_with::<Faults, 1>(ruby, eval(ruby, code)?, Faults, &[delay], count)?;
    }
    assert_eq!(built.get(), 0);
    for (code, class) in [(":nope", range_error), ("2", type_error)] {
        let refused =
            symbol_or_fields_with::<Faults, 1>(ruby, eval(ruby, code)?, Faults, &[delay], count);
        raises(refused.map(|_| ()), class, "counted");
    }
    assert_eq!(built.get(), 2);

    // Classes of different lengths: the struct half is proven, then read.
    let classes = [delay, window];
    let read = |code: &str| -> Result<Fault, Error> {
        Ok(
            match symbol_or_struct(ruby, eval(ruby, code)?, Faults, &classes, message)? {
                Choice::Symbol(fault) => fault,
                Choice::Struct(0, held) => Fault::Delay(int(held.fields::<1>(ruby, message)?[0])?),
                Choice::Struct(_, held) => {
                    let [from, to, step] = held.fields::<3>(ruby, message)?;
                    Fault::Window {
                        from: int(from)?,
                        to: int(to)?,
                        step: int(step)?,
                    }
                }
            },
        )
    };
    assert_eq!(read(":garble")?, Fault::Garble);
    assert_eq!(read("Fixture::Delay.new(4)")?, Fault::Delay(4));
    assert_eq!(
        read("Fixture::Window.new(1, 5, 2)")?,
        Fault::Window {
            from: 1,
            to: 5,
            step: 2
        }
    );
    raises(read(":window"), range_error, message);
    for code in NEITHER {
        raises(read(code), type_error, message);
    }
    let lazy = symbol_or_struct_with(ruby, eval(ruby, ":nope")?, Faults, &classes, count);
    raises(lazy.map(|_| ()), range_error, "counted");
    let lazy = symbol_or_struct_with(ruby, eval(ruby, "[]")?, Faults, &classes, count);
    raises(lazy.map(|_| ()), type_error, "counted");
    assert_eq!(built.get(), 4);
    Ok(())
}
