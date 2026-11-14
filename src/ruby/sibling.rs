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
use std::borrow::Cow;

use magnus::{Error, RClass, Ruby, Value, value::ReprValue};

use super::{
    exact::instance_of,
    fields::exact_fields_with,
    message::{Message, refusal},
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
///     format!("expected a {token} or a value with its readers, not {}", value.class())
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
    let value = value.as_value();
    if instance_of(value, own) {
        return exact_fields_with(ruby, value, own, message);
    }
    // `rb_obj_is_kind_of`, called natively: a redefined `is_a?` changes
    // nothing.
    if value.is_kind_of(own) {
        return Err(refusal(ruby, message));
    }
    for reader in readers {
        if !value.respond_to(reader, false)? {
            return Err(refusal(ruby, message));
        }
    }
    let mut out = [ruby.qnil().as_value(); N];
    for (slot, reader) in out.iter_mut().zip(readers) {
        *slot = value.funcall_public(reader, ())?;
    }
    Ok(out)
}
