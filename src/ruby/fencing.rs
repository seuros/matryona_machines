//! The fencing-token family: a `FencingToken`, the `Lease` it rides in and
//! the `Checkpoint` an authority saves, read and built for every gem that
//! carries them.
//!
//! The lease gem defines the three; the election and handoff gems hold
//! their own classes of the same shape (a gem holds its own classes) and
//! accept a sibling's value by its readers. Their bindings shared every
//! line of the conversion, so it lives here. Matryona depends on none of
//! their cores: a gem describes its core on a marker type, [`Tokens`] for
//! the token and [`Leases`] for the lease and checkpoint, and calls the
//! functions here with it.
//!
//! The [`fencing_family!`](crate::fencing_family) macro writes both impls for a core
//! whose types have the lease core's methods (`FencingToken::from_raw`,
//! `raw`, `scope`, `order`, `is_newer_than`; `Lease::new`, `owner`, `token`,
//! `expires_at`; `Checkpoint::from_parts`, `scope`, `issued`, `lease`,
//! `observed`), reading the gem's classes from its registry's `token`,
//! `lease` and `checkpoint` fields:
//!
//! ```ignore
//! matryona_machines::fencing_family! {
//!     /// The lease core, for `matryona_machines::ruby::fencing`.
//!     pub Family in crate::classes::classes {
//!         token: FencingToken,
//!         lease: Lease, owner: OwnerId,
//!         checkpoint: Checkpoint,
//!     }
//! }
//!
//! // init
//! module.define_singleton_method("native_token_order", function!(token_order::<Family>, 2))?;
//! ```
//!
//! `token: FencingToken` alone implements [`Tokens`] only, for a gem that
//! carries the token and nothing else of the family.
//!
//! Every reader takes this gem's own class read from the struct, a sibling
//! gem's value of the same shape through its public readers
//! ([`own_or_sibling_default`]), and refuses a subclass; each field is then
//! converted and the value rebuilt through the core constructor, so a forged
//! value raises before it reaches the core. A sibling's readers run Ruby:
//! call these while converting arguments, before the core.
use core::{cmp::Ordering, time::Duration};

use magnus::{Error, IntoValue, RClass, Ruby, Value, prelude::*};

use super::{instant, integer, nanoseconds, own_or_sibling_default};

/// What [`fencing_family!`](crate::fencing_family) expands to names, so a gem's binding
/// needs no import for it.
#[doc(hidden)]
pub mod __private {
    pub use core::{cmp::Ordering, option::Option, result::Result, time::Duration};
    pub use magnus::{Error, RClass, Ruby};
}

/// The `RangeError` for a raw Integer that is no token.
pub const RAW_RANGE: &str = "token must be in 1..=2**64-1 with a positive sequence number";

/// The `RangeError` for checkpoint fields that are not one history.
pub const CHECKPOINT_RANGE: &str = "issued is beyond the scope's token space, or the lease is \
                                    not the grant that history ends with";

/// A gem's fencing token: the core's type, and the gem's own class of it.
/// Implement it on a marker type in the binding; each method is the core's
/// own, delegated.
pub trait Tokens {
    /// The core's fencing token.
    type Token: Copy;

    /// The gem's own `FencingToken` class, resolved at init.
    ///
    /// # Errors
    ///
    /// When the gem's class registry is not filled (`NameError`).
    fn token_class(ruby: &Ruby) -> Result<RClass, Error>;

    /// The core's `FencingToken::from_raw`: `None` for a zero sequence.
    fn from_raw(raw: u64) -> Option<Self::Token>;

    /// The token's raw value, scope above sequence.
    fn raw(token: Self::Token) -> u64;

    /// The scope the token carries.
    fn scope(token: Self::Token) -> u16;

    /// The core's order of two tokens; `None` across scopes.
    fn order(token: Self::Token, other: Self::Token) -> Option<Ordering>;

    /// Whether `token` supersedes `other`.
    fn is_newer_than(token: Self::Token, other: Self::Token) -> bool;
}

/// A gem's lease and checkpoint, on top of its token.
pub trait Leases: Tokens {
    /// The core's lease.
    type Lease: Copy;
    /// The core's checkpoint.
    type Checkpoint;

    /// The gem's own `Lease` class, resolved at init.
    ///
    /// # Errors
    ///
    /// When the gem's class registry is not filled (`NameError`).
    fn lease_class(ruby: &Ruby) -> Result<RClass, Error>;

    /// The gem's own `Checkpoint` class, resolved at init.
    ///
    /// # Errors
    ///
    /// When the gem's class registry is not filled (`NameError`).
    fn checkpoint_class(ruby: &Ruby) -> Result<RClass, Error>;

