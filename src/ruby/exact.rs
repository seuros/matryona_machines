//! The one exact-class check, and the readers built on it.
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
//! classes ([`same_class`]) is `a.is_inherited(b) && b.is_inherited(a)`
//! (`Module#<=` both ways, which for classes holds exactly when they are the
//! same class, because the ancestor relation has no cycles).
//! `rb_class_inherited_p` is called directly, not dispatched, so redefining
//! `<=`, `==`, `class` or `instance_of?` changes nothing. Pointer equality
//! needs magnus's `rb-sys` feature for `AsRawValue`; this needs no feature,
//! no raw `VALUE` and no `unsafe` anywhere, and costs a superclass walk only
//! on the path that is already a mismatch.
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
//!
//! # Any value
//!
//! [`instance_of`] takes any `ReprValue`, a bare `Value` included. magnus
//! asks for a singleton class only through its `Object` trait, so a `Value`
//! is viewed as the heap type it is (a plain object, `Struct`/`Data`,
//! `Array`, `String`, `Hash`, typed data, `IO`, `Module`, `MatchData`)
//! before asking. A value with no such view either cannot have a singleton
//! class (an Integer, a Float, a Symbol, `nil`, `true`, `false`), so its
//! `klass` is always its class and the first rule decides it, or is a
//! `Regexp` or untyped `T_DATA` with an opened singleton class, which is
//! refused. A class object is the one value whose `klass` (its metaclass)
//! does not have its class as superclass; it is an instance of exactly
//! `Class`, as Ruby says.
use std::borrow::Cow;

use magnus::{
    Error, Module, Object, RArray, RClass, RFile, RHash, RMatch, RModule, RObject, RString,
    RStruct, RTypedData, Ruby, TryConvert, TypedData, Value, prelude::*, typed_data::Obj,
    value::ReprValue,
};

/// Whether `a` and `b` are the same class, asked natively: `Module#<=` both
/// ways through `rb_class_inherited_p`, the identity rule [`instance_of`]
/// uses. Redefining `==`, `equal?` or `<=` changes nothing.
#[must_use]
pub fn same_class(a: RClass, b: RClass) -> bool {
    a.is_inherited(b) && b.is_inherited(a)
}

/// The singleton class of `value`, through the heap type it is.
fn singleton_of(value: Value) -> Option<RClass> {
    fn ask(held: impl Object) -> Option<RClass> {
        held.singleton_class().ok()
    }
    RObject::from_value(value)
        .and_then(ask)
        .or_else(|| RStruct::from_value(value).and_then(ask))
        .or_else(|| RArray::from_value(value).and_then(ask))
        .or_else(|| RTypedData::from_value(value).and_then(ask))
        .or_else(|| RString::from_value(value).and_then(ask))
        .or_else(|| RHash::from_value(value).and_then(ask))
        .or_else(|| RFile::from_value(value).and_then(ask))
        .or_else(|| RModule::from_value(value).and_then(ask))
        .or_else(|| RMatch::from_value(value).and_then(ask))
}

/// Whether `value` is an instance of exactly `class`: `instance_of?`, never
/// `is_a?`, asked natively, for any value. See the module docs for the rule
/// and why.
#[must_use]
pub fn instance_of(value: impl ReprValue, class: RClass) -> bool {
    let value = value.as_value();
    if RClass::from_value(value).is_some() {
        return same_class(Ruby::get_with(value).class_class(), class);
    }
    let actual = value.class();
    if same_class(actual, class) {
        return true;
    }
    if !actual.is_inherited(class) {
        return false;
    }
    actual
        .superclass()
        .is_ok_and(|parent| same_class(parent, class))
        && singleton_of(value).is_some_and(|own| same_class(own, actual))
}

/// `TypeError` with `message`, built only on refusal.
fn refuse<M: Into<Cow<'static, str>>>(ruby: &Ruby, message: impl FnOnce() -> M) -> Error {
    Error::new(ruby.exception_type_error(), message())
}

