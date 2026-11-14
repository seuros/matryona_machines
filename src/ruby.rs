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
//!   itself. The readers built on it: [`exact_struct`] (and
//!   [`exact_struct_with`], its lazy message), [`exact_fields`],
//!   [`exact_array`], [`exact_array_n`] and, for wrapped Rust data,
//!   [`exact_typed`].
//! - **The binding never loses work.** [`keep`] is clone-then-keep: decide on
//!   a copy, build the answer with no `RefCell` borrow held, re-check frozen
//!   (`FrozenError`) and unchanged (`RuntimeError`), and only then store.
//! - **Malformed input raises.** [`integer`], [`positive`], [`optional`],
//!   [`duration`] and [`instant`] raise `TypeError` for a non-Integer and
//!   `RangeError` outside the type, naming the argument: `"<name> must be an
//!   Integer"`, `"<name> must be in <min>..=<max>"`. [`nanoseconds`] is the
//!   way back out, exact past `u64`.
//! - **Refusals are symbols.** [`symbol`] turns a refusal's `snake_case`
//!   name into its Symbol; [`parse`] reads a Symbol back into a variant.
//!   [`symbols!`] builds a [`Symbols`] table whose variant list the compiler
//!   checks against the enum.
mod classes;
mod exact;
mod integer;
mod keep;
mod symbol;

pub use crate::symbols;
pub use classes::{Class, Registry};
pub use exact::{
    exact_array, exact_array_n, exact_fields, exact_struct, exact_struct_with, exact_typed,
    instance_of, same_class,
};
pub use integer::{Bounded, Positive, duration, instant, integer, nanoseconds, optional, positive};
pub use keep::keep;
pub use symbol::{Symbols, parse, symbol};
#[doc(hidden)]
pub use symbol::{distinct_variants, valid_names};
