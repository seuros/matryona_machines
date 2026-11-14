//! Reals and booleans.
use magnus::{Error, Float, Ruby, Value};
use matryona_machines::ruby::{boolean, finite, real};

use crate::{eval, raises};

/// Integers whose nearest `f64` is a rounding case: past 2**53, ties to
/// even, past `i128`, past `u128`, at and past `f64::MAX`, negative ones.
const ROUNDING: &[&str] = &[
    "0",
    "-1",
    "2**53",
    "2**53 + 1",
    "2**53 + 3",
    "2**63",
    "-(2**63)",
    "2**64 - 1",
    "2**127 - 1",
    "-(2**127)",
    "2**127",
    "2**128 - 1",
    "2**128 + 2**75",
    "2**128 + 2**75 + 1",
    "2**200 + 2**147",
    "2**200 + 2**147 + 1",
    "2**200 + 2**148 + 2**147",
    "-(2**200 + 2**147 + 1)",
    "3**500",
    "-(7**333)",
    "10**308",
    "2**1024 - 2**971",
    "2**1024 - 2**970 - 1",
    "2**1024 - 2**970",
    "2**1024",
    "-(2**1024)",
    "10**400",
    "2**1152 + 1",
    "10**5000",
];

pub fn reals(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let value = |code: &str| eval(ruby, code);

    assert_eq!(real(value("0.5")?, "gain")?.to_bits(), 0.5_f64.to_bits());
    assert_eq!(real(value("-3")?, "gain")?.to_bits(), (-3.0_f64).to_bits());
    assert_eq!(
        real(value("1e300")?, "gain")?.to_bits(),
        1e300_f64.to_bits()
    );
    assert!(real(value("Float::NAN")?, "gain")?.is_nan());
    assert_eq!(real(value("-Float::INFINITY")?, "gain")?, f64::NEG_INFINITY);
    assert_eq!(
        real(value("-0.0")?, "gain")?.to_bits(),
        (-0.0_f64).to_bits()
    );

    // Every Integer lands where Ruby's own `to_f` puts it.
    for code in ROUNDING {
        let ours = real(value(code)?, "gain")?;
        let ruby_to_f: Float = ruby.eval(&format!("({code}).to_f"))?;
        assert_eq!(ours.to_bits(), ruby_to_f.to_f64().to_bits(), "{code}");
    }
    let tie: f64 = real(value("2**53 + 1")?, "gain")?;
    assert_eq!(tie.to_bits(), 9_007_199_254_740_992.0_f64.to_bits());

    // Exactly Float or Integer: not Rational, String, nil, `to_f` objects.
    for code in [
        "1r",
        "1.5r",
        "Complex(1, 0)",
        "'1.0'",
        "nil",
        "true",
        ":one",
        "Object.new.tap { |o| def o.to_f = 1.0 }",
        "Class.new(Numeric) { def to_f = 1.0 }.new",
    ] {
        raises(
            real(value(code)?, "gain"),
            type_error,
            "gain must be a Float or Integer",
        );
    }

    // A redefined `Integer#to_f` (which `rb_num2dbl` would call) is ignored.
    let redefined = value("2**200")?;
    eval(
        ruby,
        "class Integer; alias_method :__matryona_to_f, :to_f; def to_f = 42.0; end",
    )?;
    let untouched = real(redefined, "gain");
    let small = real(value("7")?, "gain");
    eval(
        ruby,
        "class Integer; alias_method :to_f, :__matryona_to_f; remove_method :__matryona_to_f; end",
    )?;
    assert_eq!(untouched?.to_bits(), 2.0_f64.powi(200).to_bits());
    assert_eq!(small?.to_bits(), 7.0_f64.to_bits());

    assert_eq!(
        finite(value("2.5")?, "budget")?.to_bits(),
        2.5_f64.to_bits()
    );
    assert_eq!(finite(value("10**308")?, "budget")?, 1e308);
    for code in [
        "Float::NAN",
        "Float::INFINITY",
        "-Float::INFINITY",
        "2**1024",
        "-(10**400)",
    ] {
        raises(
            finite(value(code)?, "budget"),
            range_error,
            "budget must be finite",
        );
    }
    raises(
        finite(value("1r")?, "budget"),
        type_error,
        "budget must be a Float or Integer",
    );
    Ok(())
}

pub fn booleans(ruby: &Ruby) -> Result<(), Error> {
    let type_error = ruby.exception_type_error();
    let read = |code: &str| -> Result<bool, Error> {
        let value: Value = eval(ruby, code)?;
        boolean(value, "eligible")
    };
    assert!(read("true")?);
    assert!(!read("false")?);
    assert!(read("1 == 1")?);
    for code in ["nil", "0", "1", "'true'", ":true", "Object.new", "[]"] {
        raises(read(code), type_error, "eligible must be true or false");
    }
    Ok(())
}