/// `value` as the `Data`/`Struct` it must be, an instance of exactly
/// `class`.
///
/// `TypeError` with `message` otherwise, a subclass included. Read
/// its fields with [`RStruct::aref`], which runs no Ruby, or take them all
/// with [`exact_fields`].
///
/// # Errors
///
/// `TypeError` when `value` is not an instance of exactly `class`.
pub fn exact_struct(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Into<Cow<'static, str>>,
) -> Result<RStruct, Error> {
    exact_struct_with(ruby, value, class, || message)
}

/// [`exact_struct`], building the message only when it refuses: for a
/// message that is `format!`ted, so an accepted value allocates nothing.
///
/// ```ignore
/// let fields = exact_struct_with(ruby, value, class, || format!("{what} must be a Point"))?;
/// ```
///
/// # Errors
///
/// `TypeError` when `value` is not an instance of exactly `class`.
pub fn exact_struct_with<M: Into<Cow<'static, str>>>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl FnOnce() -> M,
) -> Result<RStruct, Error> {
    RStruct::from_value(value.as_value())
        .filter(|held| instance_of(*held, class))
        .ok_or_else(|| refuse(ruby, message))
}

/// The `N` fields of an instance of exactly the `Data`/`Struct` `class`, in
/// member order.
///
/// They are read from the struct itself: no reader runs, so a
/// redefined one is ignored and cannot re-enter. `TypeError` with `message`
/// for anything else, a struct of exactly `class` with other than `N`
/// members included (the binding and the class disagree).
///
/// ```ignore
/// let [id, attempt] = exact_fields::<2>(ruby, value, class, "expected a Claimed")?;
/// ```
///
/// # Errors
///
/// `TypeError` as above.
pub fn exact_fields<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Into<Cow<'static, str>>,
) -> Result<[Value; N], Error> {
    let message = message.into();
    let fields = exact_struct(ruby, value, class, message.clone())?;
    if fields.size() != N {
        return Err(refuse(ruby, || message));
    }
    let mut out = [ruby.qnil().as_value(); N];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = fields.aref(index)?;
    }
    Ok(out)
}

/// `value` as an `Array` that is exactly an `Array`, no subclass, so its
/// elements are read natively and never through a method it redefined.
///
/// # Errors
///
/// `TypeError` when `value` is not exactly an `Array`.
pub fn exact_array(
    ruby: &Ruby,
    value: impl ReprValue,
    message: impl Into<Cow<'static, str>>,
) -> Result<RArray, Error> {
    RArray::from_value(value.as_value())
        .filter(|held| instance_of(*held, ruby.class_array()))
        .ok_or_else(|| refuse(ruby, || message))
}

/// The `N` elements of exactly an `Array` (no subclass) of exactly `N`
/// elements, read natively. `TypeError` with `message` for anything else,
/// an Array of another length included.
///
/// ```ignore
/// let [key, value] = exact_array_n::<2>(ruby, pair, "a pair must be an Array of two")?;
/// ```
///
/// # Errors
///
/// `TypeError` as above.
pub fn exact_array_n<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    message: impl Into<Cow<'static, str>>,
) -> Result<[Value; N], Error> {
    let message = message.into();
    exact_array(ruby, value, message.clone())?
        .to_value_array::<N>()
        .map_err(|_| refuse(ruby, || message))
}

/// `value` as the wrapped Rust `T`, an instance of exactly `class`.
///
/// `TryConvert` for `Obj<T>` accepts a subclass instance, this refuses it,
/// and anything not wrapping a `T`, with `TypeError` and `message`. Pass the
/// class `T` is wrapped as, pinned at init.
///
/// ```ignore
/// let ticket: Obj<Ticket> = exact_typed(ruby, value, classes.ticket.get(ruby),
///     "ticket must be a BanditMachines::Ticket")?;
/// ```
///
/// # Errors
///
/// `TypeError` as above.
pub fn exact_typed<T: TypedData>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Into<Cow<'static, str>>,
) -> Result<Obj<T>, Error> {
    let value = value.as_value();
    if instance_of(value, class)
        && let Ok(held) = Obj::<T>::try_convert(value)
    {
        return Ok(held);
    }
    Err(refuse(ruby, || message))
}
