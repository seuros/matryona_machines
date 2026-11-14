//! Integers and nanoseconds across the boundary.
use core::{fmt::Display, time::Duration};
use std::format;

use magnus::{Error, Integer, Ruby, TryConvert, Value};

/// A primitive integer an [`integer`] converts to, with the bounds its
/// `RangeError` names.
pub trait Bounded: TryConvert + Copy + Display + sealed::Sealed {
    /// The smallest value.
    const MIN: Self;
    /// The largest value.
    const MAX: Self;
}

mod sealed {
    pub trait Sealed {}
}

macro_rules! bounded {
    ($($ty:ty),*) => {$(
        impl sealed::Sealed for $ty {}
        impl Bounded for $ty {
            const MIN: Self = <$ty>::MIN;
            const MAX: Self = <$ty>::MAX;
        }
    )*};
}

bounded!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

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
    let ruby = Ruby::get_with(value);
    if Integer::from_value(value).is_none() {
        let message = format!("{name} must be an Integer");
        return Err(Error::new(ruby.exception_type_error(), message));
    }
    T::try_convert(value).map_err(|_| {
        let message = format!("{name} must be in {}..={}", T::MIN, T::MAX);
        Error::new(ruby.exception_range_error(), message)
    })
}

/// An instant or span arriving as Integer nanoseconds in `0..=u64::MAX`:
/// [`integer`] for `u64`, then [`Duration::from_nanos`].
///
/// # Errors
///
/// As [`integer`].
pub fn duration(value: Value, name: &str) -> Result<Duration, Error> {
    integer::<u64>(value, name).map(Duration::from_nanos)
}

/// A `Duration` leaving as Integer nanoseconds, exact however large: a core
/// may answer `now + span`, which can pass `u64`.
#[must_use]
pub fn nanoseconds(ruby: &Ruby, duration: Duration) -> Integer {
    ruby.integer_from_u128(duration.as_nanos())
}
