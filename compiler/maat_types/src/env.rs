//! Type environment for tracking variable bindings during type inference.

use std::collections::HashSet;

use indexmap::IndexMap;

use crate::ty::{FnType, Type, TypeScheme, TypeVarId};
use crate::unify::Substitution;

/// Lexically scoped type environment.
///
/// Maintains a stack of scopes, each mapping variable names to their type
/// schemes. The innermost scope is checked first during lookups, supporting
/// shadowing. Stores `TypeScheme` rather than raw `Type` to enable
/// let-polymorphism: generalized bindings are instantiated with fresh
/// variables at each use site.
pub struct TypeEnv {
    scopes: Vec<IndexMap<String, TypeScheme>>,
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

    /// Registers builtin function signatures in the type environment.
    ///
    /// Each builtin with type variables is stored as a generalized `TypeScheme`
    /// so that each call site receives fresh inference variables.
    ///
    /// `print` is variadic at runtime and is not registered
    /// here. Unknown identifiers fall back to fresh type variables, which
    /// allows any number of arguments without arity errors.
    pub fn register_builtins(&mut self) {
        // len(collection) -> usize
        self.register_builtin("len", |t| {
            Type::Function(FnType {
                params: vec![t],
                ret: Box::new(Type::Usize),
            })
        });

        // first([T]) -> T
        self.register_builtin("first", |t| {
            Type::Function(FnType {
                params: vec![Type::Array(Box::new(t.clone()))],
                ret: Box::new(t),
            })
        });

        // last([T]) -> T
        self.register_builtin("last", |t| {
            Type::Function(FnType {
                params: vec![Type::Array(Box::new(t.clone()))],
                ret: Box::new(t),
            })
        });

        // rest([T]) -> [T]
        self.register_builtin("rest", |t| {
            Type::Function(FnType {
                params: vec![Type::Array(Box::new(t.clone()))],
                ret: Box::new(Type::Array(Box::new(t))),
            })
        });

        // push([T], T) -> [T]
        self.register_builtin("push", |t| {
            Type::Function(FnType {
                params: vec![Type::Array(Box::new(t.clone())), t.clone()],
                ret: Box::new(Type::Array(Box::new(t))),
            })
        });
    }

    fn register_builtin(&mut self, name: &str, build: impl FnOnce(Type) -> Type) {
        let var = self.fresh_var();
        let var_id = match var {
            Type::Var(id) => id,
            _ => unreachable!(),
        };
        let ty = build(var);
        self.define_scheme(
            name,
            TypeScheme {
                forall: vec![var_id],
                ty,
            },
        );
    }

    /// Generates a fresh type variable.
    pub fn fresh_var(&mut self) -> Type {
        let id = self.next_var;
        self.next_var += 1;
        Type::Var(id)
    }

    /// Defines a variable with a monomorphic type in the current scope.
    pub fn define_var(&mut self, name: &str, ty: Type) {
        self.define_scheme(name, TypeScheme::monomorphic(ty));
    }

