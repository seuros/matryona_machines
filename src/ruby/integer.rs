//! Integers and nanoseconds across the boundary.
use core::{fmt::Display, num::NonZero, ops::RangeInclusive, time::Duration};
use std::format;

use magnus::{Error, Integer, Ruby, TryConvert, Value, prelude::*};

/// A primitive integer an [`integer`] converts to, with the bounds its
/// `RangeError` names.
pub trait Bounded: TryConvert + Copy + Display + sealed::Sealed {
    /// The smallest value.
    const MIN: Self;
    /// The largest value.
    const MAX: Self;
}

/// A `NonZero` integer a [`positive`] converts to: `NonZero<u8>` through
/// `NonZero<i128>`, read as its primitive in `1..=MAX`.
///
/// `positive` is generic over the `NonZero` type rather than its primitive
/// because `NonZero<T>` for a generic `T` needs the unstable
/// `ZeroablePrimitive` bound; this way the caller's annotation is enough:
/// `let n: NonZero<u32> = positive(value, "n")?`.
pub trait Positive: Copy + sealed::Nonzero {
    /// The primitive the Integer is read as before the zero check.
    type Primitive: Bounded;

    /// `primitive` when it is in `1..=MAX`.
    fn positive(primitive: Self::Primitive) -> Option<Self>;
}

mod sealed {
    pub trait Sealed: PartialOrd {
        /// One, the low end of a [`super::positive`] range.
        const ONE: Self;
    }

    pub trait Nonzero {}
}

macro_rules! bounded {
    ($($ty:ty),*) => {$(
        impl sealed::Sealed for $ty {
            const ONE: Self = 1;
        }
        impl Bounded for $ty {
            const MIN: Self = <$ty>::MIN;
            const MAX: Self = <$ty>::MAX;
        }
        impl sealed::Nonzero for NonZero<$ty> {}
        impl Positive for NonZero<$ty> {
            type Primitive = $ty;

            fn positive(primitive: $ty) -> Option<Self> {
                NonZero::new(primitive).filter(|_| primitive > 0)
            }
        }
    )*};
}

bounded!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

/// `TypeError` `"<name> must be an Integer"` unless `value` is an Integer,
/// asked natively.
fn require_integer(ruby: &Ruby, value: Value, name: &str) -> Result<(), Error> {
    if Integer::from_value(value).is_some() {
        return Ok(());
    }
    let message = format!("{name} must be an Integer");
    Err(Error::new(ruby.exception_type_error(), message))
}

/// `value` as a `T`, read natively (no `to_int`, no redefined method).
///
/// `TypeError` `"<name> must be an Integer"` for anything that is not an
/// Integer (a Float, a String, an object with `to_int`), `RangeError`
/// `"<name> must be in <MIN>..=<MAX>"` for one `T` cannot hold.
///
/// ```ignore
/// let scope: u16 = integer(value, "scope")?; // "scope must be in 0..=65535"
/// ```
///
/// # Errors
///
/// `TypeError` or `RangeError` as above.
pub fn integer<T: Bounded>(value: Value, name: &str) -> Result<T, Error> {
    integer_in(value, name, T::MIN..=T::MAX)
}

/// `value` as a `T` in the caller's `range`: [`integer`] with tighter
/// bounds, for a limit the type does not carry (a drain's `max` up to a
/// waiter capacity).
///
/// `TypeError` `"<name> must be an Integer"`, `RangeError`
/// `"<name> must be in <LO>..=<HI>"` naming `range` for any Integer outside
/// it, one `T` cannot hold included. An empty range accepts nothing.
///
/// ```ignore
/// let max: usize = integer_in(value, "max", 0..=WAITER_CAPACITY)?;
/// // -1, 2**64 and WAITER_CAPACITY + 1 alike: "max must be in 0..=64"
/// ```
///
/// # Errors
///
/// `TypeError` or `RangeError` as above.
pub fn integer_in<T: Bounded>(
    value: Value,
    name: &str,
    range: RangeInclusive<T>,
) -> Result<T, Error> {
    let ruby = Ruby::get_with(value);
    require_integer(&ruby, value, name)?;
    T::try_convert(value)
        .ok()
        .filter(|held| range.contains(held))
        .ok_or_else(|| {
            let message = format!("{name} must be in {}..={}", range.start(), range.end());
            Error::new(ruby.exception_range_error(), message)
        })
}

