//! Type unification and substitution for Hindley-Milner inference.

use indexmap::IndexMap;
use maat_ast::{Expr, NumKind, Program, Stmt};
use maat_errors::TypeErrorKind;

use crate::{FnType, Type, TypeVarId};

#[derive(Debug, Clone, Default)]
pub struct Substitution {
    map: IndexMap<TypeVarId, Type>,
}

impl Substitution {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies this substitution to a type, recursively resolving all type variables.
    pub fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(id) | Type::IntVar(id) => match self.get(id) {
                Some(resolved) => self.apply(resolved),
                None => ty.clone(),
            },
            Type::Vector(elem) => Type::Vector(Box::new(self.apply(elem))),
            Type::Array(elem, n) => Type::Array(Box::new(self.apply(elem)), *n),
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
            Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| self.apply(e)).collect()),
            _ => ty.clone(),
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
            // IntVar unifies with any integer type or another IntVar.
            (Type::IntVar(id), other) | (other, Type::IntVar(id))
                if other.is_integer() || matches!(other, Type::Var(_)) =>
            {
                self.bind_var(*id, other)
            }
            (Type::IntVar(_), other) | (other, Type::IntVar(_)) => {
                Err(UnifyError::Mismatch(other.clone(), Type::I64))
            }
            (Type::Var(id), _) => self.bind_var(*id, &b),
            (_, Type::Var(id)) => self.bind_var(*id, &a),
            (Type::Vector(ea), Type::Vector(eb)) => self.unify(ea, eb),
            (Type::Array(ea, na), Type::Array(eb, nb)) => {
                if na != nb {
                    return Err(UnifyError::Mismatch(a, b));
                }
                self.unify(ea, eb)
            }
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
            (Type::Tuple(ea), Type::Tuple(eb)) => {
                if ea.len() != eb.len() {
                    return Err(UnifyError::Mismatch(a, b));
                }
                for (pa, pb) in ea.iter().zip(eb.iter()) {
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

    pub fn get(&self, var: &TypeVarId) -> Option<&Type> {
        self.map.get(var)
    }

    pub fn resolve_inferred_literals(&self, program: &mut Program) {
        for stmt in &mut program.statements {
            self.resolve_literals_in_stmt(stmt);
        }
    }

    fn resolve_literals_in_stmt(&self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Let(let_stmt) => self.resolve_literals_in_expr(&mut let_stmt.value),
            Stmt::ReAssign(assign) => self.resolve_literals_in_expr(&mut assign.value),
            Stmt::Return(ret) => self.resolve_literals_in_expr(&mut ret.value),
            Stmt::Expr(expr_stmt) => self.resolve_literals_in_expr(&mut expr_stmt.value),
            Stmt::Block(block) => {
                for s in &mut block.statements {
                    self.resolve_literals_in_stmt(s);
                }
            }
            Stmt::FuncDef(fn_item) => {
                for s in &mut fn_item.body.statements {
                    self.resolve_literals_in_stmt(s);
                }
            }
            Stmt::Loop(loop_stmt) => {
                for s in &mut loop_stmt.body.statements {
                    self.resolve_literals_in_stmt(s);
                }
            }
            Stmt::While(while_stmt) => {
                self.resolve_literals_in_expr(&mut while_stmt.condition);
                for s in &mut while_stmt.body.statements {
                    self.resolve_literals_in_stmt(s);
                }
            }
            Stmt::For(for_stmt) => {
                self.resolve_literals_in_expr(&mut for_stmt.iterable);
                for s in &mut for_stmt.body.statements {
                    self.resolve_literals_in_stmt(s);
                }
            }
            Stmt::Mod(mod_stmt) => {
                if let Some(body) = &mut mod_stmt.body {
                    for s in body {
                        self.resolve_literals_in_stmt(s);
                    }
                }
            }
            Stmt::StructDecl(_)
            | Stmt::EnumDecl(_)
            | Stmt::TraitDecl(_)
            | Stmt::ImplBlock(_)
            | Stmt::Use(_) => {}
        }
    }

    fn resolve_literals_in_expr(&self, expr: &mut Expr) {
        match expr {
            Expr::Number(lit) if matches!(lit.kind, NumKind::Int { .. }) => {
                if let NumKind::Int { type_var } = lit.kind {
                    let resolved = self.resolve_int_vars(&Type::IntVar(type_var));
                    lit.kind = resolved.to_number_kind();
                }
            }
            Expr::Infix(infix) => {
                self.resolve_literals_in_expr(&mut infix.lhs);
                self.resolve_literals_in_expr(&mut infix.rhs);
            }
            Expr::Prefix(prefix) => {
                self.resolve_literals_in_expr(&mut prefix.operand);
            }
            Expr::Call(call) => {
                self.resolve_literals_in_expr(&mut call.function);
                for arg in &mut call.arguments {
                    self.resolve_literals_in_expr(arg);
                }
            }
            Expr::MacroCall(mc) => {
                for arg in &mut mc.arguments {
                    self.resolve_literals_in_expr(arg);
                }
            }
            Expr::MethodCall(mc) => {
                self.resolve_literals_in_expr(&mut mc.object);
                for arg in &mut mc.arguments {
                    self.resolve_literals_in_expr(arg);
                }
            }
            Expr::Index(idx) => {
                self.resolve_literals_in_expr(&mut idx.expr);
                self.resolve_literals_in_expr(&mut idx.index);
            }
            Expr::FieldAccess(fa) => {
                self.resolve_literals_in_expr(&mut fa.object);
            }
            Expr::Cast(cast) => {
                self.resolve_literals_in_expr(&mut cast.expr);
            }
            Expr::Try(try_expr) => {
                self.resolve_literals_in_expr(&mut try_expr.expr);
            }
            Expr::Vector(vec) => {
                for elem in &mut vec.elements {
                    self.resolve_literals_in_expr(elem);
                }
            }
            Expr::Map(map) => {
                for (k, v) in &mut map.pairs {
                    self.resolve_literals_in_expr(k);
                    self.resolve_literals_in_expr(v);
                }
            }
            Expr::Tuple(tuple) => {
                for elem in &mut tuple.elements {
                    self.resolve_literals_in_expr(elem);
                }
            }
            Expr::Range(range) => {
                self.resolve_literals_in_expr(&mut range.start);
                self.resolve_literals_in_expr(&mut range.end);
            }
            Expr::Array(arr) => {
                for elem in &mut arr.elements {
                    self.resolve_literals_in_expr(elem);
                }
            }
            Expr::Cond(cond) => {
                self.resolve_literals_in_expr(&mut cond.condition);
                for s in &mut cond.consequence.statements {
                    self.resolve_literals_in_stmt(s);
                }
                if let Some(alt) = &mut cond.alternative {
                    for s in &mut alt.statements {
                        self.resolve_literals_in_stmt(s);
                    }
                }
            }
            Expr::Lambda(lambda) => {
                for s in &mut lambda.body.statements {
                    self.resolve_literals_in_stmt(s);
                }
            }
            Expr::MacroLit(macro_lit) => {
                for s in &mut macro_lit.body.statements {
                    self.resolve_literals_in_stmt(s);
                }
            }
            Expr::Match(match_expr) => {
                self.resolve_literals_in_expr(&mut match_expr.scrutinee);
                for arm in &mut match_expr.arms {
                    self.resolve_literals_in_expr(&mut arm.body);
                    if let Some(guard) = &mut arm.guard {
                        self.resolve_literals_in_expr(guard);
                    }
                }
            }
            Expr::Break(break_expr) => {
                if let Some(val) = &mut break_expr.value {
                    self.resolve_literals_in_expr(val);
                }
            }
            Expr::StructLit(struct_lit) => {
                for (_, val) in &mut struct_lit.fields {
                    self.resolve_literals_in_expr(val);
                }
                if let Some(base) = &mut struct_lit.base {
                    self.resolve_literals_in_expr(base);
                }
            }
            Expr::Number(_)
            | Expr::Bool(_)
            | Expr::Str(_)
            | Expr::Char(_)
            | Expr::Ident(_)
            | Expr::Continue(_)
            | Expr::PathExpr(_) => {}
        }
    }

    pub fn resolve_int_vars(&self, ty: &Type) -> Type {
        let resolved = self.apply(ty);
        match resolved {
            Type::IntVar(_) => Type::I64,
            _ => resolved,
        }
    }

    pub fn bind(&mut self, var: TypeVarId, ty: Type) {
        self.map.insert(var, ty);
    }

    fn bind_var(&mut self, var: TypeVarId, ty: &Type) -> Result<(), UnifyError> {
        if self.occurs(var, ty) {
            return Err(UnifyError::OccursCheck(var, ty.clone()));
        }
        self.bind(var, ty.clone());
        Ok(())
    }

    fn occurs(&self, var: TypeVarId, ty: &Type) -> bool {
        match ty {
            Type::Var(id) | Type::IntVar(id) => *id == var,
            Type::Vector(elem) | Type::Array(elem, _) | Type::Set(elem) | Type::Range(elem) => {
                self.occurs(var, elem)
            }
            Type::Map(k, v) => self.occurs(var, k) || self.occurs(var, v),
            Type::Function(fn_ty) => {
                fn_ty.params.iter().any(|p| self.occurs(var, p)) || self.occurs(var, &fn_ty.ret)
            }
            Type::Tuple(elems) => elems.iter().any(|e| self.occurs(var, e)),
            Type::Struct(_, args) | Type::Enum(_, args) => args.iter().any(|a| self.occurs(var, a)),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum UnifyError {
    Mismatch(Type, Type),
    OccursCheck(TypeVarId, Type),
}

impl UnifyError {
    pub fn to_type_error(&self) -> TypeErrorKind {
        match self {
            Self::Mismatch(a, b) => TypeErrorKind::Mismatch {
                expected: a.to_string(),
                found: b.to_string(),
            },
            Self::OccursCheck(id, ty) => TypeErrorKind::OccursCheck(format!("?T{id} in {ty}")),
        }
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
