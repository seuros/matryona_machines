//! Refusals and other enums as Symbols.
use std::{format, string::String, vec::Vec};

use magnus::{Error, RArray, Ruby, Symbol, Value, prelude::*};

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
    lookup(symbol, |given| {
        all.iter().copied().find(|variant| name(*variant) == given)
    })
    .ok_or_else(|| {
        let names: Vec<&str> = all.iter().map(|v| name(*v)).collect();
        not_one_of(ruby, what, &names)
    })
}

/// The variant `find` gives for `symbol`'s name, or `None`. A Symbol whose
/// name is not valid UTF-8 can name no table entry, so it is `None` (the
/// caller's `RangeError`), never magnus's `EncodingError`.
pub(super) fn lookup<V>(symbol: Symbol, find: impl FnOnce(&str) -> Option<V>) -> Option<V> {
    symbol.name().ok().and_then(|name| find(&name))
}

/// `RangeError` `"<what> must be one of :a, :b"`, or, with no names at all,
/// `"<what> is never read from a Symbol"`.
fn not_one_of(ruby: &Ruby, what: &str, names: &[&str]) -> Error {
    let message = if names.is_empty() {
        format!("{what} is never read from a Symbol")
    } else {
        let names: Vec<String> = names.iter().map(|name| format!(":{name}")).collect();
        format!("{what} must be one of {}", names.join(", "))
    };
    Error::new(ruby.exception_range_error(), message)
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

    /// Every variant's Symbol, in `ALL`'s order, as a frozen Array: what a
    /// gem publishes as a constant (`REFUSALS`, `CLOCK_POLICIES`).
    #[must_use]
    fn every(ruby: &Ruby) -> RArray {
        let list = ruby.ary_from_iter(Self::ALL.iter().map(|variant| Self::symbol(ruby, *variant)));
        list.freeze();
        list
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
/// no `parse`: a data variant leaves as its Symbol and never comes back.
/// The unit variants of the same enum can, and
/// [`SymbolNames::parse_unit`] reads exactly those. Generate it with the
/// `names_only` form of [`symbols!`](crate::symbols), which proves the
/// `match` covers every variant and lists the unit ones.
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

    /// Every name in [`SymbolNames::NAMES`] as a Symbol, in the table's
    /// order, as a frozen Array: what a gem publishes as a constant
    /// (`REFUSALS`), the output-only twin of [`Symbols::every`].
    #[must_use]
    fn every(ruby: &Ruby) -> RArray {
        let list = ruby.ary_from_iter(Self::NAMES.iter().map(|name| symbol(ruby, name)));
        list.freeze();
        list
    }

    /// The variant, taken by value, as its Symbol: for a unit variant
    /// written where its Symbol is answered, with no binding to borrow.
    ///
    /// The name comes from the same exhaustive `match` as
    /// [`SymbolNames::symbol`], so a renamed or removed variant still does
    /// not compile. A data variant passed here is consumed; prefer
    /// [`SymbolNames::symbol`] for one you hold.
    ///
    /// ```ignore
    /// match admitted {
    ///     Ok(Admission::Buffered) => Ok(Admissions::symbol_of(ruby, Admission::Buffered).as_value()),
    ///     Ok(Admission::Flush(due)) => flush.funcall("new", (reason(ruby, due),)),
    ///     Err(refused) => Ok(refusal(ruby, refused)),
    /// }
    /// ```
    #[must_use]
    fn symbol_of(ruby: &Ruby, variant: Self::Variant) -> Symbol {
        Self::symbol(ruby, &variant)
    }

    /// The names of the unit variants, the only ones a name can build, in
    /// the table's order. A `symbols!` table lists every arm written as a
    /// bare name (`Granted => "granted"`); a hand-written impl that does not
    /// override it has none.
    const UNIT_NAMES: &'static [&'static str] = &[];

    /// The unit variant named `name`, or `None`, a data variant's name
    /// included: the lookup [`SymbolNames::parse_unit`] does.
    ///
    /// ```ignore
    /// let granted = Votes::variant("granted"); // Some(Vote::Granted)
    /// let denied = Votes::variant("denied");   // None: a Denied carries a reason
    /// ```
    #[must_use]
    fn variant(name: &str) -> Option<Self::Variant> {
        let _ = name;
        None
    }

    /// The unit variant the Symbol `value` names.
    ///
    /// # Errors
    ///
    /// `TypeError` `"<WHAT> must be a Symbol"` for a non-Symbol,
    /// `RangeError` `"<WHAT> must be one of :a, :b"` listing
    /// [`SymbolNames::UNIT_NAMES`] for any other Symbol, a data variant's
    /// name included.
    fn parse_unit(ruby: &Ruby, value: Value) -> Result<Self::Variant, Error> {
        let Some(symbol) = Symbol::from_value(value) else {
            let message = format!("{} must be a Symbol", Self::WHAT);
            return Err(Error::new(ruby.exception_type_error(), message));
        };
        lookup(symbol, Self::variant).ok_or_else(|| not_one_of(ruby, Self::WHAT, Self::UNIT_NAMES))
    }
}

/// Every table [`symbols!`](crate::symbols) writes, either form: the names
/// it can answer and a variant's name seen by reference.
///
/// A `names_only` table delegates a wrapping variant to another table
/// through it (`Lease(inner) => use LeaseRefusals`), so the inner table
/// may be a [`Symbols`] table, whose variants are `Copy`, or another
/// [`SymbolNames`] table. Its items are named apart from those two traits'
/// so that importing it beside either is never ambiguous.
pub trait Named {
    /// The enum, typically the core's.
    type Enum;
    /// Every name the table can answer, a delegated table's included, in
    /// the table's order.
    const EVERY_NAME: &'static [&'static str];

    /// The variant's Symbol name.
    fn name_of(variant: &Self::Enum) -> &'static str;
}

/// The `N` names of `parts`, in order. A `names_only` table that delegates
/// builds its `NAMES` with it, then asserts them distinct.
///
/// # Panics
///
/// At compile time, when `N` is not the parts' total length (the macro
/// computes `N` from the same parts, so it never is).
#[doc(hidden)]
#[must_use]
pub const fn joined_names<const N: usize>(parts: &[&[&'static str]]) -> [&'static str; N] {
    let mut out = [""; N];
    let mut at = 0;
    let mut part = 0;
    while part < parts.len() {
        let mut i = 0;
        while i < parts[part].len() {
            out[at] = parts[part][i];
            at += 1;
            i += 1;
        }
        part += 1;
    }
    assert!(at == N, "joined_names: N is the parts' total length");
    out
}

/// The total length of `parts`, the `N` of [`joined_names`].
#[doc(hidden)]
#[must_use]
pub const fn joined_len(parts: &[&[&'static str]]) -> usize {
    let mut total = 0;
    let mut part = 0;
    while part < parts.len() {
        total += parts[part].len();
        part += 1;
    }
    total
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
