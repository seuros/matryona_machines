//! Wrapped types' classes fixed at init: `Class::resolve_wrapped` and
//! `Class::pin_wrapped`.
use std::panic;

use magnus::{Error, IntoValue, Module, RClass, RModule, Ruby, Value};
use matryona_machines::ruby::{Class, instance_of};

use crate::{eval, raises};

#[magnus::wrap(class = "Fixture::Meter", free_immediately, size)]
struct Meter(u8);

#[magnus::wrap(class = "Fixture::Absent", free_immediately, size)]
struct Absent;

#[magnus::wrap(class = "Fixture::Stray", free_immediately, size)]
struct Stray;

#[magnus::wrap(class = "Fixture::Ghost", free_immediately, size)]
struct Ghost;

#[magnus::wrap(class = "Fixture::Built", free_immediately, size)]
struct Built;

#[magnus::wrap(class = "Fixture::Early", free_immediately, size)]
struct Early;

const SETUP: &str = r"
module Fixture
  class Meter; end
  class Stray; end
  class Other; end
  class Early; end
end
";

/// Restores `Object.const_get` and the constant the checks replaced.
const RESTORE: &str = r"
class << Object; remove_method :const_get; end
";

fn module(ruby: &Ruby) -> Result<RModule, Error> {
    ruby.class_object().const_get("Fixture")
}

/// Bug: magnus resolved a wrapped type's class on its first conversion, by
/// `Object.const_get`, so a constant redefined after init changed the
/// class of new handles, and a `const_get` that raised was a `fatal`.
pub fn redefined_after_init(ruby: &Ruby) -> Result<(), Error> {
    eval(ruby, SETUP)?;
    let module = module(ruby)?;
    let meter = Class::resolve_wrapped::<Meter>(ruby, module, "Meter")?;

    // After init: the constant replaced, `const_get` raising, the heap
    // compacted. No Ruby runs on conversion, and the class is the pinned one.
    eval(
        ruby,
        r"
        Fixture.send(:remove_const, :Meter)
        Fixture::Meter = Class.new
        class << Object; def const_get(*) = raise(IOError, 'const_get ran'); end
        GC.start; GC.compact
        ",
    )?;
    let pinned = meter.get(ruby);
    let wrapped = ruby.obj_wrap(Meter(1));
    assert!(instance_of(wrapped, pinned));
    assert_eq!(wrapped.0, 1);
    let converted: Value = Meter(2).into_value_with(ruby);
    assert!(instance_of(converted, pinned));
    let replaced: RClass = module.const_get("Meter")?;
    assert!(!instance_of(converted, replaced));
    eval(ruby, RESTORE)?;
    Ok(())
}

/// Bug: a missing class reached magnus's `unwrap` on the first conversion;
/// at init it is a `NameError`, and nothing is cached, so defining the
/// class afterwards resolves it.
pub fn missing_at_init(ruby: &Ruby) -> Result<(), Error> {
    let module = module(ruby)?;
    let name_error = ruby.exception_name_error();
    let missing = Class::resolve_wrapped::<Absent>(ruby, module, "Absent");
    raises(missing.map(|_| ()), name_error, "Absent");
    eval(ruby, "module Fixture; class Absent; end; end")?;
    let absent = Class::resolve_wrapped::<Absent>(ruby, module, "Absent")?;
    assert!(instance_of(ruby.obj_wrap(Absent), absent.get(ruby)));

    let not_class = Class::resolve_wrapped::<Absent>(ruby, module, "NotAClass");
    raises(not_class.map(|_| ()), ruby.exception_type_error(), "");
    Ok(())
}

/// The class magnus caches must be the one pinned: a wrap path naming
/// another constant, a path naming none, and a type converted before init
/// are each a `NameError`.
pub fn mismatched(ruby: &Ruby) -> Result<(), Error> {
    let module = module(ruby)?;
    let name_error = ruby.exception_name_error();

    let stray = Class::resolve_wrapped::<Stray>(ruby, module, "Other");
    let message = "Stray is wrapped as Fixture::Stray, not the pinned Fixture::Other";
    raises(stray.map(|_| ()), name_error, message);

    // magnus's own panic, caught: silence its report for this one check.
    let phantom = module.define_class("Phantom", ruby.class_object())?;
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let ghost = Class::pin_wrapped::<Ghost>(ruby, phantom);
    panic::set_hook(hook);
    let message = "Ghost could not be resolved; expected Fixture::Phantom";
    raises(ghost.map(|_| ()), name_error, message);

    let early = ruby.obj_wrap(Early);
    eval(
        ruby,
        "Fixture.send(:remove_const, :Early); Fixture::Early = Class.new",
    )?;
    let late = Class::resolve_wrapped::<Early>(ruby, module, "Early");
    // The replacement takes the constant's name too: same name, other class.
    let message = "Early is wrapped as Fixture::Early, not the pinned Fixture::Early";
    raises(late.map(|_| ()), name_error, message);
    assert!(!instance_of(early, module.const_get::<_, RClass>("Early")?));
    Ok(())
}

/// A class the binding defines itself, pinned and fixed with no lookup.
pub fn defined(ruby: &Ruby) -> Result<(), Error> {
    let module = module(ruby)?;
    let class = module.define_class("Built", ruby.class_object())?;
    let built = Class::pin_wrapped::<Built>(ruby, class)?;
    eval(
        ruby,
        "Fixture.send(:remove_const, :Built); Fixture::Built = Class.new; GC.start; GC.compact",
    )?;
    assert!(instance_of(ruby.obj_wrap(Built), built.get(ruby)));
    Ok(())
}
