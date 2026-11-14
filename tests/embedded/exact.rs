//! The exact-class check and its readers.
use std::cell::Cell;

use magnus::{
    Error, RClass, RModule, RObject, RStruct, Ruby, Symbol, TryConvert, prelude::*, typed_data::Obj,
};
use matryona_machines::ruby::{
    Class, exact_array, exact_array_n, exact_fields, exact_struct, exact_struct_with, exact_typed,
    instance_of, same_class,
};

use crate::{CLASSES, eval, raises};

pub fn exact(ruby: &Ruby) -> Result<(), Error> {
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

/// Wrapped Rust data, as a gem's handle or ticket is.
#[magnus::wrap(class = "Fixture::Gauge", free_immediately, size)]
struct Gauge(u8);

/// Another wrapped type, to stand under the wrong class.
#[magnus::wrap(class = "Fixture::Dial", free_immediately, size)]
struct Dial;

pub fn typed(ruby: &Ruby) -> Result<(), Error> {
    let module: RModule = ruby.class_object().const_get("Fixture")?;
    let gauge = Class::resolve(ruby, module, "Gauge")?.get(ruby);
    let dial = Class::resolve(ruby, module, "Dial")?.get(ruby);
    let type_error = ruby.exception_type_error();
    let message = "expected a Gauge";

    let genuine = ruby.obj_wrap(Gauge(3));
    let held: Obj<Gauge> = exact_typed(ruby, genuine, gauge, message)?;
    assert_eq!(held.0, 3);
    assert!(instance_of(held, gauge));
    assert!(instance_of(genuine.as_value(), gauge));

    let opened = ruby.obj_wrap(Gauge(4));
    opened.singleton_class()?;
    assert_eq!(
        exact_typed::<Gauge>(ruby, opened.as_value(), gauge, message)?.0,
        4
    );

    // `TryConvert` takes a subclass instance; the exact read does not.
    let subclass = RClass::from_value(eval(ruby, "Class.new(Fixture::Gauge)")?)
        .unwrap_or_else(|| panic!("a class"));
    let child = ruby.obj_wrap_as(Gauge(5), subclass);
    assert!(Obj::<Gauge>::try_convert(child.as_value()).is_ok());
    let refused = exact_typed::<Gauge>(ruby, child, gauge, message);
    raises(refused.map(|_| ()), type_error, message);

    // Another wrapped type: under its own class (exact, but not a `Gauge`)
    // and under the Gauge class.
    let other = ruby.obj_wrap(Dial);
    let wrong = exact_typed::<Gauge>(ruby, other, dial, message);
    raises(wrong.map(|_| ()), type_error, message);
    let wrong = exact_typed::<Gauge>(ruby, other, gauge, message);
    raises(wrong.map(|_| ()), type_error, message);
    let wrong = exact_typed::<Gauge>(ruby, eval(ruby, "nil")?, gauge, message);
    raises(wrong.map(|_| ()), type_error, message);
    Ok(())
}

pub fn shapes(ruby: &Ruby) -> Result<(), Error> {
    let point = CLASSES.get(ruby)?.point.get(ruby);
    let type_error = ruby.exception_type_error();
    let class = |code: &str| -> Result<RClass, Error> { RClass::try_convert(eval(ruby, code)?) };

    let pair = |code: &str| exact_array_n::<2>(ruby, eval(ruby, code)?, "a pair");
    let [first, second] = pair("[1, :b]")?;
    assert_eq!(i64::try_convert(first)?, 1);
    assert!(Symbol::from_value(second).is_some());
    assert!(pair("[1, 2].tap(&:singleton_class)").is_ok());
    for code in [
        "[1]",
        "[1, 2, 3]",
        "[]",
        "Fixture::List.new([1, 2])",
        "{a: 1}",
        "nil",
    ] {
        raises(pair(code), type_error, "a pair");
    }
    let [] = exact_array_n::<0>(ruby, eval(ruby, "[]")?, "empty")?;

    // Fields come from the struct, never from a reader it redefined.
    let pair_class = class("Fixture::Pair")?;
    let opened = eval(ruby, "Fixture::Pair.new(1, 2).tap { |p| def p.x = 9 }")?;
    let [x, y] = exact_fields::<2>(ruby, opened, pair_class, "a Pair")?;
    assert_eq!((i64::try_convert(x)?, i64::try_convert(y)?), (1, 2));
    raises(
        exact_fields::<3>(ruby, opened, pair_class, "a Pair"),
        type_error,
        "a Pair",
    );
    let child = eval(ruby, "Class.new(Fixture::Pair).new(1, 2)")?;
    raises(
        exact_fields::<2>(ruby, child, pair_class, "a Pair"),
        type_error,
        "a Pair",
    );
    let data = eval(ruby, "Fixture::Point.new(3, 4).tap(&:singleton_class)")?;
    let [x, y] = exact_fields::<2>(ruby, data, point, "a Point")?;
    assert_eq!((i64::try_convert(x)?, i64::try_convert(y)?), (3, 4));

    // The lazy message is built only on refusal.
    let built = Cell::new(false);
    let genuine = eval(ruby, "Fixture::Point.new(1, 2)")?;
    exact_struct_with(ruby, genuine, point, || {
        built.set(true);
        "never"
    })?;
    assert!(!built.get());
    let refused = exact_struct_with(ruby, eval(ruby, "[1, 2]")?, point, || {
        format!("{} must be a Point", "origin")
    });
    raises(refused, type_error, "origin must be a Point");

    // Class identity: redefined `==` and `<=` on a forged class change nothing.
    assert!(same_class(point, class("Fixture::Point")?));
    assert!(!same_class(point, class("Class.new(Fixture::Point)")?));
    let forged =
        class("Class.new(Fixture::Point) { def self.==(_) = true; def self.<=(_) = true }")?;
    assert!(!same_class(point, forged) && !same_class(forged, point));
    let singleton = class("Fixture::Point.new(1, 2).singleton_class")?;
    assert!(!same_class(point, singleton));
    Ok(())
}

pub fn any_value(ruby: &Ruby) -> Result<(), Error> {
    let point = CLASSES.get(ruby)?.point.get(ruby);
    let is = |code: &str, class: RClass| -> Result<bool, Error> {
        Ok(instance_of(eval(ruby, code)?, class))
    };
    // A bare `Value`, each heap type with an opened singleton class.
    assert!(is(
        "Fixture::Point.new(1, 2).tap(&:singleton_class)",
        point
    )?);
    assert!(!is("Class.new(Fixture::Point).new(1, 2)", point)?);
    assert!(!is("Fixture::Forged.new(1, 2)", point)?);
    assert!(is(
        "String.new('s').tap(&:singleton_class)",
        ruby.class_string()
    )?);
    assert!(is("{}.tap(&:singleton_class)", ruby.class_hash())?);
    assert!(is(
        "Object.new.tap(&:singleton_class)",
        ruby.class_object()
    )?);
    assert!(is("[].tap(&:singleton_class)", ruby.class_array())?);
    assert!(is(
        "'ab'.match(/a/).tap(&:singleton_class)",
        ruby.class_match()
    )?);
    assert!(is("(1..2).dup.tap(&:singleton_class)", ruby.class_range())?);
    assert!(is(
        "Object.new.method(:hash).tap(&:singleton_class)",
        ruby.class_method()
    )?);
    assert!(!is(
        "Class.new(String).new.tap(&:singleton_class)",
        ruby.class_string()
    )?);

    // Values that cannot have a singleton class: their klass decides.
    assert!(is("5", ruby.class_integer())?);
    assert!(is("2**70", ruby.class_integer())?);
    assert!(!is("5", ruby.class_numeric())?);
    assert!(is("1.5", ruby.class_float())?);
    assert!(is(":a", ruby.class_symbol())?);
    assert!(is("nil", ruby.class_nil_class())?);
    assert!(!is("nil", ruby.class_object())?);

    // A class is an instance of exactly Class; a module, of its class.
    assert!(is("Fixture::Point", ruby.class_class())?);
    assert!(is("Fixture::Point.singleton_class", ruby.class_class())?);
    assert!(!is("Fixture::Point", ruby.class_module())?);
    assert!(!is("Fixture::Point", point)?);
    assert!(is("Comparable", ruby.class_module())?);
    assert!(is(
        "Module.new.tap(&:singleton_class)",
        ruby.class_module()
    )?);
    assert!(!is("Class.new(Module).new", ruby.class_module())?);

    // The documented gap: a Regexp with an opened singleton class.
    assert!(is("/a/", ruby.class_regexp())?);
    assert!(!is(
        "Regexp.new('a').tap(&:singleton_class)",
        ruby.class_regexp()
    )?);
    Ok(())
}
