use std::collections::HashMap;

use super::Object;

/// Represents the execution environment of the interpreter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Env {
    store: HashMap<String, Object>,
    outer: Option<Box<Env>>,
}

impl Env {
    /// Creates an enclosed environment for use within
    /// function calls.
    pub fn new_enclosed(outer: &Self) -> Self {
        Self {
            outer: Some(Box::new(outer.clone())),
            ..Default::default()
        }
    }

    /// Returns the object associated with a `name` if found,
    /// or None, otherwise.
    pub fn get(&self, name: &str) -> Option<&Object> {
        match (self.store.get(name), &self.outer) {
            // binding found in inner env,
            // return object.
            (Some(obj), _) => Some(obj),
            // binding not found in inner env,
            // try the outer env.
            (None, Some(outer)) => outer.get(name),
            // no binding found in inner env, and
            // no outer env.
            (None, _) => None,
        }
    }

    /// Binds the `object` in the environment with the `name`.
    pub fn set(&mut self, name: String, object: &Object) {
        self.store.insert(name, object.clone());
    }
}
