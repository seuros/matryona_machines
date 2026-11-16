//! Why a reader refused, handed to the message it builds.
use std::borrow::Cow;

use magnus::{Error, ExceptionClass, Ruby, Symbol};

use super::message::Message;

/// Why a reader refused a value: what an [`Explain`] closure is handed, so
/// one message can say what went wrong instead of covering every case.
///
/// Each `_why` reader documents which of these it reports. The exception
/// class follows from the reason ([`Refused::exception`]): `RangeError` for
/// [`Refused::UnknownSymbol`] and [`Refused::NotFinite`] (the right type,
/// a value outside what is accepted), `TypeError` for the rest.
///
/// ```ignore
/// let [value] = own_or_sibling_why(ruby, watermark, own, ["value"], |why| match why {
///     Refused::Subclass => format!("{} is sealed", class_name(watermark)),
///     _ => "watermark must be a JoinMachines::Watermark or respond to #value".into(),
/// })?;
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refused<'a> {
    /// An instance of a subclass of the class, or of one of the classes,
    /// asked for: kind of it (`rb_obj_is_kind_of`, called natively) but not
    /// exactly it. A sealed class's forgery lands here.
    Subclass,
    /// No public reader by this name: the first of the readers named that
    /// the value lacks, or has only privately. No reader has run.
    MissingReader(&'a str),
    /// Exactly the class, with other than `expected` members (for an
    /// Array, elements): the binding and the class disagree.
    FieldCount {
        /// The count the binding reads.
        expected: usize,
        /// The count the value has.
        found: usize,
    },
    /// A Symbol naming nothing the table can read back, a data variant's
    /// name included.
    UnknownSymbol(Symbol),
    /// A real that is NaN or an infinity, where a finite one is required.
    NotFinite(f64),
    /// Anything else: not a Symbol where one is accepted, exactly none of
    /// the classes and kind of none of them, not a real where one is.
    WrongClass,
}

impl Refused<'_> {
    /// The exception class this refusal raises: `RangeError` for
    /// [`Refused::UnknownSymbol`] and [`Refused::NotFinite`], `TypeError`
    /// for the rest.
    #[must_use]
    pub fn exception(self, ruby: &Ruby) -> ExceptionClass {
        match self {
            Self::UnknownSymbol(_) | Self::NotFinite(_) => ruby.exception_range_error(),
            Self::Subclass
            | Self::MissingReader(_)
            | Self::FieldCount { .. }
            | Self::WrongClass => ruby.exception_type_error(),
        }
    }
}

