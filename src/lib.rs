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
//!   value equals a clone taken before it; [`testing::unchanged`] does the
//!   same for an accepted call that must change nothing;
//!   [`testing::describes`] checks each refusal's `Display` text.
//! - **`ruby`**: pulls `magnus` and `std`, for a gem's FFI crate only. A core
//!   never depends on magnus. See [`ruby`] for the binding rules each item
//!   enforces.
//!
//! No `unsafe`, here or in the tests.
//!
//! A feature left off still names its items above: the links then point at
//! docs.rs, which builds every feature, so `cargo doc` stays clean under any
//! feature set.
//!
// The blank `//!` above ends the paragraph: a link definition cannot
// continue one, and each set below is one for the feature set built.
#![cfg_attr(
    feature = "testing",
    doc = "[`testing::XorShift64`]: testing::XorShift64
[`testing::SplitMix64`]: testing::SplitMix64
[`testing::refused`]: testing::refused
[`testing::refuses`]: testing::refuses
[`testing::unchanged`]: testing::unchanged
[`testing::describes`]: testing::describes"
)]
#![cfg_attr(
    not(feature = "testing"),
    doc = "\
[`testing::XorShift64`]: https://docs.rs/matryona-machines/latest/matryona_machines/testing/
[`testing::SplitMix64`]: https://docs.rs/matryona-machines/latest/matryona_machines/testing/
[`testing::refused`]: https://docs.rs/matryona-machines/latest/matryona_machines/testing/
[`testing::refuses`]: https://docs.rs/matryona-machines/latest/matryona_machines/testing/
[`testing::unchanged`]: https://docs.rs/matryona-machines/latest/matryona_machines/testing/
[`testing::describes`]: https://docs.rs/matryona-machines/latest/matryona_machines/testing/"
)]
#![cfg_attr(feature = "ruby", doc = "[`ruby`]: ruby")]
#![cfg_attr(
    not(feature = "ruby"),
    doc = "[`ruby`]: https://docs.rs/matryona-machines/latest/matryona_machines/ruby/"
)]
#![no_std]
#![cfg_attr(not(test), deny(clippy::indexing_slicing))]

#[cfg(feature = "ruby")]
extern crate std;

#[cfg(feature = "ruby")]
pub mod ruby;

#[cfg(any(test, feature = "testing"))]
pub mod testing;
