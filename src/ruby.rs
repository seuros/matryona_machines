//! Binding glue for a gem's FFI crate (feature `ruby`).
//!
//! Each item enforces one of the fleet's Ruby binding rules
//! (`CONVENTIONS.md`, "Gems"):
//!
//! - **Classes are resolved once and pinned.** [`Class::resolve`] looks a
//!   constant up at init and registers it with `gc_register_mark_object`, so
//!   `GC.compact` cannot move what a static holds. [`Registry`] is the
//!   static-friendly holder a gem keeps them in.
//! - **The native boundary is the authority.** [`instance_of`],
//!   [`exact_struct`] and [`exact_array`] are the one exact-class check:
//!   a genuine instance (with or without an opened singleton class) passes,
//!   a subclass instance does not, and no Ruby method runs, so a forgery
//!   with a redefined `==`, `instance_of?` or `class` cannot answer for
//!   itself.
//! - **The binding never loses work.** [`keep`] is clone-then-keep: decide on
//!   a copy, build the answer with no `RefCell` borrow held, re-check frozen
//!   (`FrozenError`) and unchanged (`RuntimeError`), and only then store.
//! - **Malformed input raises.** [`integer`] and [`duration`] raise
//!   `TypeError` for a non-Integer and `RangeError` outside the type, naming
//!   the argument: `"<name> must be an Integer"`, `"<name> must be in
//!   <min>..=<max>"`. [`nanoseconds`] is the way back out, exact past `u64`.
//! - **Refusals are symbols.** [`symbol`] turns a refusal's `snake_case`
//!   name into its Symbol; [`parse`] reads a Symbol back into a variant.
mod classes;
mod integer;
mod keep;
mod symbol;

pub use classes::{Class, Registry, exact_array, exact_struct, instance_of};
pub use integer::{Bounded, duration, integer, nanoseconds};
pub use keep::keep;
pub use symbol::{parse, symbol};
