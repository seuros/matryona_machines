//! Lazy messages, optional structs, one-of-these-classes, and wrapped data
//! under a subclass.
use std::cell::Cell;

use magnus::{Class as _, Error, RClass, Ruby, TryConvert, Value, prelude::*, typed_data::Obj};
use matryona_machines::ruby::{
    exact_fields, exact_fields_with, exact_typed, exact_typed_with, optional_fields,
    optional_fields_with, optional_typed, optional_typed_with, which,
};

use crate::{CLASSES, eval, raises};

/// A wrapped handle, as pool's and wheel's are.
#[magnus::wrap(class = "Fixture::Ticket", free_immediately, size)]
struct Ticket(u32);

fn class(ruby: &Ruby, code: &str) -> Result<RClass, Error> {
    RClass::try_convert(eval(ruby, code)?)
}

pub fn lazy(ruby: &Ruby) -> Result<(), Error> {
    let point = CLASSES.get(ruby)?.point.get(ruby);
    let type_error = ruby.exception_type_error();
    let built = Cell::new(0_u8);
    let count = || {
        built.set(built.get() + 1);
        "never"
    };

    let genuine = eval(ruby, "Fixture::Point.new(1, 2).tap(&:singleton_class)")?;
    let [x, _] = exact_fields_with::<2>(ruby, genuine, point, count)?;
    assert_eq!(i64::try_convert(x)?, 1);
    assert_eq!(built.get(), 0);

    // Refused: wrong class, subclass, wrong arity; the message names what
    // arrived, and is built once.
    for code in ["[1, 2]", "Class.new(Fixture::Point).new(1, 2)", "nil"] {
        let got = eval(ruby, code)?;
        let refused = exact_fields_with::<2>(ruby, got, point, || {
            built.set(built.get() + 1);
            format!("origin must be a Fixture::Point, not {}", got.class())
        });
        raises(refused, type_error, "origin must be a Fixture::Point, not ");
    }
    assert_eq!(built.get(), 3);
    let wrong_arity = exact_fields_with::<3>(ruby, genuine, point, || "three fields");
    raises(wrong_arity, type_error, "three fields");
    let integer = eval(ruby, "7")?;
    let named = exact_fields_with::<2>(ruby, integer, point, || {
        format!("origin must be a Fixture::Point, not {}", integer.class())
    });
    raises(
        named,
        type_error,
        "origin must be a Fixture::Point, not Integer",
    );
    // The eager reader still delegates to the same rule.
    raises(
        exact_fields::<2>(ruby, integer, point, "eager"),
        type_error,
        "eager",
    );
    Ok(())
}

pub fn optional(ruby: &Ruby) -> Result<(), Error> {
    let point = CLASSES.get(ruby)?.point.get(ruby);
    let type_error = ruby.exception_type_error();
    let read = |code: &str| optional_fields::<2>(ruby, eval(ruby, code)?, point, "model");

    assert!(read("nil")?.is_none());
    let [x, y] = read("Fixture::Point.new(3, 4)")?.unwrap_or_else(|| panic!("fields"));
    assert_eq!((i64::try_convert(x)?, i64::try_convert(y)?), (3, 4));
    for code in [
        "false",
        "Class.new(Fixture::Point).new(1, 2)",
        "[3, 4]",
        "0",
    ] {
        raises(read(code), type_error, "model");
    }
    let built = Cell::new(false);
    let none = optional_fields_with::<2>(ruby, eval(ruby, "nil")?, point, || {
        built.set(true);
        "never"
    })?;
    assert!(none.is_none() && !built.get());
    let lazy = optional_fields_with::<2>(ruby, eval(ruby, ":model")?, point, || {
        format!("{} must be a Point or nil", "model")
    });
    raises(lazy, type_error, "model must be a Point or nil");
    Ok(())
}

pub fn one_of(ruby: &Ruby) -> Result<(), Error> {
    let point = CLASSES.get(ruby)?.point.get(ruby);
    let pair = class(ruby, "Fixture::Pair")?;
    let classes = [point, pair];
    let is =
        |code: &str| -> Result<Option<usize>, Error> { Ok(which(eval(ruby, code)?, &classes)) };
    assert_eq!(is("Fixture::Point.new(1, 2)")?, Some(0));
    assert_eq!(
        is("Fixture::Pair.new(1, 2).tap { |p| def p.x = 9 }")?,
        Some(1)
    );
    assert_eq!(is("Class.new(Fixture::Pair).new(1, 2)")?, None);
    assert_eq!(is("Fixture::Forged.new(1, 2)")?, None);
    assert_eq!(is("nil")?, None);
    assert_eq!(which(eval(ruby, "Fixture::Point.new(1, 2)")?, &[]), None);
    // The first match wins when a class is listed twice.
    assert_eq!(
        which(eval(ruby, "Fixture::Pair.new(1, 2)")?, &[pair, point, pair]),
        Some(0)
    );
    Ok(())
}

