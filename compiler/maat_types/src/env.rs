//! Type environment for tracking variable bindings during type inference.

use std::collections::HashSet;

use indexmap::IndexMap;
use maat_ast::{NamedType, TypeExpr};

use crate::convert::resolve_type_expr;
use crate::ty::{
    EnumDef, FnType, ImplDef, MethodSig, StructDef, TraitDef, Type, TypeScheme, TypeVarId,
    VariantDef, VariantKind,
};
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
    structs: IndexMap<String, StructDef>,
    enums: IndexMap<String, EnumDef>,
    traits: IndexMap<String, TraitDef>,
    impls: Vec<ImplDef>,
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
            structs: IndexMap::new(),
            enums: IndexMap::new(),
            traits: IndexMap::new(),
            impls: Vec::new(),
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

        self.register_builtin_enums();
    }

    /// Registers `Option<T>` and `Result<T, E>` as language-level enum types.
    fn register_builtin_enums(&mut self) {
        self.register_enum(EnumDef {
            name: "Option".to_string(),
            generic_params: vec!["T".to_string()],
            variants: vec![
                VariantDef {
                    name: "Some".to_string(),
                    kind: VariantKind::Tuple(vec![Type::Generic("T".to_string(), vec![])]),
                },
                VariantDef {
                    name: "None".to_string(),
                    kind: VariantKind::Unit,
                },
            ],
        });

        self.register_enum(EnumDef {
            name: "Result".to_string(),
            generic_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                VariantDef {
                    name: "Ok".to_string(),
                    kind: VariantKind::Tuple(vec![Type::Generic("T".to_string(), vec![])]),
                },
                VariantDef {
                    name: "Err".to_string(),
                    kind: VariantKind::Tuple(vec![Type::Generic("E".to_string(), vec![])]),
                },
            ],
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

    /// Registers a struct definition.
    pub fn register_struct(&mut self, def: StructDef) {
        self.structs.insert(def.name.clone(), def);
    }

    /// Registers an enum definition.
    pub fn register_enum(&mut self, def: EnumDef) {
        self.enums.insert(def.name.clone(), def);
    }

    /// Registers a trait definition.
    pub fn register_trait(&mut self, def: TraitDef) {
        self.traits.insert(def.name.clone(), def);
    }

    /// Registers an impl block.
    pub fn register_impl(&mut self, def: ImplDef) {
        self.impls.push(def);
    }

    /// Looks up a registered struct definition by name.
    pub fn lookup_struct(&self, name: &str) -> Option<&StructDef> {
        self.structs.get(name)
    }

    /// Looks up a registered enum definition by name.
    pub fn lookup_enum(&self, name: &str) -> Option<&EnumDef> {
        self.enums.get(name)
    }

    /// Looks up a registered trait definition by name.
    pub fn lookup_trait(&self, name: &str) -> Option<&TraitDef> {
        self.traits.get(name)
    }

    /// Looks up a method on a concrete type by searching inherent impl blocks first,
    /// then trait impl blocks.
    ///
    /// Returns the function type of the method if found.
    pub fn lookup_method(&self, self_type: &Type, method_name: &str) -> Option<&Type> {
        // Inherent impls first
        self.impls
            .iter()
            .filter(|imp| imp.trait_name.is_none() && imp.self_type == *self_type)
            .find_map(|imp| {
                imp.methods
                    .iter()
                    .find(|(name, _)| name == method_name)
                    .map(|(_, ty)| ty)
            })
            .or_else(|| {
                // Trait impls
                self.impls
                    .iter()
                    .filter(|imp| imp.trait_name.is_some() && imp.self_type == *self_type)
                    .find_map(|imp| {
                        imp.methods
                            .iter()
                            .find(|(name, _)| name == method_name)
                            .map(|(_, ty)| ty)
                    })
            })
    }

    /// Looks up a method on a type by name only (ignoring type arguments).
    ///
    /// Used for generic struct/enum types where the type arguments may not
    /// exactly match the registered impl (e.g., `Point<i64>` vs `Point`).
    pub fn lookup_method_by_name(&self, type_name: &str, method_name: &str) -> Option<&Type> {
        self.impls
            .iter()
            .filter(|imp| match &imp.self_type {
                Type::Struct(n, _) | Type::Enum(n, _) => n == type_name,
                _ => false,
            })
            .find_map(|imp| {
                imp.methods
                    .iter()
                    .find(|(name, _)| name == method_name)
                    .map(|(_, ty)| ty)
            })
    }

    /// Returns an iterator over all registered enum definitions.
    pub fn all_enums(&self) -> impl Iterator<Item = &EnumDef> {
        self.enums.values()
    }

    /// Returns an iterator over all registered impl blocks.
    pub fn all_impls(&self) -> impl Iterator<Item = &ImplDef> {
        self.impls.iter()
    }

    /// Returns all required method signatures for a trait.
    pub fn required_trait_methods(&self, trait_name: &str) -> Vec<&MethodSig> {
        self.traits
            .get(trait_name)
            .map(|t| t.methods.iter().filter(|m| !m.has_default).collect())
            .unwrap_or_default()
    }

    /// Resolves a parsed type expression into an internal type, using the
    /// type registry to recognize user-defined struct and enum names.
    ///
    /// Falls back to [`resolve_type_expr`] for primitive and compound types.
    pub fn resolve_type(&self, expr: &TypeExpr) -> Type {
        match expr {
            TypeExpr::Named(named) => self.resolve_named_type(&named.name),
            TypeExpr::Generic(name, args, _) => {
                let resolved_args = args
                    .iter()
                    .map(|a| self.resolve_type(a))
                    .collect::<Vec<Type>>();
                if self.structs.contains_key(name) {
                    Type::Struct(name.clone(), resolved_args)
                } else if self.enums.contains_key(name) {
                    Type::Enum(name.clone(), resolved_args)
                } else {
                    Type::Generic(name.clone(), vec![])
                }
            }
            _ => resolve_type_expr(expr),
        }
    }

    /// Resolves a named type string, checking the registry before falling
    /// back to primitives.
    fn resolve_named_type(&self, name: &str) -> Type {
        if self.structs.contains_key(name) {
            Type::Struct(name.to_string(), vec![])
        } else if self.enums.contains_key(name) {
            Type::Enum(name.to_string(), vec![])
        } else {
            // Delegate to the standalone resolver for primitives and generics.
            resolve_type_expr(&TypeExpr::Named(NamedType {
                name: name.to_string(),
                span: maat_span::Span::ZERO,
            }))
        }
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
        Type::Struct(_, args) | Type::Enum(_, args) => {
            for a in args {
                collect_free_vars(a, vars);
            }
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
