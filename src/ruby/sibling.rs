//! A gem's own value read natively, or a sibling gem's by its readers.
//!
//! The conventions (`CONVENTIONS.md`, "Gems") say both: the native boundary
//! is the authority, reading a gem's own `Data` values from the struct and
//! never through a reader; and a sibling gem's value is read by duck typing
//! on its readers, since a gem never depends on a sibling gem. A lease
//! token handed to an election, or to a handoff, is the lease gem's class,
//! which neither of them may name.
//!
//! [`own_or_sibling`] is the one rule they shared:
//!
//! 1. exactly the gem's own class (an opened singleton class allowed): its
//!    fields, read from the struct. No Ruby runs.
//! 2. a subclass instance of the own class: refused. The constant leaks
//!    past the seal and `Data#initialize` can be bound directly, and such
//!    a value has every reader the own class has, so letting it through
//!    the duck-typed path would undo rule 1.
//! 3. anything else that publicly responds to every reader: the readers'
//!    answers, in order, each called as a public method. Every reader is
//!    asked for before any is called, so a value missing one runs none.
//!    **This runs Ruby**: `respond_to?` (when redefined) and each reader
//!    are dispatched, may be redefined, and may re-enter the binding.
//!
//! So call it while converting arguments, before any core call and with no
//! `RefCell` borrow held, as every conversion is called: whatever a reader
//! does then can only raise, never lose work. The readers' answers are
//! values like any other argument; convert and validate them through the
//! core as the own class's fields would be.
//!
//! [`own_or_sibling_why`] hands its message closure the rule that refused
//! ([`Refused`]: a subclass, a missing reader, a struct of the wrong
//! length); [`own_or_sibling_default`] raises [`sibling_message`]'s
//! built-in text. A sibling's value the gem has no class of its own for
//! is read by [`sibling`](super::sibling) and, one of several shapes, by
//! [`sibling_shape`](super::sibling_shape).
use std::borrow::Cow;

use magnus::{Error, RClass, Ruby, Value, value::ReprValue};

use super::{
    exact::instance_of,
    fields::exact_fields_why,
    message::Message,
    names::sibling_message,
    refused::{Explain, Refused, ignoring, refused},
    shape::sibling_why,
};

/// The `N` fields of `value`, in `readers` order: from the struct when it
/// is exactly `own`, else through the public readers named, so a sibling
/// gem's value of the same shape is accepted (see the module docs).
///
/// `readers` name the own class's members in member order, which is also
/// the order the sibling's readers are called in.
///
/// ```ignore
/// let token = classes(ruby)?.token.get(ruby);
/// let [raw] = own_or_sibling(ruby, value, token, ["raw"],
///     "expected a LeaseMachines::FencingToken or a value with its raw reader")?;
/// ```
///
/// # Errors
///
/// `TypeError` with `message` for a subclass instance of `own`, for an
/// instance of exactly `own` with other than `N` members, and for any other
/// value lacking a public reader; whatever a reader raises, as it raised.
pub fn own_or_sibling<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    own: RClass,
    readers: [&str; N],
    message: impl Into<Cow<'static, str>>,
) -> Result<[Value; N], Error> {
    own_or_sibling_with(ruby, value, own, readers, || message)
}

/// [`own_or_sibling`], building the message only when it refuses.
///
/// ```ignore
/// let [raw] = own_or_sibling_with(ruby, value, token, ["raw"], || {
///     format!("expected a {token} or a value with its readers, not {}", class_name(value))
/// })?;
/// ```
///
/// # Errors
///
/// As [`own_or_sibling`].
pub fn own_or_sibling_with<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    own: RClass,
    readers: [&str; N],
    message: impl Message,
) -> Result<[Value; N], Error> {
    own_or_sibling_why(ruby, value, own, readers, ignoring(message))
}

/// [`own_or_sibling`] with a built-in message naming `own` and the
/// readers.
///
/// It is the message the lease, election and handoff gems each wrote,
/// [`sibling_message`]'s `"expected a LeaseMachines::Lease
/// or a value with #owner, #token, #expires_at"` (`"an ElectionMachines::Lease"`
/// where the name starts with a vowel), built only on refusal.
///
/// ```ignore
/// let lease = classes(ruby)?.lease.get(ruby);
/// let [owner, held, expires_at] =
///     own_or_sibling_default(ruby, value, lease, ["owner", "token", "expires_at"])?;
/// ```
///
/// # Errors
///
/// As [`own_or_sibling`], with that message for every refusal.
pub fn own_or_sibling_default<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    own: RClass,
    readers: [&str; N],
) -> Result<[Value; N], Error> {
    own_or_sibling_why(ruby, value, own, readers, |_| {
        sibling_message(own, &readers)
    })
}

/// [`own_or_sibling`], building the message only when it refuses, from
/// why it refused:
///
/// - [`Refused::FieldCount`]: exactly `own`, with other than `N` members;
/// - [`Refused::Subclass`]: a subclass instance of `own`;
/// - [`Refused::MissingReader`]: anything else, naming the first reader
///   it lacks publicly. No reader has run.
///
/// ```ignore
/// let [value] = own_or_sibling_why(ruby, watermark, own, ["value"], |why| match why {
///     Refused::Subclass => format!("{} is sealed", class_name(watermark)),
///     _ => "watermark must be a JoinMachines::Watermark or respond to #value".into(),
/// })?;
/// ```
///
/// # Errors
///
/// `TypeError` as [`own_or_sibling`], with `message`'s text for the reason;
/// whatever a reader raises, as it raised.
pub fn own_or_sibling_why<const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    own: RClass,
    readers: [&str; N],
    message: impl Explain,
) -> Result<[Value; N], Error> {
    let value = value.as_value();
    if instance_of(value, own) {
        return exact_fields_why(ruby, value, own, message);
    }
    // `rb_obj_is_kind_of`, called natively: a redefined `is_a?` changes
    // nothing.
    if value.is_kind_of(own) {
        return Err(refused(ruby, message, Refused::Subclass));
    }
    sibling_why(ruby, value, readers, message)
}
