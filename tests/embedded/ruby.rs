//! Every `ruby` feature item, inside a live VM booted on the main thread.
use core::time::Duration;
use std::cell::RefCell;

use magnus::{
    Error, ExceptionClass, Integer, RClass, RModule, RObject, RStruct, Ruby, Symbol, Value,
    prelude::*,
};
use matryona_machines::ruby::{
    Class, Registry, duration, exact_array, exact_struct, instance_of, integer, keep, nanoseconds,
    parse, symbol,
};

/// `result` raised `class` with a message containing `message`.
#[track_caller]
fn raises<T: core::fmt::Debug>(result: Result<T, Error>, class: ExceptionClass, message: &str) {
    match result {
        Ok(value) => panic!("expected {class}, got Ok({value:?})"),
        Err(error) => {
            assert!(error.is_kind_of(class), "expected {class}, got {error}");
            let text = error.to_string();
            assert!(text.contains(message), "{text:?} lacks {message:?}");
        }
    }
}

fn eval(ruby: &Ruby, code: &str) -> Result<Value, Error> {
    ruby.eval(code)
}

struct Fixtures {
    point: Class,
}

static CLASSES: Registry<Fixtures> = Registry::new("matryona_fixture");

const SETUP: &str = r"
module Fixture
  Point = Data.define(:x, :y)
  Pair = Struct.new(:x, :y)
  class Sub < Point; end
  class Forged < Point
    def ==(_) = true
    def instance_of?(_) = true
    def class = Point
    def is_a?(_) = true
  end
  NotAClass = 42
  class List < Array; end
end
";

fn registry(ruby: &Ruby, module: RModule) -> Result<(), Error> {
    let runtime = ruby.exception_runtime_error();
    raises(
        CLASSES.get(ruby).map(|_| ()),
        runtime,
        "load matryona_fixture",
    );
    let point = Class::resolve(ruby, module, "Point")?;
    CLASSES.set(ruby, Fixtures { point })?;
    let again = CLASSES.set(ruby, Fixtures { point });
    raises(again.map(|_| ()), runtime, "initialised twice");
    let name_error = ruby.exception_name_error();
    raises(
        Class::resolve(ruby, module, "Missing").map(|_| ()),
        name_error,
        "Missing",
    );
    let not_class = Class::resolve(ruby, module, "NotAClass").map(|_| ());
    raises(not_class, ruby.exception_type_error(), "");

    // Pinned: the constant is gone and the heap compacted, the class still
    // builds values.
    let orphan = Class::resolve(ruby, module, "Sub")?;
    eval(
        ruby,
        "Fixture.send(:remove_const, :Sub); GC.start; GC.compact",
    )?;
    let built: Value = orphan.get(ruby).funcall("new", (1, 2))?;
    let fields = RStruct::from_value(built).unwrap_or_else(|| panic!("a struct"));
    assert!(!instance_of(fields, CLASSES.get(ruby)?.point.get(ruby)));
    assert_eq!(fields.aref::<_, i64>(1)?, 2);
    Ok(())
}

