//! Classes resolved once at init, and the one exact-class check.
//!
//! # The exact-class decision
//!
//! The fleet grew three shapes of "is this value exactly our class":
//!
//! 1. `own.is_inherited(class) && class.is_inherited(own)` on `value.class()`.
//!    `value.class()` is the object's `klass` field, which is its singleton
//!    class once one was opened, so a genuine instance that had
//!    `singleton_class` called on it was refused.
//! 2. Pointer equality (`as_raw() ==`) on `value.class()`, falling back to
//!    "the klass is the value's own singleton and its superclass is the
//!    class" for an `RStruct` passed as a `Value`.
//! 3. The same as 2 on an `RStruct` parameter.
//!
//! The settled shape keeps 2's semantics and 1's mechanism: identity of two
//! classes is `a.is_inherited(b) && b.is_inherited(a)` (`Module#<=` both
//! ways, which for classes holds exactly when they are the same class,
//! because the ancestor relation has no cycles). `rb_class_inherited_p` is
//! called directly, not dispatched, so redefining `<=`, `==`, `class` or
//! `instance_of?` changes nothing. Pointer equality needs magnus's `rb-sys`
//! feature for `AsRawValue`; this needs no feature, no raw `VALUE` and no
//! `unsafe` anywhere, and costs a superclass walk only on the path that is
//! already a mismatch.
//!
//! The check, on the object's `klass`:
//!
//! - the class itself: accepted;
//! - not even a descendant of the class: refused;
//! - a class whose superclass is the class *and* which is the value's own
//!   singleton class: accepted (a genuine instance whose singleton class was
//!   opened, a `clone` of one included);
//! - anything else, every subclass instance included: refused.
//!
//! Asking for the value's singleton class creates one on a value that has
//! none. That only happens for a direct-subclass instance, which is refused
//! either way.
use std::borrow::Cow;
use std::format;
use std::sync::OnceLock;

use magnus::{
    Error, Module, Object, RArray, RClass, RModule, RStruct, Ruby, Value, prelude::*, value::Opaque,
};

/// A class resolved once and pinned against `GC.compact`. `Copy`, `Send`
/// and `Sync`, so a static (a [`Registry`]) may hold it.
#[derive(Clone, Copy)]
pub struct Class(Opaque<RClass>);

impl Class {
    /// The class `scope::name`, pinned with `gc_register_mark_object`.
    /// `NameError` when the constant is missing, `TypeError` when it is not a
    /// class. Call it from init, never per call.
    ///
    /// # Errors
    ///
    /// When the constant is missing or not a class.
    pub fn resolve(ruby: &Ruby, scope: RModule, name: &str) -> Result<Self, Error> {
        let class: RClass = scope.const_get(name)?;
        ruby.gc_register_mark_object(class);
        Ok(Self(Opaque::from(class)))
    }

    /// The class, for the thread holding the VM.
    #[must_use]
    pub fn get(self, ruby: &Ruby) -> RClass {
        ruby.get_inner(self.0)
    }
}

/// A gem's resolved classes (any `T`, typically a struct of [`Class`]),
/// filled once from init and read from every call:
///
/// ```ignore
/// struct Classes { claimed: Class, entry: Class }
/// static CLASSES: Registry<Classes> = Registry::new("outbox_machines");
///
/// // init
/// CLASSES.set(ruby, Classes {
///     claimed: Class::resolve(ruby, module, "Claimed")?,
///     entry: Class::resolve(ruby, module, "Entry")?,
/// })?;
/// // a call
/// let class = CLASSES.get(ruby)?.claimed.get(ruby);
/// ```
pub struct Registry<T> {
    held: OnceLock<T>,
    gem: &'static str,
}

impl<T> Registry<T> {
    /// An empty registry for `gem`, named in the errors it raises.
    #[must_use]
    pub const fn new(gem: &'static str) -> Self {
        Self {
            held: OnceLock::new(),
            gem,
        }
    }

    /// Stores the classes. `RuntimeError` when they were already stored: the
    /// extension was initialised twice.
    ///
    /// # Errors
    ///
    /// When called a second time.
    pub fn set(&self, ruby: &Ruby, classes: T) -> Result<&T, Error> {
        if self.held.set(classes).is_err() {
            let message = format!("the {} extension was initialised twice", self.gem);
            return Err(Error::new(ruby.exception_runtime_error(), message));
        }
        self.get(ruby)
    }

    /// The classes stored at init. `RuntimeError` before then: the extension
    /// was loaded without the gem that defines them.
    ///
    /// # Errors
    ///
    /// When [`Registry::set`] has not run.
    pub fn get(&self, ruby: &Ruby) -> Result<&T, Error> {
        self.held.get().ok_or_else(|| {
            let message = format!(
                "the {gem} extension is not initialised; load {gem}, not the extension alone",
                gem = self.gem
            );
            Error::new(ruby.exception_runtime_error(), message)
        })
    }
}

/// Whether two classes are the same class, asked natively: `<=` both ways.
fn same(a: RClass, b: RClass) -> bool {
    a.is_inherited(b) && b.is_inherited(a)
}

/// Whether `value` is an instance of exactly `class`: `instance_of?`, never
/// `is_a?`, asked natively. See the module docs for the rule and why.
#[must_use]
pub fn instance_of<T: Object>(value: T, class: RClass) -> bool {
    let actual = value.class();
    if same(actual, class) {
        return true;
    }
    if !actual.is_inherited(class) {
        return false;
    }
    actual.superclass().is_ok_and(|parent| same(parent, class))
        && value.singleton_class().is_ok_and(|own| same(own, actual))
}

/// `value` as the `Data`/`Struct` it must be, an instance of exactly
/// `class`; `TypeError` with `message` otherwise, a subclass included. Read
/// its fields with [`RStruct::aref`], which runs no Ruby.
///
/// # Errors
///
/// `TypeError` when `value` is not an instance of exactly `class`.
pub fn exact_struct(
    ruby: &Ruby,
    value: Value,
    class: RClass,
    message: impl Into<Cow<'static, str>>,
) -> Result<RStruct, Error> {
    RStruct::from_value(value)
        .filter(|held| instance_of(*held, class))
        .ok_or_else(|| Error::new(ruby.exception_type_error(), message))
}

/// `value` as an `Array` that is exactly an `Array`, no subclass, so its
/// elements are read natively and never through a method it redefined.
///
/// # Errors
///
/// `TypeError` when `value` is not exactly an `Array`.
pub fn exact_array(
    ruby: &Ruby,
    value: Value,
    message: impl Into<Cow<'static, str>>,
) -> Result<RArray, Error> {
    RArray::from_value(value)
        .filter(|held| instance_of(*held, ruby.class_array()))
        .ok_or_else(|| Error::new(ruby.exception_type_error(), message))
}
