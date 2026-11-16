//! What a message closure typically needs, in one glob import.
//!
//! ```ignore
//! use matryona_machines::ruby::prelude::*;
//! ```
//!
//! [`Refused`] to match on, [`explain`] to bind an [`Explain`] closure to a
//! `let`, [`class_name`] to name what arrived, and magnus's [`ReprValue`],
//! where `inspect` lives, for `symbol.inspect()` in a
//! [`Refused::UnknownSymbol`] arm. [`Message`] and the table traits
//! ([`Symbols`], [`SymbolNames`], [`Named`]) are here too, so
//! `Table::symbol(..)` and its kin resolve.
//!
//! ```no_run
//! use magnus::{Error, RClass, Ruby, Value};
//! use matryona_machines::ruby::{Choice, prelude::*, symbol_or_fields_why};
//!
//! #[derive(Debug, PartialEq)]
//! enum Mode { Off, Fixed(u64) }
//!
//! matryona_machines::symbols! {
//!     Modes for Mode as "a mode" => names_only { Off => "off", Fixed(..) => "fixed" }
//! }
//!
//! fn mode(ruby: &Ruby, value: Value, fixed: RClass) -> Result<Choice<Mode, [Value; 1]>, Error> {
//!     let message = explain(|why| match why {
//!         Refused::UnknownSymbol(symbol) => format!("unknown mode {}", symbol.inspect()),
//!         _ => format!("a mode is :off or a Fixed, not {}", class_name(value)),
//!     });
//!     symbol_or_fields_why(ruby, value, Modes, &[fixed], message)
//! }
//! # let _ = Mode::Fixed(0);
//! ```
pub use magnus::value::ReprValue;

pub use super::{Explain, Message, Named, Refused, SymbolNames, Symbols, class_name, explain};
