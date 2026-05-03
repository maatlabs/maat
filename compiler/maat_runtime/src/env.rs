use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;

use crate::Value;

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
    pub fn new_enclosed(outer: &Self) -> Self {
        Self {
            inner: Rc::new(RefCell::new(EnvInner {
                store: IndexMap::new(),
                outer: Some(outer.clone()),
            })),
        }
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        let inner = self.inner.borrow();
        match inner.store.get(name) {
            Some(val) => Some(val.clone()),
            None => inner.outer.as_ref().and_then(|outer| outer.get(name)),
        }
    }

    pub fn set(&self, name: String, value: &Value) {
        self.inner.borrow_mut().store.insert(name, value.clone());
    }

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
