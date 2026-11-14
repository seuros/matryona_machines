//! Clone-then-keep.
use std::cell::RefCell;

use magnus::{Error, RObject, Ruby, prelude::*};
use matryona_machines::ruby::keep;

use crate::{eval, raises};

pub fn keeping(ruby: &Ruby) -> Result<(), Error> {
    let receiver =
        RObject::from_value(eval(ruby, "Object.new")?).unwrap_or_else(|| panic!("an object"));
    let cell = RefCell::new(10_u32);

    // Honoured: decided on a copy, stored after the answer was built.
    let answer = keep(
        ruby,
        receiver,
        &cell,
        |n| {
            *n += 1;
            *n
        },
        |n| {
            assert!(cell.try_borrow_mut().is_ok(), "no borrow held");
            assert_eq!(
                *cell.borrow(),
                10,
                "nothing stored before the answer exists"
            );
            Ok(n * 2)
        },
    )?;
    assert_eq!((answer, *cell.borrow()), (22, 11));

    // A raise while building applies nothing.
    let failed: Result<u32, Error> = keep(
        ruby,
        receiver,
        &cell,
        |n| *n += 1,
        |()| Err(Error::new(ruby.exception_arg_error(), "boom")),
    );
    raises(failed, ruby.exception_arg_error(), "boom");
    assert_eq!(*cell.borrow(), 11);

    // A re-entrant change while building: RuntimeError, outer call dropped.
    let reentrant = keep(
        ruby,
        receiver,
        &cell,
        |n| *n += 100,
        |()| {
            *cell.borrow_mut() = 50;
            Ok(())
        },
    );
    let runtime = ruby.exception_runtime_error();
    raises(reentrant, runtime, "changed while the answer was built");
    assert_eq!(*cell.borrow(), 50);

    // An equal value written back is not a change.
    let rewritten = keep(
        ruby,
        receiver,
        &cell,
        |n| *n += 1,
        |()| {
            *cell.borrow_mut() = 50;
            Ok(())
        },
    );
    assert!(rewritten.is_ok());
    assert_eq!(*cell.borrow(), 51);

    // A borrow outstanding is RuntimeError, never a panic.
    let held = cell.borrow_mut();
    raises(keep(ruby, receiver, &cell, |_| (), Ok), runtime, "changed");
    drop(held);

    // Frozen meanwhile: FrozenError, nothing applied.
    let frozen = keep(
        ruby,
        receiver,
        &cell,
        |n| *n = 0,
        |()| {
            receiver.freeze();
            Ok(())
        },
    );
    raises(frozen, ruby.exception_frozen_error(), "frozen");
    assert_eq!(*cell.borrow(), 51);

    // Frozen up front: FrozenError before anything runs.
    let ran = core::cell::Cell::new(false);
    let early = keep(ruby, receiver, &cell, |_| ran.set(true), Ok);
    raises(early, ruby.exception_frozen_error(), "frozen");
    assert!(!ran.get());
    Ok(())
}
