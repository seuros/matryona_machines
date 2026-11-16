//! One of several value classes: one exact-class check, then the fields.
//!
//! [`which`](super::which) answers which class a value is exactly an
//! instance of; reading its fields after took a second check through
//! [`exact_fields`](super::exact_fields). These readers do both at once,
//! and [`ExactStruct`] carries the first check's answer to a field read of
//! any length, for classes whose member counts differ.
use std::borrow::Cow;

use magnus::{Error, RClass, RStruct, Ruby, Value, value::ReprValue};

use super::{
    exact::which,
    fields::{mismatch, struct_fields},
    message::{Message, refusal},
    refused::{Explain, Refused, ignoring, refused},
};

/// A `Data`/`Struct` already proven an instance of exactly one of the
/// classes asked for, by [`which_struct`] or
/// [`symbol_or_struct`](super::symbol_or_struct).
///
/// Only those readers make one, so reading its fields checks the length
/// and nothing else: no second class check, and no reader runs.
#[derive(Clone, Copy)]
pub struct ExactStruct(RStruct);

impl ExactStruct {
    /// A struct the caller has just proven exactly one of its classes.
    pub(super) const fn proven(held: RStruct) -> Self {
        Self(held)
    }

    /// The struct, for [`RStruct::aref`], which runs no Ruby.
    #[must_use]
    pub const fn get(self) -> RStruct {
        self.0
    }

    /// Its `N` fields, in member order, read from the struct.
    ///
    /// ```ignore
    /// let (index, held) = which_struct(ruby, value, &[lead, window], message)?;
    /// match index {
    ///     0 => { let [at] = held.fields::<1>(ruby, message)?; /* ... */ }
    ///     _ => { let [from, to, step] = held.fields::<3>(ruby, message)?; /* ... */ }
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// `TypeError` with `message` when it has other than `N` members (the
    /// binding and the class disagree).
    pub fn fields<const N: usize>(
        self,
        ruby: &Ruby,
        message: impl Into<Cow<'static, str>>,
    ) -> Result<[Value; N], Error> {
        self.fields_with(ruby, || message)
    }

    /// [`ExactStruct::fields`], building the message only when it refuses.
    ///
    /// # Errors
    ///
    /// As [`ExactStruct::fields`].
    pub fn fields_with<const N: usize>(
        self,
        ruby: &Ruby,
        message: impl Message,
    ) -> Result<[Value; N], Error> {
        if self.0.size() != N {
            return Err(refusal(ruby, message));
        }
        struct_fields(ruby, self.0)
    }
}

/// The index of the class `value` is exactly an instance of, with the
/// struct proven so: one check, by [`instance_of`](super::instance_of)'s
/// rule. The first match wins when a class is listed twice.
///
/// For classes with different member counts; when they all have `N`,
/// [`which_fields`] reads them in the same call.
///
/// # Errors
///
/// `TypeError` with `message` when `value` is exactly none of `classes`, a
/// subclass instance of any of them included.
pub fn which_struct(
    ruby: &Ruby,
    value: impl ReprValue,
    classes: &[RClass],
    message: impl Into<Cow<'static, str>>,
) -> Result<(usize, ExactStruct), Error> {
    which_struct_with(ruby, value, classes, || message)
}

/// [`which_struct`], building the message only when it refuses.
///
/// # Errors
///
/// As [`which_struct`].
pub fn which_struct_with(
    ruby: &Ruby,
    value: impl ReprValue,
    classes: &[RClass],
    message: impl Message,
) -> Result<(usize, ExactStruct), Error> {
    which_struct_why(ruby, value, classes, ignoring(message))
}

/// [`which_struct`], building the message only when it refuses, from why:
/// [`Refused::Subclass`] for a subclass instance of one of `classes`,
/// [`Refused::WrongClass`] for anything else.
///
/// # Errors
///
/// As [`which_struct`], with `message`'s text for the reason.
pub fn which_struct_why(
    ruby: &Ruby,
    value: impl ReprValue,
    classes: &[RClass],
    message: impl Explain,
) -> Result<(usize, ExactStruct), Error> {
    let value = value.as_value();
    exactly_one(value, classes)
        .map(|(index, held)| (index, ExactStruct::proven(held)))
        .ok_or_else(|| refused(ruby, message, mismatch(value, classes)))
}

