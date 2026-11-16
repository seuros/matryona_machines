//! Class names for refusal messages.
//!
//! `value.class()` is the object's `klass`: its singleton class once one
//! was opened, which formats as `#<Class:#<Object:0x...>>`, and its
//! `Display` dispatches `to_s`, which a class may redefine. magnus's own
//! native name readers (`RClass::name`, `Value::classname`) are `unsafe`.
//! [`class_name`] asks Ruby's own `Kernel#class` (which skips singleton
//! classes) and `Module#to_s`, bound to the value rather than dispatched
//! on it, so a value redefining `class` or a class redefining `to_s`
//! cannot rename itself in the message.
use std::{format, string::String};

use magnus::{Error, RClass, Ruby, Value, value::ReprValue};

/// The name of `value`'s class, for a refusal message: the real class,
/// never a singleton class.
///
/// A value with an opened singleton class names
/// its class (`"Fixture::Point"`), a subclass instance its subclass, an
/// anonymous class Ruby's `#<Class:0x...>`.
///
/// It runs `Kernel#class` and `Module#to_s` through `bind_call`, which is
/// Ruby, so call it where refusal messages are built: in a [`Message`] or
/// [`Explain`] closure, which runs only on refusal, before any core call.
///
/// ```ignore
/// let handle = exact_typed_with::<Handle>(ruby, value, class, || {
///     format!("handle must be a PoolMachines::Handle, not {}", class_name(value))
/// })?;
/// ```
///
/// [`Message`]: super::Message
/// [`Explain`]: super::Explain
#[must_use]
pub fn class_name(value: impl ReprValue) -> String {
    let value = value.as_value();
    real_class_name(value).unwrap_or_else(|_| value.class().inspect())
}

/// `Kernel#class` bound to `value`, then `Module#to_s` bound to that.
fn real_class_name(value: Value) -> Result<String, Error> {
    let ruby = Ruby::get_with(value);
    let class: Value = ruby
        .module_kernel()
        .funcall::<_, _, Value>("instance_method", (ruby.to_symbol("class"),))?
        .funcall("bind_call", (value,))?;
    module_name(&ruby, class)
}

/// `Module#to_s` bound to `class`, falling back to its `inspect`: the
/// class's own name for a message, built on refusal only.
pub(super) fn named(class: RClass) -> String {
    let ruby = Ruby::get_with(class);
    module_name(&ruby, class.as_value()).unwrap_or_else(|_| class.inspect())
}

/// `Module#to_s` bound to `module`: its name, or `#<Class:0x...>`.
fn module_name(ruby: &Ruby, module: Value) -> Result<String, Error> {
    ruby.class_module()
        .funcall::<_, _, Value>("instance_method", (ruby.to_symbol("to_s"),))?
        .funcall("bind_call", (module,))
}

/// `"an"` before a name that starts with a vowel letter, `"a"` before
/// anything else.
///
/// The letters are `A`, `E`, `I`, `O`, `U`, either case: by spelling, which
/// suits `CamelCase` class names (`"an ElectionMachines::Lease"`, `"a
/// LeaseMachines::Lease"`). A name read aloud otherwise (`"a UTF8Error"`)
/// needs its own message.
#[must_use]
pub fn article(name: &str) -> &'static str {
    match name.bytes().next().map(|first| first.to_ascii_lowercase()) {
        Some(b'a' | b'e' | b'i' | b'o' | b'u') => "an",
        _ => "a",
    }
}

/// The refusal message
/// [`own_or_sibling_default`](super::own_or_sibling_default) raises.
///
/// `"expected a LeaseMachines::Lease or a value with #owner,
/// #token, #expires_at"`, the article picked by [`article`]. Ruby's
/// `Module#to_s` names `own`, bound, so call it on refusal.
#[must_use]
pub fn sibling_message(own: RClass, readers: &[&str]) -> String {
    let name = named(own);
    let article = article(&name);
    if readers.is_empty() {
        return format!("expected {article} {name}");
    }
    format!(
        "expected {article} {name} or a value with #{}",
        readers.join(", #")
    )
}
