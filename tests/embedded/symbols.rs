//! Refusals as Symbols.
use core::cmp::Ordering;

use magnus::{Error, Ruby, Symbol, prelude::*};
use matryona_machines::ruby::{Symbols, parse, symbol};

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
