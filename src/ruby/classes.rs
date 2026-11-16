//! Classes resolved once at init, or pinned when the binding defines them.
use core::any::type_name;
use std::format;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;

use magnus::{Error, Module, RClass, RModule, Ruby, TypedData, value::Opaque};

use super::{exact::same_class, names::named};

/// A class resolved once and pinned against `GC.compact`. `Copy`, `Send`
/// and `Sync`, so a static (a [`Registry`]) may hold it.
#[derive(Clone, Copy)]
pub struct Class(Opaque<RClass>);

impl Class {
    /// The class `scope::name`, pinned with `gc_register_mark_object`.
    /// `NameError` when the constant is missing, `TypeError` when it is not a
    /// class. Call it from init, never per call.
    ///
    /// # Errors
    ///
    /// When the constant is missing or not a class.
    pub fn resolve(ruby: &Ruby, scope: RModule, name: &str) -> Result<Self, Error> {
        let class: RClass = scope.const_get(name)?;
        Ok(Self::pin(ruby, class))
    }

    /// A class the binding defined itself (`define_class`, `define_data`),
    /// pinned with `gc_register_mark_object`, with no lookup: it already
    /// holds the class, so there is no constant to miss. Call it from init.
    #[must_use]
    pub fn pin(ruby: &Ruby, class: RClass) -> Self {
        ruby.gc_register_mark_object(class);
        Self(Opaque::from(class))
    }

    /// The class, for the thread holding the VM.
    #[must_use]
    pub fn get(self, ruby: &Ruby) -> RClass {
        ruby.get_inner(self.0)
    }

    /// [`Class::resolve`] for the class a wrapped Rust type `T`
    /// (`#[magnus::wrap]`, `#[derive(TypedData)]`) answers under, which
    /// then fixes magnus's own cached class for `T` to it.
    ///
    /// magnus resolves a wrapped type's class lazily: the first conversion
    /// of a `T` (returning it from a method, `into_value_with`,
    /// `Obj::wrap`) runs `Object.const_get("Gem::Name")`, a Ruby method,
    /// and caches whatever it answers for the life of the process, an
    /// exception being an `unwrap` panic, so an uncatchable `fatal`. Left
    /// to that, the first conversion runs Ruby after the core decided:
    /// a redefined constant makes handles of the wrong class (refused by
    /// the gem's own exact read, so stranded), a redefined `const_get`
    /// runs a caller's code mid-call, and one that raises kills the
    /// process. Called from init, this resolves `scope::name` natively
    /// first (`NameError` when missing, before magnus is asked, so its
    /// panic cannot happen for a missing constant), then forces the cache
    /// and checks it holds exactly this class. After it, converting a `T`
    /// runs no Ruby and always answers this class, whatever later
    /// redefines the constant.
    ///
    /// Covers [`TypedData::class`] (the struct-level `class = ".."`),
    /// which is all [`TypedData::class_for`] answers unless the type is
    /// an enum with per-variant `#[magnus(class = "..")]` arms; those arms
    /// hold one cache each, filled on their variant's first conversion,
    /// and are not reached from here.
    ///
    /// ```ignore
    /// #[magnus::wrap(class = "JournalMachines::Pin", free_immediately, size)]
    /// struct PinHandle { pin: Pin }
    ///
    /// // init, before any method can build a PinHandle
    /// let pin = Class::resolve_wrapped::<PinHandle>(ruby, module, "Pin")?;
    /// ```
    ///
    /// # Errors
    ///
    /// `NameError` when the constant is missing, or when magnus caches a
    /// different class for `T` (its `#[magnus::wrap]` path names another
    /// constant, or something converted a `T` before init); `TypeError`
    /// when the constant is not a class.
    pub fn resolve_wrapped<T: TypedData>(
        ruby: &Ruby,
        scope: RModule,
        name: &str,
    ) -> Result<Self, Error> {
        Self::resolve(ruby, scope, name)?.wrapping::<T>(ruby)
    }

    /// [`Class::pin`] for a class the binding defined itself to wrap `T`
    /// (`define_class` in init), fixing magnus's cached class for `T` to
    /// it the way [`Class::resolve_wrapped`] does. Define the class (and
    /// its methods) first, then call this, still in init.
    ///
    /// ```ignore
    /// let class = module.define_class("Journal", ruby.class_object())?;
    /// class.define_method("size", method!(Journal::size, 0))?;
    /// Class::pin_wrapped::<Journal>(ruby, class)?;
    /// ```
    ///
    /// # Errors
    ///
    /// `NameError` when magnus caches a different class for `T`, or its
    /// `#[magnus::wrap]` path names no constant.
    pub fn pin_wrapped<T: TypedData>(ruby: &Ruby, class: RClass) -> Result<Self, Error> {
        Self::pin(ruby, class).wrapping::<T>(ruby)
    }

