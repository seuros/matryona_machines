//! Refusals as Symbols.
use core::cmp::Ordering;

use magnus::{Error, Ruby, Symbol, prelude::*};
use matryona_machines::ruby::{Named, SymbolNames, Symbols, parse, symbol};

use crate::{eval, raises};

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Pending,
    TimeRegression,
}

const fn name(state: State) -> &'static str {
    match state {
        State::Pending => "pending",
        State::TimeRegression => "time_regression",
    }
}

pub fn symbols(ruby: &Ruby) -> Result<(), Error> {
    let made: Symbol = symbol(ruby, name(State::TimeRegression));
    let expected: Symbol = ruby.eval(":time_regression")?;
    assert!(made.equal(expected)?);
    let all = [State::Pending, State::TimeRegression];
    let read = |code: &str| parse(ruby, eval(ruby, code)?, "state", &all, name);
    assert_eq!(read(":time_regression")?, State::TimeRegression);
    assert_eq!(read("'pending'.to_sym")?, State::Pending);
    raises(
        read("'pending'"),
        ruby.exception_type_error(),
        "state must be a Symbol",
    );
    raises(
        read(":dead"),
        ruby.exception_range_error(),
        "state must be one of :pending, :time_regression",
    );
    // A Symbol whose name is not valid UTF-8 names no entry: the same
    // RangeError, never magnus's EncodingError (found by behavior_machines).
    let binary = "\"\\xFF\".b.to_sym";
    raises(
        read(binary),
        ruby.exception_range_error(),
        "state must be one of :pending, :time_regression",
    );
    raises(
        States::parse(ruby, eval(ruby, binary)?).map(|_| ()),
        ruby.exception_range_error(),
        "a state must be one of :pending, :time_regression",
    );
    Ok(())
}

matryona_machines::symbols! {
    /// A local enum's table.
    States for State as "a state" {
        Pending => "pending",
        TimeRegression => "time_regression",
    }
}

matryona_machines::ruby::symbols! {
    /// A foreign enum's table: `core::cmp::Ordering` belongs to neither
    /// crate, and the orphan rule still holds.
    Orderings for core::cmp::Ordering as "an ordering" {
        Less => "less",
        Equal => "equal",
        Greater => "greater",
    }
}

pub fn table(ruby: &Ruby) -> Result<(), Error> {
    assert_eq!(States::ALL, [State::Pending, State::TimeRegression]);
    for state in States::ALL {
        assert_eq!(States::name(*state), name(*state));
    }
    let made = States::symbol(ruby, State::TimeRegression);
    assert!(made.equal(ruby.eval::<Symbol>(":time_regression")?)?);
    assert_eq!(
        States::parse(ruby, eval(ruby, ":pending")?)?,
        State::Pending
    );

    assert_eq!(
        Orderings::ALL,
        [Ordering::Less, Ordering::Equal, Ordering::Greater]
    );
    assert_eq!(Orderings::name(Ordering::Greater), "greater");
    // Every Symbol, in table order, frozen: a gem's published constant.
    let every = Orderings::every(ruby);
    assert!(every.is_frozen());
    let expected = eval(ruby, "[:less, :equal, :greater]")?;
    assert!(every.funcall::<_, _, bool>("==", (expected,))?);
    let read = |code: &str| Orderings::parse(ruby, eval(ruby, code)?);
    assert_eq!(read(":equal")?, Ordering::Equal);
    raises(
        read("'equal'"),
        ruby.exception_type_error(),
        "an ordering must be a Symbol",
    );
    raises(
        read(":sideways"),
        ruby.exception_range_error(),
        "an ordering must be one of :less, :equal, :greater",
    );
    Ok(())
}

pub fn variants() {
    assert_eq!(
        States::variant("time_regression"),
        Some(State::TimeRegression)
    );
    assert_eq!(States::variant("pending"), Some(State::Pending));
    assert_eq!(States::variant("Pending"), None);
    assert_eq!(States::variant(""), None);
    assert_eq!(Orderings::variant("less"), Some(Ordering::Less));
    assert_eq!(Orderings::variant(":less"), None);
}

/// A refusal with data variants, as flow's, journal's and election's are.
#[derive(Debug)]
enum Refusal {
    Insufficient { needed: u64, held: u64 },
    Lease(u32, u32),
    Full,
}

matryona_machines::symbols! {
    /// An output-only table.
    Refusals for Refusal as "a refusal" => names_only {
        Insufficient { .. } => "insufficient",
        Lease(..) => "lease",
        Full => "full",
    }
}

