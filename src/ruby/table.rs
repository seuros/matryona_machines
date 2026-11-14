//! The `symbols!` macro: compiler-checked Symbol tables.
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
///   compile either: this form is for fieldless enums only, which is also
///   what makes the discriminant cast possible (see `names_only` below);
/// - a `#[non_exhaustive]` foreign enum can never be matched exhaustively,
///   so it cannot have a table (the conventions forbid the attribute).
///
/// A missing variant does not compile:
///
/// ```compile_fail,E0004
/// #[derive(Clone, Copy)]
/// enum Light { Red, Amber, Green }
///
/// matryona_machines::symbols! {
///     Lights for Light as "a light" { Red => "red", Green => "green" }
/// }
/// ```
///
/// Nor does a variant listed twice:
///
/// ```compile_fail,E0080
/// #[derive(Clone, Copy)]
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
/// ```compile_fail,E0080
/// #[derive(Clone, Copy)]
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
/// assert_eq!(Lights::variant("amber"), Some(Light::Amber));
/// assert_eq!(Lights::variant("blue"), None);
/// ```
///
/// # Output only: enums with data variants
///
/// A refusal that carries data (`Insufficient { needed, held }`,
/// `Lease(token)`) leaves as its Symbol but can never be built back from
/// one. The `names_only` form writes the same exhaustive `match` from
/// patterns and implements [`SymbolNames`](crate::ruby::SymbolNames)
/// instead: `name(&variant)`, `symbol(ruby, &variant)` and `NAMES`, with
/// no `ALL` and no `parse`. Write each variant as its shape, `Pinned { .. }`
/// for a struct variant, `Lease(..)` for a tuple one, the bare name for a
/// unit one; a shape that does not match the variant does not compile.
///
/// ```
/// use matryona_machines::ruby::SymbolNames;
///
/// enum Refusal { Pinned { by: u64 }, Lease(u32), Full }
///
/// matryona_machines::symbols! {
///     Refusals for Refusal as "a refusal" => names_only {
///         Pinned { .. } => "pinned",
///         Lease(..) => "lease",
///         Full => "full",
///     }
/// }
///
/// assert_eq!(Refusals::name(&Refusal::Lease(7)), "lease");
/// assert_eq!(Refusals::name(&Refusal::Pinned { by: 1 }), "pinned");
/// assert_eq!(Refusals::NAMES, ["pinned", "lease", "full"]);
/// # let _ = Refusal::Full;
/// ```
///
/// The same checks hold: a missing variant is a non-exhaustive `match`
/// (`E0004`), a variant listed twice is a duplicate definition (`E0428`),
/// and a repeated or non-`snake_case` name fails the `const` assertion.
///
/// ```compile_fail,E0004
/// enum Refusal { Pinned { by: u64 }, Lease(u32), Full }
///
/// matryona_machines::symbols! {
///     Refusals for Refusal as "a refusal" => names_only {
///         Pinned { .. } => "pinned",
///         Full => "full",
///     }
/// }
/// ```
///
/// ```compile_fail,E0080
/// enum Refusal { Pinned { by: u64 }, Lease(u32), Full }
///
/// matryona_machines::symbols! {
///     Refusals for Refusal as "a refusal" => names_only {
///         Pinned { .. } => "pinned",
///         Lease(..) => "pinned",
///         Full => "full",
///     }
/// }
/// ```
///
/// ```compile_fail,E0428
/// enum Refusal { Pinned { by: u64 }, Lease(u32), Full }
///
/// matryona_machines::symbols! {
///     Refusals for Refusal as "a refusal" => names_only {
///         Pinned { .. } => "pinned",
///         Lease(..) => "lease",
///         Full => "full",
///         Lease(..) => "leased",
///     }
/// }
/// ```
#[macro_export]
macro_rules! symbols {
    (
        $(#[$meta:meta])*
        $vis:vis $table:ident for $enum:ty as $what:literal => names_only {
            $(
                $variant:ident $({ $($fields:tt)* })? $(( $($tuple:tt)* ))? => $name:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug)]
        $vis struct $table;

        const _: () = {
            type MatryonaSymbolsEnum = $enum;

            impl $crate::ruby::SymbolNames for $table {
                type Variant = MatryonaSymbolsEnum;
                const WHAT: &'static str = $what;
                const NAMES: &'static [&'static str] = &[$($name),+];

                fn name(variant: &MatryonaSymbolsEnum) -> &'static str {
                    match variant {
                        $(
                            MatryonaSymbolsEnum::$variant
                                $({ $($fields)* })? $(( $($tuple)* ))? => $name,
                        )+
                    }
                }
            }

            // A variant listed twice is a second variant of one name here,
            // which is a hard error (E0428), not a silenced lint.
            #[allow(dead_code)]
            enum MatryonaListedOnce { $($variant),+ }

            assert!(
                $crate::ruby::valid_names(&[$($name),+]),
                "every name in a symbols! table is distinct snake_case"
            );
        };
    };
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
