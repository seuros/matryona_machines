use super::{SplitMix64, XorShift64, ZERO_SEED, refused, refuses, unchanged};

#[test]
fn xorshift_follows_the_published_recurrence() {
    let mut rng = XorShift64::new(1);
    assert_eq!(rng.next_u64(), 0x4082_2041);
    let mut by_hand = 0x4082_2041_u64;
    by_hand ^= by_hand << 13;
    by_hand ^= by_hand >> 7;
    by_hand ^= by_hand << 17;
    assert_eq!(rng.next_u64(), by_hand);
}

#[test]
fn xorshift_replaces_a_zero_seed() {
    assert_eq!(XorShift64::new(0), XorShift64::new(ZERO_SEED));
    let mut rng = XorShift64::new(0);
    assert!((0..1_000).all(|_| rng.next_u64() != 0));
}

#[test]
fn splitmix_matches_the_reference_stream() {
    // The first outputs of the reference implementation seeded with zero.
    let mut rng = SplitMix64::new(0);
    assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    assert_eq!(rng.next_u64(), 0x06C4_5D18_8009_454F);
}

#[test]
fn the_same_seed_gives_the_same_stream() {
    let (mut a, mut b) = (XorShift64::new(42), XorShift64::new(42));
    let (mut c, mut d) = (SplitMix64::new(42), SplitMix64::new(42));
    for _ in 0..100 {
        assert_eq!(a.next_u64(), b.next_u64());
        assert_eq!(c.next_u64(), d.next_u64());
    }
}

#[test]
fn below_stays_in_range_and_reaches_every_value() {
    let mut x = XorShift64::new(9);
    let mut s = SplitMix64::new(9);
    let (mut seen_x, mut seen_s) = (0_u8, 0_u8);
    for _ in 0..1_000 {
        let (a, b) = (x.below(7), s.below(7));
        assert!(a < 7 && b < 7);
        seen_x |= 1 << a;
        seen_s |= 1 << b;
    }
    assert_eq!((seen_x, seen_s), (0b111_1111, 0b111_1111));
    assert_eq!(x.below(1), 0);
}

#[test]
fn below_an_empty_range_answers_zero_and_advances() {
    let mut rng = XorShift64::new(5);
    let mut twin = rng;
    assert_eq!(rng.below(0), 0);
    twin.next_u64();
    assert_eq!(rng, twin);
    let mut mix = SplitMix64::new(5);
    let mut mixed_twin = mix;
    assert_eq!(mix.below(0), 0);
    mixed_twin.next_u64();
    assert_eq!(mix, mixed_twin);
}

/// A small subject with a refusing operation.
#[derive(Clone, Debug, PartialEq)]
struct Counter {
    value: u8,
    limit: u8,
}

#[derive(Debug, PartialEq)]
enum Refusal {
    Full,
}

impl Counter {
    const fn bump(&mut self) -> Result<u8, Refusal> {
        if self.value == self.limit {
            return Err(Refusal::Full);
        }
        self.value += 1;
        Ok(self.value)
    }

    /// Wrong on purpose: records the attempt even when it refuses.
    const fn leaky_bump(&mut self) -> Result<u8, Refusal> {
        if self.value == self.limit {
            self.limit += 1;
            return Err(Refusal::Full);
        }
        self.bump()
    }
}

#[test]
fn refused_returns_the_refusal_of_an_unchanged_value() {
    let mut full = Counter { value: 3, limit: 3 };
    assert_eq!(refused(&mut full, Counter::bump), Refusal::Full);
    refuses(&mut full, Refusal::Full, Counter::bump);
    assert_eq!(full, Counter { value: 3, limit: 3 });
}

#[test]
#[should_panic(expected = "the call was honoured")]
fn refused_fails_an_honoured_call() {
    let mut room = Counter { value: 0, limit: 3 };
    refused(&mut room, Counter::bump);
}

#[test]
#[should_panic(expected = "a refusal changed the value")]
fn refused_fails_a_refusal_that_changed_the_value() {
    let mut full = Counter { value: 3, limit: 3 };
    refused(&mut full, Counter::leaky_bump);
}

#[test]
#[should_panic(expected = "refused for another reason")]
fn refuses_fails_another_refusal() {
    let mut full = Counter { value: 3, limit: 3 };
    refuses(&mut full, "other", |c| c.bump().map_err(|_| "full"));
}

#[test]
fn unchanged_returns_the_answer_of_a_call_that_changed_nothing() {
    let mut full = Counter { value: 3, limit: 3 };
    assert_eq!(unchanged(&mut full, |c| c.value), 3);
    // A refusal is an answer too; `unchanged` does not judge it.
    assert_eq!(unchanged(&mut full, Counter::bump), Err(Refusal::Full));
    // A write of the value already held is no change.
    unchanged(&mut full, |c| c.value = 3);
    assert_eq!(full, Counter { value: 3, limit: 3 });
}

#[test]
#[should_panic(expected = "the call changed the value")]
fn unchanged_fails_a_call_that_changed_the_value() {
    let mut room = Counter { value: 0, limit: 3 };
    let _ = unchanged(&mut room, Counter::bump);
}

#[test]
#[should_panic(expected = "the call changed the value")]
fn unchanged_fails_a_refusal_that_changed_the_value() {
    let mut full = Counter { value: 3, limit: 3 };
    let _ = unchanged(&mut full, Counter::leaky_bump);
}