    /// The core's `Lease::new` of an owner, a token and an expiry.
    fn lease(owner: u64, token: Self::Token, expires_at: Duration) -> Self::Lease;

    /// A lease's owner, token and expiry.
    fn lease_parts(lease: Self::Lease) -> (u64, Self::Token, Duration);

    /// The core's `Checkpoint::from_parts`: `None` for fields that are not
    /// one history.
    fn checkpoint(
        scope: u16,
        issued: u64,
        lease: Option<Self::Lease>,
        observed: Duration,
    ) -> Option<Self::Checkpoint>;

    /// A checkpoint's scope, issued count, lease and watermark.
    fn checkpoint_parts(checkpoint: &Self::Checkpoint)
    -> (u16, u64, Option<Self::Lease>, Duration);
}

/// A token from its raw Integer: `TypeError` `"token must be an Integer"`,
/// `RangeError` [`RAW_RANGE`] for one the core refuses.
///
/// # Errors
///
/// As above.
pub fn raw_token<F: Tokens>(value: Value) -> Result<F::Token, Error> {
    F::from_raw(integer(value, "token")?)
        .ok_or_else(|| Error::new(Ruby::get_with(value).exception_range_error(), RAW_RANGE))
}

/// What `FencingToken.new` validates: [`raw_token`], answering nothing.
///
/// # Errors
///
/// As [`raw_token`].
pub fn validate_raw<F: Tokens>(raw: Value) -> Result<(), Error> {
    raw_token::<F>(raw).map(drop)
}

/// A token as the core's: this gem's `FencingToken` or a sibling's.
///
/// # Errors
///
/// `TypeError` for anything else (a subclass of the own class included),
/// then as [`raw_token`].
pub fn token<F: Tokens>(ruby: &Ruby, value: Value) -> Result<F::Token, Error> {
    let [raw] = own_or_sibling_default(ruby, value, F::token_class(ruby)?, ["raw"])?;
    raw_token::<F>(raw)
}

/// This gem's `FencingToken` of `token`, built by its `new`.
///
/// # Errors
///
/// Whatever `new` raises.
pub fn token_object<F: Tokens>(ruby: &Ruby, token: F::Token) -> Result<Value, Error> {
    F::token_class(ruby)?.funcall("new", (F::raw(token),))
}

/// A token read as [`token`] reads it, answered as this gem's own: what a
/// `Data` holding a token keeps in place of the one it was given.
///
/// # Errors
///
/// As [`token`] and [`token_object`].
pub fn own_token<F: Tokens>(ruby: &Ruby, value: Value) -> Result<Value, Error> {
    token_object::<F>(ruby, token::<F>(ruby, value)?)
}

/// `FencingToken#scope`; reading the token is what validates it.
///
/// # Errors
///
/// As [`token`].
pub fn token_scope<F: Tokens>(ruby: &Ruby, held: Value) -> Result<u16, Error> {
    token::<F>(ruby, held).map(F::scope)
}

/// `FencingToken#order`: -1, 0, 1, or `nil` across scopes.
///
/// # Errors
///
/// As [`token`], for either.
pub fn token_order<F: Tokens>(ruby: &Ruby, held: Value, other: Value) -> Result<Option<i8>, Error> {
    let (held, other) = (token::<F>(ruby, held)?, token::<F>(ruby, other)?);
    Ok(F::order(held, other).map(|order| order as i8))
}

/// `FencingToken#newer_than?`.
///
/// # Errors
///
/// As [`token`], for either.
pub fn token_newer<F: Tokens>(ruby: &Ruby, held: Value, other: Value) -> Result<bool, Error> {
    let (held, other) = (token::<F>(ruby, held)?, token::<F>(ruby, other)?);
    Ok(F::is_newer_than(held, other))
}

/// A lease as the core's: this gem's `Lease` or a sibling's, its token read
/// as [`token`] reads one.
///
/// # Errors
///
/// `TypeError` for anything else, then each field's own error.
pub fn lease<F: Leases>(ruby: &Ruby, value: Value) -> Result<F::Lease, Error> {
    let readers = ["owner", "token", "expires_at"];
    let [owner, held, expires_at] =
        own_or_sibling_default(ruby, value, F::lease_class(ruby)?, readers)?;
    let owner = integer(owner, "owner")?;
    let held = token::<F>(ruby, held)?;
    Ok(F::lease(owner, held, instant(expires_at, "expires_at")?))
}