pub fn names_only(ruby: &Ruby) -> Result<(), Error> {
    let insufficient = Refusal::Insufficient { needed: 3, held: 1 };
    let lease = Refusal::Lease(1, 2);
    assert_eq!(Refusals::name(&insufficient), "insufficient");
    assert_eq!(Refusals::name(&lease), "lease");
    assert_eq!(Refusals::name(&Refusal::Full), "full");
    assert_eq!(Refusals::NAMES, ["insufficient", "lease", "full"]);
    assert_eq!(Refusals::WHAT, "a refusal");
    let made = Refusals::symbol(ruby, &lease);
    assert!(made.equal(ruby.eval::<Symbol>(":lease")?)?);
    // A unit variant by value, with no binding to borrow.
    let full = Refusals::symbol_of(ruby, Refusal::Full);
    assert!(full.equal(ruby.eval::<Symbol>(":full")?)?);
    if let Refusal::Insufficient { needed, held } = insufficient {
        assert!(needed > held);
    }
    if let Refusal::Lease(a, b) = lease {
        assert!(a < b);
    }
    Ok(())
}

/// A sibling's refusal, carried whole, as election's `Lease` variant is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LeaseRefusal {
    Held,
    Expired,
}

matryona_machines::symbols! {
    /// The sibling's own table.
    LeaseRefusals for LeaseRefusal as "a lease refusal" {
        Held => "held",
        Expired => "expired",
    }
}

/// A reason with data of its own, so its table is `names_only` too.
#[derive(Debug)]
enum Mismatch {
    Depth(u8),
    Fanout,
}

matryona_machines::symbols! {
    /// A names-only inner table.
    Mismatches for Mismatch as "a mismatch" => names_only {
        Depth(..) => "depth_mismatch",
        Fanout => "fanout_mismatch",
    }
}

/// Wraps both: one Symbol space across three tables.
#[derive(Debug)]
enum Wrapping {
    NoCandidates,
    Lease(LeaseRefusal),
    Pinned { by: u8 },
    Schema(Mismatch),
}

matryona_machines::symbols! {
    /// Delegates two variants.
    Wrappings for Wrapping as "a refusal" => names_only {
        NoCandidates => "no_candidates",
        Lease(carried) => use LeaseRefusals,
        Pinned { .. } => "pinned",
        Schema(reason) => use Mismatches,
    }
}

pub fn delegated(ruby: &Ruby) -> Result<(), Error> {
    let lease = Wrapping::Lease(LeaseRefusal::Expired);
    assert_eq!(Wrappings::name(&lease), "expired");
    assert_eq!(
        Wrappings::name(&Wrapping::Lease(LeaseRefusal::Held)),
        "held"
    );
    assert_eq!(
        Wrappings::name(&Wrapping::Schema(Mismatch::Depth(2))),
        "depth_mismatch"
    );
    assert_eq!(
        Wrappings::name(&Wrapping::Schema(Mismatch::Fanout)),
        "fanout_mismatch"
    );
    assert_eq!(Wrappings::name(&Wrapping::Pinned { by: 1 }), "pinned");
    assert_eq!(Wrappings::name(&Wrapping::NoCandidates), "no_candidates");
    assert_eq!(
        Wrappings::NAMES,
        [
            "no_candidates",
            "held",
            "expired",
            "pinned",
            "depth_mismatch",
            "fanout_mismatch"
        ]
    );
    assert_eq!(Wrappings::EVERY_NAME, Wrappings::NAMES);
    // Every name, a delegated table's included, in table order, frozen.
    let every = Wrappings::every(ruby);
    assert!(every.is_frozen());
    let expected = eval(
        ruby,
        "[:no_candidates, :held, :expired, :pinned, :depth_mismatch, :fanout_mismatch]",
    )?;
    assert!(every.funcall::<_, _, bool>("==", (expected,))?);
    // Delegated variants stay output only.
    assert_eq!(Wrappings::UNIT_NAMES, ["no_candidates"]);
    assert!(Wrappings::variant("expired").is_none());
    assert!(Wrappings::variant("no_candidates").is_some());
    raises(
        Wrappings::parse_unit(ruby, eval(ruby, ":held")?).map(|_| ()),
        ruby.exception_range_error(),
        "a refusal must be one of :no_candidates",
    );
    let made = Wrappings::symbol(ruby, &lease);
    assert!(made.equal(ruby.eval::<Symbol>(":expired")?)?);

    // Every table is `Named`, either form, the names by reference.
    assert_eq!(LeaseRefusals::EVERY_NAME, ["held", "expired"]);
    assert_eq!(LeaseRefusals::name_of(&LeaseRefusal::Held), "held");
    assert_eq!(Mismatches::EVERY_NAME, Mismatches::NAMES);
    assert_eq!(Mismatches::name_of(&Mismatch::Fanout), "fanout_mismatch");
    assert_eq!(States::EVERY_NAME, ["pending", "time_regression"]);
    assert_eq!(Refusals::name_of(&Refusal::Full), "full");
    for value in [
        Wrapping::Pinned { by: 4 },
        Wrapping::Schema(Mismatch::Depth(4)),
    ] {
        if let Wrapping::Pinned { by } | Wrapping::Schema(Mismatch::Depth(by)) = value {
            assert_eq!(by, 4);
        }
    }
    Ok(())
}
