//! An argument that is a Symbol or one of several value classes.
//!
//! A mixed enum crosses into Ruby as a Symbol for each unit variant and a
//! `Data` value for each data variant: a distribution is `:exponential` or
//! a `Normal`, a fault `:raise`, `:garble` or a `Delay`, a disposition
//! `:dead` or a `Retry`. Reading one back is two halves, and every gem
//! wrote the Symbol half by hand. These readers take the Symbol half from
//! the enum's `names_only` [`symbols!`](crate::symbols) table, whose unit
//! arms the compiler checks, and the struct half from
//! [`which_fields`](super::which_fields)'s one exact-class check.
use std::borrow::Cow;

use magnus::{Error, RClass, RStruct, Ruby, Symbol, Value, value::ReprValue};

use super::{
    exact::which,
    fields::struct_fields,
    message::{Message, refusal},
    symbol::SymbolNames,
    which::ExactStruct,
};

/// What [`symbol_or_fields`] and [`symbol_or_struct`] read: a unit variant
/// named by its Symbol, or a struct of exactly one of the classes, with
/// the index of that class in the slice given.
#[derive(Clone, Copy)]
pub enum Choice<V, S> {
    /// A unit variant, read from its Symbol.
    Symbol(V),
    /// Exactly an instance of the class at this index: its fields
    /// ([`symbol_or_fields`]) or the proven struct ([`symbol_or_struct`]).
    Struct(usize, S),
}

/// A unit variant of `T`'s enum, from its Symbol, or the index and `N`
/// fields of an instance of exactly one of `classes`.
///
/// The table is passed by value (a `symbols!` table is a unit struct) so
/// that `N` can be inferred from the pattern the answer meets:
///
/// ```ignore
/// matryona_machines::symbols! {
///     pub Votes for Vote as "a vote" => names_only {
///         Granted => "granted",
///         Duplicate => "duplicate",
///         Denied(..) => "denied",
///     }
/// }
///
/// let message = "vote must be :granted, :duplicate or a Denied";
/// let vote = match symbol_or_fields(ruby, value, Votes, &[denied], message)? {
///     Choice::Symbol(vote) => vote,
///     Choice::Struct(_, [reason]) => Vote::Denied(Denials::parse(ruby, reason)?),
/// };
/// ```
///
/// A data variant's name (`:denied`) builds nothing: it is refused like
/// any other Symbol the table cannot read.
///
/// # Errors
///
/// `RangeError` with `message` for a Symbol naming no unit variant;
/// `TypeError` with `message` for anything else that is exactly none of
/// `classes` (a subclass instance included) or has other than `N` members.
pub fn symbol_or_fields<T: SymbolNames, const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    table: T,
    classes: &[RClass],
    message: impl Into<Cow<'static, str>>,
) -> Result<Choice<T::Variant, [Value; N]>, Error> {
    symbol_or_fields_with(ruby, value, table, classes, || message)
}

/// [`symbol_or_fields`], building the message only when it refuses.
///
/// # Errors
///
/// As [`symbol_or_fields`].
pub fn symbol_or_fields_with<T: SymbolNames, const N: usize>(
    ruby: &Ruby,
    value: impl ReprValue,
    table: T,
    classes: &[RClass],
    message: impl Message,
) -> Result<Choice<T::Variant, [Value; N]>, Error> {
    let value = value.as_value();
    if let Some(symbol) = Symbol::from_value(value) {
        return unit(ruby, symbol, table, message).map(Choice::Symbol);
    }
    let Some((index, held)) = RStruct::from_value(value)
        .filter(|held| held.size() == N)
        .and_then(|held| Some((which(held, classes)?, held)))
    else {
        return Err(refusal(ruby, message));
    };
    Ok(Choice::Struct(index, struct_fields(ruby, held)?))
}

/// [`symbol_or_fields`] for classes whose member counts differ: the struct
/// half answers the [`ExactStruct`], already proven exactly the class at
/// its index, for [`ExactStruct::fields`] to read at that class's length.
///
/// ```ignore
/// match symbol_or_struct(ruby, value, Actions, &actions, MESSAGE)? {
///     Choice::Symbol(action) => action,
///     Choice::Struct(0, held) => { let [until] = held.fields::<1>(ruby, MESSAGE)?; /* .. */ }
///     Choice::Struct(_, held) => { let [from, to] = held.fields::<2>(ruby, MESSAGE)?; /* .. */ }
/// }
/// ```
///
/// # Errors
///
/// `RangeError` with `message` for a Symbol naming no unit variant;
/// `TypeError` with `message` for anything else that is exactly none of
/// `classes`.
pub fn symbol_or_struct<T: SymbolNames>(
    ruby: &Ruby,
    value: impl ReprValue,
    table: T,
    classes: &[RClass],
    message: impl Into<Cow<'static, str>>,
) -> Result<Choice<T::Variant, ExactStruct>, Error> {
    symbol_or_struct_with(ruby, value, table, classes, || message)
}

/// [`symbol_or_struct`], building the message only when it refuses.
///
/// # Errors
///
/// As [`symbol_or_struct`].
pub fn symbol_or_struct_with<T: SymbolNames>(
    ruby: &Ruby,
    value: impl ReprValue,
    table: T,
    classes: &[RClass],
    message: impl Message,
) -> Result<Choice<T::Variant, ExactStruct>, Error> {
    let value = value.as_value();
    if let Some(symbol) = Symbol::from_value(value) {
        return unit(ruby, symbol, table, message).map(Choice::Symbol);
    }
    RStruct::from_value(value)
        .and_then(|held| {
            Some(Choice::Struct(
                which(held, classes)?,
                ExactStruct::proven(held),
            ))
        })
        .ok_or_else(|| refusal(ruby, message))
}

/// The unit variant `symbol` names in `T`'s table; `RangeError` with
/// `message` for any other name.
fn unit<T: SymbolNames>(
    ruby: &Ruby,
    symbol: Symbol,
    _table: T,
    message: impl Message,
) -> Result<T::Variant, Error> {
    T::variant(&symbol.name()?)
        .ok_or_else(|| Error::new(ruby.exception_range_error(), message.text()))
}
