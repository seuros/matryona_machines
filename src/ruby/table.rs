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
/// It declares the unit struct `Refusals` and implements [`Symbols`](crate::ruby::Symbols) for
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
/// no `ALL` and no `parse` (its unit arms alone are read back, below).
/// Write each variant as its shape, `Pinned { .. }` for a struct variant,
/// `Lease(..)` for a tuple one, the bare name for a unit one, or
/// `Lease(inner) => use Table` to answer under another table (below); a
/// shape that does not match the variant does not compile.
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
///
/// # Unit variants read back
///
/// An arm written as a bare name is a unit variant, and it is the one kind
/// a name can build. The table lists those arms in
/// [`SymbolNames::UNIT_NAMES`](crate::ruby::SymbolNames::UNIT_NAMES) and
/// writes `variant(name)` and `parse_unit(ruby, value)` over exactly them:
/// a mixed enum's Symbols come back without a hand-written `match`, and a
/// data variant's name builds nothing. To read "a unit Symbol or one of
/// these classes" in one call, see
/// [`symbol_or_fields`](crate::ruby::symbol_or_fields).
///
/// ```
/// use matryona_machines::ruby::SymbolNames;
///
/// #[derive(Debug, PartialEq)]
/// enum Vote { Granted, Duplicate, Denied(u8) }
///
/// matryona_machines::symbols! {
///     Votes for Vote as "a vote" => names_only {
///         Granted => "granted",
///         Duplicate => "duplicate",
///         Denied(..) => "denied",
///     }
/// }
///
/// assert_eq!(Votes::UNIT_NAMES, ["granted", "duplicate"]);
/// assert_eq!(Votes::variant("duplicate"), Some(Vote::Duplicate));
/// assert_eq!(Votes::variant("denied"), None);
/// assert_eq!(Votes::name(&Vote::Denied(3)), "denied");
/// # let _ = Vote::Granted;
/// ```
///
/// The marking is checked: a data variant written as a bare name is not a
/// unit pattern (`E0533`, and `E0308` where `variant` would build it), so
/// no arm can claim a name builds a variant that needs data.
///
/// ```compile_fail,E0533
/// enum Vote { Granted, Duplicate, Denied(u8) }
///
/// matryona_machines::symbols! {
///     Votes for Vote as "a vote" => names_only {
///         Granted => "granted",
///         Duplicate => "duplicate",
///         Denied => "denied",
///     }
/// }
/// ```
///
/// # Wrapping variants answer under the inner table
///
/// A variant that carries a sibling's refusal whole (`Lease(LeaseRefusal)`)
/// or a reason with its own table (`Schema(Mismatch)`) leaves as the inner
/// value's own Symbol. Write it `Variant(binding) => use Table`: the arm
/// hands the payload to `Table`, any [`symbols!`](crate::symbols) table of
/// either form (it implements [`Named`](crate::ruby::Named)), and `NAMES`
/// is the union, in the table's order with the inner names spliced in at
/// that arm. The variant stays output only: `variant` and `parse_unit` do
/// not reach into it.
///
/// ```
/// use matryona_machines::ruby::{SymbolNames, Symbols};
///
/// #[derive(Clone, Copy)]
/// enum LeaseRefusal { Held, Expired }
/// enum Refusal { NoCandidates, Lease(LeaseRefusal) }
///
/// matryona_machines::symbols! {
///     LeaseRefusals for LeaseRefusal as "a lease refusal" {
///         Held => "held",
///         Expired => "expired",
///     }
/// }
///
/// matryona_machines::symbols! {
///     Refusals for Refusal as "a refusal" => names_only {
///         NoCandidates => "no_candidates",
///         Lease(inner) => use LeaseRefusals,
///     }
/// }
///
/// assert_eq!(Refusals::name(&Refusal::Lease(LeaseRefusal::Expired)), "expired");
/// assert_eq!(Refusals::NAMES, ["no_candidates", "held", "expired"]);
/// assert_eq!(Refusals::UNIT_NAMES, ["no_candidates"]);
/// assert!(Refusals::variant("held").is_none());
/// # let _ = (Refusal::NoCandidates, LeaseRefusal::Held, LeaseRefusals::ALL);
/// ```
///
/// Every check still holds, across the combined set. A name the outer and
/// an inner table both answer fails the `const` assertion, since a Symbol
/// could no longer say which arm it came from:
///
/// ```compile_fail,E0080
/// #[derive(Clone, Copy)]
/// enum LeaseRefusal { Held, Expired }
/// enum Refusal { Expired, Lease(LeaseRefusal) }
///
/// matryona_machines::symbols! {
///     LeaseRefusals for LeaseRefusal as "a lease refusal" {
///         Held => "held",
///         Expired => "expired",
///     }
/// }
///
/// matryona_machines::symbols! {
///     Refusals for Refusal as "a refusal" => names_only {
///         Expired => "expired",
///         Lease(inner) => use LeaseRefusals,
///     }
/// }
/// ```
///
/// A table for another enum than the payload's is a type mismatch:
///
/// ```compile_fail,E0308
/// #[derive(Clone, Copy)]
/// enum LeaseRefusal { Held, Expired }
/// #[derive(Clone, Copy)]
/// enum Mismatch { Depth }
/// enum Refusal { NoCandidates, Lease(LeaseRefusal) }
///
/// matryona_machines::symbols! {
///     Mismatches for Mismatch as "a mismatch" { Depth => "depth_mismatch" }
/// }
///
/// matryona_machines::symbols! {
///     Refusals for Refusal as "a refusal" => names_only {
///         NoCandidates => "no_candidates",
///         Lease(inner) => use Mismatches,
///     }
/// }
/// # let _ = (LeaseRefusal::Held, LeaseRefusal::Expired);
/// ```
///
/// A delegated variant missing from the list is still a non-exhaustive
/// `match`, and one listed twice still a duplicate definition:
///
/// ```compile_fail,E0004
/// #[derive(Clone, Copy)]
/// enum LeaseRefusal { Held, Expired }
/// enum Refusal { NoCandidates, Lease(LeaseRefusal) }
///
/// matryona_machines::symbols! {
///     LeaseRefusals for LeaseRefusal as "a lease refusal" {
///         Held => "held",
///         Expired => "expired",
///     }
/// }
///
/// matryona_machines::symbols! {
///     Refusals for Refusal as "a refusal" => names_only {
///         NoCandidates => "no_candidates",
///     }
/// }
/// ```
///
/// ```compile_fail,E0428
/// #[derive(Clone, Copy)]
/// enum LeaseRefusal { Held, Expired }
/// enum Refusal { NoCandidates, Lease(LeaseRefusal) }
///
/// matryona_machines::symbols! {
///     LeaseRefusals for LeaseRefusal as "a lease refusal" {
///         Held => "held",
///         Expired => "expired",
///     }
/// }
///
/// matryona_machines::symbols! {
///     Refusals for Refusal as "a refusal" => names_only {
///         NoCandidates => "no_candidates",
///         Lease(inner) => use LeaseRefusals,
///         Lease(..) => "lease",
///     }
/// }
/// ```
///
/// Only a one-field tuple variant can delegate: the arm binds that field.
/// A unit variant handed to a table is not a tuple pattern (`E0164`).
///
/// ```compile_fail,E0164
/// #[derive(Clone, Copy)]
/// enum LeaseRefusal { Held, Expired }
/// enum Refusal { NoCandidates, Lease(LeaseRefusal) }
///
/// matryona_machines::symbols! {
///     LeaseRefusals for LeaseRefusal as "a lease refusal" {
///         Held => "held",
///         Expired => "expired",
///     }
/// }
///
/// matryona_machines::symbols! {
///     Refusals for Refusal as "a refusal" => names_only {
///         NoCandidates(inner) => use LeaseRefusals,
///         Lease(..) => "lease",
///     }
/// }
/// ```
#[macro_export]
macro_rules! symbols {
    (
        $(#[$meta:meta])*
        $vis:vis $table:ident for $enum:ty as $what:literal => names_only {
            $($arms:tt)+
        }
    ) => {
        $crate::symbols! {
            @names_only { $(#[$meta])* $vis $table for $enum as $what }
            arms [] parts [] units [] unit_names [] once []
            { $($arms)+ }
        }
    };
    // A wrapping variant answers under the inner table's names.
    (
        @names_only $head:tt
        arms [$($arms:tt)*] parts [$($parts:tt)*] units [$($units:tt)*]
        unit_names [$($unit_names:tt)*] once [$($once:tt)*]
        { $variant:ident ( $bind:ident ) => use $inner:path $(, $($rest:tt)*)? }
    ) => {
        $crate::symbols! {
            @names_only $head
            arms [
                $($arms)*
                MatryonaSymbolsEnum::$variant($bind) =>
                    <$inner as $crate::ruby::Named>::name_of($bind),
            ]
            parts [$($parts)* <$inner as $crate::ruby::Named>::EVERY_NAME,]
            units [$($units)*] unit_names [$($unit_names)*] once [$($once)* $variant,]
            { $($($rest)*)? }
        }
    };
    // A struct variant: output only.
    (
        @names_only $head:tt
        arms [$($arms:tt)*] parts [$($parts:tt)*] units [$($units:tt)*]
        unit_names [$($unit_names:tt)*] once [$($once:tt)*]
        { $variant:ident { $($fields:tt)* } => $name:literal $(, $($rest:tt)*)? }
    ) => {
        $crate::symbols! {
            @names_only $head
            arms [$($arms)* MatryonaSymbolsEnum::$variant { $($fields)* } => $name,]
            parts [$($parts)* &[$name],]
            units [$($units)*] unit_names [$($unit_names)*] once [$($once)* $variant,]
            { $($($rest)*)? }
        }
    };
    // A tuple variant: output only.
    (
        @names_only $head:tt
        arms [$($arms:tt)*] parts [$($parts:tt)*] units [$($units:tt)*]
        unit_names [$($unit_names:tt)*] once [$($once:tt)*]
        { $variant:ident ( $($tuple:tt)* ) => $name:literal $(, $($rest:tt)*)? }
    ) => {
        $crate::symbols! {
            @names_only $head
            arms [$($arms)* MatryonaSymbolsEnum::$variant( $($tuple)* ) => $name,]
            parts [$($parts)* &[$name],]
            units [$($units)*] unit_names [$($unit_names)*] once [$($once)* $variant,]
            { $($($rest)*)? }
        }
    };
    // A unit variant: out, and back in by name.
    (
        @names_only $head:tt
        arms [$($arms:tt)*] parts [$($parts:tt)*] units [$($units:tt)*]
        unit_names [$($unit_names:tt)*] once [$($once:tt)*]
        { $variant:ident => $name:literal $(, $($rest:tt)*)? }
    ) => {
        $crate::symbols! {
            @names_only $head
            arms [$($arms)* MatryonaSymbolsEnum::$variant => $name,]
            parts [$($parts)* &[$name],]
            units [$($units)* $name => ::core::option::Option::Some(MatryonaSymbolsEnum::$variant),]
            unit_names [$($unit_names)* $name,]
            once [$($once)* $variant,]
            { $($($rest)*)? }
        }
    };
    (
        @names_only { $(#[$meta:meta])* $vis:vis $table:ident for $enum:ty as $what:literal }
        arms [$($arms:tt)*] parts [$($parts:tt)*] units [$($units:tt)*]
        unit_names [$($unit_names:tt)*] once [$($once:tt)*]
        {}
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug)]
        $vis struct $table;

        const _: () = {
            type MatryonaSymbolsEnum = $enum;

            const MATRYONA_PARTS: &[&[&str]] = &[$($parts)*];
            const MATRYONA_NAMES: [&str; $crate::ruby::joined_len(MATRYONA_PARTS)] =
                $crate::ruby::joined_names(MATRYONA_PARTS);

            impl $crate::ruby::SymbolNames for $table {
                type Variant = MatryonaSymbolsEnum;
                const WHAT: &'static str = $what;
                const NAMES: &'static [&'static str] = &MATRYONA_NAMES;
                const UNIT_NAMES: &'static [&'static str] = &[$($unit_names)*];

                fn name(variant: &MatryonaSymbolsEnum) -> &'static str {
                    match variant {
                        $($arms)*
                    }
                }

                fn variant(name: &str) -> ::core::option::Option<MatryonaSymbolsEnum> {
                    match name {
                        $($units)*
                        _ => ::core::option::Option::None,
                    }
                }
            }

            impl $crate::ruby::Named for $table {
                type Enum = MatryonaSymbolsEnum;
                const EVERY_NAME: &'static [&'static str] = &MATRYONA_NAMES;

                fn name_of(variant: &MatryonaSymbolsEnum) -> &'static str {
                    <$table as $crate::ruby::SymbolNames>::name(variant)
                }
            }

            // A variant listed twice is a second variant of one name here,
            // which is a hard error (E0428), not a silenced lint.
            #[allow(dead_code)]
            enum MatryonaListedOnce { $($once)* }

            // Every name, a delegated table's included: two arms answering
            // one Symbol could not be told apart.
            assert!(
                $crate::ruby::valid_names(&MATRYONA_NAMES),
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

            impl $crate::ruby::Named for $table {
                type Enum = MatryonaSymbolsEnum;
                const EVERY_NAME: &'static [&'static str] = &[$($name),+];

                fn name_of(variant: &MatryonaSymbolsEnum) -> &'static str {
                    <$table as $crate::ruby::Symbols>::name(*variant)
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
