//! Classes resolved once at init, or pinned when the binding defines them.
use std::format;
use std::sync::OnceLock;

use magnus::{Error, Module, RClass, RModule, Ruby, value::Opaque};

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
    /// extension was initialised twice.
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

    /// The classes stored at init. `RuntimeError` before then: the extension
    /// was loaded without the gem that defines them.
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
            Error::new(ruby.exception_runtime_error(), message)
        })
    }
}
