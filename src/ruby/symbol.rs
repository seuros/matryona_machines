//! Refusals and other unit-variant enums as Symbols.
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

    /// The variant the Symbol `value` names: [`parse`] over `ALL`.
    ///
    /// # Errors
    ///
    /// `TypeError` for a non-Symbol, `RangeError` listing every Symbol.
    fn parse(ruby: &Ruby, value: Value) -> Result<Self::Variant, Error> {
        parse(ruby, value, Self::WHAT, Self::ALL, Self::name)
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

/// A compiler-checked Symbol table for a unit-variant enum, the core's own
/// included.
///
/// ```ignore
/// use matryona_machines::ruby::Symbols;
/// use outbox_machines::Refusal;
///
/// matryona_machines::symbols! {
///     /// The core's refusals as Symbols.
///     pub Refusals for Refusal as "a refusal" {
///         Full => "full",
///         Stale => "stale",
///         TimeRegression => "time_regression",
///     }
/// }
///
/// Ok(Refusals::symbol(ruby, refusal).as_value())     // out
/// let refusal = Refusals::parse(ruby, value)?;        // in
/// ```
///
/// It declares the unit struct `Refusals` and implements [`Symbols`] for
/// it. The table is a local type, so the core's enum may be foreign: the
/// impl is of a foreign trait for a local type, which the orphan rule
/// allows, and nothing is implemented on the enum itself.
///
/// # Why the list cannot miss a variant
///
/// The one list of `Variant => "name"` pairs is expanded twice: into
/// `name`, a `match` with no wildcard arm, and into `ALL`. A variant added
/// to the enum but not to the table makes the `match` non-exhaustive, which
/// is a hard error (`E0004`), so the crate does not build until the variant
/// is listed, and then it is in `ALL` too, because `ALL` is made from the
/// same tokens. No count needs asserting: `ALL` holds exactly the arms of
/// an exhaustive match. The rest is checked as well:
///
/// - a variant listed twice fails a `const` assertion on the variants'
///   discriminants, so `ALL` holds no duplicate (the `match` would only
///   warn of an unreachable arm, and a lint inside another crate's macro is
///   silenced);
/// - two variants given one name, or a name that is not `[a-z][a-z0-9_]*`,
///   fail a `const` assertion, so `parse` can always tell them apart;
/// - a variant that carries data is not a unit pattern, so it does not
///   compile either: the table is for fieldless enums only, which
///   is also what makes the discriminant cast possible;
/// - a `#[non_exhaustive]` foreign enum can never be matched exhaustively,
///   so it cannot have a table (the conventions forbid the attribute).
///
/// A missing variant does not compile:
///
/// ```compile_fail
/// enum Light { Red, Amber, Green }
///
/// matryona_machines::symbols! {
///     Lights for Light as "a light" { Red => "red", Green => "green" }
/// }
/// ```
///
/// Nor does a variant listed twice:
///
/// ```compile_fail
/// enum Light { Red, Amber, Green }
///
/// matryona_machines::symbols! {
///     Lights for Light as "a light" {
///         Red => "red", Amber => "amber", Green => "green", Red => "rouge"
///     }
/// }
/// ```
///
/// Nor a name given twice:
///
/// ```compile_fail
/// enum Light { Red, Amber, Green }
///
/// matryona_machines::symbols! {
///     Lights for Light as "a light" { Red => "red", Amber => "red", Green => "green" }
/// }
/// ```
///
/// The complete table does:
///
/// ```
/// use matryona_machines::ruby::Symbols;
///
/// #[derive(Clone, Copy, Debug, PartialEq)]
/// enum Light { Red, Amber, Green }
///
/// matryona_machines::symbols! {
///     Lights for Light as "a light" { Red => "red", Amber => "amber", Green => "green" }
/// }
///
/// assert_eq!(Lights::ALL, [Light::Red, Light::Amber, Light::Green]);
/// assert_eq!(Lights::name(Light::Amber), "amber");
/// ```
#[macro_export]
macro_rules! symbols {
    (
        $(#[$meta:meta])*
        $vis:vis $table:ident for $enum:ty as $what:literal {
            $($variant:ident => $name:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug)]
        $vis struct $table;

        const _: () = {
            type MatryonaSymbolsEnum = $enum;

            impl $crate::ruby::Symbols for $table {
                type Variant = MatryonaSymbolsEnum;
                const WHAT: &'static str = $what;
                const ALL: &'static [MatryonaSymbolsEnum] = &[$(MatryonaSymbolsEnum::$variant),+];

                fn name(variant: MatryonaSymbolsEnum) -> &'static str {
                    match variant {
                        $(MatryonaSymbolsEnum::$variant => $name,)+
                    }
                }
            }

            assert!(
                $crate::ruby::distinct_variants(&[$(MatryonaSymbolsEnum::$variant as i128),+]),
                "every variant in a symbols! table is listed once"
            );
            assert!(
                $crate::ruby::valid_names(&[$($name),+]),
                "every name in a symbols! table is distinct snake_case"
            );
        };
    };
}
