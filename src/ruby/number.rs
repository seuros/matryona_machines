//! An argument that is a Symbol from a table or a number.
//!
//! A caller's object may answer either a refusal or a real: batch's
//! `adapt` steps the caller's controller, whose `step` answers a
//! `feedback_machines` refusal by its Symbol or the output as a Float.
//! These readers take the Symbol half from a [`Symbols`] table and the
//! number half by [`real`]'s rule (exactly a Float or an Integer, read by
//! type tag, no `to_f` run) or [`finite`](super::finite)'s. Nothing here runs a Ruby
//! method.
use std::borrow::Cow;

use magnus::{Error, Float, Integer, Ruby, Symbol, Value, value::ReprValue};

use super::{
    refused::{Explain, Refused, refused},
    scalar::real,
    symbol::{Symbols, lookup},
};

/// What [`symbol_or_real`] and [`symbol_or_finite`] read: a variant named
/// by its Symbol, or a real.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SymbolOrReal<V> {
    /// A variant of the table, read from its Symbol.
    Symbol(V),
    /// Exactly a Float, or an Integer rounded to nearest, as [`real`]
    /// reads it.
    Real(f64),
}

/// A variant of `T`'s table, from its Symbol, or exactly a Float or an
/// Integer as an `f64`, NaN and the infinities included.
///
/// ```ignore
/// let stepped: Value = pid.funcall("step", (error, elapsed))?;
/// let message = "step must answer a Float or a refusal Symbol";
/// match symbol_or_real(ruby, stepped, ControllerRefusals, message)? {
///     SymbolOrReal::Symbol(refused) => return Ok(refusal(ruby, controller_refusal(refused))),
///     SymbolOrReal::Real(output) => Self::count(ruby, this, |batch| batch.retarget(output)),
/// }
/// ```
///
/// # Errors
///
/// `RangeError` with `message` for a Symbol naming no variant; `TypeError`
/// with `message` for anything else that is not exactly a Float or an
/// Integer (a Rational, a `BigDecimal`, a String included).
pub fn symbol_or_real<T: Symbols>(
    ruby: &Ruby,
    value: impl ReprValue,
    table: T,
    message: impl Into<Cow<'static, str>>,
) -> Result<SymbolOrReal<T::Variant>, Error> {
    symbol_or_real_why(ruby, value, table, |_| message)
}

/// [`symbol_or_real`], building the message only when it refuses, from
/// why: [`Refused::UnknownSymbol`] (`RangeError`) for a Symbol naming no
/// variant, [`Refused::WrongClass`] for anything else.
///
/// # Errors
///
/// As [`symbol_or_real`], with `message`'s text for the reason.
pub fn symbol_or_real_why<T: Symbols>(
    ruby: &Ruby,
    value: impl ReprValue,
    table: T,
    message: impl Explain,
) -> Result<SymbolOrReal<T::Variant>, Error> {
    read(value, table)?.map_err(|why| refused(ruby, message, why))
}

/// [`symbol_or_real`], refusing NaN and both infinities, an Integer past
/// `f64::MAX` included, as [`finite`](super::finite) does.
///
/// # Errors
///
/// As [`symbol_or_real`], and `RangeError` with `message` for a real that
/// is not finite.
pub fn symbol_or_finite<T: Symbols>(
    ruby: &Ruby,
    value: impl ReprValue,
    table: T,
    message: impl Into<Cow<'static, str>>,
) -> Result<SymbolOrReal<T::Variant>, Error> {
    symbol_or_finite_why(ruby, value, table, |_| message)
}

/// [`symbol_or_finite`], building the message only when it refuses, from
/// why: as [`symbol_or_real_why`], and [`Refused::NotFinite`]
/// (`RangeError`) with the real read, for NaN or an infinity.
///
/// ```ignore
/// let output = symbol_or_finite_why(ruby, stepped, ControllerRefusals, |why| match why {
///     Refused::UnknownSymbol(symbol) => format!("step answered {}, no refusal", symbol.inspect()),
///     Refused::NotFinite(real) => format!("step answered {real}"),
///     _ => format!("step must answer a Float or a refusal Symbol, not {}", class_name(stepped)),
/// })?;
/// ```
///
/// # Errors
///
/// As [`symbol_or_finite`], with `message`'s text for the reason.
pub fn symbol_or_finite_why<T: Symbols>(
    ruby: &Ruby,
    value: impl ReprValue,
    table: T,
    message: impl Explain,
) -> Result<SymbolOrReal<T::Variant>, Error> {
    let read = read(value, table)?.and_then(|read| match read {
        SymbolOrReal::Real(real) if !real.is_finite() => Err(Refused::NotFinite(real)),
        read => Ok(read),
    });
    read.map_err(|why| refused(ruby, message, why))
}

/// The variant or real `value` is, or why not.
fn read<T: Symbols>(
    value: impl ReprValue,
    _table: T,
) -> Result<Result<SymbolOrReal<T::Variant>, Refused<'static>>, Error> {
    let value = value.as_value();
    if let Some(symbol) = Symbol::from_value(value) {
        let variant = lookup(symbol, T::variant);
        return Ok(variant
            .map(SymbolOrReal::Symbol)
            .ok_or(Refused::UnknownSymbol(symbol)));
    }
    if !is_real(value) {
        return Ok(Err(Refused::WrongClass));
    }
    real(value, "").map(|real| Ok(SymbolOrReal::Real(real)))
}

/// Whether `value` is exactly a Float or an Integer, by type tag.
fn is_real(value: Value) -> bool {
    Float::from_value(value).is_some() || Integer::from_value(value).is_some()
}
