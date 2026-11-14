//! Every `ruby` feature item, inside a live VM booted on the main thread.
mod exact;
mod integers;
mod keeping;
mod symbols;

use magnus::{Error, ExceptionClass, RModule, RStruct, Ruby, Value, prelude::*};
use matryona_machines::ruby::{Class, Registry, instance_of};

/// `result` raised `class` with a message containing `message`.
#[track_caller]
pub(crate) fn raises<T: core::fmt::Debug>(
    result: Result<T, Error>,
    class: ExceptionClass,
    message: &str,
) {
    match result {
        Ok(value) => panic!("expected {class}, got Ok({value:?})"),
        Err(error) => {
            assert!(error.is_kind_of(class), "expected {class}, got {error}");
            let text = error.to_string();
            assert!(text.contains(message), "{text:?} lacks {message:?}");
        }
    }
}

pub(crate) fn eval(ruby: &Ruby, code: &str) -> Result<Value, Error> {
    ruby.eval(code)
}

pub(crate) struct Fixtures {
    pub(crate) point: Class,
}

pub(crate) static CLASSES: Registry<Fixtures> = Registry::new("matryona_fixture");

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
  class Gauge; end
  class Dial; end
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

    // A class the binding defines holds no constant to look up: pinned as
    // it is, it survives a compaction too.
    let defined = Class::pin(ruby, ruby.define_data(None, ("level",))?);
    eval(ruby, "GC.start; GC.compact")?;
    let built: Value = defined.get(ruby).funcall("new", (7,))?;
    let fields = RStruct::from_value(built).unwrap_or_else(|| panic!("a struct"));
    assert!(instance_of(fields, defined.get(ruby)));
    assert_eq!(fields.aref::<_, i64>(0)?, 7);
    Ok(())
}

fn suite(ruby: &Ruby) -> Result<(), Error> {
    eval(ruby, SETUP)?;
    let module: RModule = ruby.class_object().const_get("Fixture")?;
    registry(ruby, module)?;
    exact::exact(ruby)?;
    exact::typed(ruby)?;
    exact::shapes(ruby)?;
    exact::any_value(ruby)?;
    keeping::keeping(ruby)?;
    integers::integers(ruby)?;
    integers::wide(ruby)?;
    symbols::symbols(ruby)?;
    symbols::table(ruby)
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
