//! Clone-then-keep.
use core::cell::RefCell;

use magnus::{Error, Ruby, value::ReprValue};

const CHANGED: &str = "the receiver changed while the answer was built; the call was not applied";

fn changed(ruby: &Ruby) -> Error {
    Error::new(ruby.exception_runtime_error(), CHANGED)
}

/// Runs a mutating call whose answer is a Ruby object without ever losing
/// work or panicking into an uncatchable `fatal`:
///
/// 1. `receiver` frozen: `FrozenError`, nothing runs.
/// 2. The core in `cell` is copied under a short borrow, released at once.
/// 3. `decide` runs on the copy (Rust only: the core's call).
/// 4. `build` turns its answer into Ruby with **no borrow held**, so a
///    redefined `new` that calls back into the receiver finds it free.
///    A raise here leaves the core as it was.
/// 5. `receiver` frozen meanwhile: `FrozenError`; the core no longer equal
///    to the copy taken in step 2 (a re-entrant call changed it):
///    `RuntimeError`. Either way the decided copy is dropped.
/// 6. Only then is the copy stored and the answer returned.
///
/// A borrow conflict, which no caller following the rules can cause, is the
/// same `RuntimeError` rather than a panic.
///
/// ```ignore
/// fn claim(ruby: &Ruby, this: Obj<Outbox>, now: Value, lease: Value) -> Result<Value, Error> {
///     let (now, lease) = (duration(now, "now")?, duration(lease, "lease")?);
///     let class = CLASSES.get(ruby)?.claimed.get(ruby);
///     keep(ruby, this, &this.core, |core| core.claim(now, lease), |claimed| match claimed {
///         Ok(Some(c)) => class.funcall("new", (c.id(), c.attempt())),
///         Ok(None) => Ok(ruby.qnil().as_value()),
///         Err(refusal) => Ok(symbol(ruby, refusal_name(refusal)).as_value()),
///     })
/// }
/// ```
///
/// # Errors
///
/// `FrozenError` and `RuntimeError` as above, and whatever `build` raises.
pub fn keep<C, A, R>(
    ruby: &Ruby,
    receiver: impl ReprValue,
    cell: &RefCell<C>,
    decide: impl FnOnce(&mut C) -> A,
    build: impl FnOnce(A) -> Result<R, Error>,
) -> Result<R, Error>
where
    C: Clone + PartialEq,
{
    receiver.check_frozen()?;
    let before = cell.try_borrow().map_err(|_| changed(ruby))?.clone();
    let mut after = before.clone();
    let answer = build(decide(&mut after))?;
    receiver.check_frozen()?;
    let mut held = cell.try_borrow_mut().map_err(|_| changed(ruby))?;
    if *held != before {
        return Err(changed(ruby));
    }
    *held = after;
    drop(held);
    Ok(answer)
}
