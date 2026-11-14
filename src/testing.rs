//! Test kit for cores: deterministic generators and the refusal check.
//!
//! `no_std` and allocation-free, so a core's tests run on the same footing as
//! the core. These are test helpers: the assertions panic, which is what a
//! failed test does. Nothing here belongs in a core's runtime code.
//!
//! ```
//! use matryona_machines::testing::{SplitMix64, XorShift64, refused};
//!
//! let mut rng = XorShift64::new(7);
//! assert!(rng.below(10) < 10);
//! assert_eq!(SplitMix64::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
//!
//! // A refusal is an Err and leaves the value exactly as it was.
//! let mut stack = [1_u8, 2, 3];
//! let refusal = refused(&mut stack, |s| s.get(9).copied().ok_or("unknown"));
//! assert_eq!(refusal, "unknown");
//!
//! // An accepted call that must change nothing.
//! assert_eq!(matryona_machines::testing::unchanged(&mut stack, |s| s[1]), 2);
//! ```
use core::fmt::Debug;

/// The seed [`XorShift64::new`] uses in place of zero, the one state an
/// xorshift generator never leaves.
pub const ZERO_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Marsaglia's xorshift64 (shifts 13, 7, 17): one `u64` of state, the same
/// stream for the same seed on every platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XorShift64(u64);

impl XorShift64 {
    /// A generator seeded with `seed`. Zero would stay zero forever, so it is
    /// replaced by [`ZERO_SEED`].
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(if seed == 0 { ZERO_SEED } else { seed })
    }

    /// The next draw; never zero.
    pub const fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// The next draw in `0..bound`, by remainder (the bias is irrelevant to a
    /// test). An empty range, `bound == 0`, answers 0 and still advances.
    pub const fn below(&mut self, bound: u64) -> u64 {
        below(self.next_u64(), bound)
    }
}

/// Steele, Lea and Flood's splitmix64: a Weyl sequence through a mixer.
/// Every seed, zero included, gives a full-period stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// A generator seeded with `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next draw.
    pub const fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The next draw in `0..bound`, by remainder. An empty range,
    /// `bound == 0`, answers 0 and still advances.
    pub const fn below(&mut self, bound: u64) -> u64 {
        below(self.next_u64(), bound)
    }
}

const fn below(draw: u64, bound: u64) -> u64 {
    match draw.checked_rem(bound) {
        Some(value) => value,
        None => 0,
    }
}

/// Runs `call` on `subject`, asserts it answered `Err`, asserts `subject`
/// equals the clone taken before the call, and returns the error so the
/// caller can name the refusal it expected.
///
/// # Panics
///
/// When `call` answers `Ok`, or when it changed `subject`.
#[track_caller]
pub fn refused<S, T, E>(subject: &mut S, call: impl FnOnce(&mut S) -> Result<T, E>) -> E
where
    S: Clone + PartialEq + Debug,
    T: Debug,
{
    let before = subject.clone();
    let refusal = match call(subject) {
        Ok(honoured) => panic!("expected a refusal, the call was honoured: {honoured:?}"),
        Err(refusal) => refusal,
    };
    assert!(
        *subject == before,
        "a refusal changed the value\n before: {before:?}\n  after: {subject:?}"
    );
    refusal
}

/// [`refused`], asserting the refusal is `expected`, or what `expected`
/// converts into (a sibling's refusal carried whole, for instance).
///
/// # Panics
///
/// When `call` answers `Ok`, answers another refusal, or changed `subject`.
#[track_caller]
pub fn refuses<S, T, E>(
    subject: &mut S,
    expected: impl Into<E>,
    call: impl FnOnce(&mut S) -> Result<T, E>,
) where
    S: Clone + PartialEq + Debug,
    T: Debug,
    E: PartialEq + Debug,
{
    let refusal = refused(subject, call);
    assert_eq!(refusal, expected.into(), "refused for another reason");
}

/// Runs `call` on `subject`, asserts `subject` equals the clone taken
/// before the call, and returns the answer.
///
/// For an accepted call that must change nothing: a query through `&mut`,
/// an idempotent repeat, a drain with nothing due.
///
/// ```
/// use matryona_machines::testing::unchanged;
///
/// let mut ids = [4_u8, 7];
/// let found = unchanged(&mut ids, |ids| ids.iter().position(|id| *id == 7));
/// assert_eq!(found, Some(1));
/// ```
///
/// # Panics
///
/// When `call` changed `subject`.
#[track_caller]
pub fn unchanged<S, T>(subject: &mut S, call: impl FnOnce(&mut S) -> T) -> T
where
    S: Clone + PartialEq + Debug,
{
    let before = subject.clone();
    let answer = call(subject);
    assert!(
        *subject == before,
        "the call changed the value\n before: {before:?}\n  after: {subject:?}"
    );
    answer
}

#[cfg(test)]
mod tests;
