//! Sibling values by their readers alone, the built-in sibling message,
//! why `own_or_sibling` refused, and class names for messages.
use std::cell::Cell;

use magnus::{Error, RClass, Ruby, TryConvert, Value};
use matryona_machines::ruby::{
    Refused, article, class_name, own_or_sibling_default, own_or_sibling_why, sibling,
    sibling_message, sibling_shape, sibling_why,
};

use crate::{eval, raises, reasons::tag};

const SETUP: &str = r"
module Fixture
  class PointSub < Point; end
  Ticket = Struct.new(:id, :raw)
  SiblingTicket = Data.define(:id, :raw)
  Objective = Data.define(:good, :per)
  Within = Data.define(:remaining)
  Exhausted = Data.define(:overspent)
  $duck_reads = 0
  class Counting
    def good = ($duck_reads += 1; 9)
    def per = ($duck_reads += 1; 10)
  end
  class PrivatePer
    def good = ($duck_reads += 1; 9)
    private def per = ($duck_reads += 1; 10)
  end
  class Both
    def remaining = ($duck_reads += 1; 1)
    def overspent = ($duck_reads += 1; 2)
  end
  class Lying
    def respond_to?(*) = raise(IOError, 'respond_to? raised')
  end
  class Loud
    def self.to_s = 'Quiet'
    def self.name = 'Quiet'
    def self.inspect = 'Quiet'
  end
end
module ElectionFixture
  Lease = Data.define(:owner)
end
";

fn class(ruby: &Ruby, code: &str) -> Result<RClass, Error> {
    RClass::try_convert(eval(ruby, code)?)
}

fn int(value: Value) -> Result<u64, Error> {
    u64::try_convert(value)
}

fn reads(ruby: &Ruby) -> Result<i64, Error> {
    i64::try_convert(eval(ruby, "$duck_reads")?)
}

pub fn own_reasons(ruby: &Ruby) -> Result<(), Error> {
    eval(ruby, SETUP)?;
    let type_error = ruby.exception_type_error();
    let own = class(ruby, "Fixture::Ticket")?;
    let readers = ["id", "raw"];
    let read = |code: &str| own_or_sibling_why(ruby, eval(ruby, code)?, own, readers, tag);
    let [id, raw] = read("Fixture::SiblingTicket.new(1, 2)")?;
    assert_eq!((int(id)?, int(raw)?), (1, 2));
    raises(
        read("Class.new(Fixture::Ticket).new(1, 2)"),
        type_error,
        "subclass",
    );
    raises(read("Fixture::Within.new(1)"), type_error, "missing #id");
    raises(read("Struct.new(:id).new(1)"), type_error, "missing #raw");
    let short = own_or_sibling_why(
        ruby,
        eval(ruby, "Fixture::Ticket.new(1, 2)")?,
        own,
        ["id"],
        tag,
    );
    raises(short, type_error, "count 1/2");

    // A sealed class's subclass answers its own message, the rest another.
    let sealed = |code: &str| {
        let value = eval(ruby, code)?;
        own_or_sibling_why(ruby, value, own, readers, |why| match why {
            Refused::Subclass => format!("{} is sealed", class_name(value)),
            _ => "ticket must be a Fixture::Ticket or respond to #id, #raw".into(),
        })
    };
    raises(
        sealed("Class.new(Fixture::Ticket).new(1, 2)"),
        type_error,
        "is sealed",
    );
    raises(sealed("1"), type_error, "respond to #id, #raw");

    // The built-in message names the class and the readers, a/an picked.
    let message = "expected a Fixture::Ticket or a value with #id, #raw";
    assert_eq!(sibling_message(own, &readers), message);
    assert_eq!(sibling_message(own, &[]), "expected a Fixture::Ticket");
    let lease = class(ruby, "ElectionFixture::Lease")?;
    assert_eq!(
        sibling_message(lease, &["owner"]),
        "expected an ElectionFixture::Lease or a value with #owner"
    );
    let default = |code: &str| own_or_sibling_default(ruby, eval(ruby, code)?, own, readers);
    assert_eq!(int(default("Fixture::Ticket.new(3, 4)")?[1])?, 4);
    for code in [
        "Class.new(Fixture::Ticket).new(1, 2)",
        "nil",
        "Fixture::Within.new(1)",
    ] {
        raises(default(code), type_error, message);
    }
    let election = own_or_sibling_default(ruby, eval(ruby, "1")?, lease, ["owner"]);
    raises(
        election,
        type_error,
        "expected an ElectionFixture::Lease or",
    );

    for (name, expected) in [
        ("ElectionMachines::Lease", "an"),
        ("LeaseMachines::Lease", "a"),
        ("item", "an"),
        ("Umbrella", "an"),
        ("Integer", "an"),
        ("Object", "an"),
        ("", "a"),
        ("#<Class:0x1>", "a"),
    ] {
        assert_eq!(article(name), expected, "{name}");
    }
    Ok(())
}