/// The struct `value` is and the index of the class it is exactly an
/// instance of, or `None`.
pub(super) fn exactly_one(value: Value, classes: &[RClass]) -> Option<(usize, RStruct)> {
    RStruct::from_value(value).and_then(|held| Some((which(held, classes)?, held)))
}

/// The `N` fields of the struct `value` is, exactly one of `classes`, with
/// that class's index; why not, otherwise.
pub(super) fn fields_of_one<const N: usize>(
    ruby: &Ruby,
    value: Value,
    classes: &[RClass],
) -> Result<Result<(usize, [Value; N]), Refused<'static>>, Error> {
    let Some((index, held)) = exactly_one(value, classes) else {
        return Ok(Err(mismatch(value, classes)));
    };
    if held.size() != N {
        let found = held.size();
        return Ok(Err(Refused::FieldCount { expected: N, found }));
    }
    Ok(Ok((index, struct_fields(ruby, held)?)))
}

/// The index of the class `value` is exactly an instance of, and its `N`
/// fields read from the struct: one class check, no reader run.
///
/// ```ignore
/// let held = [classes.within.get(ruby), classes.exhausted.get(ruby)];
/// let (index, [field]) = which_fields::<1>(ruby, value, &held, MESSAGE)?;
/// ```
///
/// # Errors
///
/// `TypeError` with `message` when `value` is exactly none of `classes` (a
/// subclass instance included) or has other than `N` members.
pub fn which_fields<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    classes: &[RClass],
    message: impl Into<Cow<'static, str>>,
) -> Result<(usize, [Value; N]), Error> {
    which_fields_with(ruby, value, classes, || message)
}

/// [`which_fields`], building the message only when it refuses.
///
/// ```ignore
/// let (index, [low, high]) = which_fields_with::<2>(ruby, value, &models, || {
///     format!("model must be a Reference or a Paired, not {}", class_name(value))
/// })?;
/// ```
///
/// # Errors
///
/// As [`which_fields`].
pub fn which_fields_with<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    classes: &[RClass],
    message: impl Message,
) -> Result<(usize, [Value; N]), Error> {
    which_fields_why(ruby, value, classes, ignoring(message))
}

/// [`which_fields`], building the message only when it refuses, from why:
///
/// - [`Refused::FieldCount`]: exactly one of `classes`, with other than
///   `N` members;
/// - [`Refused::Subclass`]: a subclass instance of one of `classes`;
/// - [`Refused::WrongClass`]: anything else.
///
/// # Errors
///
/// As [`which_fields`], with `message`'s text for the reason.
pub fn which_fields_why<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    classes: &[RClass],
    message: impl Explain,
) -> Result<(usize, [Value; N]), Error> {
    fields_of_one(ruby, value.as_value(), classes)?.map_err(|why| refused(ruby, message, why))
}

/// `nil` as `None`, anything else as [`which_fields`] reads it: for an
/// optional argument that is one of several classes.
///
/// # Errors
///
/// As [`which_fields`], for a value that is not `nil`.
pub fn optional_which_fields<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    classes: &[RClass],
    message: impl Into<Cow<'static, str>>,
) -> Result<Option<(usize, [Value; N])>, Error> {
    optional_which_fields_with(ruby, value, classes, || message)
}

/// [`optional_which_fields`], building the message only when it refuses.
///
/// # Errors
///
/// As [`which_fields`], for a value that is not `nil`.
pub fn optional_which_fields_with<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    classes: &[RClass],
    message: impl Message,
) -> Result<Option<(usize, [Value; N])>, Error> {
    if value.is_nil() {
        return Ok(None);
    }
    which_fields_with(ruby, value, classes, message).map(Some)
}

/// [`optional_which_fields`], building the message only when it refuses,
/// from why, as [`which_fields_why`].
///
/// # Errors
///
/// As [`which_fields_why`], for a value that is not `nil`.
pub fn optional_which_fields_why<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    classes: &[RClass],
    message: impl Explain,
) -> Result<Option<(usize, [Value; N])>, Error> {
    if value.is_nil() {
        return Ok(None);
    }
    which_fields_why(ruby, value, classes, message).map(Some)
}