/// `value` as a `NonZero`: [`integer`], with zero (and, for a signed type,
/// every negative) out of range too, so one `RangeError`
/// `"<name> must be in 1..=<MAX>"` covers both ends.
///
/// ```ignore
/// let capacity: NonZero<u32> = positive(value, "capacity")?;
/// // 0 and 2**32 alike: "capacity must be in 1..=4294967295"
/// ```
///
/// # Errors
///
/// `TypeError` `"<name> must be an Integer"`, or `RangeError` as above.
pub fn positive<P: Positive>(value: Value, name: &str) -> Result<P, Error> {
    let primitive = integer_in(
        value,
        name,
        <P::Primitive as sealed::Sealed>::ONE..=P::Primitive::MAX,
    )?;
    P::positive(primitive).ok_or_else(|| {
        // Unreachable: the range starts at one.
        let message = format!("{name} must be in 1..={}", P::Primitive::MAX);
        Error::new(Ruby::get_with(value).exception_range_error(), message)
    })
}

/// `nil` as `None`, anything else as [`positive`] reads it, messages
/// included: an optional count (a fair queue's `heads`).
///
/// ```ignore
/// let heads: Option<NonZero<u32>> = optional_positive(value, "heads")?;
/// ```
///
/// # Errors
///
/// As [`positive`], for a value that is not `nil`.
pub fn optional_positive<P: Positive>(value: Value, name: &str) -> Result<Option<P>, Error> {
    if value.is_nil() {
        return Ok(None);
    }
    positive(value, name).map(Some)
}

/// `nil` as `None`, anything else as [`integer`] reads it, messages
/// included.
///
/// # Errors
///
/// As [`integer`], for a value that is not `nil`.
pub fn optional<T: Bounded>(value: Value, name: &str) -> Result<Option<T>, Error> {
    if value.is_nil() {
        return Ok(None);
    }
    integer(value, name).map(Some)
}

/// A span arriving as Integer nanoseconds in `0..=u64::MAX`: [`integer`] for
/// `u64`, then [`Duration::from_nanos`]. An instant a core may have
/// answered as `now + span` is an [`instant`].
///
/// # Errors
///
/// As [`integer`].
pub fn duration(value: Value, name: &str) -> Result<Duration, Error> {
    integer::<u64>(value, name).map(Duration::from_nanos)
}

/// An instant or span arriving as Integer nanoseconds over the whole
/// `Duration` range, past `u64` included: the way back in for what
/// [`nanoseconds`] sent out (an expiry is `now + ttl`).
///
/// `TypeError` `"<name> must be an Integer"`, `RangeError`
/// `"<name> must be in 0..=Duration::MAX nanoseconds"` for a negative one
/// or one past `Duration::MAX`.
///
/// # Errors
///
/// `TypeError` or `RangeError` as above.
pub fn instant(value: Value, name: &str) -> Result<Duration, Error> {
    let ruby = Ruby::get_with(value);
    require_integer(&ruby, value, name)?;
    u128::try_convert(value)
        .ok()
        .filter(|nanos| *nanos <= Duration::MAX.as_nanos())
        .map(Duration::from_nanos_u128)
        .ok_or_else(|| {
            let message = format!("{name} must be in 0..=Duration::MAX nanoseconds");
            Error::new(ruby.exception_range_error(), message)
        })
}

/// A `Duration` leaving as Integer nanoseconds, exact however large, up to
/// `Duration::MAX`: a core may answer `now + span`, which can pass `u64`.
/// [`instant`] reads every value it produces back.
#[must_use]
pub fn nanoseconds(ruby: &Ruby, duration: Duration) -> Integer {
    ruby.integer_from_u128(duration.as_nanos())
}
