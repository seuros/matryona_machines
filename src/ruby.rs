//! Binding glue for a gem's FFI crate (feature `ruby`).
//!
//! Each item enforces one of the fleet's Ruby binding rules
//! (`CONVENTIONS.md`, "Gems"):
//!
//! - **Classes are resolved once and pinned.** [`Class::resolve`] looks a
//!   constant up at init and registers it with `gc_register_mark_object`, so
//!   `GC.compact` cannot move what a static holds; [`Class::pin`] does the
//!   same for a class the binding defined itself. [`Registry`] is the
//!   static-friendly holder a gem keeps them in.
//! - **The native boundary is the authority.** [`instance_of`] is the one
//!   exact-class check, for any value, and [`same_class`] its identity rule:
//!   a genuine instance (with or without an opened singleton class) passes,
//!   a subclass instance does not, and no Ruby method runs, so a forgery
//!   with a redefined `==`, `instance_of?` or `class` cannot answer for
//!   itself. [`which`] asks it of several classes at once. The readers
//!   built on it: [`exact_struct`], [`exact_fields`], [`optional_fields`]
//!   (nil or the struct), [`exact_array`], [`exact_array_n`],
//!   [`exact_string`] and, for wrapped Rust data, [`exact_typed`]. Each
//!   struct and typed reader has a `_with` twin ([`exact_struct_with`],
//!   [`exact_fields_with`], [`optional_fields_with`], [`exact_typed_with`])
//!   whose [`Message`] is built only on refusal.
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
//!   checks against the enum, or, for an enum with data variants, an
//!   output-only [`SymbolNames`] table.
mod classes;
mod exact;
mod fields;
mod integer;
mod keep;
mod message;
mod scalar;
mod string;
mod symbol;
mod table;

pub use crate::symbols;
pub use classes::{Class, Registry};
pub use exact::{
    exact_array, exact_array_n, exact_struct, exact_struct_with, instance_of, same_class, which,
};
pub use fields::{
    exact_fields, exact_fields_with, exact_typed, exact_typed_with, optional_fields,
    optional_fields_with,
};
pub use integer::{
    Bounded, Positive, duration, instant, integer, integer_in, nanoseconds, optional,
    optional_positive, positive,
};
pub use keep::keep;
pub use message::Message;
pub use scalar::{boolean, finite, real};
pub use string::{bytes, exact_string, utf8};
pub use symbol::{SymbolNames, Symbols, parse, symbol};
#[doc(hidden)]
pub use symbol::{distinct_variants, valid_names};
