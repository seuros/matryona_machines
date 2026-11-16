//! The fencing-token family, over a stand-in core with the lease core's
//! rules: scope in the top 16 bits, a sequence counted from one.
use core::{cmp::Ordering, time::Duration};

use magnus::{Error, RArray, Ruby, TryConvert, Value, prelude::*};
use matryona_machines::ruby::{
    Class,
    fencing::{
        checkpoint, checkpoint_row, lease, lease_object, lease_row, own_lease, own_token, token,
        token_newer, token_object, token_order, token_scope, validate_checkpoint, validate_lease,
        validate_raw,
    },
};

use crate::{eval, raises};

const SETUP: &str = r"
module Fenced
  FencingToken = Data.define(:raw)
  Lease = Data.define(:owner, :token, :expires_at)
  Checkpoint = Data.define(:scope, :issued, :lease, :observed)
  class SubToken < FencingToken; end
  # Another gem's token and lease: same readers, their own classes.
  SiblingToken = Data.define(:raw)
  SiblingLease = Data.define(:owner, :token, :expires_at)
end
";

const SEQUENCE: u64 = (1 << 48) - 1;

/// The stand-in core's types, with the lease core's methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Token(u64);

impl Token {
    fn from_raw(raw: u64) -> Option<Self> {
        (raw & SEQUENCE != 0).then_some(Self(raw))
    }

    const fn raw(self) -> u64 {
        self.0
    }

    fn scope(self) -> u16 {
        u16::try_from(self.0 >> 48).unwrap_or(u16::MAX)
    }

    fn order(self, other: Self) -> Option<Ordering> {
        (self.scope() == other.scope()).then(|| self.0.cmp(&other.0))
    }

