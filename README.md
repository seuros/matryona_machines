# matryona-machines

> **AMS Malyutin**, NCC-4237, *Malyutin*-class nesting tender.

Deck-watch flight manual: carry a core's answer out through its binding and
its gem without losing it, and prove a refusal left the core as it found it.

Matryona is the doll's actual name; *matryoshka* is only "little Matryona".
Sergey Malyutin painted the first set in 1890: a mother holding her children
holding theirs. A machines gem is built the same way, a `no_std` core inside
a magnus binding inside a Ruby gem, and every gem used to carve its own outer
doll: the class registry, the exact-class check, clone-then-keep, the Integer
converters, the refusal symbols, and in the core's tests the same xorshift and
the same "refused and unchanged" assertion. They drifted. Three exact-class
checks disagreed about a value whose singleton class had been opened. This
tender holds the one shell they all share.

`no_std` by default, no default features, no `unsafe`. This is a Rust-only
support crate with no Ruby gem.

```toml
# A core's Cargo.toml: test kit only, never magnus.
[dev-dependencies]
matryona-machines = { version = ">=0.1", features = ["testing"] }

# A gem's FFI crate.
[dependencies]
matryona-machines = { version = ">=0.1", features = ["ruby"] }
```

```rust
use matryona_machines::testing::{XorShift64, refused, refuses};

#[derive(Clone, Debug, PartialEq)]
struct Gauge { level: u8 }

#[derive(Debug, PartialEq)]
enum Refusal { Full }

impl Gauge {
    fn fill(&mut self) -> Result<u8, Refusal> {
        if self.level == 3 { return Err(Refusal::Full); }
        self.level += 1;
        Ok(self.level)
    }
}

let mut gauge = Gauge { level: 3 };
// Err, and the gauge equals the clone taken before the call.
assert_eq!(refused(&mut gauge, Gauge::fill), Refusal::Full);
refuses(&mut gauge, Refusal::Full, Gauge::fill);

let mut rng = XorShift64::new(42);
assert!(rng.below(6) < 6);
```

## Features

- **`testing`** (`no_std`). `XorShift64` and `SplitMix64`: one `u64` of
  state, `next_u64`, `below(n)`. `refused(&mut value, call)` runs the call,
  asserts `Err`, asserts the value equals its clone from before, and returns
  the refusal; `refuses(&mut value, expected, call)` also names it.
- **`ruby`** (pulls `magnus` and `std`).
  - `Class::resolve(ruby, module, "Name")` looks a constant up once at init
    and pins it with `gc_register_mark_object`; `Registry<T>` is the static
    that holds a gem's classes and raises `RuntimeError` when read before
    init or set twice.
  - `instance_of`, `exact_struct`, `exact_array`: the one exact-class check.
  - `keep(ruby, receiver, &cell, decide, build)`: clone-then-keep.
  - `integer::<T>(value, "name")`, `duration(value, "name")`,
    `nanoseconds(ruby, duration)`.
  - `symbol(ruby, "snake_case")` and `parse(ruby, value, what, ALL, name)`.

## Contract

- **The exact-class check** accepts an instance of exactly the class, with
  or without an opened singleton class (a `clone` of one included), and
  refuses every subclass instance and any forgery. Class identity is
  `Module#<=` both ways, called natively through `rb_class_inherited_p`,
  so redefining `==`, `instance_of?`, `class` or `<=` changes nothing. It
  needs neither magnus's `rb-sys` feature nor raw pointers. Asking a
  direct-subclass instance for its singleton class creates one; that
  value is refused either way.
- **Clone-then-keep** raises `FrozenError` before anything runs on a frozen
  receiver, copies the core under a short borrow, decides on the copy, builds
  the answer with no borrow held, then raises `FrozenError` if the receiver
  was frozen meanwhile or `RuntimeError` if the core no longer equals the
  copy taken before. Only then is the copy stored. A borrow conflict is the
  same `RuntimeError`, never a panic.
- **Integers** are read natively: a Float, a String or an object with
  `to_int` is `TypeError` `"<name> must be an Integer"`; one outside the type
  is `RangeError` `"<name> must be in <MIN>..=<MAX>"`. Time enters as
  `u64` nanoseconds and leaves exact through `u128`, since `now + span` can
  pass `u64`.
- **Symbols** come from the binding's exhaustive `match`; there is no
  catch-all. `parse` raises `TypeError` for a non-Symbol and `RangeError`
  listing every accepted Symbol.
- **Generators** are for tests: `below(0)` answers 0 rather than panic, and
  `XorShift64::new(0)` uses `ZERO_SEED`, since zero is the one state
  xorshift never leaves.

The `ruby` feature is tested inside a live VM: `tests/embedded` boots Ruby
with magnus's `embed` (linked into that test binary only) and runs every
item, a build after `GC.compact` included.
`cargo test -p matryona-machines-embedded`.

## Not Included

No `Data` class definitions, no validation (that is each core's
constructor), no Ruby-side sealing. Nothing here decides what a core decides.

## License

MIT
