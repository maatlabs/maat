use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::Object;

/// Represents the execution environment of the interpreter.
#[derive(Debug, Clone, Default)]
pub struct Env {
    inner: Rc<RefCell<EnvInner>>,
}

#[derive(Debug, Default)]
struct EnvInner {
    store: HashMap<String, Object>,
    outer: Option<Env>,
}

impl Env {
    /// Creates an enclosed environment for use within function calls.
    ///
    /// The enclosed environment can access bindings from the outer environment
    /// while maintaining its own local bindings.
    pub fn new_enclosed(outer: &Self) -> Self {
        Self {
            inner: Rc::new(RefCell::new(EnvInner {
                store: HashMap::new(),
                outer: Some(outer.clone()),
            })),
        }
    }

    /// Returns a clone of the object associated with a `name` if found.
    pub fn get(&self, name: &str) -> Option<Object> {
        let inner = self.inner.borrow();
        match inner.store.get(name) {
            Some(obj) => Some(obj.clone()),
            None => inner.outer.as_ref().and_then(|outer| outer.get(name)),
        }
    }

    /// Binds the `object` in the environment with the `name`.
    pub fn set(&self, name: String, object: &Object) {
        self.inner.borrow_mut().store.insert(name, object.clone());
    }
}

impl PartialEq for Env {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}
