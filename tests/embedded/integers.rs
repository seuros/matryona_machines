//! Integers and nanoseconds.
use core::{num::NonZero, time::Duration};

use magnus::{Error, Integer, Ruby, prelude::*};
use matryona_machines::ruby::{duration, instant, integer, nanoseconds, optional, positive};

use crate::{eval, raises};

pub fn integers(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let value = |code: &str| eval(ruby, code);
    assert_eq!(integer::<u16>(value("65535")?, "scope")?, 65_535);
    raises(
        integer::<u16>(value("65536")?, "scope"),
        range_error,
        "scope must be in 0..=65535",
    );
    raises(
        integer::<u16>(value("-1")?, "scope"),
        range_error,
        "scope must be in 0..=65535",
    );
    raises(
        integer::<u16>(value("1.0")?, "scope"),
        type_error,
        "scope must be an Integer",
    );
    raises(
        integer::<u16>(value("'1'")?, "scope"),
        type_error,
        "scope must be an Integer",
    );
    raises(
        integer::<u16>(value("nil")?, "scope"),
        type_error,
        "scope must be an Integer",
    );
    let to_int = value("Object.new.tap { |o| def o.to_int = 1 }")?;
    raises(
        integer::<u64>(to_int, "id"),
        type_error,
        "id must be an Integer",
    );
    assert_eq!(integer::<u64>(value("2**64 - 1")?, "id")?, u64::MAX);
    raises(
        integer::<u64>(value("2**64")?, "id"),
        range_error,
        "id must be in 0..=18446744073709551615",
    );
    assert_eq!(integer::<u128>(value("2**100")?, "wide")?, 1 << 100);
    raises(
        integer::<i8>(value("-129")?, "delta"),
        range_error,
        "delta must be in -128..=127",
    );
    assert_eq!(integer::<i64>(value("-5")?, "delta")?, -5);
    assert_eq!(integer::<usize>(value("0")?, "index")?, 0);

    assert_eq!(
        duration(value("1_500")?, "now")?,
        Duration::from_nanos(1_500)
    );
    raises(
        duration(value("2**64")?, "now"),
        range_error,
        "now must be in 0..=18446744073709551615",
    );
    raises(
        duration(value("1.5")?, "now"),
        type_error,
        "now must be an Integer",
    );
    raises(
        duration(value("-1")?, "lease"),
        range_error,
        "lease must be in 0..=",
    );

    let max: Integer = nanoseconds(ruby, Duration::MAX);
    let expected: Integer = ruby.eval("18446744073709551615 * 1_000_000_000 + 999_999_999")?;
    assert_eq!(max.to_u128()?, expected.to_u128()?);
    assert_eq!(nanoseconds(ruby, Duration::from_nanos(7)).to_u64()?, 7);
    let round_trip = duration(
        nanoseconds(ruby, Duration::from_nanos(u64::MAX)).as_value(),
        "t",
    )?;
    assert_eq!(round_trip, Duration::from_nanos(u64::MAX));
    Ok(())
}

pub fn wide(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let value = |code: &str| eval(ruby, code);

    // One range message for zero, negatives and overflow alike.
    let capacity: NonZero<u32> = positive(value("1")?, "capacity")?;
    assert_eq!(capacity.get(), 1);
    assert_eq!(
        positive::<NonZero<u32>>(value("2**32 - 1")?, "capacity")?.get(),
        u32::MAX
    );
    for code in ["0", "-1", "2**32", "2**200"] {
        raises(
            positive::<NonZero<u32>>(value(code)?, "capacity"),
            range_error,
            "capacity must be in 1..=4294967295",
        );
    }
    raises(
        positive::<NonZero<u32>>(value("1.0")?, "capacity"),
        type_error,
        "capacity must be an Integer",
    );
    assert_eq!(positive::<NonZero<i8>>(value("127")?, "lanes")?.get(), 127);
    for code in ["0", "-5", "-128", "128"] {
        raises(
            positive::<NonZero<i8>>(value(code)?, "lanes"),
            range_error,
            "lanes must be in 1..=127",
        );
    }
    let top = positive::<NonZero<u128>>(value("2**128 - 1")?, "wide")?;
    assert_eq!(top.get(), u128::MAX);

    // nil, or what `integer` reads, messages included.
    assert_eq!(optional::<u8>(value("nil")?, "until")?, None);
    assert_eq!(optional::<u8>(value("255")?, "until")?, Some(255));
    raises(
        optional::<u8>(value("256")?, "until"),
        range_error,
        "until must be in 0..=255",
    );
    raises(
        optional::<u8>(value("false")?, "until"),
        type_error,
        "until must be an Integer",
    );
    raises(
        optional::<u8>(value("'1'")?, "until"),
        type_error,
        "until must be an Integer",
    );

    // The whole `Duration` range, past `u64`, one message past either end.
    assert_eq!(instant(value("0")?, "now")?, Duration::ZERO);
    assert_eq!(
        instant(value("2**64")?, "now")?,
        Duration::from_nanos_u128(1 << 64)
    );
    let max = value("18446744073709551615 * 1_000_000_000 + 999_999_999")?;
    assert_eq!(instant(max, "expires")?, Duration::MAX);
    let message = "expires must be in 0..=Duration::MAX nanoseconds";
    for code in [
        "18446744073709551615 * 1_000_000_000 + 1_000_000_000",
        "-1",
        "2**128",
        "2**200",
    ] {
        raises(instant(value(code)?, "expires"), range_error, message);
    }
    raises(
        instant(value("1.5")?, "now"),
        type_error,
        "now must be an Integer",
    );
    let to_int = value("Object.new.tap { |o| def o.to_int = 1 }")?;
    raises(instant(to_int, "now"), type_error, "now must be an Integer");
    for sent in [
        Duration::ZERO,
        Duration::from_nanos(u64::MAX) * 3,
        Duration::MAX,
    ] {
        assert_eq!(instant(nanoseconds(ruby, sent).as_value(), "t")?, sent);
    }
    Ok(())
}
