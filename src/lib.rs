//! The glue every machines gem used to copy, in one place.
//!
//! Matryona is the outer doll: she holds a core inside a binding inside a
//! gem. Nothing here decides anything a core decides; it is the shell that
//! carries a core's answer across the Ruby boundary without losing it, and
//! the test kit cores use to prove a refusal changed nothing.
//!
//! - **No features** (the default): an empty `no_std` crate.
//! - **`testing`**: `no_std`, for a core's `[dev-dependencies]`.
//!   [`testing::XorShift64`] and [`testing::SplitMix64`] are deterministic
//!   generators with `below(n)`; [`testing::refused`] and
//!   [`testing::refuses`] run a call, assert it was refused, and assert the
//!   value equals a clone taken before it.
//! - **`ruby`**: pulls `magnus` and `std`, for a gem's FFI crate only. A core
//!   never depends on magnus. See [`ruby`] for the binding rules each item
//!   enforces.
//!
//! No `unsafe`, here or in the tests.
#![no_std]

#[cfg(feature = "ruby")]
extern crate std;

#[cfg(feature = "ruby")]
pub mod ruby;

#[cfg(any(test, feature = "testing"))]
pub mod testing;