    fn is_newer_than(self, other: Self) -> bool {
        self.order(other) == Some(Ordering::Greater)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Owner(u64);

impl Owner {
    const fn new(owner: u64) -> Self {
        Self(owner)
    }

    const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Held(Owner, Token, Duration);

impl Held {
    const fn new(owner: Owner, token: Token, expires_at: Duration) -> Self {
        Self(owner, token, expires_at)
    }

    const fn owner(self) -> Owner {
        self.0
    }

    const fn token(self) -> Token {
        self.1
    }

    const fn expires_at(self) -> Duration {
        self.2
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Saved(u16, u64, Option<Held>, Duration);

impl Saved {
    fn from_parts(
        scope: u16,
        issued: u64,
        lease: Option<Held>,
        observed: Duration,
    ) -> Option<Self> {
        let last = (u64::from(scope) << 48) | issued;
        let ends = lease.is_none_or(|held| held.1 == Token(last));
        (issued <= SEQUENCE && ends).then_some(Self(scope, issued, lease, observed))
    }

    const fn scope(&self) -> u16 {
        self.0
    }

    const fn issued(&self) -> u64 {
        self.1
    }

    const fn lease(&self) -> Option<Held> {
        self.2
    }

    const fn observed(&self) -> Duration {
        self.3
    }
}

struct Classes {
    token: Class,
    lease: Class,
    checkpoint: Class,
}

matryona_machines::registry! {
    static CLASSES: Classes = "fenced_fixture";
    /// The fixture's classes, through the accessor the macro declares.
    fn classes;
}

matryona_machines::fencing_family! {
    /// The stand-in core, the whole family.
    Core in classes {
        token: Token,
        lease: Held, owner: Owner,
        checkpoint: Saved,
    }
}

matryona_machines::fencing_family! {
    /// The stand-in core's token alone.
    TokenOnly in classes {
        token: Token,
    }
}

const fn secs(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

pub fn fencing(ruby: &Ruby) -> Result<(), Error> {
    eval(ruby, SETUP)?;
    let module = ruby.class_object().const_get("Fenced")?;
    let class = |name| Class::resolve(ruby, module, name);
    let classes = Classes {
        token: class("FencingToken")?,
        lease: class("Lease")?,
        checkpoint: class("Checkpoint")?,
    };
    CLASSES.set(ruby, classes)?;
    tokens(ruby)?;
    leases(ruby)?;
    checkpoints(ruby)
}

const SIBLING: &str = "Fenced::SiblingLease.new(4, Fenced::SiblingToken.new(2), 9_000_000_000)";
const HELD: Held = Held(Owner(4), Token(2), secs(9));

fn tokens(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let at = |code: &str| eval(ruby, code);

    // Tokens: own class, sibling, refused subclass, the core's from_raw.
    let scoped = (7_u64 << 48) | 3;
    assert_eq!(
        token::<Core>(ruby, at(&format!("Fenced::FencingToken.new({scoped})"))?)?,
        Token(scoped)
    );
    assert_eq!(
        token::<Core>(ruby, at("Fenced::SiblingToken.new(5)")?)?,
        Token(5)
    );
    raises(
        token::<Core>(ruby, at("Fenced::SubToken.new(5)")?),
        type_error,
        "expected a Fenced::FencingToken or a value with #raw",
    );
    raises(
        validate_raw::<Core>(at("7 << 48")?),
        range_error,
        "positive sequence number",
    );
    raises(
        validate_raw::<Core>(at("1.0")?),
        type_error,
        "token must be an Integer",
    );
    validate_raw::<Core>(at("1")?)?;
    // The token-only family reads the same token class.
    let five = at("Fenced::SiblingToken.new(5)")?;
    assert_eq!(token::<TokenOnly>(ruby, five)?, Token(5));
    let own = own_token::<Core>(ruby, at("Fenced::SiblingToken.new(5)")?)?;
    assert!(own.is_kind_of(CLASSES.get(ruby)?.token.get(ruby)));
    assert_eq!(
        token_object::<Core>(ruby, Token(9))?.funcall::<_, _, u64>("raw", ())?,
        9
    );

    // The token's own questions, answered by the core.
    let (one, two) = (
        at("Fenced::FencingToken.new(1)")?,
        at("Fenced::SiblingToken.new(2)")?,
    );
    let far = at(&format!("Fenced::FencingToken.new({scoped})"))?;
    assert_eq!(token_scope::<Core>(ruby, far)?, 7);
    assert_eq!(token_order::<Core>(ruby, one, two)?, Some(-1));
    assert_eq!(token_order::<Core>(ruby, two, two)?, Some(0));
    assert_eq!(token_order::<Core>(ruby, two, one)?, Some(1));
    assert_eq!(token_order::<Core>(ruby, one, far)?, None);
    assert!(token_newer::<Core>(ruby, two, one)?);
    assert!(!token_newer::<Core>(ruby, one, far)?);
    Ok(())
}

fn leases(ruby: &Ruby) -> Result<(), Error> {
    let (type_error, range_error) = (ruby.exception_type_error(), ruby.exception_range_error());
    let at = |code: &str| eval(ruby, code);
    let (held, sibling) = (HELD, SIBLING);

    // Leases: own or sibling, the token inside read the same way.
    assert_eq!(lease::<Core>(ruby, at(sibling)?)?, held);
    let built = lease_object::<Core>(ruby, held)?;
    assert!(built.is_kind_of(CLASSES.get(ruby)?.lease.get(ruby)));
    assert_eq!(lease::<Core>(ruby, built)?, held);
    let kept = own_lease::<Core>(ruby, at(sibling)?)?;
    assert!(kept.funcall::<_, _, Value>("==", (built,))?.to_bool());
    let row = RArray::try_convert(lease_row::<Core>(ruby, held))?;
    assert_eq!(row.to_vec::<u64>()?, [4, 2, 9_000_000_000]);
    let bad_owner = "Fenced::Lease.new(-1, Fenced::FencingToken.new(1), 0)";
    raises(lease::<Core>(ruby, at(bad_owner)?), range_error, "owner");
    let own_class =
        validate_lease::<Core>(ruby, at("4")?, at("Fenced::SiblingToken.new(2)")?, at("0")?)?;
    assert!(own_class.is_kind_of(CLASSES.get(ruby)?.token.get(ruby)));
    raises(
        validate_lease::<Core>(ruby, at("4")?, at("Fenced::SubToken.new(2)")?, at("0")?),
        type_error,
        "FencingToken",
    );
    Ok(())
}

fn checkpoints(ruby: &Ruby) -> Result<(), Error> {
    let range_error = ruby.exception_range_error();
    let at = |code: &str| eval(ruby, code);
    let (held, sibling) = (HELD, SIBLING);

    // Checkpoints: rebuilt through the core's from_parts.
    let saved = Saved(0, 2, Some(held), secs(1));
    let good = format!("Fenced::Checkpoint.new(0, 2, {sibling}, 1_000_000_000)");
    assert_eq!(checkpoint::<Core>(ruby, at(&good)?)?, saved);
    let empty = "Fenced::Checkpoint.new(0, 5, nil, 0)";
    assert_eq!(
        checkpoint::<Core>(ruby, at(empty)?)?,
        Saved(0, 5, None, secs(0))
    );
    let superseded = format!("Fenced::Checkpoint.new(0, 3, {sibling}, 0)");
    raises(
        checkpoint::<Core>(ruby, at(&superseded)?),
        range_error,
        "history ends with",
    );
    let row = RArray::try_convert(checkpoint_row::<Core>(ruby, &saved))?;
    assert_eq!(row.len(), 4);
    assert_eq!(row.entry::<u64>(1)?, 2);
    assert_eq!(
        RArray::try_convert(row.entry::<Value>(2)?)?.to_vec::<u64>()?,
        [4, 2, 9_000_000_000]
    );
    let none = validate_checkpoint::<Core>(ruby, at("0")?, at("5")?, None, at("0")?)?;
    assert!(none.is_none());
    let rebuilt =
        validate_checkpoint::<Core>(ruby, at("0")?, at("2")?, Some(at(sibling)?), at("0")?)?;
    let rebuilt = rebuilt.unwrap_or_else(|| panic!("a lease"));
    assert!(rebuilt.is_kind_of(CLASSES.get(ruby)?.lease.get(ruby)));
    raises(
        validate_checkpoint::<Core>(ruby, at("0")?, at("3")?, Some(at(sibling)?), at("0")?),
        range_error,
        "history ends with",
    );
    Ok(())
}
