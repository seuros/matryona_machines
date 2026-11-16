//! Binding glue for a gem's FFI crate (feature `ruby`).
//!
//! Each item enforces one of the fleet's Ruby binding rules
//! (`CONVENTIONS.md`, "Gems"):
//!
//! - **Classes are resolved once and pinned.** [`Class::resolve`] looks a
//!   constant up at init and registers it with `gc_register_mark_object`, so
//!   `GC.compact` cannot move what a static holds; [`Class::pin`] does the
//!   same for a class the binding defined itself. [`Registry`] is the
//!   static-friendly holder a gem keeps them in, and [`registry!`] declares
//!   that static with the gem's `classes(ruby)` accessor over it.
//! - **The native boundary is the authority.** [`instance_of`] is the one
//!   exact-class check, for any value, and [`same_class`] its identity rule:
//!   a genuine instance (with or without an opened singleton class) passes,
//!   a subclass instance does not, and no Ruby method runs, so a forgery
//!   with a redefined `==`, `instance_of?` or `class` cannot answer for
//!   itself. [`which`] asks it of several classes at once. The readers
//!   built on it: [`exact_struct`], [`exact_fields`], [`optional_fields`]
//!   (nil or the struct), [`exact_array`], [`exact_array_n`],
//!   [`exact_string`] and, for wrapped Rust data, [`exact_typed`] and
//!   [`optional_typed`] (nil or the typed value). Every reader that takes
//!   a message has a `_with` twin ([`exact_struct_with`],
//!   [`exact_fields_with`], [`exact_typed_with`], [`exact_array_with`],
//!   [`exact_string_with`], [`bytes_with`], [`utf8_with`], ...) whose
//!   [`Message`] is built only on refusal. A reader that refuses for more
//!   than one reason also has a `_why` twin ([`exact_fields_why`],
//!   [`which_fields_why`], [`symbol_or_fields_why`],
//!   [`own_or_sibling_why`], [`exact_array_n_why`], ...) whose [`Explain`]
//!   closure is handed the [`Refused`] reason: a subclass, a missing
//!   reader, a wrong field count, the unknown Symbol, a non-finite real.
//!   Inline, its argument needs no annotation; bound to a `let`, wrap it
//!   in [`explain`] (or write `|why: Refused<'_>|`). [`prelude`] imports
//!   what such a closure needs, magnus's `ReprValue` (for
//!   `symbol.inspect()`) included.
//!   [`class_name`] names what arrived, never a singleton class.
//! - **One of several classes, or a Symbol.** [`which_fields`] is
//!   [`which`] and the field read in one check ([`optional_which_fields`]
//!   for nil too); [`which_struct`] answers an [`ExactStruct`] for classes
//!   of different lengths. [`symbol_or_fields`] and [`symbol_or_struct`]
//!   read a mixed enum's argument, a unit variant's Symbol (from its
//!   `names_only` table) or one of its data classes, as a [`Choice`].
//! - **A sibling gem's value by its readers.** [`own_or_sibling`] reads the
//!   gem's own class from the struct and anything else through the public
//!   readers named, refusing a subclass of its own class;
//!   [`own_or_sibling_default`] with the built-in [`sibling_message`]
//!   (`"expected an ElectionMachines::Lease or a value with #owner, ..."`,
//!   [`article`] picking a/an). With no own class, [`sibling`] reads the
//!   readers alone and [`sibling_shape`] picks one of several reader
//!   shapes ([`SiblingShape`]). That path runs Ruby: call it while
//!   converting arguments, before the core.
//! - **The fencing-token family.** [`fencing`] reads and builds the
//!   lease gem's `FencingToken`, `Lease` and `Checkpoint` (and the election
//!   and handoff gems' copies of them) for any core a gem describes with
//!   [`fencing::Tokens`] and [`fencing::Leases`], which
//!   [`fencing_family!`] implements: own class or sibling, rebuilt through
//!   the core, answered as the gem's own.
//! - **A Symbol or a number.** [`symbol_or_real`] and [`symbol_or_finite`]
//!   read a [`Symbols`] table's variant or a real, as a [`SymbolOrReal`].
//! - **The binding never loses work.** [`keep`] is clone-then-keep: decide on
//!   a copy, build the answer with no `RefCell` borrow held, re-check frozen
//!   (`FrozenError`) and unchanged (`RuntimeError`), and only then store.
//! - **Malformed input raises.** [`integer`], [`integer_in`], [`positive`],
//!   [`optional`], [`optional_positive`], [`duration`] and [`instant`] raise
//!   `TypeError` for a non-Integer and `RangeError` outside the type or the
//!   caller's range, naming the argument: `"<name> must be an Integer"`,
//!   `"<name> must be in <min>..=<max>"`. [`nanoseconds`] is the way back
//!   out, exact past `u64`. [`real`] takes exactly a Float or an Integer
//!   (`"<name> must be a Float or Integer"`) and [`finite`] also refuses
//!   NaN and the infinities (`"<name> must be finite"`); [`boolean`] takes
//!   exactly `true` or `false`; [`bytes`] and [`utf8`] copy exactly a
//!   String out (`"<name> must be valid UTF-8"`). None of them runs a Ruby
//!   method.
//! - **Refusals are symbols.** [`symbol`] turns a refusal's `snake_case`
//!   name into its Symbol; [`parse`] reads a Symbol back into a variant.
//!   [`symbols!`] builds a [`Symbols`] table whose variant list the compiler
//!   checks against the enum, or, for an enum with data variants, a
//!   [`SymbolNames`] table: every variant out, the unit ones back in
//!   ([`SymbolNames::parse_unit`]), and a wrapping variant answered under
//!   its payload's own table (`Lease(inner) => use LeaseRefusals`). Every
//!   table is [`Named`].
//! - **A const-generic core behind a runtime length.** [`sized!`] matches
//!   a length against the sizes the binding compiles for and evaluates its
//!   body with `const N` bound to the one that matched, or its `_` arm.
mod choice;
mod classes;
mod exact;
pub mod fencing;
mod fields;
mod integer;
mod keep;
mod message;
mod names;
mod number;
pub mod prelude;
mod refused;
mod scalar;
mod shape;
mod sibling;
mod sized;
mod string;
// Const fns run inside `symbols!` at compile time, where an index miss is a
// compile error; every index sits under its own `while i < len` guard.
#[allow(
    clippy::indexing_slicing,
    reason = "const fns: `get` is not const, every index is guarded by its loop"
)]
mod symbol;
mod table;
mod which;