    /// Defines a variable with a polymorphic type scheme in the current scope.
    pub fn define_scheme(&mut self, name: &str, scheme: TypeScheme) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), scheme);
        }
    }

    /// Looks up a variable's type scheme, searching from innermost to outermost scope.
    pub fn lookup_scheme(&self, name: &str) -> Option<&TypeScheme> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    /// Looks up a variable and instantiates its scheme with fresh type variables.
    ///
    /// This is the standard lookup used during expression inference: each use
    /// of a let-bound identifier gets a fresh copy of its polymorphic type.
    pub fn instantiate(&mut self, name: &str, subst: &Substitution) -> Option<Type> {
        let scheme = self.lookup_scheme(name)?.clone();
        Some(self.instantiate_scheme(&scheme, subst))
    }

    /// Instantiates a type scheme by replacing each quantified variable with
    /// a fresh type variable.
    pub fn instantiate_scheme(&mut self, scheme: &TypeScheme, subst: &Substitution) -> Type {
        if scheme.forall.is_empty() {
            return subst.apply(&scheme.ty);
        }

        let mut local_subst = subst.clone();
        for &var in &scheme.forall {
            let fresh = self.fresh_var();
            local_subst.bind(var, fresh);
        }
        local_subst.apply(&scheme.ty)
    }

    /// Generalizes a type into a `TypeScheme` by quantifying over free type
    /// variables that do not appear free in the environment.
    pub fn generalize(&self, ty: &Type, subst: &Substitution) -> TypeScheme {
        let resolved = subst.apply(ty);
        let ty_vars = free_type_vars(&resolved);
        let env_vars = self.free_env_vars(subst);
        let forall = ty_vars
            .difference(&env_vars)
            .copied()
            .collect::<Vec<TypeVarId>>();

        TypeScheme {
            forall,
            ty: resolved,
        }
    }

    /// Collects all free type variables across all scopes in the environment.
    fn free_env_vars(&self, subst: &Substitution) -> HashSet<TypeVarId> {
        let mut vars = HashSet::new();
        for scope in &self.scopes {
            for scheme in scope.values() {
                let resolved = subst.apply(&scheme.ty);
                let scheme_free = free_type_vars(&resolved);
                let quantified = scheme
                    .forall
                    .iter()
                    .copied()
                    .collect::<HashSet<TypeVarId>>();
                vars.extend(scheme_free.difference(&quantified));
            }
        }
        vars
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

/// Collects all free type variables in a type.
pub fn free_type_vars(ty: &Type) -> HashSet<TypeVarId> {
    let mut vars = HashSet::new();
    collect_free_vars(ty, &mut vars);
    vars
}

fn collect_free_vars(ty: &Type, vars: &mut HashSet<TypeVarId>) {
    match ty {
        Type::Var(id) => {
            vars.insert(*id);
        }
        Type::Array(elem) => collect_free_vars(elem, vars),
        Type::Hash(k, v) => {
            collect_free_vars(k, vars);
            collect_free_vars(v, vars);
        }
        Type::Function(FnType { params, ret }) => {
            for p in params {
                collect_free_vars(p, vars);
            }
            collect_free_vars(ret, vars);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn define_and_lookup_var() {
        let mut env = TypeEnv::new();
        env.define_var("x", Type::I64);
        let subst = Substitution::new();
        let ty = env.instantiate("x", &subst);
        assert_eq!(ty, Some(Type::I64));
        assert_eq!(env.instantiate("y", &subst), None);
    }

    #[test]
    fn scope_shadowing() {
        let mut env = TypeEnv::new();
        let subst = Substitution::new();
        env.define_var("x", Type::I64);
        env.push_scope();
        env.define_var("x", Type::Bool);
        assert_eq!(env.instantiate("x", &subst), Some(Type::Bool));
        env.pop_scope();
        assert_eq!(env.instantiate("x", &subst), Some(Type::I64));
    }

    #[test]
    fn fresh_variables() {
        let mut env = TypeEnv::new();
        assert_eq!(env.fresh_var(), Type::Var(0));
        assert_eq!(env.fresh_var(), Type::Var(1));
        assert_eq!(env.fresh_var(), Type::Var(2));
    }

    #[test]
    fn generalize_and_instantiate() {
        let mut env = TypeEnv::new();
        let subst = Substitution::new();

        // Type: fn(?T0) -> ?T0  (identity function)
        let fn_ty = Type::Function(FnType {
            params: vec![Type::Var(0)],
            ret: Box::new(Type::Var(0)),
        });
        env.next_var = 1;

        let scheme = env.generalize(&fn_ty, &subst);
        assert_eq!(scheme.forall, vec![0]);

        let inst1 = env.instantiate_scheme(&scheme, &subst);
        let inst2 = env.instantiate_scheme(&scheme, &subst);

        match (&inst1, &inst2) {
            (Type::Function(f1), Type::Function(f2)) => {
                assert_ne!(f1.params[0], f2.params[0]);
            }
            _ => panic!("expected function types"),
        }
    }

    #[test]
    fn monomorphic_not_generalized() {
        let mut env = TypeEnv::new();
        let subst = Substitution::new();

        // Define x: ?T0 in the environment (simulates a lambda parameter)
        env.define_var("x", Type::Var(0));
        env.next_var = 1;

        // ?T0 is free in the env, so generalize should NOT quantify it
        let scheme = env.generalize(&Type::Var(0), &subst);
        assert!(scheme.forall.is_empty());
    }
}
