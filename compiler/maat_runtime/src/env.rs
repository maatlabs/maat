use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;

use crate::Value;

/// Represents the execution environment of the interpreter.
#[derive(Debug, Clone, Default)]
pub struct Env {
    inner: Rc<RefCell<EnvInner>>,
}

#[derive(Debug, Default)]
struct EnvInner {
    store: IndexMap<String, Value>,
    outer: Option<Env>,
}

impl Env {
    /// Creates an enclosed environment for use within function calls and macros.
    ///
    /// The enclosed environment can access bindings from the outer environment
    /// while maintaining its own local bindings.
    pub fn new_enclosed(outer: &Self) -> Self {
        Self {
            inner: Rc::new(RefCell::new(EnvInner {
                store: IndexMap::new(),
                outer: Some(outer.clone()),
            })),
        }
    }

    /// Returns a clone of the value associated with a `name` if found.
    pub fn get(&self, name: &str) -> Option<Value> {
        let inner = self.inner.borrow();
        match inner.store.get(name) {
            Some(val) => Some(val.clone()),
            None => inner.outer.as_ref().and_then(|outer| outer.get(name)),
        }
    }

    /// Binds the `value` in the environment with the `name`.
    pub fn set(&self, name: String, value: &Value) {
        self.inner.borrow_mut().store.insert(name, value.clone());
    }

    /// Updates an existing binding in the nearest enclosing scope that contains it.
    ///
    /// Walks the scope chain from the current environment outward. If the binding
    /// is found, it is updated in-place. If no enclosing scope contains the name,
    /// the binding is created in the current scope as a fallback.
    pub fn update(&self, name: String, value: &Value) {
        if self.inner.borrow().store.contains_key(&name) {
            self.inner.borrow_mut().store.insert(name, value.clone());
            return;
        }
        if let Some(outer) = self.inner.borrow().outer.as_ref().cloned()
            && outer.contains(&name)
        {
            outer.update(name, value);
            return;
        }
        self.inner.borrow_mut().store.insert(name, value.clone());
    }

    /// Returns `true` if `name` is defined in this scope or any enclosing scope.
    fn contains(&self, name: &str) -> bool {
        let inner = self.inner.borrow();
        inner.store.contains_key(name)
            || inner
                .outer
                .as_ref()
                .is_some_and(|outer| outer.contains(name))
    }
}

impl PartialEq for Env {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}
