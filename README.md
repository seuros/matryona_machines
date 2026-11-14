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
  `unchanged(&mut value, call)` is the same proof for an accepted call that
  must change nothing (a query through `&mut`, an honoured repeat), and
  returns its answer.
- **`ruby`** (pulls `magnus` and `std`).
  - `Class::resolve(ruby, module, "Name")` looks a constant up once at init
    and pins it with `gc_register_mark_object`; `Class::pin(ruby, class)`
    pins a class the binding defined itself, with no lookup. `Registry<T>`
    is the static that holds a gem's classes and raises `RuntimeError` when
    read before init or set twice.
  - `instance_of(value, class)` for any value, and `same_class(a, b)`: the
    one exact-class check. The readers on it: `exact_struct` (and
    `exact_struct_with`, its message built only on refusal),
    `exact_fields::<N>` (a struct's fields as `[Value; N]`, no reader run),
    `exact_array`, `exact_array_n::<N>`, and `exact_typed::<T>` for wrapped
    Rust data, which refuses the subclass instance `Obj<T>`'s `TryConvert`
    accepts.
  - `keep(ruby, receiver, &cell, decide, build)`: clone-then-keep.
  - `integer::<T>(value, "name")`, `positive::<NonZero<T>>`,
    `optional::<T>` (nil or an Integer), `duration` (`u64` nanoseconds),
    `instant` (the whole `Duration` range), `nanoseconds(ruby, duration)`.
  - `symbol(ruby, "snake_case")` and `parse(ruby, value, what, ALL, name)`;
    `symbols!` builds a `Symbols` table (`Table::symbol`, `Table::parse`)
    whose variant list the compiler checks against the enum, the core's own
    included.

```rust,ignore
use core::num::NonZero;
use matryona_machines::ruby::{Symbols, exact_typed, instant, positive};
use outbox_machines::Refusal;

matryona_machines::symbols! {
    /// The core's refusals as Symbols.
    pub Refusals for Refusal as "a refusal" {
        Full => "full",
        Stale => "stale",
    }
}

let capacity: NonZero<u32> = positive(value, "capacity")?; // 0 and 2**32: 1..=4294967295
let expires = instant(value, "expires")?;                  // past u64 nanoseconds
let ticket: Obj<Ticket> = exact_typed(ruby, value, class, "expected a Ticket")?;
let refusal = Refusals::parse(ruby, value)?;
Ok(Refusals::symbol(ruby, refusal).as_value())
```

## Contract

- **The exact-class check** accepts an instance of exactly the class, with
  or without an opened singleton class (a `clone` of one included), and
  refuses every subclass instance and any forgery. Class identity is
  `Module#<=` both ways, called natively through `rb_class_inherited_p`,
  so redefining `==`, `instance_of?`, `class` or `<=` changes nothing. It
  needs neither magnus's `rb-sys` feature nor raw pointers; `same_class`
  is that identity rule on its own. Asking a direct-subclass instance for
  its singleton class creates one; that value is refused either way.
  Any value is accepted, a bare `Value` included; a class is an instance
  of exactly `Class`. The one gap: a `Regexp` or untyped `T_DATA` with an
  opened singleton class is refused, since magnus asks for a singleton
  class only through types those values have none of. Readers raise
  `TypeError` with the caller's message, an Array or struct of the wrong
  length included.
- **Clone-then-keep** raises `FrozenError` before anything runs on a frozen
  receiver, copies the core under a short borrow, decides on the copy, builds
  the answer with no borrow held, then raises `FrozenError` if the receiver
  was frozen meanwhile or `RuntimeError` if the core no longer equals the
  copy taken before. Only then is the copy stored. A borrow conflict is the
  same `RuntimeError`, never a panic.
- **Integers** are read natively: a Float, a String or an object with
  `to_int` is `TypeError` `"<name> must be an Integer"`; one outside the type
  is `RangeError` `"<name> must be in <MIN>..=<MAX>"`, and for `positive`
  `"<name> must be in 1..=<MAX>"` at both ends. Spans enter as `u64`
  nanoseconds (`duration`); instants, which a core may have answered as
  `now + span`, enter over the whole `Duration` range (`instant`,
  `"<name> must be in 0..=Duration::MAX nanoseconds"`) and leave exact
  through `u128` (`nanoseconds`).
- **Symbols** come from the binding's exhaustive `match`; there is no
  catch-all. `parse` raises `TypeError` for a non-Symbol and `RangeError`
  listing every accepted Symbol. A `symbols!` table writes that `match`
  and its `ALL` list from one list of `Variant => "name"` pairs: a variant
  missing from it is a non-exhaustive `match` (`E0004`) and the gem does
  not build; a variant listed twice, a name given twice or a name that is
  not `snake_case` fails a `const` assertion. The table is a local unit
  struct implementing `Symbols`, so the enum may be the core's (no orphan
  rule trouble).
- **Generators** are for tests: `below(0)` answers 0 rather than panic, and
  `XorShift64::new(0)` uses `ZERO_SEED`, since zero is the one state
  xorshift never leaves.

The `ruby` feature is tested inside a live VM: `tests/embedded` boots Ruby
with magnus's `embed` (linked into that test binary only) and runs every
item, a build after `GC.compact` included.
`cargo test -p matryona-machines-embedded`. The `symbols!` compile-time
checks are `compile_fail` doc tests: `cargo test --doc --features ruby`.

## Not Included

No `Data` class definitions, no validation (that is each core's
constructor), no Ruby-side sealing. Nothing here decides what a core decides.

## License

MIT