/// Ruby cannot allocate a subclass instance of a wrapped class (the
/// subclass has no allocator), but Rust can wrap a `T` under any class:
/// these are values `Obj<T>`'s `TryConvert` takes and the exact read must
/// refuse.
pub fn wrapped_subclass(ruby: &Ruby) -> Result<(), Error> {
    eval(
        ruby,
        "class Fixture::Ticket; end; class Fixture::SubTicket < Fixture::Ticket; end",
    )?;
    let ticket = class(ruby, "Fixture::Ticket")?;
    let type_error = ruby.exception_type_error();

    let genuine = ruby.obj_wrap(Ticket(1));
    assert_eq!(exact_typed::<Ticket>(ruby, genuine, ticket, "ticket")?.0, 1);
    let built = Cell::new(false);
    let held = exact_typed_with::<Ticket>(ruby, genuine, ticket, || {
        built.set(true);
        "never"
    })?;
    assert!(held.0 == 1 && !built.get());

    // As a gem defines its wrapped classes: no allocator, so Ruby cannot
    // build one, and a subclass inherits that.
    let named = class(ruby, "Fixture::SubTicket")?;
    ticket.undef_default_alloc_func();
    named.undef_default_alloc_func();
    let refused = eval(
        ruby,
        "begin; Fixture::SubTicket.allocate; rescue TypeError; :refused; end",
    )?;
    assert_eq!(refused.inspect(), ":refused");
    let grandchild = class(ruby, "Class.new(Fixture::SubTicket)")?;
    let anonymous = class(ruby, "Class.new(Fixture::Ticket)")?;
    for (number, subclass) in [(2, named), (3, grandchild), (4, anonymous)] {
        let child = ruby.obj_wrap_as(Ticket(number), subclass);
        let opened = ruby.obj_wrap_as(Ticket(number), subclass);
        opened.singleton_class()?;
        for value in [child, opened] {
            // Really a Ticket, and `TryConvert` says so.
            let converted = Obj::<Ticket>::try_convert(value.as_value())?;
            assert_eq!(converted.0, number);
            assert!(value.is_kind_of(ticket));
            raises(
                exact_typed::<Ticket>(ruby, value, ticket, "ticket must be a Fixture::Ticket"),
                type_error,
                "ticket must be a Fixture::Ticket",
            );
            let lazy = exact_typed_with::<Ticket>(ruby, value, ticket, || {
                format!("ticket must be a Fixture::Ticket, not {}", value.class())
            });
            raises(lazy, type_error, "ticket must be a Fixture::Ticket, not ");
        }
    }
    // The check gave a direct-subclass instance a singleton class, which
    // is the `klass` a message formatting `value.class()` shows.
    let child = ruby.obj_wrap_as(Ticket(5), named);
    let lazy = exact_typed_with::<Ticket>(ruby, child, ticket, || {
        format!("ticket must be a Fixture::Ticket, not {}", child.class())
    });
    raises(lazy, type_error, "not #<Class:#<Fixture::SubTicket");
    // nil, or exactly the wrapped class: the subclass instances stay out.
    let maybe = |value: Value| optional_typed::<Ticket>(ruby, value, ticket, "nil or a Ticket");
    assert!(maybe(ruby.qnil().as_value())?.is_none());
    let held = maybe(genuine.as_value())?.unwrap_or_else(|| panic!("a ticket"));
    assert_eq!(held.0, 1);
    for value in [
        ruby.obj_wrap_as(Ticket(6), named).as_value(),
        ruby.qfalse().as_value(),
        eval(ruby, "Object.new")?,
    ] {
        raises(
            maybe(value).map(|held| held.map(|t| t.0)),
            type_error,
            "nil or a Ticket",
        );
        let lazy = optional_typed_with::<Ticket>(ruby, value, ticket, || {
            format!("replacing must be a Ticket or nil, not {}", value.class())
        });
        raises(
            lazy.map(|held| held.map(|t| t.0)),
            type_error,
            "replacing must be a Ticket or nil, not ",
        );
    }
    let built = Cell::new(false);
    let none = optional_typed_with::<Ticket>(ruby, ruby.qnil(), ticket, || {
        built.set(true);
        "never"
    })?;
    assert!(none.is_none() && !built.get());
    let some = optional_typed_with::<Ticket>(ruby, genuine, ticket, || {
        built.set(true);
        "never"
    })?;
    assert!(some.is_some_and(|held| held.0 == 1) && !built.get());
    // A wrong-type value names its class plainly.
    let integer = eval(ruby, "7")?;
    let plain = exact_typed_with::<Ticket>(ruby, integer, ticket, || {
        format!("ticket must be a Fixture::Ticket, not {}", integer.class())
    });
    raises(
        plain,
        type_error,
        "ticket must be a Fixture::Ticket, not Integer",
    );
    Ok(())
}