/// This gem's `Lease` of `lease`, built by its `new`.
///
/// # Errors
///
/// Whatever `new` raises.
pub fn lease_object<F: Leases>(ruby: &Ruby, lease: F::Lease) -> Result<Value, Error> {
    let (owner, held, expires_at) = F::lease_parts(lease);
    let held = token_object::<F>(ruby, held)?;
    let expires_at = nanoseconds(ruby, expires_at);
    F::lease_class(ruby)?.funcall("new", (owner, held, expires_at))
}

/// A lease as `[owner, raw token, expires_at]`, for Ruby to build a `Lease`
/// of. Runs no Ruby method, so it may be built under a borrow.
#[must_use]
pub fn lease_row<F: Leases>(ruby: &Ruby, lease: F::Lease) -> Value {
    let (owner, held, expires_at) = F::lease_parts(lease);
    (owner, F::raw(held), nanoseconds(ruby, expires_at)).into_value_with(ruby)
}

/// The core's `Checkpoint::from_parts`, or `RangeError` [`CHECKPOINT_RANGE`]
/// for the `None` it answers.
///
/// # Errors
///
/// As above.
pub fn checkpoint_of<F: Leases>(
    ruby: &Ruby,
    scope: u16,
    issued: u64,
    lease: Option<F::Lease>,
    observed: Duration,
) -> Result<F::Checkpoint, Error> {
    F::checkpoint(scope, issued, lease, observed)
        .ok_or_else(|| Error::new(ruby.exception_range_error(), CHECKPOINT_RANGE))
}

/// A checkpoint as the core's: this gem's `Checkpoint` or a sibling's, its
/// lease (or nil) read as [`lease`] reads one.
///
/// # Errors
///
/// `TypeError` for anything else, then each field's own error, then as
/// [`checkpoint_of`].
pub fn checkpoint<F: Leases>(ruby: &Ruby, value: Value) -> Result<F::Checkpoint, Error> {
    let readers = ["scope", "issued", "lease", "observed"];
    let [scope, issued, held, observed] =
        own_or_sibling_default(ruby, value, F::checkpoint_class(ruby)?, readers)?;
    let (scope, issued) = (integer(scope, "scope")?, integer(issued, "issued")?);
    let held = if held.is_nil() {
        None
    } else {
        Some(lease::<F>(ruby, held)?)
    };
    checkpoint_of::<F>(ruby, scope, issued, held, instant(observed, "observed")?)
}

/// A checkpoint as `[scope, issued, lease row or nil, observed]`, for Ruby
/// to build a `Checkpoint` of. Runs no Ruby method, so it may be built under
/// a borrow.
#[must_use]
pub fn checkpoint_row<F: Leases>(ruby: &Ruby, checkpoint: &F::Checkpoint) -> Value {
    let (scope, issued, held, observed) = F::checkpoint_parts(checkpoint);
    let held = held.map(|held| lease_row::<F>(ruby, held));
    (scope, issued, held, nanoseconds(ruby, observed)).into_value_with(ruby)
}

/// What `Lease.new` validates: an owner, a token as [`token`] reads it, an
/// expiry. Answers the token as this gem's own, to keep in place of the one
/// given.
///
/// # Errors
///
/// Each field's own error, in order.
pub fn validate_lease<F: Leases>(
    ruby: &Ruby,
    owner: Value,
    held: Value,
    expires_at: Value,
) -> Result<Value, Error> {
    integer::<u64>(owner, "owner")?;
    let held = token::<F>(ruby, held)?;
    instant(expires_at, "expires_at")?;
    token_object::<F>(ruby, held)
}

/// A lease read as [`lease`] reads it, answered as this gem's own: what a
/// `Data` holding a lease keeps in place of the one it was given.
///
/// # Errors
///
/// As [`lease`] and [`lease_object`].
pub fn own_lease<F: Leases>(ruby: &Ruby, held: Value) -> Result<Value, Error> {
    lease_object::<F>(ruby, lease::<F>(ruby, held)?)
}

/// What `Checkpoint.new` validates: the fields, then the core's
/// `from_parts`. Answers the lease as this gem's own (or nil), to keep in
/// place of the one given.
///
/// # Errors
///
/// Each field's own error, in order, then as [`checkpoint_of`].
pub fn validate_checkpoint<F: Leases>(
    ruby: &Ruby,
    scope: Value,
    issued: Value,
    held: Option<Value>,
    observed: Value,
) -> Result<Option<Value>, Error> {
    let (scope, issued) = (integer(scope, "scope")?, integer(issued, "issued")?);
    let observed = instant(observed, "observed")?;
    let held = held.map(|held| lease::<F>(ruby, held)).transpose()?;
    checkpoint_of::<F>(ruby, scope, issued, held, observed)?;
    held.map(|held| lease_object::<F>(ruby, held)).transpose()
}

