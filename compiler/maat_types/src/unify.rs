//! Type unification and substitution for Hindley-Milner inference.

use indexmap::IndexMap;

use crate::{FnType, Type, TypeVarId};

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
            Type::Vector(elem) | Type::Set(elem) | Type::Range(elem) => self.occurs(var, elem),
            Type::Map(k, v) => self.occurs(var, k) || self.occurs(var, v),
            Type::Function(fn_ty) => {
                fn_ty.params.iter().any(|p| self.occurs(var, p)) || self.occurs(var, &fn_ty.ret)
            }
            Type::Struct(_, args) | Type::Enum(_, args) => args.iter().any(|a| self.occurs(var, a)),
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
            (Type::Vector(ea), Type::Vector(eb)) => self.unify(ea, eb),
            (Type::Set(ea), Type::Set(eb)) => self.unify(ea, eb),
            (Type::Range(ea), Type::Range(eb)) => self.unify(ea, eb),
            (Type::Map(ka, va), Type::Map(kb, vb)) => {
                self.unify(ka, kb)?;
                self.unify(va, vb)
            }
            (Type::Struct(na, args_a), Type::Struct(nb, args_b))
            | (Type::Enum(na, args_a), Type::Enum(nb, args_b)) => {
                if na != nb || args_a.len() != args_b.len() {
                    return Err(UnifyError::Mismatch(a, b));
                }
                for (pa, pb) in args_a.iter().zip(args_b.iter()) {
                    self.unify(pa, pb)?;
                }
                Ok(())
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
    ///
    /// # Stack depth
    ///
    /// The `Type::Var` branch recurses through chained variable bindings
    /// (`?0 -> ?1 -> ?2 -> ... -> T`). In well-formed substitutions produced by
    /// `unify`, chain depth is bounded by the number of distinct type variables
    /// in the program. However, pathologically deep chains could exhaust the
    /// call stack in debug builds (where frames are larger and tail-call
    /// optimization is absent). This is acceptable for the current compiler
    /// because real Maat programs produce substitution chains of trivial depth.
    pub fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(id) => match self.get(id) {
                Some(resolved) => self.apply(resolved),
                None => ty.clone(),
            },
            Type::Vector(elem) => Type::Vector(Box::new(self.apply(elem))),
            Type::Set(elem) => Type::Set(Box::new(self.apply(elem))),
            Type::Range(elem) => Type::Range(Box::new(self.apply(elem))),
            Type::Map(k, v) => Type::Map(Box::new(self.apply(k)), Box::new(self.apply(v))),
            Type::Function(fn_ty) => Type::Function(FnType {
                params: fn_ty.params.iter().map(|p| self.apply(p)).collect(),
                ret: Box::new(self.apply(&fn_ty.ret)),
            }),
            Type::Struct(name, args) => {
                Type::Struct(name.clone(), args.iter().map(|a| self.apply(a)).collect())
            }
            Type::Enum(name, args) => {
                Type::Enum(name.clone(), args.iter().map(|a| self.apply(a)).collect())
            }
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
        let result = subst.unify(&Type::Var(0), &Type::Vector(Box::new(Type::Var(0))));
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