    /// Forces magnus's cached class for `T` and checks it is this one.
    fn wrapping<T: TypedData>(self, ruby: &Ruby) -> Result<Self, Error> {
        let pinned = self.get(ruby);
        // magnus `unwrap`s its `const_get`; caught here, at init, that panic
        // is a `NameError` rather than a `fatal` from the loader. Its
        // `OnceLock` stays empty, and the gem does not finish loading.
        let cached = catch_unwind(AssertUnwindSafe(|| T::class(ruby)));
        let message = match cached {
            Ok(cached) if same_class(cached, pinned) => return Ok(self),
            Ok(cached) => format!(
                "{} is wrapped as {}, not the pinned {}",
                type_name::<T>(),
                named(cached),
                named(pinned)
            ),
            Err(_) => format!(
                "the #[magnus::wrap] class of {} could not be resolved; expected {}",
                type_name::<T>(),
                named(pinned)
            ),
        };
        Err(Error::new(ruby.exception_name_error(), message))
    }
}

/// A gem's resolved classes (any `T`, typically a struct of [`Class`]),
/// filled once from init and read from every call:
///
/// ```ignore
/// struct Classes { claimed: Class, entry: Class }
/// static CLASSES: Registry<Classes> = Registry::new("outbox_machines");
///
/// // init
/// CLASSES.set(ruby, Classes {
///     claimed: Class::resolve(ruby, module, "Claimed")?,
///     entry: Class::resolve(ruby, module, "Entry")?,
/// })?;
/// // a call
/// let class = CLASSES.get(ruby)?.claimed.get(ruby);
/// ```
///
/// [`registry!`](crate::registry) declares the static and the gem's
/// `classes(ruby)` accessor over it in one item.
pub struct Registry<T> {
    held: OnceLock<T>,
    gem: &'static str,
}

impl<T> Registry<T> {
    /// An empty registry for `gem`, named in the errors it raises.
    #[must_use]
    pub const fn new(gem: &'static str) -> Self {
        Self {
            held: OnceLock::new(),
            gem,
        }
    }

    /// Stores the classes. `RuntimeError` when they were already stored: the
    /// extension was initialised twice, a misuse of the loader rather than a
    /// missing constant (the `CONVENTIONS.md` carve-out).
    ///
    /// # Errors
    ///
    /// When called a second time.
    pub fn set(&self, ruby: &Ruby, classes: T) -> Result<&T, Error> {
        if self.held.set(classes).is_err() {
            let message = format!("the {} extension was initialised twice", self.gem);
            return Err(Error::new(ruby.exception_runtime_error(), message));
        }
        self.get(ruby)
    }

    /// The classes stored at init. `NameError` before then: the extension
    /// was loaded without the gem that defines them, so the constants it
    /// names are missing, the fleet's load-time missing-constant rule.
    ///
    /// # Errors
    ///
    /// When [`Registry::set`] has not run.
    pub fn get(&self, ruby: &Ruby) -> Result<&T, Error> {
        self.held.get().ok_or_else(|| {
            let message = format!(
                "the {gem} extension is not initialised; load {gem}, not the extension alone",
                gem = self.gem
            );
            Error::new(ruby.exception_name_error(), message)
        })
    }
}

/// What [`registry!`](crate::registry) expands to names, so a gem's binding
/// needs no import for it.
#[doc(hidden)]
pub mod __private {
    pub use core::result::Result;
    pub use magnus::{Error, Ruby};
}

/// Declares a gem's [`Registry`] static and its class accessor, a function
/// of `&Ruby` answering `Result<&'static T, Error>` through
/// [`Registry::get`]: the classes, or `NameError` before init.
///
/// ```
/// use magnus::{Error, RClass, RModule, Ruby};
/// use matryona_machines::ruby::Class;
///
/// pub struct Classes { pub claimed: Class }
///
/// matryona_machines::registry! {
///     static CLASSES: Classes = "outbox_machines";
///     /// The classes resolved at init.
///     pub fn classes;
/// }
///
/// fn init(ruby: &Ruby, module: RModule) -> Result<(), Error> {
///     let claimed = Class::resolve(ruby, module, "Claimed")?;
///     CLASSES.set(ruby, Classes { claimed }).map(drop)
/// }
///
/// fn claimed(ruby: &Ruby) -> Result<RClass, Error> {
///     Ok(classes(ruby)?.claimed.get(ruby))
/// }
/// ```
///
/// expands to
///
/// ```ignore
/// static CLASSES: Registry<Classes> = Registry::new("outbox_machines");
/// /// The classes resolved at init.
/// pub fn classes(ruby: &Ruby) -> Result<&'static Classes, Error> {
///     CLASSES.get(ruby)
/// }
/// ```
///
/// Attributes (doc comments included) are written before the `static` and
/// before the `fn`, and each takes its own visibility. The accessor's name
/// is the caller's, so it also serves as the `$classes` path of
/// [`fencing_family!`](crate::fencing_family).
#[macro_export]
macro_rules! registry {
    (
        $(#[$static_meta:meta])*
        $static_vis:vis static $registry:ident: $classes:ty = $gem:literal;
        $(#[$fn_meta:meta])*
        $fn_vis:vis fn $accessor:ident $(;)?
    ) => {
        $(#[$static_meta])*
        $static_vis static $registry: $crate::ruby::Registry<$classes> =
            $crate::ruby::Registry::new($gem);

        $(#[$fn_meta])*
        $fn_vis fn $accessor(
            ruby: &$crate::ruby::__classes::Ruby,
        ) -> $crate::ruby::__classes::Result<&'static $classes, $crate::ruby::__classes::Error> {
            $registry.get(ruby)
        }
    };
}
