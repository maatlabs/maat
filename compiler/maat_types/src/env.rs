//! Type environment for tracking variable bindings during type inference.

use indexmap::IndexMap;

use crate::ty::{Type, TypeVarId};

/// Lexically scoped type environment.
///
/// Maintains a stack of scopes, each mapping variable names to their types.
/// The innermost scope is checked first during lookups, supporting shadowing.
pub struct TypeEnv {
    scopes: Vec<IndexMap<String, Type>>,
    next_var: TypeVarId,
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeEnv {
    /// Creates a new type environment with a single empty scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![IndexMap::new()],
            next_var: 0,
        }
    }

    /// Generates a fresh type variable.
    pub fn fresh_var(&mut self) -> Type {
        let id = self.next_var;
        self.next_var += 1;
        Type::Var(id)
    }

    /// Defines a variable in the current (innermost) scope.
    pub fn define_var(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    /// Looks up a variable, searching from innermost to outermost scope.
    pub fn lookup_var(&self, name: &str) -> Option<&Type> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    /// Pushes a new empty scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(IndexMap::new());
    }

    /// Pops the innermost scope.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn define_and_lookup_var() {
        let mut env = TypeEnv::new();
        env.define_var("x", Type::I64);
        assert_eq!(env.lookup_var("x"), Some(&Type::I64));
        assert_eq!(env.lookup_var("y"), None);
    }

    #[test]
    fn scope_shadowing() {
        let mut env = TypeEnv::new();
        env.define_var("x", Type::I64);
        env.push_scope();
        env.define_var("x", Type::Bool);
        assert_eq!(env.lookup_var("x"), Some(&Type::Bool));
        env.pop_scope();
        assert_eq!(env.lookup_var("x"), Some(&Type::I64));
    }

    #[test]
    fn fresh_variables() {
        let mut env = TypeEnv::new();
        assert_eq!(env.fresh_var(), Type::Var(0));
        assert_eq!(env.fresh_var(), Type::Var(1));
        assert_eq!(env.fresh_var(), Type::Var(2));
    }
}
