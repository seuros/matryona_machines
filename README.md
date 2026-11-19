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
  returns its answer. `describes(&[(Refusal::Full, "the gauge is full"),
  ...])` asserts each refusal's `Display` text, with no allocation.
- **`ruby`** (pulls `magnus` and `std`).
  - `Class::resolve(ruby, module, "Name")` looks a constant up once at init
    and pins it with `gc_register_mark_object`; `Class::pin(ruby, class)`
    pins a class the binding defined itself, with no lookup. `Registry<T>`
    is the static that holds a gem's classes; it raises `NameError` when
    read before init (the extension loaded without its gem) and
    `RuntimeError` when set twice. `registry! { static CLASSES: Classes =
    "gem"; pub fn classes; }` declares that static and the gem's
    `classes(ruby)` accessor over it in one item.
  - `Class::resolve_wrapped::<T>(ruby, module, "Name")` and
    `Class::pin_wrapped::<T>(ruby, class)` do the same for the class a
    `#[magnus::wrap]` type `T` answers under, and fix magnus's own cached
    class for `T` to it. magnus otherwise resolves that class on the
    first conversion of a `T` (a method returning it, `into_value_with`,
    `Obj::wrap`) through `Object.const_get`, a Ruby method, after the core
    has decided: a constant redefined before then makes handles of the
    wrong class, a redefined `const_get` runs caller code mid-call, and
    one that raises hits magnus's `unwrap`, an uncatchable `fatal`. Call
    one for every wrapped type in init: a missing constant is a
    `NameError` there, and so is magnus caching any other class (a wrap
    path naming another constant, a `T` converted before init). Enum wraps
    with per-variant `#[magnus(class = ..)]` arms keep one cache per arm,
    which this does not reach.
  - `sized!(len, [1, 2, 3], |N| Core::<N>::new(..), _ => return Err(..))`
    picks a const-generic size at runtime: the body runs with `const N`
    bound to the matching literal, any other length runs the `_` arm.
  - `instance_of(value, class)` for any value, `same_class(a, b)`, and
    `which(value, &[classes])` (the index of the class it is exactly an
    instance of): the one exact-class check. The readers on it:
    `exact_struct`, `exact_fields::<N>` (a struct's fields as
    `[Value; N]`, no reader run), `optional_fields::<N>` (nil or those
    fields), `exact_array`, `exact_array_n::<N>`, `exact_string`, and
    `exact_typed::<T>` for wrapped Rust data, which refuses the subclass
    instance `Obj<T>`'s `TryConvert` accepts. The struct and typed readers
    have `_with` twins (`exact_fields_with::<N>`, `exact_typed_with::<T>`,
    ...) taking a closure (`Message`) run only on refusal, so a gem can
    name the class it got without allocating on success; so do the
    String and Array readers (`exact_string_with`, `bytes_with`,
    `utf8_with(value, name, message)`, `exact_array_with`,
    `exact_array_n_with::<N>`). `optional_typed::<T>` is nil or
    `exact_typed`.
  - Readers that refuse for more than one reason have `_why` twins
    (`exact_fields_why`, `optional_fields_why`, `exact_array_n_why`,
    `which_fields_why`, `optional_which_fields_why`, `which_struct_why`,
    `symbol_or_fields_why`, `symbol_or_struct_why`, `own_or_sibling_why`)
    taking an `Explain` closure, handed the `Refused` reason: `Subclass`,
    `MissingReader(name)`, `FieldCount { expected, found }`,
    `UnknownSymbol(symbol)`, `NotFinite(real)` or `WrongClass`. The
    reason also picks the exception (`Refused::exception`): `RangeError`
    for an unknown Symbol or a non-finite real, `TypeError` otherwise.
    `class_name(value)` names what arrived for such a message: the real
    class, never `#<Class:#<Object:0x...>>`, and no redefined `class` or
    `to_s` consulted. Written inline as the reader's argument, the
    closure needs no annotation; bound to a `let` first, it needs
    `explain(|why| ..)` or `|why: Refused<'_>|`, or the compiler infers
    it for one lifetime only ("implementation of `FnOnce` is not general
    enough"). `symbol.inspect()` in an `UnknownSymbol` arm needs
    magnus's `ReprValue` in scope; `use matryona_machines::ruby::prelude::*`
    brings it with `Refused`, `explain`, `class_name`, `Message` and the
    table traits.
  - `which_fields::<N>(ruby, value, &[classes], message)`: which class,
    and its fields, in one exact-class check (`optional_which_fields`
    also takes nil). `which_struct` answers `(index, ExactStruct)` for
    classes of different lengths; `ExactStruct::fields::<N>` reads it
    with no second class check.
  - `symbol_or_fields(ruby, value, Table, &[classes], message)` reads a
    mixed enum's argument, a unit variant's Symbol or exactly one of the
    data classes, as `Choice::Symbol(variant)` or
    `Choice::Struct(index, [fields])`; `symbol_or_struct` answers an
    `ExactStruct` instead. `RangeError` for any other Symbol, `TypeError`
    for any other value, both with the caller's message.
  - `own_or_sibling::<N>(ruby, value, own, ["readers"], message)`: the
    gem's own class read from the struct, a sibling gem's value through
    its public readers, a subclass of the own class refused. The sibling
    path runs Ruby, so it belongs with the other conversions, before any
    core call. `own_or_sibling_default` raises the built-in
    `sibling_message(own, readers)`, `"expected a LeaseMachines::Lease or
    a value with #owner, #token, #expires_at"`, with `article` picking
    "an" before a vowel (`"an ElectionMachines::Lease"`).
  - `sibling::<N>(ruby, value, ["readers"], message)`: a sibling's value
    with no own class, by its readers alone; `sibling_shape(value,
    &[&["remaining"], &["overspent"]])` picks the first shape whose
    readers the value has, as `(index, SiblingShape)`, and
    `SiblingShape::read::<N>` calls them. Same rules: public readers only,
    all asked for before any runs.
  - `fencing`: the lease gem's `FencingToken`, `Lease` and `Checkpoint`,
    and the election and handoff gems' copies of them, read and built for
    any core. A gem describes its core with `fencing_family!` (on a marker
    type, from the lease core's method names; no dependency on any core),
    then calls `fencing::token::<Family>`, `lease`, `checkpoint` (own class
    or sibling, rebuilt through the core), `token_object`, `lease_object`,
    `lease_row`, `checkpoint_row`, and the hooks a gem binds directly:
    `validate_raw`, `validate_lease`, `validate_checkpoint`, `own_token`,
    `own_lease`, `token_scope`, `token_order`, `token_newer`
    (`function!(fencing::token_order::<Family>, 2)`). `token: FencingToken`
    alone implements the token half, for a gem that carries only that.
  - `symbol_or_real(ruby, value, Table, message)`: a `Symbols` table's
    variant or a real (`real`'s rule), as `SymbolOrReal::Symbol(variant)`
    or `SymbolOrReal::Real(f64)`; `symbol_or_finite` also refuses NaN and
    the infinities.
  - `keep(ruby, receiver, &cell, decide, build)`: clone-then-keep.
  - `integer::<T>(value, "name")`, `integer_in(value, "name", lo..=hi)`,
    `positive::<NonZero<T>>`, `optional::<T>` (nil or an Integer),
    `optional_positive`, `duration` (`u64` nanoseconds), `instant` (the
    whole `Duration` range), `optional_instant` (nil or an instant),
    `nanoseconds(ruby, duration)`.
  - `real(value, "name")` (exactly a Float or an Integer, as `f64`),
    `finite` (also refusing NaN and the infinities), `boolean` (exactly
    `true` or `false`), `bytes` and `utf8` (exactly a String, copied out
    with no Ruby method run and no `unsafe`).
  - `symbol(ruby, "snake_case")` and `parse(ruby, value, what, ALL, name)`;
    `symbols!` builds a `Symbols` table (`Table::symbol`, `Table::parse`,
    `Table::variant("name")`) whose variant list the compiler checks
    against the enum, the core's own included; its `names_only` form
    builds a `SymbolNames` table for an enum whose variants carry data
    (`Pinned { .. } => "pinned"`): every variant out, the unit ones
    (written as a bare name) back in through `Table::parse_unit` and
    `Table::variant`, and a wrapping variant answered under its payload's
    table (`Lease(inner) => use LeaseRefusals`). `Table::symbol_of(ruby,
    Admission::Buffered)` takes the variant by value, so a unit arm
    answers its Symbol with no `buffered @` binding to borrow.
    `Table::every(ruby)` is every Symbol in table order as a frozen Array,
    the constant a gem publishes (`REFUSALS`), for either form (a
    `names_only` table's `NAMES`, delegated names included). Every table is `Named`
    (`EVERY_NAME`, `name_of(&variant)`).

```rust,ignore
use core::num::NonZero;
use matryona_machines::ruby::{
    Choice, Symbols, duration, exact_typed, exact_typed_with, finite, instant, integer_in,
    positive, symbol_or_fields, utf8,
};
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
let handle = exact_typed_with::<Handle>(ruby, value, class, || {
    format!("handle must be a PoolMachines::Handle, not {}", class_name(value))
})?;
let max: usize = integer_in(value, "max", 0..=WAITER_CAPACITY)?; // "max must be in 0..=64"
let gain = finite(value, "gain")?;                              // 1 and 0.5; not 1r, not NaN
let key = utf8(value, "key")?;                                  // exactly a String, UTF-8 text
let refusal = Refusals::parse(ruby, value)?;
let fault = match symbol_or_fields(ruby, value, Faults, &[delay], "a fault is :raise or a Delay")? {
    Choice::Symbol(fault) => fault,                           // a unit arm of Faults' names_only table
    Choice::Struct(_, [span]) => Fault::Delay(duration(span, "span")?),
};
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
- **Reals** are exactly a Float or an Integer, by type tag: a Rational,
  a `BigDecimal`, a String or an object with `to_f` is `TypeError`
  `"<name> must be a Float or Integer"`, and no `to_f` runs, a redefined
  `Integer#to_f` included (magnus's own `f64` conversion would call it).
  An Integer becomes the nearest `f64`, ties to even, which is what
  Ruby's `Integer#to_f` answers: a Float is rounded already, so an
  Integer is held to the same rule, and `10**17` means the real it reads
  as. Past `f64::MAX` that is an infinity, as in Ruby. `real` passes NaN
  and the infinities through; `finite` refuses them with `RangeError`
  `"<name> must be finite"`. Booleans are exactly `true` or `false`
  (`"<name> must be true or false"`), never truthiness.
- **Strings** are exactly a String (`"<name> must be a String"`, a
  subclass and a `to_str` object included), and their bytes are copied
  with magnus's `RString::to_bytes` (its `bytes` feature): no `#bytes`, no
  `#to_s`, no `unsafe`. `bytes` takes any encoding. `utf8` takes a UTF-8
  String with valid bytes, or ASCII bytes in any ASCII-compatible encoding
  (`"abc".b` is the text `abc`), and never transcodes: anything else is
  `RangeError` `"<name> must be valid UTF-8"`, a binary `"caf\xC3\xA9".b`
  and a UTF-16 String included.
- **Symbols** come from the binding's exhaustive `match`; there is no
  catch-all. `parse` raises `TypeError` for a non-Symbol and `RangeError`
  listing every accepted Symbol. A `symbols!` table writes that `match`
  and its `ALL` list from one list of `Variant => "name"` pairs: a variant
  missing from it is a non-exhaustive `match` (`E0004`) and the gem does
  not build; a variant listed twice, a name given twice or a name that is
  not `snake_case` fails a `const` assertion. The table is a local unit
  struct implementing `Symbols`, so the enum may be the core's (no orphan
  rule trouble). The `names_only` form, for an enum with data variants,
  writes the same exhaustive `match` from shapes (`Pinned { .. }`,
  `Lease(..)`, `Full`) and has no `ALL` and no `parse`: a data variant
  cannot be built from a name. Its bare-name arms are the unit variants,
  and `parse_unit` reads exactly those back; a data variant written as a
  bare name does not compile (`E0533`). An arm `Lease(inner) => use
  Table` answers under the inner table's names, and `NAMES` is the
  union, asserted distinct at compile time across every table involved;
  a table for another type than the payload is `E0308`. A variant listed
  twice there is a duplicate definition (`E0428`).
- **Sibling values** are read by their public readers only when the value
  is not the gem's own class or a subclass of it; every reader is asked
  for before any is called. That path runs Ruby (`respond_to?` when
  redefined, and each reader), so it happens while converting arguments,
  before any core call and with no borrow held.
- **Refusal reasons** are found only on refusal, natively
  (`rb_obj_is_kind_of` for `Subclass`), so an accepted value pays
  nothing. `_with` readers run on their `_why` twins with the reason
  ignored, so their messages and exception classes are unchanged.
  `class_name` and `sibling_message` run Ruby's own `Kernel#class` and
  `Module#to_s` through `bind_call` (magnus's native name readers are
  `unsafe`), which is why they belong in a message closure.
- **Generators** are for tests: `below(0)` answers 0 rather than panic, and
  `XorShift64::new(0)` uses `ZERO_SEED`, since zero is the one state
  xorshift never leaves.

The `ruby` feature is tested inside a live VM: `tests/embedded` boots Ruby
with magnus's `embed` (linked into that test binary only) and runs every
item, a build after `GC.compact` included.
`cargo test -p matryona-machines-embedded`. The `symbols!` compile-time
checks, and those on `Explain` closures and `SiblingShape`, are
`compile_fail` doc tests: `cargo test --doc --features ruby`.

## Not Included

No `Data` class definitions, no validation (that is each core's
constructor), no Ruby-side sealing. Nothing here decides what a core decides.

## License

MIT
