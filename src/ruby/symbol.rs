//! Refusals and other unit-variant enums as Symbols.
use std::{format, string::String, vec::Vec};

use magnus::{Error, Ruby, Symbol, Value};

/// The Symbol a refusal travels as, from its variant's `snake_case` name.
///
/// The name comes from an exhaustive `match` in the binding, so a variant
/// added to the core does not compile until it has one; there is no
/// catch-all symbol.
///
/// ```ignore
/// fn name(refusal: Refusal) -> &'static str {
///     match refusal {
///         Refusal::Unknown => "unknown",
///         Refusal::TimeRegression => "time_regression",
///     }
/// }
/// Ok(symbol(ruby, name(refusal)).as_value())
/// ```
#[must_use]
pub fn symbol(ruby: &Ruby, name: &str) -> Symbol {
    ruby.to_symbol(name)
}

/// The variant of `all` whose `name` is the Symbol `value`.
///
/// `TypeError` `"<what> must be a Symbol"` for anything else, `RangeError`
/// `"<what> must be one of :a, :b"` for a Symbol naming no variant.
///
/// # Errors
///
/// `TypeError` or `RangeError` as above.
pub fn parse<T: Copy>(
    ruby: &Ruby,
    value: Value,
    what: &str,
    all: &[T],
    name: impl Fn(T) -> &'static str,
) -> Result<T, Error> {
    let Some(symbol) = Symbol::from_value(value) else {
        let message = format!("{what} must be a Symbol");
        return Err(Error::new(ruby.exception_type_error(), message));
    };
    let given = symbol.name()?;
    all.iter()
        .copied()
        .find(|variant| name(*variant) == given)
        .ok_or_else(|| {
            let names: Vec<String> = all.iter().map(|v| format!(":{}", name(*v))).collect();
            let message = format!("{what} must be one of {}", names.join(", "));
            Error::new(ruby.exception_range_error(), message)
        })
}
