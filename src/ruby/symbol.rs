//! Refusals and other enums as Symbols.
use std::{format, string::String, vec::Vec};

use magnus::{Error, Ruby, Symbol, Value};

/// The Symbol a refusal travels as, from its variant's `snake_case` name.
///
/// The name comes from an exhaustive `match` in the binding, so a variant
/// added to the core does not compile until it has one; there is no
/// catch-all symbol. A [`symbols!`](crate::symbols) table writes that
/// `match` for you.
#[must_use]
pub fn symbol(ruby: &Ruby, name: &str) -> Symbol {
    ruby.to_symbol(name)
}

/// The variant of `all` whose `name` is the Symbol `value`.
///
/// `TypeError` `"<what> must be a Symbol"` for anything else, `RangeError`
/// `"<what> must be one of :a, :b"` for a Symbol naming no variant.
///
/// A hand-written `all` silently misses a variant added later; prefer a
/// [`symbols!`](crate::symbols) table and [`Symbols::parse`], whose list the
/// compiler checks.
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

/// A unit-variant enum's Symbol table.
///
/// Every variant, each one's name, and what the value is called in errors.
/// Generate it with
/// [`symbols!`](crate::symbols), which proves `ALL` lists every variant;
/// implementing it by hand gives up that proof.
pub trait Symbols {
    /// The enum, typically the core's.
    type Variant: Copy + 'static;
    /// What a value of the enum is, for error messages (`"a refusal"`).
    const WHAT: &'static str;
    /// Every variant, in declaration order of the table.
    const ALL: &'static [Self::Variant];

    /// The variant's Symbol name.
    fn name(variant: Self::Variant) -> &'static str;

    /// The variant as its Symbol.
    #[must_use]
    fn symbol(ruby: &Ruby, variant: Self::Variant) -> Symbol {
        symbol(ruby, Self::name(variant))
    }

    /// The variant named `name`, or `None`: the lookup [`parse`] does, for
    /// a name that is not a Symbol argument (two tables sharing a name,
    /// a name read from a `Data` field).
    ///
    /// ```ignore
    /// let overtaken = Ballots::variant("overtaken"); // Some(Ballot::Overtaken)
    /// ```
    #[must_use]
    fn variant(name: &str) -> Option<Self::Variant> {
        Self::ALL
            .iter()
            .copied()
            .find(|variant| Self::name(*variant) == name)
    }

    /// The variant the Symbol `value` names: [`parse`] over `ALL`.
    ///
    /// # Errors
    ///
    /// `TypeError` for a non-Symbol, `RangeError` listing every Symbol.
    fn parse(ruby: &Ruby, value: Value) -> Result<Self::Variant, Error> {
        parse(ruby, value, Self::WHAT, Self::ALL, Self::name)
    }
}

/// An output-only Symbol table, for an enum whose variants carry data
/// (`Pinned { .. }`, `Lease(..)`): each variant's name, and the variant
/// as its Symbol.
///
/// Such a variant cannot be built from its name, so there is no `ALL` and
/// no `parse`; refusals leave as Symbols and never come back. Generate it
/// with the `names_only` form of [`symbols!`](crate::symbols), which proves
/// the `match` covers every variant.
pub trait SymbolNames {
    /// The enum, typically the core's.
    type Variant;
    /// What a value of the enum is (`"a refusal"`), for documentation and
    /// for messages a gem writes about it.
    const WHAT: &'static str;
    /// Every name, in the table's order.
    const NAMES: &'static [&'static str];

    /// The variant's Symbol name.
    fn name(variant: &Self::Variant) -> &'static str;

    /// The variant as its Symbol.
    #[must_use]
    fn symbol(ruby: &Ruby, variant: &Self::Variant) -> Symbol {
        symbol(ruby, Self::name(variant))
    }
}

/// Whether every name is a lowercase `snake_case` Symbol name
/// (`[a-z][a-z0-9_]*`) and no two are equal. [`symbols!`](crate::symbols)
/// asserts it at compile time.
#[doc(hidden)]
#[must_use]
pub const fn valid_names(names: &[&str]) -> bool {
    let mut i = 0;
    while i < names.len() {
        if !snake_case(names[i].as_bytes()) {
            return false;
        }
        let mut j = i + 1;
        while j < names.len() {
            if equal(names[i].as_bytes(), names[j].as_bytes()) {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Whether no two discriminants are equal: no variant is listed twice.
/// [`symbols!`](crate::symbols) asserts it at compile time.
#[doc(hidden)]
#[must_use]
pub const fn distinct_variants(discriminants: &[i128]) -> bool {
    let mut i = 0;
    while i < discriminants.len() {
        let mut j = i + 1;
        while j < discriminants.len() {
            if discriminants[i] == discriminants[j] {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

const fn snake_case(name: &[u8]) -> bool {
    if name.is_empty() || !name[0].is_ascii_lowercase() {
        return false;
    }
    let mut i = 1;
    while i < name.len() {
        let byte = name[i];
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_') {
            return false;
        }
        i += 1;
    }
    true
}

const fn equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
