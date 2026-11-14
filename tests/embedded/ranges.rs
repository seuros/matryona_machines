//! Integers in a caller's range, and optional positives.
use core::num::NonZero;

use magnus::{Error, Ruby};
use matryona_machines::ruby::{integer_in, optional_positive};

use crate::{eval, raises};

/// The waiter capacity coalesce bounds `drain(max)` by.
const WAITER_CAPACITY: usize = 64;

pub fn ranges(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let value = |code: &str| eval(ruby, code);
    let drain = |code: &str| integer_in(value(code)?, "max", 0..=WAITER_CAPACITY);

    assert_eq!(drain("0")?, 0);
    assert_eq!(drain("64")?, 64);
    assert_eq!(drain("17")?, 17);
    // Past the range, past the type and below zero: one message, the range.
    for code in ["65", "-1", "2**64", "-(2**64)", "2**200"] {
        raises(drain(code), range_error, "max must be in 0..=64");
    }
    for code in [
        "1.0",
        "nil",
        "'1'",
        "Object.new.tap { |o| def o.to_int = 1 }",
    ] {
        raises(drain(code), type_error, "max must be an Integer");
    }

    let signed = |code: &str| integer_in::<i16>(value(code)?, "delta", -5..=5);
    assert_eq!(signed("-5")?, -5);
    assert_eq!(signed("5")?, 5);
    for code in ["-6", "6", "40000"] {
        raises(signed(code), range_error, "delta must be in -5..=5");
    }
    // An empty range accepts nothing.
    #[expect(clippy::reversed_empty_ranges, reason = "the empty range is the case")]
    let empty = integer_in::<u8>(value("4")?, "slot", 5..=3);
    raises(empty, range_error, "slot must be in 5..=3");
    let wide = integer_in::<u128>(value("2**100")?, "wide", 1 << 99..=1 << 101)?;
    assert_eq!(wide, 1 << 100);
    Ok(())
}

pub fn optional_positives(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let heads = |code: &str| optional_positive::<NonZero<u32>>(eval(ruby, code)?, "heads");

    assert_eq!(heads("nil")?, None);
    assert_eq!(heads("3")?, NonZero::new(3));
    assert_eq!(heads("2**32 - 1")?, NonZero::new(u32::MAX));
    for code in ["0", "-1", "2**32"] {
        raises(heads(code), range_error, "heads must be in 1..=4294967295");
    }
    for code in ["false", "1.0", "'3'"] {
        raises(heads(code), type_error, "heads must be an Integer");
    }
    Ok(())
}