pub fn siblings_only(ruby: &Ruby) -> Result<(), Error> {
    let type_error = ruby.exception_type_error();
    let readers = ["good", "per"];
    let read = |code: &str| sibling(ruby, eval(ruby, code)?, readers, "an objective");
    let [good, per] = read("Fixture::Objective.new(1, 2)")?;
    assert_eq!((int(good)?, int(per)?), (1, 2));
    let [good, per] = read("Fixture::Counting.new")?;
    assert_eq!((int(good)?, int(per)?), (9, 10));
    assert_eq!(reads(ruby)?, 2);
    let swapped = sibling(
        ruby,
        eval(ruby, "Fixture::Objective.new(1, 2)")?,
        ["per", "good"],
        "",
    )?;
    assert_eq!((int(swapped[0])?, int(swapped[1])?), (2, 1));
    // Missing or private readers: refused before any reader runs.
    for code in [
        "Fixture::PrivatePer.new",
        "nil",
        "1",
        "Fixture::Within.new(1)",
    ] {
        raises(read(code), type_error, "an objective");
    }
    assert_eq!(reads(ruby)?, 2);
    let why = |code: &str| sibling_why(ruby, eval(ruby, code)?, readers, tag);
    raises(why("Fixture::PrivatePer.new"), type_error, "missing #per");
    raises(why("Object.new"), type_error, "missing #good");
    assert_eq!(reads(ruby)?, 2);
    raises(
        read("Fixture::Lying.new"),
        ruby.exception_io_error(),
        "respond_to? raised",
    );
    let none: [Value; 0] = sibling(ruby, eval(ruby, "1")?, [], "never")?;
    assert!(none.is_empty());

    // One of several shapes, told apart by the readers it has.
    let shapes: [&[&str]; 2] = [&["remaining"], &["overspent"]];
    let shape = |code: &str| sibling_shape(eval(ruby, code)?, &shapes);
    let Some((index, within)) = shape("Fixture::Within.new(7)")? else {
        panic!("a Within");
    };
    assert_eq!((index, within.readers()), (0, &["remaining"][..]));
    assert_eq!(int(within.read::<1>(ruby, "one")?[0])?, 7);
    let Some((index, exhausted)) = shape("Fixture::Exhausted.new(3)")? else {
        panic!("an Exhausted");
    };
    assert_eq!(index, 1);
    assert_eq!(class_name(exhausted.value()), "Fixture::Exhausted");
    raises(exhausted.read::<2>(ruby, "two"), type_error, "two");
    raises(exhausted.read_why::<0>(ruby, tag), type_error, "count 0/1");
    let before = reads(ruby)?;
    let Some((index, _)) = shape("Fixture::Both.new")? else {
        panic!("both shapes");
    };
    assert_eq!(index, 0);
    assert!(shape("Fixture::Objective.new(1, 2)")?.is_none());
    assert!(shape("nil")?.is_none());
    assert_eq!(reads(ruby)?, before);
    raises(
        shape("Fixture::Lying.new").map(|found| found.map(|(index, _)| index)),
        ruby.exception_io_error(),
        "respond_to? raised",
    );

    // Lazy: built only on refusal.
    let built = Cell::new(0_u8);
    let count = |why: Refused<'_>| {
        built.set(built.get() + 1);
        tag(why)
    };
    sibling_why(
        ruby,
        eval(ruby, "Fixture::Objective.new(1, 2)")?,
        readers,
        count,
    )?;
    assert_eq!(built.get(), 0);
    raises(
        sibling_why(ruby, eval(ruby, "2")?, readers, count),
        type_error,
        "missing #good",
    );
    assert_eq!(built.get(), 1);
    Ok(())
}

pub fn class_names(ruby: &Ruby) -> Result<(), Error> {
    let name = |code: &str| eval(ruby, code).map(class_name);
    for (code, expected) in [
        ("Fixture::Point.new(1, 2)", "Fixture::Point"),
        (
            "Fixture::Point.new(1, 2).tap(&:singleton_class)",
            "Fixture::Point",
        ),
        (
            "Fixture::Pair.new(1, 2).tap { |p| def p.class = Integer }",
            "Fixture::Pair",
        ),
        (
            "Fixture::Pair.new(1, 2).tap { |p| p.extend(Comparable) }",
            "Fixture::Pair",
        ),
        ("Fixture::PointSub.new(1, 2)", "Fixture::PointSub"),
        (
            "Fixture::PointSub.new(1, 2).tap(&:singleton_class)",
            "Fixture::PointSub",
        ),
        ("Fixture::Forged.new(1, 2)", "Fixture::Forged"),
        ("Fixture::Loud.new", "Fixture::Loud"),
        ("Object.new.tap(&:singleton_class)", "Object"),
        ("BasicObject.new", "BasicObject"),
        ("'text'.tap(&:singleton_class)", "String"),
        ("[1].tap(&:singleton_class)", "Array"),
        ("1", "Integer"),
        ("1.5", "Float"),
        ("nil", "NilClass"),
        (":a", "Symbol"),
        ("Fixture::Point", "Class"),
        ("Fixture", "Module"),
        ("Fixture.tap(&:singleton_class)", "Module"),
    ] {
        assert_eq!(name(code)?, expected, "{code}");
    }
    // An anonymous class is Ruby's `#<Class:0x...>`, never the singleton.
    for code in [
        "Class.new(Fixture::Point).new(1, 2)",
        "Class.new(Fixture::Point).new(1, 2).tap(&:singleton_class)",
        "Data.define(:a).new(1)",
    ] {
        let anonymous = name(code)?;
        assert!(anonymous.starts_with("#<Class:0x"), "{code}: {anonymous}");
        assert!(!anonymous.contains("#<Class:#<"), "{code}: {anonymous}");
    }
    Ok(())
}
