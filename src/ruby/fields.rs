//! Exact readers for a struct's fields and for wrapped Rust data.
use std::borrow::Cow;

use magnus::{
    Error, RClass, RStruct, Ruby, TryConvert, TypedData, Value, typed_data::Obj, value::ReprValue,
};

use super::{
    exact::instance_of,
    message::{Message, refusal},
    refused::{Explain, Refused, ignoring, refused},
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
///     format!("bound must be a CanaryMachines::Bound, not {}", class_name(value))
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
    exact_fields_why(ruby, value, class, ignoring(message))
}

/// [`exact_fields`], building the message only when it refuses, from why:
///
/// - [`Refused::FieldCount`]: exactly `class`, with other than `N` members;
/// - [`Refused::Subclass`]: a subclass instance of `class`;
/// - [`Refused::WrongClass`]: anything else.
///
/// ```ignore
/// let [low, high] = exact_fields_why::<2>(ruby, value, class, |why| match why {
///     Refused::Subclass => "CanaryMachines::Bound is sealed".into(),
///     _ => format!("bound must be a CanaryMachines::Bound, not {}", class_name(value)),
/// })?;
/// ```
///
/// # Errors
///
/// `TypeError` as [`exact_fields`], with `message`'s text for the reason.
pub fn exact_fields_why<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Explain,
) -> Result<[Value; N], Error> {
    let value = value.as_value();
    match RStruct::from_value(value).filter(|held| instance_of(*held, class)) {
        Some(held) if held.size() == N => struct_fields(ruby, held),
        Some(held) => {
            let why = Refused::FieldCount {
                expected: N,
                found: held.size(),
            };
            Err(refused(ruby, message, why))
        }
        None => Err(refused(ruby, message, mismatch(value, &[class]))),
    }
}

/// Why `value`, exactly none of `classes`, is refused: a subclass
/// instance of one of them, or something else.
pub(super) fn mismatch(value: Value, classes: &[RClass]) -> Refused<'static> {
    // `rb_obj_is_kind_of`, called natively.
    if classes.iter().any(|class| value.is_kind_of(*class)) {
        Refused::Subclass
    } else {
        Refused::WrongClass
    }
}

/// The first `N` fields of `held`, read from the struct; the caller has
/// checked its class and that it has `N` members.
pub(super) fn struct_fields<const N: usize>(
    ruby: &Ruby,
    held: RStruct,
) -> Result<[Value; N], Error> {
    let mut out = [ruby.qnil().as_value(); N];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = held.aref(index)?;
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

/// [`optional_fields`], building the message only when it refuses, from
/// why, as [`exact_fields_why`].
///
/// # Errors
///
/// As [`exact_fields_why`], for a value that is not `nil`.
pub fn optional_fields_why<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Explain,
) -> Result<Option<[Value; N]>, Error> {
    if value.is_nil() {
        return Ok(None);
    }
    exact_fields_why(ruby, value, class, message).map(Some)
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
///     format!("handle must be a PoolMachines::Handle, not {}", class_name(value))
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

/// `nil` as `None`, anything else as [`exact_typed`] reads it: for an
/// optional wrapped argument (a replacement that may be absent).
///
/// ```ignore
/// let replacing = optional_typed::<Replacement>(ruby, value, class,
///     "replacing must be a JournalMachines::Replacement or nil")?;
/// ```
///
/// # Errors
///
/// `TypeError` as [`exact_typed`], for a value that is not `nil`.
pub fn optional_typed<T: TypedData>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Into<Cow<'static, str>>,
) -> Result<Option<Obj<T>>, Error> {
    optional_typed_with(ruby, value, class, || message)
}

/// [`optional_typed`], building the message only when it refuses.
///
/// # Errors
///
/// `TypeError` as [`exact_typed`], for a value that is not `nil`.
pub fn optional_typed_with<T: TypedData>(
    ruby: &Ruby,
    value: impl ReprValue,
    class: RClass,
    message: impl Message,
) -> Result<Option<Obj<T>>, Error> {
    if value.is_nil() {
        return Ok(None);
    }
    exact_typed_with(ruby, value, class, message).map(Some)
}