/// A refusal message built only when it is needed, from why.
///
/// Any `FnOnce(Refused<'_>) -> M` whose `M` is a `&'static str`, a `String` or
/// a `Cow<'static, str>`. The `_why` readers take one; [`Message`] is the
/// same without the reason.
///
/// Written inline, as the reader's argument, the closure's argument needs
/// no annotation. Every arm must answer one type: write `.into()` on a
/// literal arm beside a `format!` one.
///
/// ```ignore
/// use matryona_machines::ruby::prelude::*; // Refused, ReprValue for inspect()
///
/// let refusal = symbol_or_fields_why(ruby, value, Revalidations, &one_of, |why| match why {
///     Refused::UnknownSymbol(symbol) => format!("unknown revalidation {}", symbol.inspect()),
///     _ => NOT_ONE.into(),
/// })?;
/// ```
///
/// Bound to a `let` first, it does need help: with no reader around it,
/// the compiler infers the closure for one lifetime of [`Refused`] instead
/// of every lifetime, and the reader refuses it ("implementation of
/// `FnOnce` is not general enough"). Wrap it in [`explain`], or annotate
/// the argument `|why: Refused<'_>|`. Either compiles:
///
/// ```no_run
/// use magnus::{Error, RClass, Ruby, Value};
/// use matryona_machines::ruby::{Refused, class_name, explain, own_or_sibling_why};
///
/// fn wrapped(ruby: &Ruby, value: Value, own: RClass) -> Result<[Value; 1], Error> {
///     let message = explain(|why| match why {
///         Refused::MissingReader(reader) => format!("a token needs #{reader}"),
///         _ => format!("expected a token, not {}", class_name(value)),
///     });
///     own_or_sibling_why(ruby, value, own, ["raw"], message)
/// }
///
/// fn annotated(ruby: &Ruby, value: Value, own: RClass) -> Result<[Value; 1], Error> {
///     let message = |why: Refused<'_>| match why {
///         Refused::MissingReader(reader) => format!("a token needs #{reader}"),
///         _ => format!("expected a token, not {}", class_name(value)),
///     };
///     own_or_sibling_why(ruby, value, own, ["raw"], message)
/// }
/// ```
///
/// A bare `let`-bound closure does not:
///
/// ```compile_fail
/// use magnus::{Error, RClass, Ruby, Value};
/// use matryona_machines::ruby::{Refused, own_or_sibling_why};
///
/// fn bare(ruby: &Ruby, value: Value, own: RClass) -> Result<[Value; 1], Error> {
///     let message = |why| match why {
///         Refused::MissingReader(reader) => format!("a token needs #{reader}"),
///         _ => String::from("expected a token"),
///     };
///     own_or_sibling_why(ruby, value, own, ["raw"], message)
/// }
/// ```
///
/// An [`Refused::UnknownSymbol`] arm that writes `symbol.inspect()` needs
/// `magnus::value::ReprValue` in scope, where `inspect` lives; the
/// [`prelude`](super::prelude) brings it with [`Refused`], [`explain`] and
/// [`class_name`](super::class_name).
///
/// Like [`Message`] it is a trait, its text type an associated type, so
/// the readers keep one explicit generic argument:
/// `exact_fields_why::<2>(..)`. The trait's `FnOnce` supertrait is what
/// lets the compiler infer the closure's argument, higher-ranked over the
/// reader names it borrows:
///
/// ```no_run
/// use magnus::{Error, RClass, Ruby, Value};
/// use matryona_machines::ruby::{Refused, class_name, own_or_sibling_why};
///
/// fn token(ruby: &Ruby, value: Value, own: RClass) -> Result<[Value; 1], Error> {
///     own_or_sibling_why(ruby, value, own, ["raw"], |why| match why {
///         Refused::Subclass => "Fixture::Token is sealed".into(),
///         Refused::MissingReader(reader) => format!("a token needs #{reader}"),
///         _ => format!("expected a token, not {}", class_name(value)),
///     })
/// }
/// ```
///
/// A closure answering anything but text does not compile:
///
/// ```compile_fail,E0277
/// use magnus::{Error, RClass, Ruby, Value};
/// use matryona_machines::ruby::own_or_sibling_why;
///
/// fn token(ruby: &Ruby, value: Value, own: RClass) -> Result<[Value; 1], Error> {
///     own_or_sibling_why(ruby, value, own, ["raw"], |_| 7_u8)
/// }
/// ```
///
/// Nor does a [`Message`], which takes no reason, handed to a `_why`
/// reader (the `_with` twin takes it):
///
/// ```compile_fail,E0593
/// use magnus::{Error, RClass, Ruby, Value};
/// use matryona_machines::ruby::own_or_sibling_why;
///
/// fn token(ruby: &Ruby, value: Value, own: RClass) -> Result<[Value; 1], Error> {
///     own_or_sibling_why(ruby, value, own, ["raw"], || "no reason taken")
/// }
/// ```
pub trait Explain: for<'a> FnOnce(Refused<'a>) -> <Self as Explain>::Text {
    /// What the closure answers.
    type Text: Into<Cow<'static, str>>;
}

impl<F, M> Explain for F
where
    F: for<'a> FnOnce(Refused<'a>) -> M,
    M: Into<Cow<'static, str>>,
{
    type Text = M;
}

/// `message`, as an [`Explain`]: what lets a closure bound to a `let`
/// leave its argument unannotated.
///
/// Passed straight to a function whose bound is higher-ranked, the closure
/// is inferred for every lifetime of [`Refused`], as it is when written
/// inline as a reader's argument. A closure written inline needs no
/// wrapping. It answers the closure itself, not an opaque type, so one
/// that is `Copy` (it captures only `Copy` values) can serve several
/// reads.
///
/// ```no_run
/// use magnus::{Error, RClass, Ruby, Value};
/// use matryona_machines::ruby::{Refused, explain, exact_fields_why};
///
/// fn point(ruby: &Ruby, value: Value, class: RClass) -> Result<[Value; 2], Error> {
///     let message = explain(|why| match why {
///         Refused::Subclass => "Fixture::Point is sealed".into(),
///         _ => String::from("expected a Fixture::Point"),
///     });
///     exact_fields_why::<2>(ruby, value, class, message)
/// }
/// ```
pub const fn explain<F, M>(message: F) -> F
where
    F: for<'a> FnOnce(Refused<'a>) -> M,
    M: Into<Cow<'static, str>>,
{
    message
}

/// The error for `why`: [`Refused::exception`]'s class, `message`'s text,
/// built only now.
pub(super) fn refused(ruby: &Ruby, message: impl Explain, why: Refused<'_>) -> Error {
    Error::new(why.exception(ruby), message(why).into())
}

/// A [`Message`] as an [`Explain`] that ignores why: how each `_with`
/// reader runs on its `_why` twin.
pub(super) fn ignoring(message: impl Message) -> impl Explain {
    move |_: Refused<'_>| message.text()
}
