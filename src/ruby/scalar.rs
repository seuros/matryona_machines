//! Reals and booleans across the boundary, read natively.
//!
//! # Which values are reals
//!
//! A real is exactly a Float or an Integer, decided by the value's type
//! tag. A Rational, a `BigDecimal`, a String and any object answering
//! `to_f` are `TypeError`: magnus's own `f64` conversion (`rb_num2dbl`)
//! takes a Rational and calls `to_f` on anything else, and even for an
//! Integer it calls a redefined `Integer#to_f`. Nothing here dispatches.
//!
//! # Integers round to nearest
//!
//! An Integer becomes the nearest `f64`, ties to even: the value Ruby's own
//! `Integer#to_f` gives, so `real(2**53 + 1)` is what `(2**53 + 1).to_f`
//! is in Ruby. Refusing an Integer `f64` cannot hold exactly would refuse
//! `10**17`, which a caller writing a gain or a budget means as the real
//! it reads as; and a Float argument is rounded already, so an Integer
//! rounded the same way is held to the same rule, not a looser one. An
//! Integer past `f64::MAX` (after rounding) is an infinity, as in Ruby;
//! [`finite`] refuses it with the non-finite Floats.
//!
//! Integers in `i128` convert with Rust's own correctly-rounded cast.
//! Larger ones are reduced natively (`rb_big_div`, `rb_big_mul`, never
//! a method) to 65 to 128 significant bits with a sticky bit for whatever
//! was shifted out, cast, and scaled back by exact powers of two, which
//! rounds once, exactly as the infinitely precise value would.
use std::format;

use magnus::{
    Error, Float, Integer, Ruby, Value,
    value::{Qfalse, Qtrue},
};

/// 2**64 as an `f64`: exact.
const TWO_TO_64: f64 = 18_446_744_073_709_551_616.0;

/// The sign-less `magnitude` (an Integer past `i128`) as the nearest `f64`.
fn magnitude_to_f64(ruby: &Ruby, mut magnitude: Integer) -> f64 {
    let base = ruby.integer_from_u128(1 << 64);
    let mut shifts = 0_u32;
    let mut sticky = false;
    let top = loop {
        if let Ok(top) = magnitude.to_u128() {
            break top;
        }
        // At least 2**128 after 16 shifts of 64 bits: at least 2**1152.
        if shifts == 16 {
            return f64::INFINITY;
        }
        let next = magnitude / base;
        sticky |= next * base != magnitude;
        magnitude = next;
        shifts += 1;
    };
    // After a shift `top` holds at least 65 bits, so its lowest bit lies
    // below the rounding position and may stand for everything shifted out.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the one rounding, to nearest, ties to even"
    )]
    let mut rounded = (top | u128::from(sticky)) as f64;
    for _ in 0..shifts {
        rounded *= TWO_TO_64;
    }
    rounded
}

/// `integer` as the nearest `f64`, ties to even, natively.
fn integer_to_f64(integer: Integer) -> f64 {
    if let Ok(small) = integer.to_i128() {
        #[expect(
            clippy::cast_precision_loss,
            reason = "Integer#to_f rounds to nearest too"
        )]
        return small as f64;
    }
    let ruby = Ruby::get_with(integer);
    let zero = ruby.integer_from_i64(0);
    if integer < zero {
        -magnitude_to_f64(&ruby, zero - integer)
    } else {
        magnitude_to_f64(&ruby, integer)
    }
}

/// `value` as an `f64`, when it is exactly a Float or an Integer.
///
/// Read natively (see the module docs): a Float as it is, NaN and the
/// infinities included; an Integer rounded to nearest, ties to even, as
/// `Integer#to_f` would.
///
/// `TypeError` `"<name> must be a Float or Integer"` for anything else, a
/// Rational, a `BigDecimal` and an object with `to_f` included. Use
/// [`finite`] where NaN and the infinities are malformed.
///
/// ```ignore
/// let gain = real(value, "gain")?; // 1, 0.5, Float::NAN; not 1r, not "1"
/// ```
///
/// # Errors
///
/// `TypeError` as above.
pub fn real(value: Value, name: &str) -> Result<f64, Error> {
    if let Some(float) = Float::from_value(value) {
        return Ok(float.to_f64());
    }
    if let Some(integer) = Integer::from_value(value) {
        return Ok(integer_to_f64(integer));
    }
    let message = format!("{name} must be a Float or Integer");
    Err(Error::new(
        Ruby::get_with(value).exception_type_error(),
        message,
    ))
}

/// [`real`], refusing NaN and both infinities, an Integer past `f64::MAX`
/// included, with `RangeError` `"<name> must be finite"`: the right type,
/// a value outside what a core computes with.
///
/// ```ignore
/// let budget = finite(value, "budget")?; // Float::INFINITY: "budget must be finite"
/// ```
///
/// # Errors
///
/// `TypeError` as [`real`], or `RangeError` as above.
pub fn finite(value: Value, name: &str) -> Result<f64, Error> {
    let real = real(value, name)?;
    if real.is_finite() {
        return Ok(real);
    }
    let message = format!("{name} must be finite");
    Err(Error::new(
        Ruby::get_with(value).exception_range_error(),
        message,
    ))
}

/// `value` as a `bool` when it is exactly `true` or `false`.
///
/// magnus's `bool` conversion is Ruby truthiness, which takes `nil`, `0`
/// and every object. This takes the two booleans only, and raises
/// `TypeError` `"<name> must be true or false"` for anything else, `nil`
/// included.
///
/// ```ignore
/// let eligible = boolean(value, "eligible")?;
/// ```
///
/// # Errors
///
/// `TypeError` as above.
pub fn boolean(value: Value, name: &str) -> Result<bool, Error> {
    if Qtrue::from_value(value).is_some() {
        return Ok(true);
    }
    if Qfalse::from_value(value).is_some() {
        return Ok(false);
    }
    let message = format!("{name} must be true or false");
    Err(Error::new(
        Ruby::get_with(value).exception_type_error(),
        message,
    ))
}
