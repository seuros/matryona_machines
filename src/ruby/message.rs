//! Refusal messages built only when a reader refuses.
use std::borrow::Cow;

use magnus::{Error, Ruby};

/// A refusal message built only when it is needed: any `FnOnce() -> M`
/// whose `M` is a `&'static str`, a `String` or a `Cow<'static, str>`.
///
/// The `_with` readers take one, so a gem can name what it actually got
/// (`"handle must be a PoolMachines::Handle, not Integer"`) and an accepted
/// value allocates nothing:
///
/// ```ignore
/// let handle: Obj<Handle> = exact_typed_with(ruby, value, class, || {
///     format!("handle must be a PoolMachines::Handle, not {}", class_name(value))
/// })?;
/// ```
///
/// Name what arrived with [`class_name`](super::class_name), not
/// `value.class()`. `value.class()` is the value's `klass`: a value whose
/// singleton class was opened, and the rare direct-subclass instance the
/// exact check gave one while refusing it, show as `#<Class:#<Sub ...>>`,
/// and its `Display` dispatches a `to_s` the class may redefine.
/// `class_name` answers the real class's name, `Sub`, through Ruby's own
/// `Kernel#class` and `Module#to_s`. Either is Ruby, which is fine here:
/// the closure runs only on refusal, which raises before the core is
/// called, so no work can be lost. A closure that needs to know why the
/// value was refused is an [`Explain`](super::Explain), taken by the
/// `_why` readers.
///
/// It is a trait, rather than a second generic parameter beside the
/// closure, so the readers keep one explicit generic argument:
/// `exact_typed_with::<Ticket>(..)`, `exact_fields_with::<2>(..)`.
pub trait Message {
    /// Builds the message.
    fn text(self) -> Cow<'static, str>;
}

impl<F, M> Message for F
where
    F: FnOnce() -> M,
    M: Into<Cow<'static, str>>,
{
    fn text(self) -> Cow<'static, str> {
        self().into()
    }
}

/// `TypeError` with `message`, built only now, on refusal.
pub(super) fn refusal(ruby: &Ruby, message: impl Message) -> Error {
    Error::new(ruby.exception_type_error(), message.text())
}