fn exact(ruby: &Ruby) -> Result<(), Error> {
    let point = CLASSES.get(ruby)?.point.get(ruby);
    let type_error = ruby.exception_type_error();
    let accepts = |code: &str| -> Result<bool, Error> {
        let value = eval(ruby, code)?;
        Ok(exact_struct(ruby, value, point, "expected a Point").is_ok())
    };
    assert!(accepts("Fixture::Point.new(1, 2)")?);
    assert!(accepts("Fixture::Point.new(1, 2).tap(&:singleton_class)")?);
    assert!(accepts(
        "Fixture::Point.new(1, 2).tap(&:singleton_class).clone"
    )?);
    // The fleet's former `is_inherited`-on-`klass` shape refused this value,
    // since its klass is the singleton; the settled check accepts it.
    let opened = eval(ruby, "Fixture::Point.new(1, 2).tap(&:singleton_class)")?;
    let opened = RStruct::from_value(opened).unwrap_or_else(|| panic!("a struct"));
    let klass = opened.class();
    assert!(!(klass.is_inherited(point) && point.is_inherited(klass)));
    assert!(instance_of(opened, point));
    let subclass = "Class.new(Fixture::Point).new(1, 2)";
    assert!(!accepts(subclass)?);
    assert!(!accepts(
        "Class.new(Fixture::Point).new(1, 2).tap(&:singleton_class)"
    )?);
    assert!(!accepts("Class.new(Class.new(Fixture::Point)).new(1, 2)")?);
    assert!(!accepts("Fixture::Forged.new(1, 2)")?);
    assert!(!accepts("Data.define(:x, :y).new(1, 2)")?);
    assert!(!accepts("Struct.new(:x, :y).new(1, 2)")?);
    assert!(!accepts("[1, 2]")?);
    assert!(!accepts("nil")?);
    raises(
        exact_struct(ruby, eval(ruby, subclass)?, point, "expected a Point"),
        type_error,
        "expected a Point",
    );

    // A `Data` value is frozen, so its singleton class can be opened but not
    // written. A mutable `Struct` shows the rest: a singleton with methods
    // and modules is still the class, and its fields come from the struct,
    // not from a reader it redefined.
    let pair = Class::resolve(ruby, ruby.class_object().const_get("Fixture")?, "Pair")?;
    let opened = eval(
        ruby,
        "Fixture::Pair.new(1, 2).extend(Comparable).tap { |p| def p.x = 9 }",
    )?;
    let fields = exact_struct(ruby, opened, pair.get(ruby), "expected a Pair")?;
    assert_eq!(fields.aref::<_, i64>(0)?, 1);
    let sub_pair = eval(
        ruby,
        "Class.new(Fixture::Pair).new(1, 2).tap { |p| def p.x = 9 }",
    )?;
    assert!(exact_struct(ruby, sub_pair, pair.get(ruby), "").is_err());

    let array = |code: &str| -> Result<bool, Error> {
        Ok(exact_array(ruby, eval(ruby, code)?, "expected an Array").is_ok())
    };
    assert!(array("[1, 2]")?);
    assert!(array("[1, 2].tap(&:singleton_class)")?);
    assert!(!array("Fixture::List.new([1, 2])")?);
    assert!(!array("Fixture::Point.new(1, 2)")?);
    raises(
        exact_array(ruby, eval(ruby, "Fixture::List.new")?, "expected an Array"),
        type_error,
        "expected an Array",
    );

    // A plain object, BasicObject included: no superclass walk past the root.
    let basic =
        RObject::from_value(eval(ruby, "BasicObject.new")?).unwrap_or_else(|| panic!("an object"));
    assert!(!instance_of(basic, point));
    assert!(instance_of(basic, ruby.class_basic_object()));
    let object: RClass = ruby.class_object();
    assert!(!instance_of(basic, object));
    Ok(())
}

fn keeping(ruby: &Ruby) -> Result<(), Error> {
    let receiver =
        RObject::from_value(eval(ruby, "Object.new")?).unwrap_or_else(|| panic!("an object"));
    let cell = RefCell::new(10_u32);

    // Honoured: decided on a copy, stored after the answer was built.
    let answer = keep(
        ruby,
        receiver,
        &cell,
        |n| {
            *n += 1;
            *n
        },
        |n| {
            assert!(cell.try_borrow_mut().is_ok(), "no borrow held");
            assert_eq!(
                *cell.borrow(),
                10,
                "nothing stored before the answer exists"
            );
            Ok(n * 2)
        },
    )?;
    assert_eq!((answer, *cell.borrow()), (22, 11));

    // A raise while building applies nothing.
    let failed: Result<u32, Error> = keep(
        ruby,
        receiver,
        &cell,
        |n| *n += 1,
        |()| Err(Error::new(ruby.exception_arg_error(), "boom")),
    );
    raises(failed, ruby.exception_arg_error(), "boom");
    assert_eq!(*cell.borrow(), 11);

    // A re-entrant change while building: RuntimeError, outer call dropped.
    let reentrant = keep(
        ruby,
        receiver,
        &cell,
        |n| *n += 100,
        |()| {
            *cell.borrow_mut() = 50;
            Ok(())
        },
    );
    let runtime = ruby.exception_runtime_error();
    raises(reentrant, runtime, "changed while the answer was built");
    assert_eq!(*cell.borrow(), 50);

    // An equal value written back is not a change.
    let rewritten = keep(
        ruby,
        receiver,
        &cell,
        |n| *n += 1,
        |()| {
            *cell.borrow_mut() = 50;
            Ok(())
        },
    );
    assert!(rewritten.is_ok());
    assert_eq!(*cell.borrow(), 51);

    // A borrow outstanding is RuntimeError, never a panic.
    let held = cell.borrow_mut();
    raises(keep(ruby, receiver, &cell, |_| (), Ok), runtime, "changed");
    drop(held);

    // Frozen meanwhile: FrozenError, nothing applied.
    let frozen = keep(
        ruby,
        receiver,
        &cell,
        |n| *n = 0,
        |()| {
            receiver.freeze();
            Ok(())
        },
    );
    raises(frozen, ruby.exception_frozen_error(), "frozen");
    assert_eq!(*cell.borrow(), 51);

    // Frozen up front: FrozenError before anything runs.
    let ran = core::cell::Cell::new(false);
    let early = keep(ruby, receiver, &cell, |_| ran.set(true), Ok);
    raises(early, ruby.exception_frozen_error(), "frozen");
    assert!(!ran.get());
    Ok(())
}

