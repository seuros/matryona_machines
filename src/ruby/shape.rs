//! A sibling gem's value read by its readers alone, with no own class.
//!
//! Some arguments are only ever a sibling's: canary's `Rate.failure_of`
//! takes a `budget_machines` objective (`good`, `per`), and its
//! `Guardrail.from` a budget standing that is one of two shapes
//! (`remaining` or `overspent`). The gem defines no class of its own for
//! them, so there is nothing to read from a struct and no subclass to
//! refuse: only the readers, under the rules
//! [`own_or_sibling`](super::own_or_sibling) applies to its sibling path.
//! Every reader is asked for, public only, before any is called, so a
//! value missing one runs none; then each is called as a public method.
//!
//! **This runs Ruby**: `respond_to?` (when redefined) and each reader are
//! dispatched, may be redefined, and may re-enter the binding. Call these
//! while converting arguments, before any core call and with no `RefCell`
//! borrow held.
use std::borrow::Cow;

use magnus::{Error, Ruby, Value, value::ReprValue};

use super::refused::{Explain, Refused, refused};

/// The answers of `value`'s public `readers`, in order: a sibling gem's
/// value read by duck typing, every reader asked for before any is called.
///
/// ```ignore
/// let message = "expected an Objective (good, per)";
/// let [good, per] = sibling(ruby, objective, ["good", "per"], message)?;
/// ```
///
/// # Errors
///
/// `TypeError` with `message` when `value` lacks a public reader, before
/// any reader runs; whatever a reader raises, as it raised.
pub fn sibling<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    readers: [&str; N],
    message: impl Into<Cow<'static, str>>,
) -> Result<[Value; N], Error> {
    sibling_why(ruby, value, readers, |_| message)
}

/// [`sibling`], building the message only when it refuses, from why:
/// always [`Refused::MissingReader`], naming the first reader `value`
/// lacks publicly.
///
/// ```ignore
/// let [good, per] = sibling_why(ruby, objective, ["good", "per"], |why| match why {
///     Refused::MissingReader(reader) => format!("an objective needs #{reader}"),
///     _ => "expected an Objective (good, per)".into(),
/// })?;
/// ```
///
/// # Errors
///
/// As [`sibling`].
pub fn sibling_why<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    readers: [&str; N],
    message: impl Explain,
) -> Result<[Value; N], Error> {
    let value = value.as_value();
    if let Some(missing) = missing_reader(value, &readers)? {
        return Err(refused(ruby, message, Refused::MissingReader(missing)));
    }
    call_readers(ruby, value, &readers)
}

/// The first of `readers` that `value` does not respond to publicly.
fn missing_reader<'r>(value: Value, readers: &[&'r str]) -> Result<Option<&'r str>, Error> {
    for reader in readers {
        if !value.respond_to(*reader, false)? {
            return Ok(Some(reader));
        }
    }
    Ok(None)
}

/// Each of `readers` called on `value` as a public method, in order; the
/// caller has asked for every one.
fn call_readers<const N: usize>(
    ruby: &Ruby,
    value: Value,
    readers: &[&str],
) -> Result<[Value; N], Error> {
    let mut out = [ruby.qnil().as_value(); N];
    for (slot, reader) in out.iter_mut().zip(readers) {
        *slot = value.funcall_public(*reader, ())?;
    }
    Ok(out)
}

/// A sibling's value whose every reader of one shape it was asked for
/// answers publicly, found by [`sibling_shape`]. No reader has run yet.
///
/// Only [`sibling_shape`] makes one, so [`SiblingShape::read`] calls the
/// readers without asking for them again. Its fields are private: a
/// binding cannot claim a shape the value was never asked for.
///
/// ```compile_fail,E0451
/// use magnus::Value;
/// use matryona_machines::ruby::SiblingShape;
///
/// fn forged(value: Value) -> SiblingShape<'static> {
///     SiblingShape { value, readers: &["remaining"] }
/// }
/// ```
#[derive(Clone, Copy)]
pub struct SiblingShape<'s> {
    value: Value,
    readers: &'s [&'s str],
}

impl<'s> SiblingShape<'s> {
    /// The value.
    #[must_use]
    pub const fn value(self) -> Value {
        self.value
    }

    /// The readers of the shape found, in order.
    #[must_use]
    pub const fn readers(self) -> &'s [&'s str] {
        self.readers
    }

    /// The answers of the shape's `N` readers, in order, each called as a
    /// public method.
    ///
    /// # Errors
    ///
    /// `TypeError` with `message` when the shape has other than `N`
    /// readers (the binding disagrees with its own table), before any
    /// reader runs; whatever a reader raises, as it raised.
    pub fn read<const N: usize>(
        self,
        ruby: &Ruby,
        message: impl Into<Cow<'static, str>>,
    ) -> Result<[Value; N], Error> {
        self.read_why(ruby, |_| message)
    }

    /// [`SiblingShape::read`], building the message only when it refuses,
    /// from why: always [`Refused::FieldCount`].
    ///
    /// # Errors
    ///
    /// As [`SiblingShape::read`].
    pub fn read_why<const N: usize>(
        self,
        ruby: &Ruby,
        message: impl Explain,
    ) -> Result<[Value; N], Error> {
        if self.readers.len() != N {
            let found = self.readers.len();
            let why = Refused::FieldCount { expected: N, found };
            return Err(refused(ruby, message, why));
        }
        call_readers(ruby, self.value, self.readers)
    }
}

/// The first of `shapes` whose every reader `value` answers publicly,
/// with its index.
///
/// For a sibling's value that is one of several shapes, told
/// apart by the readers it has. `None` when it has every reader of none.
/// No reader runs; [`SiblingShape::read`] calls them.
///
/// List a shape before any whose readers it includes: the first match
/// wins.
///
/// ```ignore
/// const STANDINGS: [&[&str]; 2] = [&["remaining"], &["overspent"]];
/// let Some((index, shape)) = sibling_shape(value, &STANDINGS)? else {
///     return Err(Error::new(ruby.exception_type_error(), MESSAGE));
/// };
/// let [field] = shape.read::<1>(ruby, MESSAGE)?;
/// let standing = match index {
///     0 => Standing::Within { remaining: integer(field, "remaining")? },
///     _ => Standing::Exhausted { overspent: integer(field, "overspent")? },
/// };
/// ```
///
/// # Errors
///
/// Whatever a redefined `respond_to?` raises.
pub fn sibling_shape<'s>(
    value: impl ReprValue,
    shapes: &[&'s [&'s str]],
) -> Result<Option<(usize, SiblingShape<'s>)>, Error> {
    let value = value.as_value();
    for (index, readers) in shapes.iter().enumerate() {
        if missing_reader(value, readers)?.is_none() {
            return Ok(Some((index, SiblingShape { value, readers })));
        }
    }
    Ok(None)
}
