//! Type unification and substitution for Hindley-Milner inference.

use indexmap::IndexMap;

use crate::ty::{FnType, Type, TypeVarId};

/// Unification error details.
#[derive(Debug, Clone)]
pub enum UnifyError {
    /// These two types cannot be unified.
    Mismatch(Type, Type),
    /// A type variable occurs in the type it would be bound to (infinite type).
    OccursCheck(TypeVarId, Type),
}

/// A mapping from type variables to their resolved types.
#[derive(Debug, Clone, Default)]
pub struct Substitution {
    map: IndexMap<TypeVarId, Type>,
}

impl Substitution {
    /// Creates an empty substitution.
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a type variable to a type.
    pub fn bind(&mut self, var: TypeVarId, ty: Type) {
        self.map.insert(var, ty);
    }

    /// Binds a type variable to a type, performing the `occurs` check.
    fn bind_var(&mut self, var: TypeVarId, ty: &Type) -> Result<(), UnifyError> {
        if self.occurs(var, ty) {
            return Err(UnifyError::OccursCheck(var, ty.clone()));
        }
        self.bind(var, ty.clone());
        Ok(())
    }

    /// Returns `true` if the type variable `var` occurs anywhere in `ty`.
    fn occurs(&self, var: TypeVarId, ty: &Type) -> bool {
        match ty {
            Type::Var(id) => *id == var,
            Type::Array(elem) => self.occurs(var, elem),
            Type::Hash(k, v) => self.occurs(var, k) || self.occurs(var, v),
            Type::Function(fn_ty) => {
                fn_ty.params.iter().any(|p| self.occurs(var, p)) || self.occurs(var, &fn_ty.ret)
            }
            _ => false,
        }
    }

    /// Unifies two types, producing a substitution that makes them equal.
    ///
    /// Implements [Robinson's unification algorithm] with an `occurs` check
    /// to prevent infinite types.
    ///
    /// [Robinson's unification algorithm]: https://en.wikipedia.org/wiki/Unification_(computer_science)#Unification_algorithms
    pub fn unify(&mut self, a: &Type, b: &Type) -> Result<(), UnifyError> {
        let a = self.apply(a);
        let b = self.apply(b);

        match (&a, &b) {
            _ if a == b => Ok(()),

            (Type::Never, _) | (_, Type::Never) => Ok(()),

            (Type::Var(id), _) => self.bind_var(*id, &b),
            (_, Type::Var(id)) => self.bind_var(*id, &a),

            (Type::Array(ea), Type::Array(eb)) => self.unify(ea, eb),

            (Type::Hash(ka, va), Type::Hash(kb, vb)) => {
                self.unify(ka, kb)?;
                self.unify(va, vb)
            }

            (Type::Function(fa), Type::Function(fb)) => {
                if fa.params.len() != fb.params.len() {
                    return Err(UnifyError::Mismatch(a, b));
                }
                for (pa, pb) in fa.params.iter().zip(fb.params.iter()) {
                    self.unify(pa, pb)?;
                }
                self.unify(&fa.ret, &fb.ret)
            }

            _ => Err(UnifyError::Mismatch(a, b)),
        }
    }

    /// Applies this substitution to a type, recursively resolving all type variables.
    pub fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(id) => match self.get(id) {
                Some(resolved) => self.apply(resolved),
                None => ty.clone(),
            },
            Type::Array(elem) => Type::Array(Box::new(self.apply(elem))),
            Type::Hash(k, v) => Type::Hash(Box::new(self.apply(k)), Box::new(self.apply(v))),
            Type::Function(fn_ty) => Type::Function(FnType {
                params: fn_ty.params.iter().map(|p| self.apply(p)).collect(),
                ret: Box::new(self.apply(&fn_ty.ret)),
            }),
            _ => ty.clone(),
        }
    }

    /// Looks up the binding for a type variable.
    pub fn get(&self, var: &TypeVarId) -> Option<&Type> {
        self.map.get(var)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unify_identical() {
        let mut subst = Substitution::new();
        assert!(subst.unify(&Type::I64, &Type::I64).is_ok());
    }

    #[test]
    fn unify_var_to_concrete() {
        let mut subst = Substitution::new();
        assert!(subst.unify(&Type::Var(0), &Type::I64).is_ok());
        assert_eq!(subst.apply(&Type::Var(0)), Type::I64);
    }

    #[test]
    fn unify_mismatch() {
        let mut subst = Substitution::new();
        assert!(subst.unify(&Type::I64, &Type::Bool).is_err());
    }

    #[test]
    fn unify_occurs_check() {
        let mut subst = Substitution::new();
        let result = subst.unify(&Type::Var(0), &Type::Array(Box::new(Type::Var(0))));
        assert!(matches!(result, Err(UnifyError::OccursCheck(_, _))));
    }

    #[test]
    fn unify_function_types() {
        let mut subst = Substitution::new();
        let f1 = Type::Function(FnType {
            params: vec![Type::Var(0)],
            ret: Box::new(Type::Var(1)),
        });
        let f2 = Type::Function(FnType {
            params: vec![Type::I64],
            ret: Box::new(Type::Bool),
        });
        assert!(subst.unify(&f1, &f2).is_ok());
        assert_eq!(subst.apply(&Type::Var(0)), Type::I64);
        assert_eq!(subst.apply(&Type::Var(1)), Type::Bool);
    }

    #[test]
    fn never_unifies_with_anything() {
        let mut subst = Substitution::new();
        assert!(subst.unify(&Type::Never, &Type::I64).is_ok());
        assert!(subst.unify(&Type::Bool, &Type::Never).is_ok());
    }
}