fn integers(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let value = |code: &str| eval(ruby, code);
    assert_eq!(integer::<u16>(value("65535")?, "scope")?, 65_535);
    raises(
        integer::<u16>(value("65536")?, "scope"),
        range_error,
        "scope must be in 0..=65535",
    );
    raises(
        integer::<u16>(value("-1")?, "scope"),
        range_error,
        "scope must be in 0..=65535",
    );
    raises(
        integer::<u16>(value("1.0")?, "scope"),
        type_error,
        "scope must be an Integer",
    );
    raises(
        integer::<u16>(value("'1'")?, "scope"),
        type_error,
        "scope must be an Integer",
    );
    raises(
        integer::<u16>(value("nil")?, "scope"),
        type_error,
        "scope must be an Integer",
    );
    let to_int = value("Object.new.tap { |o| def o.to_int = 1 }")?;
    raises(
        integer::<u64>(to_int, "id"),
        type_error,
        "id must be an Integer",
    );
    assert_eq!(integer::<u64>(value("2**64 - 1")?, "id")?, u64::MAX);
    raises(
        integer::<u64>(value("2**64")?, "id"),
        range_error,
        "id must be in 0..=18446744073709551615",
    );
    assert_eq!(integer::<u128>(value("2**100")?, "wide")?, 1 << 100);
    raises(
        integer::<i8>(value("-129")?, "delta"),
        range_error,
        "delta must be in -128..=127",
    );
    assert_eq!(integer::<i64>(value("-5")?, "delta")?, -5);
    assert_eq!(integer::<usize>(value("0")?, "index")?, 0);

    assert_eq!(
        duration(value("1_500")?, "now")?,
        Duration::from_nanos(1_500)
    );
    raises(
        duration(value("2**64")?, "now"),
        range_error,
        "now must be in 0..=18446744073709551615",
    );
    raises(
        duration(value("1.5")?, "now"),
        type_error,
        "now must be an Integer",
    );
    raises(
        duration(value("-1")?, "lease"),
        range_error,
        "lease must be in 0..=",
    );

    let max: Integer = nanoseconds(ruby, Duration::MAX);
    let expected: Integer = ruby.eval("18446744073709551615 * 1_000_000_000 + 999_999_999")?;
    assert_eq!(max.to_u128()?, expected.to_u128()?);
    assert_eq!(nanoseconds(ruby, Duration::from_nanos(7)).to_u64()?, 7);
    let round_trip = duration(
        nanoseconds(ruby, Duration::from_nanos(u64::MAX)).as_value(),
        "t",
    )?;
    assert_eq!(round_trip, Duration::from_nanos(u64::MAX));
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Pending,
    TimeRegression,
}

const fn name(state: State) -> &'static str {
    match state {
        State::Pending => "pending",
        State::TimeRegression => "time_regression",
    }
}

fn symbols(ruby: &Ruby) -> Result<(), Error> {
    let made: Symbol = symbol(ruby, name(State::TimeRegression));
    let expected: Symbol = ruby.eval(":time_regression")?;
    assert!(made.equal(expected)?);
    let all = [State::Pending, State::TimeRegression];
    let read = |code: &str| parse(ruby, eval(ruby, code)?, "state", &all, name);
    assert_eq!(read(":time_regression")?, State::TimeRegression);
    assert_eq!(read("'pending'.to_sym")?, State::Pending);
    raises(
        read("'pending'"),
        ruby.exception_type_error(),
        "state must be a Symbol",
    );
    raises(
        read(":dead"),
        ruby.exception_range_error(),
        "state must be one of :pending, :time_regression",
    );
    Ok(())
}

fn suite(ruby: &Ruby) -> Result<(), Error> {
    eval(ruby, SETUP)?;
    let module: RModule = ruby.class_object().const_get("Fixture")?;
    registry(ruby, module)?;
    exact(ruby)?;
    keeping(ruby)?;
    integers(ruby)?;
    symbols(ruby)
}

/// Fails while the VM is still up: magnus formats an error returned from
/// `init` only after tearing the VM down, which cannot work.
fn checked(ruby: &Ruby) -> Result<(), Error> {
    suite(ruby).inspect_err(|error| panic!("{error}"))
}

fn main() {
    if let Err(error) = Ruby::init(checked) {
        panic!("{error}");
    }
    println!("matryona-machines ruby feature: every check passed inside a live VM");
}
