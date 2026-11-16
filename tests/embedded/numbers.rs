//! A Symbol from a table, or a real.
use magnus::{Error, Ruby};
use matryona_machines::ruby::{
    SymbolOrReal, symbol_or_finite, symbol_or_finite_why, symbol_or_real, symbol_or_real_why,
};

use crate::{eval, raises, reasons::tag};

/// A controller's refusals, as feedback's are.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Refusal {
    Saturated,
    Unstable,
}

matryona_machines::symbols! {
    /// Refusals by name.
    Refusals for Refusal as "a refusal" {
        Saturated => "saturated",
        Unstable => "unstable",
    }
}

/// Not real numbers, or not by type tag.
const NOT_REAL: &[&str] = &[
    "'1.5'",
    "1r",
    "nil",
    "true",
    "Fixture::Point.new(1, 2)",
    "1i",
];

pub fn symbol_or_number(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let real = |code: &str| symbol_or_real(ruby, eval(ruby, code)?, Refusals, "step answer");
    assert_eq!(
        real(":saturated")?,
        SymbolOrReal::Symbol(Refusal::Saturated)
    );
    assert_eq!(real(":unstable")?, SymbolOrReal::Symbol(Refusal::Unstable));
    assert_eq!(real("1.5")?, SymbolOrReal::Real(1.5));
    assert_eq!(real("-2")?, SymbolOrReal::Real(-2.0));
    assert_eq!(
        real("2**53 + 1")?,
        SymbolOrReal::Real(9_007_199_254_740_992.0)
    );
    assert_eq!(real("Float::INFINITY")?, SymbolOrReal::Real(f64::INFINITY));
    assert_eq!(real("10**400")?, SymbolOrReal::Real(f64::INFINITY));
    let SymbolOrReal::Real(nan) = real("Float::NAN")? else {
        panic!("a real");
    };
    assert!(nan.is_nan());
    raises(real(":nope"), range_error, "step answer");
    for code in NOT_REAL {
        raises(real(code), type_error, "step answer");
    }
    // A redefined `to_f` never runs: an Integer is read by its tag.
    eval(
        ruby,
        "class Integer; alias __to_f to_f; def to_f = 0.0; end",
    )?;
    assert_eq!(real("7")?, SymbolOrReal::Real(7.0));
    eval(ruby, "class Integer; alias to_f __to_f; end")?;

    let why = |code: &str| symbol_or_real_why(ruby, eval(ruby, code)?, Refusals, tag);
    raises(why(":nope"), range_error, "symbol :nope");
    raises(why("'x'"), type_error, "wrong");

    let finite = |code: &str| symbol_or_finite(ruby, eval(ruby, code)?, Refusals, "finite answer");
    assert_eq!(finite("0.25")?, SymbolOrReal::Real(0.25));
    assert_eq!(
        finite(":saturated")?,
        SymbolOrReal::Symbol(Refusal::Saturated)
    );
    for code in [
        "Float::NAN",
        "Float::INFINITY",
        "-Float::INFINITY",
        "10**400",
    ] {
        raises(finite(code), range_error, "finite answer");
    }
    raises(finite(":nope"), range_error, "finite answer");
    raises(finite("1r"), type_error, "finite answer");

    let why = |code: &str| symbol_or_finite_why(ruby, eval(ruby, code)?, Refusals, tag);
    raises(why("Float::INFINITY"), range_error, "not finite inf");
    raises(why("-10**400"), range_error, "not finite -inf");
    raises(why("Float::NAN"), range_error, "not finite NaN");
    raises(why(":saturated_x"), range_error, "symbol :saturated_x");
    raises(why("[]"), type_error, "wrong");
    Ok(())
}