pub use crate::{fencing_family, registry, sized, symbols};
pub use choice::{
    Choice, symbol_or_fields, symbol_or_fields_why, symbol_or_fields_with, symbol_or_struct,
    symbol_or_struct_why, symbol_or_struct_with,
};
#[doc(hidden)]
pub use classes::__private as __classes;
pub use classes::{Class, Registry};
pub use exact::{
    exact_array, exact_array_n, exact_array_n_why, exact_array_n_with, exact_array_with,
    exact_struct, exact_struct_with, instance_of, same_class, which,
};
pub use fields::{
    exact_fields, exact_fields_why, exact_fields_with, exact_typed, exact_typed_with,
    optional_fields, optional_fields_why, optional_fields_with, optional_typed,
    optional_typed_with,
};
pub use integer::{
    Bounded, Positive, duration, instant, integer, integer_in, nanoseconds, optional,
    optional_positive, positive,
};
pub use keep::keep;
pub use message::Message;
pub use names::{article, class_name, sibling_message};
pub use number::{
    SymbolOrReal, symbol_or_finite, symbol_or_finite_why, symbol_or_real, symbol_or_real_why,
};
pub use refused::{Explain, Refused, explain};
pub use scalar::{boolean, finite, real};
pub use shape::{SiblingShape, sibling, sibling_shape, sibling_why};
pub use sibling::{
    own_or_sibling, own_or_sibling_default, own_or_sibling_why, own_or_sibling_with,
};
pub use string::{bytes, bytes_with, exact_string, exact_string_with, utf8, utf8_with};
pub use symbol::{Named, SymbolNames, Symbols, parse, symbol};
#[doc(hidden)]
pub use symbol::{distinct_variants, joined_len, joined_names, valid_names};
pub use which::{
    ExactStruct, optional_which_fields, optional_which_fields_why, optional_which_fields_with,
    which_fields, which_fields_why, which_fields_with, which_struct, which_struct_why,
    which_struct_with,
};
