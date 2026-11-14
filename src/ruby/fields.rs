//! Exact readers for a struct's fields and for wrapped Rust data.
use std::borrow::Cow;

use magnus::{
    Error, RClass, RStruct, Ruby, TryConvert, TypedData, Value, typed_data::Obj, value::ReprValue,
};

use super::{
    exact::instance_of,
    message::{Message, refusal},
};

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
    exact_fields_with(ruby, value, class, || message)
}

/// [`exact_fields`], building the message only when it refuses, so a
/// message that names what arrived allocates nothing for a value accepted.
///
/// ```ignore
/// let [low, high] = exact_fields_with::<2>(ruby, value, class, || {
///     format!("bound must be a CanaryMachines::Bound, not {}", value.class())
/// })?;
/// ```
///
/// # Errors
///
/// `TypeError` as [`exact_fields`].
pub fn exact_fields_with<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Message,
) -> Result<[Value; N], Error> {
    let Some(fields) = RStruct::from_value(value.as_value())
        .filter(|held| held.size() == N && instance_of(*held, class))
    else {
        return Err(refusal(ruby, message));
    };
    let mut out = [ruby.qnil().as_value(); N];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = fields.aref(index)?;
    }
    Ok(out)
}

/// `nil` as `None`, anything else as [`exact_fields`] reads it: for an
/// optional value argument (a model that may be absent).
///
/// ```ignore
/// let model = optional_fields::<3>(ruby, value, class, "model must be a Model or nil")?;
/// ```
///
/// # Errors
///
/// `TypeError` as [`exact_fields`], for a value that is not `nil`.
pub fn optional_fields<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Into<Cow<'static, str>>,
) -> Result<Option<[Value; N]>, Error> {
    optional_fields_with(ruby, value, class, || message)
}

/// [`optional_fields`], building the message only when it refuses.
///
/// # Errors
///
/// `TypeError` as [`exact_fields`], for a value that is not `nil`.
pub fn optional_fields_with<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Message,
) -> Result<Option<[Value; N]>, Error> {
    if value.is_nil() {
        return Ok(None);
    }
    exact_fields_with(ruby, value, class, message).map(Some)
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
    exact_typed_with(ruby, value, class, || message)
}

/// [`exact_typed`], building the message only when it refuses, so a gem
/// can name the class it got without allocating on success.
///
/// ```ignore
/// let handle = exact_typed_with::<Handle>(ruby, value, class, || {
///     format!("handle must be a PoolMachines::Handle, not {}", value.class())
/// })?;
/// ```
///
/// # Errors
///
/// `TypeError` as [`exact_typed`].
pub fn exact_typed_with<T: TypedData>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Message,
) -> Result<Obj<T>, Error> {
    let value = value.as_value();
    if instance_of(value, class)
        && let Ok(held) = Obj::<T>::try_convert(value)
    {
        return Ok(held);
    }
    Err(refusal(ruby, message))
}
