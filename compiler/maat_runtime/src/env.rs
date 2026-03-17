use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;

use crate::Object;

/// Represents the execution environment of the interpreter.
#[derive(Debug, Clone, Default)]
pub struct Env {
    inner: Rc<RefCell<EnvInner>>,
}

#[derive(Debug, Default)]
struct EnvInner {
    store: IndexMap<String, Object>,
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

    /// Updates an existing binding in the nearest enclosing scope that contains it.
    ///
    /// Walks the scope chain from the current environment outward. If the binding
    /// is found, it is updated in-place. If no enclosing scope contains the name,
    /// the binding is created in the current scope as a fallback.
    pub fn update(&self, name: String, object: &Object) {
        if self.inner.borrow().store.contains_key(&name) {
            self.inner.borrow_mut().store.insert(name, object.clone());
            return;
        }
        if let Some(outer) = self.inner.borrow().outer.as_ref().cloned()
            && outer.contains(&name)
        {
            outer.update(name, object);
            return;
        }
        self.inner.borrow_mut().store.insert(name, object.clone());
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