/// Implements [`Tokens`] (and [`Leases`]) on a new marker type.
///
/// For a core whose types have the lease core's methods. `$classes` is the
/// gem's class accessor, a function of `&Ruby` answering
/// `Result<&Classes, Error>` whose `token` (and `lease`, `checkpoint`)
/// fields are [`Class`](crate::ruby::Class)es. See the
/// [module docs](mod@crate::ruby::fencing).
///
/// [`Tokens`]: crate::ruby::fencing::Tokens
/// [`Leases`]: crate::ruby::fencing::Leases
#[macro_export]
macro_rules! fencing_family {
    (
        $(#[$meta:meta])*
        $vis:vis $family:ident in $classes:path {
            token: $token:ty,
            lease: $lease:ty, owner: $owner:ty,
            checkpoint: $checkpoint:ty $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $family;
        $crate::fencing_family!(@tokens $family $classes, $token);
        $crate::fencing_family!(@leases $family $classes, $token, $lease, $owner, $checkpoint);
    };
    (
        $(#[$meta:meta])*
        $vis:vis $family:ident in $classes:path {
            token: $token:ty $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $family;
        $crate::fencing_family!(@tokens $family $classes, $token);
    };
    (@tokens $family:ident $classes:path, $token:ty) => {
        impl $crate::ruby::fencing::Tokens for $family {
            type Token = $token;

            fn token_class(
                ruby: &$crate::ruby::fencing::__private::Ruby,
            ) -> $crate::ruby::fencing::__private::Result<
                $crate::ruby::fencing::__private::RClass,
                $crate::ruby::fencing::__private::Error,
            > {
                $crate::ruby::fencing::__private::Result::Ok($classes(ruby)?.token.get(ruby))
            }

            fn from_raw(raw: u64) -> $crate::ruby::fencing::__private::Option<$token> {
                <$token>::from_raw(raw)
            }

            fn raw(token: $token) -> u64 {
                token.raw()
            }

            fn scope(token: $token) -> u16 {
                token.scope()
            }

            fn order(
                token: $token,
                other: $token,
            ) -> $crate::ruby::fencing::__private::Option<
                $crate::ruby::fencing::__private::Ordering,
            > {
                token.order(other)
            }

            fn is_newer_than(token: $token, other: $token) -> bool {
                token.is_newer_than(other)
            }
        }
    };
    (@leases $family:ident $classes:path, $token:ty, $lease:ty, $owner:ty, $checkpoint:ty) => {
        impl $crate::ruby::fencing::Leases for $family {
            type Lease = $lease;
            type Checkpoint = $checkpoint;

            fn lease_class(
                ruby: &$crate::ruby::fencing::__private::Ruby,
            ) -> $crate::ruby::fencing::__private::Result<
                $crate::ruby::fencing::__private::RClass,
                $crate::ruby::fencing::__private::Error,
            > {
                $crate::ruby::fencing::__private::Result::Ok($classes(ruby)?.lease.get(ruby))
            }

            fn checkpoint_class(
                ruby: &$crate::ruby::fencing::__private::Ruby,
            ) -> $crate::ruby::fencing::__private::Result<
                $crate::ruby::fencing::__private::RClass,
                $crate::ruby::fencing::__private::Error,
            > {
                $crate::ruby::fencing::__private::Result::Ok($classes(ruby)?.checkpoint.get(ruby))
            }

            fn lease(
                owner: u64,
                token: $token,
                expires_at: $crate::ruby::fencing::__private::Duration,
            ) -> $lease {
                <$lease>::new(<$owner>::new(owner), token, expires_at)
            }

            fn lease_parts(
                lease: $lease,
            ) -> (u64, $token, $crate::ruby::fencing::__private::Duration) {
                (lease.owner().value(), lease.token(), lease.expires_at())
            }

            fn checkpoint(
                scope: u16,
                issued: u64,
                lease: $crate::ruby::fencing::__private::Option<$lease>,
                observed: $crate::ruby::fencing::__private::Duration,
            ) -> $crate::ruby::fencing::__private::Option<$checkpoint> {
                <$checkpoint>::from_parts(scope, issued, lease, observed)
            }

            fn checkpoint_parts(
                checkpoint: &$checkpoint,
            ) -> (
                u16,
                u64,
                $crate::ruby::fencing::__private::Option<$lease>,
                $crate::ruby::fencing::__private::Duration,
            ) {
                let observed = checkpoint.observed();
                (checkpoint.scope(), checkpoint.issued(), checkpoint.lease(), observed)
            }
        }
    };
}
