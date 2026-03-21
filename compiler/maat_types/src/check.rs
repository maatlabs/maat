//! Compile-time type checker for the AST.

use std::rc::Rc;

use maat_ast::*;
use maat_errors::{
    MissingTraitMethodError, TraitMethodSignatureMismatchError, TypeError, TypeErrorKind,
};
use maat_span::Span;

use crate::promote::{self, PromotionError};
use crate::unify::{Substitution, UnifyError};
use crate::{
    EnumDef, FnType, ImplDef, MethodSig, StructDef, TraitDef, Type, TypeEnv, VariantDef,
    VariantKind,
};

/// The type checker.
///
/// Performs Hindley-Milner-style type inference with explicit annotations,
/// numeric promotion rules, compile-time overflow checking, and full
/// validation of user-defined types (structs, enums, traits, impls).
pub struct TypeChecker {
    env: TypeEnv,
    subst: Substitution,
    errors: Vec<TypeError>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    /// Creates a new type checker with builtins pre-registered.
    pub fn new() -> Self {
        let mut env = TypeEnv::new();
        env.register_builtins();
        Self {
            env,
            subst: Substitution::new(),
            errors: Vec::new(),
        }
    }

    /// Returns a reference to the type environment.
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// Returns a mutable reference to the type environment.
    pub fn env_mut(&mut self) -> &mut TypeEnv {
        &mut self.env
    }

    /// Type-checks a program, mutating the AST to insert promotion casts.
    ///
    /// Performs two passes: the first registers all type declarations (structs,
    /// enums, traits) so that forward references resolve correctly; the second
    /// checks all statements including impl blocks and expressions.
    ///
    /// Returns accumulated type errors (empty if the program is well-typed).
    pub fn check_program(mut self, program: &mut Program) -> Vec<TypeError> {
        self.check_program_mut(program);
        self.errors
    }

    /// Type-checks a program without consuming the checker.
    ///
    /// Behaves identically to [`check_program`](Self::check_program) but
    /// borrows `self` mutably, allowing the caller to inspect the type
    /// environment afterwards (e.g., to extract module exports).
    pub fn check_program_mut(&mut self, program: &mut Program) {
        for stmt in &program.statements {
            match stmt {
                Stmt::StructDecl(decl) => self.register_struct(decl),
                Stmt::EnumDecl(decl) => self.register_enum(decl),
                Stmt::TraitDecl(decl) => self.register_trait(decl),
                _ => {}
            }
        }
        for stmt in &mut program.statements {
            self.check_statement(stmt);
        }
    }

    /// Returns the accumulated type errors.
    pub fn errors(&self) -> &[TypeError] {
        &self.errors
    }

    fn register_struct(&mut self, decl: &StructDecl) {
        if self.env.lookup_struct(&decl.name).is_some() {
            self.errors
                .push(TypeErrorKind::DuplicateType(decl.name.clone()).at(decl.span));
            return;
        }
        let generic_params = decl
            .generic_params
            .iter()
            .map(|g| g.name.clone())
            .collect::<Vec<String>>();
        let fields = decl
            .fields
            .iter()
            .map(|f| {
                (
                    f.name.clone(),
                    self.resolve_field_type(&f.ty, &generic_params),
                )
            })
            .collect();
        self.env.register_struct(StructDef {
            name: decl.name.clone(),
            generic_params,
            fields,
        });
    }

    fn register_enum(&mut self, decl: &EnumDecl) {
        if self.env.lookup_enum(&decl.name).is_some() {
            self.errors
                .push(TypeErrorKind::DuplicateType(decl.name.clone()).at(decl.span));
            return;
        }
        let generic_params = decl
            .generic_params
            .iter()
            .map(|g| g.name.clone())
            .collect::<Vec<String>>();
        let variants = decl
            .variants
            .iter()
            .map(|v| VariantDef {
                name: v.name.clone(),
                kind: match &v.kind {
                    EnumVariantKind::Unit => VariantKind::Unit,
                    EnumVariantKind::Tuple(tys) => VariantKind::Tuple(
                        tys.iter()
                            .map(|t| self.resolve_field_type(t, &generic_params))
                            .collect(),
                    ),
                    EnumVariantKind::Struct(fields) => VariantKind::Struct(
                        fields
                            .iter()
                            .map(|f| {
                                (
                                    f.name.clone(),
                                    self.resolve_field_type(&f.ty, &generic_params),
                                )
                            })
                            .collect(),
                    ),
                },
            })
            .collect();
        self.env.register_enum(EnumDef {
            name: decl.name.clone(),
            generic_params,
            variants,
        });
    }

    fn register_trait(&mut self, decl: &TraitDecl) {
        if self.env.lookup_trait(&decl.name).is_some() {
            self.errors
                .push(TypeErrorKind::DuplicateType(decl.name.clone()).at(decl.span));
            return;
        }
        let generic_params = decl
            .generic_params
            .iter()
            .map(|g| g.name.clone())
            .collect::<Vec<String>>();
        let methods = decl
            .methods
            .iter()
            .map(|m| {
                let takes_self = m.params.first().is_some_and(|p| p.name == "self");
                let params = m
                    .params
                    .iter()
                    .filter(|p| p.name != "self")
                    .map(|p| {
                        p.type_expr
                            .as_ref()
                            .map(|t| self.resolve_field_type(t, &generic_params))
                            .unwrap_or(Type::Var(0))
                    })
                    .collect();
                let ret = m
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_field_type(t, &generic_params))
                    .unwrap_or(Type::Null);
                MethodSig {
                    name: m.name.clone(),
                    params,
                    ret,
                    has_default: m.default_body.is_some(),
                    takes_self,
                }
            })
            .collect();
        self.env.register_trait(TraitDef {
            name: decl.name.clone(),
            generic_params,
            methods,
        });
    }

    /// Resolves a type expression for a struct/enum field, treating names
    /// that match a generic parameter as `Type::Generic`.
    fn resolve_field_type(&mut self, ty: &TypeExpr, generic_params: &[String]) -> Type {
        match ty {
            TypeExpr::Named(named) if generic_params.contains(&named.name) => {
                Type::Generic(Rc::from(named.name.as_str()), vec![])
            }
            _ => self.env.resolve_type(ty),
        }
    }

    fn check_statement(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Let(let_stmt) => self.check_let(let_stmt),
            Stmt::ReAssign(assign_stmt) => {
                self.infer_expression(&mut assign_stmt.value);
            }
            Stmt::Return(ret_stmt) => {
                self.infer_expression(&mut ret_stmt.value);
            }
            Stmt::Expr(expr_stmt) => {
                self.infer_expression(&mut expr_stmt.value);
            }
            Stmt::Block(block) => self.check_block(block),
            Stmt::FuncDef(fn_item) => self.check_fn_item(fn_item),
            Stmt::Loop(loop_stmt) => self.check_block(&mut loop_stmt.body),
            Stmt::While(while_stmt) => {
                let cond_ty = self.infer_expression(&mut while_stmt.condition);
                let cond_resolved = self.subst.apply(&cond_ty);
                self.require_bool(&cond_resolved, while_stmt.condition.span());
                self.check_block(&mut while_stmt.body);
            }
            Stmt::For(for_stmt) => {
                let iter_ty = self.infer_expression(&mut for_stmt.iterable);
                let resolved = self.subst.apply(&iter_ty);
                let elem_ty = match resolved {
                    Type::Vector(elem) => *elem,
                    Type::Range(elem) => *elem,
                    Type::Var(_) => self.env.fresh_var(),
                    _ => {
                        self.errors.push(
                            TypeErrorKind::Mismatch {
                                expected: "[T] or Range<T>".to_string(),
                                found: resolved.to_string(),
                            }
                            .at(for_stmt.iterable.span()),
                        );
                        self.env.fresh_var()
                    }
                };
                self.env.push_scope();
                self.env.define_var(&for_stmt.ident, elem_ty);
                self.check_block(&mut for_stmt.body);
                self.env.pop_scope();
            }
            // Struct/Enum/Trait declarations were registered in [self.check_program] (pass 1); nothing to do here.
            Stmt::StructDecl(_) | Stmt::EnumDecl(_) | Stmt::TraitDecl(_) => {}
            Stmt::ImplBlock(impl_block) => self.check_impl_block(impl_block),
            // Module declarations (`mod foo;` / `mod foo { ... }`) and import
            // statements (`use foo::bar;`) are resolved by the module orchestrator
            // before per-module type checking runs. No action needed here.
            Stmt::Use(_) | Stmt::Mod(_) => {}
        }
    }

    fn check_let(&mut self, let_stmt: &mut LetStmt) {
        let inferred = self.infer_expression(&mut let_stmt.value);
        let ty = if let Some(ann) = &let_stmt.type_annotation {
            let expected = self.env.resolve_type(ann);
            self.check_literal_range(&let_stmt.value, &expected, let_stmt.span);

            let is_coercible_literal = expected.is_integer()
                && inferred.is_integer()
                && let_stmt.value.is_integer_literal();
            if is_coercible_literal {
                expected.coerce_literal(&mut let_stmt.value);
            } else if let Err(e) = self.subst.unify(&expected, &inferred) {
                self.report_unify_error(e, let_stmt.span);
            }
            expected
        } else {
            inferred
        };
        let scheme = self.env.generalize(&ty, &self.subst);
        self.env.define_scheme(&let_stmt.ident, scheme);
    }

    fn check_block(&mut self, block: &mut BlockStmt) {
        self.env.push_scope();
        for stmt in &mut block.statements {
            self.check_statement(stmt);
        }
        self.env.pop_scope();
    }

    /// Type-checks a named function declaration and binds it in the environment.
    fn check_fn_item(&mut self, fn_item: &mut FuncDef) {
        let fn_ty = self.infer_fn_body(
            &fn_item.generic_params,
            &fn_item.params,
            &fn_item.return_type,
            &mut fn_item.body,
            fn_item.span,
        );
        let scheme = self.env.generalize(&fn_ty, &self.subst);
        self.env.define_scheme(&fn_item.name, scheme);
    }

    /// Shared inference logic for function bodies (used by both `FuncDef` and `Lambda`).
    fn infer_fn_body(
        &mut self,
        generic_params: &[GenericParam],
        params: &[TypedParam],
        return_type: &Option<TypeExpr>,
        body: &mut BlockStmt,
        span: Span,
    ) -> Type {
        self.env.push_scope();
        for gp in generic_params {
            let var = self.env.fresh_var();
            self.env.define_var(&gp.name, var);
        }
        let param_types: Vec<Type> = params
            .iter()
            .map(|p| {
                let ty = p
                    .type_expr
                    .as_ref()
                    .map(|te| self.env.resolve_type(te))
                    .unwrap_or_else(|| self.env.fresh_var());
                self.env.define_var(&p.name, ty.clone());
                ty
            })
            .collect();

        let body_ty = self.infer_block(body);
        let ret_ty = return_type
            .as_ref()
            .map(|te| self.env.resolve_type(te))
            .unwrap_or_else(|| body_ty.clone());
        if return_type.is_some()
            && let Err(e) = self.subst.unify(&ret_ty, &body_ty)
        {
            self.report_unify_error(e, span);
        }
        self.env.pop_scope();
        Type::Function(FnType {
            params: param_types,
            ret: Box::new(self.subst.apply(&ret_ty)),
        })
    }

    fn check_impl_block(&mut self, impl_block: &mut ImplBlock) {
        let self_type = self.env.resolve_type(&impl_block.self_type);
        let trait_name = impl_block.trait_name.as_ref().and_then(|te| match te {
            TypeExpr::Named(n) => Some(n.name.clone()),
            TypeExpr::Generic(name, _, _) => Some(name.clone()),
            _ => None,
        });
        if let Some(ref name) = trait_name
            && self.env.lookup_trait(name).is_none()
        {
            self.errors
                .push(TypeErrorKind::UnknownTrait(name.clone()).at(impl_block.span));
        }
        let mut method_types = Vec::new();
        for method in &mut impl_block.methods {
            let has_self = method.params.first().is_some_and(|p| p.name == "self");
            let non_self_params = method
                .params
                .iter()
                .filter(|p| p.name != "self")
                .cloned()
                .collect::<Vec<TypedParam>>();
            self.env.push_scope();
            if has_self {
                self.env.define_var("self", self_type.clone());
            }
            let fn_ty = self.infer_fn_body(
                &method.generic_params,
                &non_self_params,
                &method.return_type,
                &mut method.body,
                method.span,
            );
            self.env.pop_scope();
            method_types.push((method.name.clone(), fn_ty));
        }
        if let Some(ref name) = trait_name {
            self.verify_trait_satisfaction(name, &self_type, &method_types, impl_block.span);
        }
        self.env.register_impl(ImplDef {
            self_type,
            trait_name,
            methods: method_types,
        });
    }

    /// Verifies that an impl block provides all required trait methods with
    /// compatible signatures.
    fn verify_trait_satisfaction(
        &mut self,
        trait_name: &str,
        self_type: &Type,
        methods: &[(String, Type)],
        span: Span,
    ) {
        let required = self
            .env
            .required_trait_methods(trait_name)
            .into_iter()
            .cloned()
            .collect::<Vec<MethodSig>>();
        for sig in &required {
            match methods.iter().find(|(name, _)| name == &sig.name) {
                None => {
                    self.errors.push(
                        TypeErrorKind::MissingTraitMethod(Box::new(MissingTraitMethodError {
                            trait_name: trait_name.to_string(),
                            self_type: self_type.to_string(),
                            method: sig.name.clone(),
                        }))
                        .at(span),
                    );
                }
                Some((_, impl_ty)) => {
                    let expected = Type::Function(FnType {
                        params: sig.params.clone(),
                        ret: Box::new(sig.ret.clone()),
                    });
                    if self.subst.unify(&expected, impl_ty).is_err() {
                        self.errors.push(
                            TypeErrorKind::TraitMethodSignatureMismatch(Box::new(
                                TraitMethodSignatureMismatchError {
                                    trait_name: trait_name.to_string(),
                                    self_type: self_type.to_string(),
                                    method: sig.name.clone(),
                                    expected: expected.to_string(),
                                    found: impl_ty.to_string(),
                                },
                            ))
                            .at(span),
                        );
                    }
                }
            }
        }
    }

    /// Infers the type of an expression, potentially mutating it
    /// (e.g., inserting promotion casts on infix operands).
    fn infer_expression(&mut self, expr: &mut Expr) -> Type {
        match expr {
            Expr::Number(lit) => match lit.kind {
                NumberKind::I8 => Type::I8,
                NumberKind::I16 => Type::I16,
                NumberKind::I32 => Type::I32,
                NumberKind::I64 => Type::I64,
                NumberKind::I128 => Type::I128,
                NumberKind::Isize => Type::Isize,
                NumberKind::U8 => Type::U8,
                NumberKind::U16 => Type::U16,
                NumberKind::U32 => Type::U32,
                NumberKind::U64 => Type::U64,
                NumberKind::U128 => Type::U128,
                NumberKind::Usize => Type::Usize,
            },
            Expr::Bool(_) => Type::Bool,
            Expr::Str(_) => Type::String,
            Expr::Ident(ident) => self
                .env
                .instantiate(&ident.value, &self.subst)
                .or_else(|| self.resolve_bare_variant(&ident.value))
                .unwrap_or_else(|| self.env.fresh_var()),
            Expr::Vector(vector) => self.infer_vector(vector),
            Expr::Map(map) => self.infer_map(map),
            Expr::Index(idx) => self.infer_index(idx),
            Expr::Prefix(prefix) => self.infer_prefix(prefix),
            Expr::Infix(infix) => self.check_infix(infix),
            Expr::Cond(cond) => self.infer_conditional(cond),
            Expr::Lambda(lambda) => self.infer_fn_body(
                &lambda.generic_params,
                &lambda.params,
                &lambda.return_type,
                &mut lambda.body,
                lambda.span,
            ),
            Expr::Call(call) => self.check_call_expr(call),
            Expr::Cast(cast) => {
                self.infer_expression(&mut cast.expr);
                Type::from_number_kind(&cast.target)
            }
            Expr::Break(break_expr) => {
                if let Some(val) = &mut break_expr.value {
                    self.infer_expression(val);
                }
                Type::Never
            }
            Expr::Continue(_) => Type::Never,
            Expr::Macro(_) => self.env.fresh_var(),
            Expr::Match(match_expr) => self.check_match_expr(match_expr),
            Expr::FieldAccess(field_access) => self.check_field_access(field_access),
            Expr::MethodCall(method_call) => self.check_method_call(method_call),
            Expr::StructLit(struct_lit) => self.check_struct_literal(struct_lit),
            Expr::PathExpr(path_expr) => self.check_path_expr(path_expr),
            Expr::Range(range) => self.infer_range(range),
        }
    }

    /// Infers the element type of a vector.
    fn infer_vector(&mut self, vector: &mut Vector) -> Type {
        if vector.elements.is_empty() {
            let elem = self.env.fresh_var();
            Type::Vector(Box::new(elem))
        } else {
            let first = self.infer_expression(&mut vector.elements[0]);
            for elem in &mut vector.elements[1..] {
                let elem_ty = self.infer_expression(elem);
                if let Err(e) = self.subst.unify(&first, &elem_ty) {
                    self.report_unify_error(e, elem.span());
                }
            }
            Type::Vector(Box::new(self.subst.apply(&first)))
        }
    }

    /// Infers the key and value types of a map literal.
    fn infer_map(&mut self, map: &mut Map) -> Type {
        if map.pairs.is_empty() {
            let k = self.env.fresh_var();
            let v = self.env.fresh_var();
            Type::Map(Box::new(k), Box::new(v))
        } else {
            let (first_k, first_v) = {
                let k = self.infer_expression(&mut map.pairs[0].0);
                let v = self.infer_expression(&mut map.pairs[0].1);
                (k, v)
            };
            for (k_expr, v_expr) in &mut map.pairs[1..] {
                let k = self.infer_expression(k_expr);
                let v = self.infer_expression(v_expr);
                if let Err(e) = self.subst.unify(&first_k, &k) {
                    self.report_unify_error(e, k_expr.span());
                }
                if let Err(e) = self.subst.unify(&first_v, &v) {
                    self.report_unify_error(e, v_expr.span());
                }
            }
            Type::Map(
                Box::new(self.subst.apply(&first_k)),
                Box::new(self.subst.apply(&first_v)),
            )
        }
    }

    /// Infers the result type of an index expression (`expr[index]`).
    fn infer_index(&mut self, idx: &mut IndexExpr) -> Type {
        let collection = self.infer_expression(&mut idx.expr);
        let _index_ty = self.infer_expression(&mut idx.index);
        let resolved = self.subst.apply(&collection);
        match resolved {
            Type::Vector(elem) => *elem,
            Type::Map(_, v) => *v,
            _ => self.env.fresh_var(),
        }
    }

    /// Infers the result type of a prefix (unary) expression.
    fn infer_prefix(&mut self, prefix: &mut PrefixExpr) -> Type {
        let operand_ty = self.infer_expression(&mut prefix.operand);
        let resolved = self.subst.apply(&operand_ty);
        match prefix.operator.as_str() {
            "!" => {
                self.require_bool(&resolved, prefix.span);
                Type::Bool
            }
            "-" => {
                if !resolved.is_integer() && !matches!(resolved, Type::Var(_)) {
                    self.errors.push(
                        TypeErrorKind::Mismatch {
                            expected: "numeric".to_string(),
                            found: resolved.to_string(),
                        }
                        .at(prefix.span),
                    );
                }
                resolved
            }
            _ => self.env.fresh_var(),
        }
    }

    /// Infers the result type of an `if`/`else` conditional expression.
    fn infer_conditional(&mut self, cond: &mut CondExpr) -> Type {
        let cond_ty = self.infer_expression(&mut cond.condition);
        let cond_resolved = self.subst.apply(&cond_ty);
        self.require_bool(&cond_resolved, cond.condition.span());

        self.env.push_scope();
        let cons_ty = self.infer_block(&mut cond.consequence);
        self.env.pop_scope();

        if let Some(alt) = &mut cond.alternative {
            self.env.push_scope();
            let alt_ty = self.infer_block(alt);
            self.env.pop_scope();
            if let Err(e) = self.subst.unify(&cons_ty, &alt_ty) {
                self.report_unify_error(e, cond.span);
            }
            self.subst.apply(&cons_ty)
        } else {
            cons_ty
        }
    }

    /// Infers the type of a range expression (`start..end` or `start..=end`).
    fn infer_range(&mut self, range: &mut RangeExpr) -> Type {
        let start_ty = self.infer_expression(&mut range.start);
        let end_ty = self.infer_expression(&mut range.end);
        let start_resolved = self.subst.apply(&start_ty);
        let end_resolved = self.subst.apply(&end_ty);
        if let Err(e) = self.subst.unify(&start_resolved, &end_resolved) {
            self.report_unify_error(e, range.span);
        }
        if !start_resolved.is_integer() && !matches!(start_resolved, Type::Var(_)) {
            self.errors.push(
                TypeErrorKind::Mismatch {
                    expected: "integer".to_string(),
                    found: start_resolved.to_string(),
                }
                .at(range.span),
            );
        }
        Type::Range(Box::new(self.subst.apply(&start_resolved)))
    }

    /// Type-checks a function call expression.
    fn check_call_expr(&mut self, call: &mut CallExpr) -> Type {
        let func_ty = self.infer_expression(&mut call.function);
        let resolved = self.subst.apply(&func_ty);
        let arg_types = call
            .arguments
            .iter_mut()
            .map(|a| self.infer_expression(a))
            .collect::<Vec<Type>>();
        match resolved {
            Type::Function(fn_ty) => {
                if fn_ty.params.len() != arg_types.len() {
                    self.errors.push(
                        TypeErrorKind::WrongArity {
                            expected: fn_ty.params.len(),
                            found: arg_types.len(),
                        }
                        .at(call.span),
                    );
                } else {
                    for (param, arg) in fn_ty.params.iter().zip(arg_types.iter()) {
                        let p = self.subst.apply(param);
                        let a = self.subst.apply(arg);
                        if p.is_integer()
                            && a.is_integer()
                            && p != a
                            && promote::common_numeric_type(&p, &a).is_ok()
                        {
                            continue;
                        }
                        if let Err(e) = self.subst.unify(&p, &a) {
                            self.report_unify_error(e, call.span);
                        }
                    }
                }
                self.subst.apply(&fn_ty.ret)
            }
            Type::Var(_) => {
                let ret = self.env.fresh_var();
                let expected_fn = Type::Function(FnType {
                    params: arg_types,
                    ret: Box::new(ret.clone()),
                });
                if let Err(e) = self.subst.unify(&resolved, &expected_fn) {
                    self.report_unify_error(e, call.span);
                }
                self.subst.apply(&ret)
            }
            _ => {
                self.errors
                    .push(TypeErrorKind::NotCallable(resolved.to_string()).at(call.span));
                self.env.fresh_var()
            }
        }
    }

    /// Type-checks a struct literal expression (e.g., `Point { x: 1, y: 2 }`)
    /// or with functional update syntax (e.g., `Point { x: 10, ..other }`).
    fn check_struct_literal(&mut self, lit: &mut StructLitExpr) -> Type {
        let struct_def = self.env.lookup_struct(&lit.name).cloned();
        let Some(def) = struct_def else {
            self.errors
                .push(TypeErrorKind::UnknownType(lit.name.clone()).at(lit.span));
            return self.env.fresh_var();
        };
        let type_args = def
            .generic_params
            .iter()
            .map(|_| self.env.fresh_var())
            .collect::<Vec<Type>>();
        for (field_name, field_expr) in &mut lit.fields {
            let field_ty = self.infer_expression(field_expr);
            if let Some((_, expected_ty)) = def.fields.iter().find(|(n, _)| n == field_name) {
                let resolved =
                    self.instantiate_generic_type(expected_ty, &def.generic_params, &type_args);
                if let Err(e) = self.subst.unify(&resolved, &field_ty) {
                    self.report_unify_error(e, field_expr.span());
                }
            } else {
                self.errors.push(
                    TypeErrorKind::UnknownField {
                        ty: lit.name.clone(),
                        field: field_name.clone(),
                    }
                    .at(lit.span),
                );
            }
        }
        let expected_struct_ty = Type::Struct(
            Rc::from(lit.name.as_str()),
            type_args.iter().map(|t| self.subst.apply(t)).collect(),
        );
        if let Some(base_expr) = &mut lit.base {
            let base_ty = self.infer_expression(base_expr);
            if let Err(e) = self.subst.unify(&expected_struct_ty, &base_ty) {
                self.report_unify_error(e, base_expr.span());
            }
        } else {
            for (def_field_name, _) in &def.fields {
                if !lit.fields.iter().any(|(n, _)| n == def_field_name) {
                    self.errors.push(
                        TypeErrorKind::UnknownField {
                            ty: format!("missing field `{}` in `{}`", def_field_name, lit.name),
                            field: def_field_name.clone(),
                        }
                        .at(lit.span),
                    );
                }
            }
        }
        expected_struct_ty
    }

    /// Type-checks a path expression (e.g., `Option::Some`, `Color::Red`).
    fn check_path_expr(&mut self, path: &PathExpr) -> Type {
        if path.segments.len() == 2 {
            let type_name = &path.segments[0];
            let variant_name = &path.segments[1];
            if let Some(enum_def) = self.env.lookup_enum(type_name).cloned() {
                let type_args = enum_def
                    .generic_params
                    .iter()
                    .map(|_| self.env.fresh_var())
                    .collect::<Vec<Type>>();
                if let Some(variant) = enum_def.variants.iter().find(|v| v.name == *variant_name) {
                    match &variant.kind {
                        VariantKind::Unit => {
                            return Type::Enum(
                                Rc::from(type_name.as_str()),
                                type_args.iter().map(|t| self.subst.apply(t)).collect(),
                            );
                        }
                        VariantKind::Tuple(field_types) => {
                            let params = field_types
                                .iter()
                                .map(|t| {
                                    self.instantiate_generic_type(
                                        t,
                                        &enum_def.generic_params,
                                        &type_args,
                                    )
                                })
                                .collect();
                            let ret = Type::Enum(
                                Rc::from(type_name.as_str()),
                                type_args.iter().map(|t| self.subst.apply(t)).collect(),
                            );
                            return Type::Function(FnType {
                                params,
                                ret: Box::new(ret),
                            });
                        }
                        VariantKind::Struct(_) => {
                            return Type::Enum(
                                Rc::from(type_name.as_str()),
                                type_args.iter().map(|t| self.subst.apply(t)).collect(),
                            );
                        }
                    }
                } else {
                    self.errors.push(
                        TypeErrorKind::UnknownField {
                            ty: type_name.clone(),
                            field: variant_name.clone(),
                        }
                        .at(path.span),
                    );
                }
            }
        }
        // Try as a qualified method lookup (e.g., `Counter::new`).
        if path.segments.len() == 2 {
            let type_name = &path.segments[0];
            let method_name = &path.segments[1];
            if let Some(method_ty) = self.env.lookup_method_by_name(type_name, method_name) {
                return method_ty.clone();
            }
        }
        // Fallback: try as a variable lookup.
        let full_name = path.segments.join("::");
        self.env
            .instantiate(&full_name, &self.subst)
            .unwrap_or_else(|| {
                self.errors
                    .push(TypeErrorKind::UnknownType(full_name).at(path.span));
                self.env.fresh_var()
            })
    }

    fn check_field_access(&mut self, fa: &mut FieldAccessExpr) -> Type {
        let obj_ty = self.infer_expression(&mut fa.object);
        let resolved = self.subst.apply(&obj_ty);
        match &resolved {
            Type::Struct(name, type_args) => {
                let struct_def = self.env.lookup_struct(name).cloned();
                match struct_def {
                    Some(def) => match def.fields.iter().find(|(fname, _)| fname == &fa.field) {
                        Some((_, field_ty)) => {
                            self.instantiate_generic_type(field_ty, &def.generic_params, type_args)
                        }
                        None => {
                            self.errors.push(
                                TypeErrorKind::UnknownField {
                                    ty: resolved.to_string(),
                                    field: fa.field.clone(),
                                }
                                .at(fa.span),
                            );
                            self.env.fresh_var()
                        }
                    },
                    None => {
                        self.errors
                            .push(TypeErrorKind::UnknownType(name.to_string()).at(fa.span));
                        self.env.fresh_var()
                    }
                }
            }
            Type::Var(_) => self.env.fresh_var(),
            _ => {
                self.errors.push(
                    TypeErrorKind::UnknownField {
                        ty: resolved.to_string(),
                        field: fa.field.clone(),
                    }
                    .at(fa.span),
                );
                self.env.fresh_var()
            }
        }
    }

    fn check_method_call(&mut self, mc: &mut MethodCallExpr) -> Type {
        let obj_ty = self.infer_expression(&mut mc.object);
        let resolved = self.subst.apply(&obj_ty);
        mc.receiver = Self::receiver_type_name(&resolved);

        let arg_types = mc
            .arguments
            .iter_mut()
            .map(|a| self.infer_expression(a))
            .collect::<Vec<Type>>();
        let method_ty = self
            .env
            .instantiate_builtin_method(&resolved, &mc.method, &self.subst)
            .map(|(inst_self, inst_fn)| {
                if let Err(e) = self.subst.unify(&inst_self, &resolved) {
                    self.report_unify_error(e, mc.span);
                }
                self.subst.apply(&inst_fn)
            })
            .or_else(|| {
                self.env
                    .lookup_method(&resolved, &mc.method)
                    .or_else(|| match &resolved {
                        Type::Struct(name, _) | Type::Enum(name, _) => {
                            self.env.lookup_method_by_name(name, &mc.method)
                        }
                        _ => None,
                    })
                    .cloned()
                    .map(|ty| self.subst.apply(&ty))
            });
        match method_ty {
            Some(Type::Function(fn_ty)) => {
                if fn_ty.params.len() != arg_types.len() {
                    self.errors.push(
                        TypeErrorKind::WrongArity {
                            expected: fn_ty.params.len(),
                            found: arg_types.len(),
                        }
                        .at(mc.span),
                    );
                } else {
                    for (param, arg) in fn_ty.params.iter().zip(arg_types.iter()) {
                        let p = self.subst.apply(param);
                        let a = self.subst.apply(arg);
                        if let Err(e) = self.subst.unify(&p, &a) {
                            self.report_unify_error(e, mc.span);
                        }
                    }
                }
                self.subst.apply(&fn_ty.ret)
            }
            Some(_) => self.env.fresh_var(),
            None => {
                if matches!(resolved, Type::Var(_)) {
                    return self.env.fresh_var();
                }
                self.errors.push(
                    TypeErrorKind::UnknownMethod {
                        ty: resolved.to_string(),
                        method: mc.method.clone(),
                    }
                    .at(mc.span),
                );
                self.env.fresh_var()
            }
        }
    }

    fn check_match_expr(&mut self, expr: &mut MatchExpr) -> Type {
        let scrutinee_ty = self.infer_expression(&mut expr.scrutinee);
        let scrutinee_resolved = self.subst.apply(&scrutinee_ty);

        let mut arm_result_ty: Option<Type> = None;

        for arm in &mut expr.arms {
            self.env.push_scope();
            self.check_pattern(&arm.pattern, &scrutinee_resolved);

            if let Some(guard) = &mut arm.guard {
                let guard_ty = self.infer_expression(guard);
                let guard_resolved = self.subst.apply(&guard_ty);
                self.require_bool(&guard_resolved, guard.span());
            }
            let body_ty = self.infer_expression(&mut arm.body);
            self.env.pop_scope();
            match &arm_result_ty {
                Some(prev) => {
                    if let Err(e) = self.subst.unify(prev, &body_ty) {
                        self.report_unify_error(e, arm.span);
                    }
                }
                None => arm_result_ty = Some(body_ty),
            }
        }
        self.check_exhaustiveness(&scrutinee_resolved, expr);
        arm_result_ty
            .map(|ty| self.subst.apply(&ty))
            .unwrap_or(Type::Never)
    }

    /// Checks a pattern against the scrutinee type, introducing bindings
    /// into the current scope.
    fn check_pattern(&mut self, p: &Pattern, scrutinee_ty: &Type) {
        match p {
            Pattern::Wildcard(_) => {}
            Pattern::Ident(name, _) => {
                self.env.define_var(name, scrutinee_ty.clone());
            }
            Pattern::Literal(expr) => {
                let lit_ty = self.infer_literal_pattern_type(expr);
                if self.subst.unify(&lit_ty, scrutinee_ty).is_err() {
                    self.errors.push(
                        TypeErrorKind::Mismatch {
                            expected: scrutinee_ty.to_string(),
                            found: lit_ty.to_string(),
                        }
                        .at(expr.span()),
                    );
                }
            }
            Pattern::TupleStruct { path, fields, span } => {
                self.check_tuple_struct_pattern(path, fields, scrutinee_ty, *span);
            }
            Pattern::Struct { path, fields, span } => {
                self.check_struct_pattern(path, fields, scrutinee_ty, *span);
            }
            Pattern::Or(patterns, _) => {
                for pat in patterns {
                    self.check_pattern(pat, scrutinee_ty);
                }
            }
        }
    }

    /// Infers the type of a literal expression used in a pattern context.
    fn infer_literal_pattern_type(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Number(lit) => match lit.kind {
                NumberKind::I8 => Type::I8,
                NumberKind::I16 => Type::I16,
                NumberKind::I32 => Type::I32,
                NumberKind::I64 => Type::I64,
                NumberKind::I128 => Type::I128,
                NumberKind::Isize => Type::Isize,
                NumberKind::U8 => Type::U8,
                NumberKind::U16 => Type::U16,
                NumberKind::U32 => Type::U32,
                NumberKind::U64 => Type::U64,
                NumberKind::U128 => Type::U128,
                NumberKind::Usize => Type::Usize,
            },
            Expr::Bool(_) => Type::Bool,
            Expr::Str(_) => Type::String,
            _ => Type::Null,
        }
    }

    /// Checks a tuple-struct pattern (e.g., `Some(x)`) against the scrutinee.
    fn check_tuple_struct_pattern(
        &mut self,
        variant_name: &str,
        fields: &[Pattern],
        scrutinee_ty: &Type,
        span: Span,
    ) {
        let enum_info = match scrutinee_ty {
            Type::Enum(name, type_args) => self
                .env
                .lookup_enum(name)
                .cloned()
                .map(|def| (def, type_args.clone())),
            _ => self.find_enum_for_variant(variant_name),
        };
        let Some((enum_def, type_args)) = enum_info else {
            // Not a known variant; skip checking but still bind identifiers.
            for field in fields {
                if let Pattern::Ident(name, _) = field {
                    let fresh = self.env.fresh_var();
                    self.env.define_var(name, fresh);
                }
            }
            return;
        };
        let Some(variant) = enum_def.variants.iter().find(|v| v.name == variant_name) else {
            self.errors.push(
                TypeErrorKind::UnknownField {
                    ty: enum_def.name.clone(),
                    field: variant_name.to_string(),
                }
                .at(span),
            );
            return;
        };
        match &variant.kind {
            VariantKind::Tuple(payload_types) => {
                if payload_types.len() != fields.len() {
                    self.errors.push(
                        TypeErrorKind::WrongArity {
                            expected: payload_types.len(),
                            found: fields.len(),
                        }
                        .at(span),
                    );
                    return;
                }
                for (field_pat, payload_ty) in fields.iter().zip(payload_types.iter()) {
                    let resolved = self.instantiate_generic_type(
                        payload_ty,
                        &enum_def.generic_params,
                        &type_args,
                    );
                    self.check_pattern(field_pat, &resolved);
                }
            }
            VariantKind::Unit => {
                if !fields.is_empty() {
                    self.errors.push(
                        TypeErrorKind::WrongArity {
                            expected: 0,
                            found: fields.len(),
                        }
                        .at(span),
                    );
                }
            }
            VariantKind::Struct(_) => {
                self.errors.push(
                    TypeErrorKind::Mismatch {
                        expected: "struct pattern".to_string(),
                        found: "tuple pattern".to_string(),
                    }
                    .at(span),
                );
            }
        }
    }

    /// Checks a struct pattern (e.g., `Point { x, y }`) against the scrutinee.
    fn check_struct_pattern(
        &mut self,
        type_name: &str,
        fields: &[PatternField],
        scrutinee_ty: &Type,
        span: Span,
    ) {
        let struct_info = match scrutinee_ty {
            Type::Struct(name, type_args) if name.as_ref() == type_name => self
                .env
                .lookup_struct(name)
                .cloned()
                .map(|def| (def, type_args.clone())),
            _ => None,
        };
        let Some((struct_def, type_args)) = struct_info else {
            for field in fields {
                let name = field
                    .pattern
                    .as_ref()
                    .and_then(|p| match p.as_ref() {
                        Pattern::Ident(n, _) => Some(n.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| field.name.clone());
                let fresh = self.env.fresh_var();
                self.env.define_var(&name, fresh);
            }
            return;
        };
        for pf in fields {
            match struct_def
                .fields
                .iter()
                .find(|(fname, _)| fname == &pf.name)
            {
                Some((_, field_ty)) => {
                    let resolved = self.instantiate_generic_type(
                        field_ty,
                        &struct_def.generic_params,
                        &type_args,
                    );
                    match &pf.pattern {
                        Some(sub_pat) => self.check_pattern(sub_pat, &resolved),
                        None => self.env.define_var(&pf.name, resolved),
                    }
                }
                None => {
                    self.errors.push(
                        TypeErrorKind::UnknownField {
                            ty: struct_def.name.clone(),
                            field: pf.name.clone(),
                        }
                        .at(span),
                    );
                }
            }
        }
    }

    /// Resolves a bare identifier as an enum variant constructor.
    ///
    /// Enables prelude-style usage: `Some(x)`, `None`, `Ok(v)`, `Err(e)`
    /// without requiring qualified paths like `Option::Some(x)`.
    fn resolve_bare_variant(&mut self, name: &str) -> Option<Type> {
        let (enum_def, type_args) = self.find_enum_for_variant(name)?;
        let variant = enum_def.variants.iter().find(|v| v.name == name)?;
        let enum_name: Rc<str> = Rc::from(enum_def.name.as_str());
        match &variant.kind {
            VariantKind::Unit => Some(Type::Enum(
                enum_name,
                type_args.iter().map(|t| self.subst.apply(t)).collect(),
            )),
            VariantKind::Tuple(field_types) => {
                let params = field_types
                    .iter()
                    .map(|t| self.instantiate_generic_type(t, &enum_def.generic_params, &type_args))
                    .collect();
                let ret = Type::Enum(
                    enum_name,
                    type_args.iter().map(|t| self.subst.apply(t)).collect(),
                );
                Some(Type::Function(FnType {
                    params,
                    ret: Box::new(ret),
                }))
            }
            VariantKind::Struct(_) => Some(Type::Enum(
                enum_name,
                type_args.iter().map(|t| self.subst.apply(t)).collect(),
            )),
        }
    }

    /// Searches all registered enums for a variant with the given name.
    fn find_enum_for_variant(&mut self, variant_name: &str) -> Option<(EnumDef, Vec<Type>)> {
        let def = self
            .env
            .all_enums()
            .find(|def| def.variants.iter().any(|v| v.name == variant_name))
            .cloned()?;
        let type_args = def
            .generic_params
            .iter()
            .map(|_| self.env.fresh_var())
            .collect();
        Some((def, type_args))
    }

    /// Substitutes generic type parameters with concrete type arguments.
    fn instantiate_generic_type(
        &self,
        ty: &Type,
        generic_params: &[String],
        type_args: &[Type],
    ) -> Type {
        match ty {
            Type::Generic(name, _) => generic_params
                .iter()
                .position(|g| g.as_str() == name.as_ref())
                .and_then(|i| type_args.get(i))
                .cloned()
                .unwrap_or_else(|| ty.clone()),
            Type::Vector(elem) => Type::Vector(Box::new(self.instantiate_generic_type(
                elem,
                generic_params,
                type_args,
            ))),
            Type::Range(elem) => Type::Range(Box::new(self.instantiate_generic_type(
                elem,
                generic_params,
                type_args,
            ))),
            Type::Map(k, v) => Type::Map(
                Box::new(self.instantiate_generic_type(k, generic_params, type_args)),
                Box::new(self.instantiate_generic_type(v, generic_params, type_args)),
            ),
            Type::Function(fn_ty) => Type::Function(FnType {
                params: fn_ty
                    .params
                    .iter()
                    .map(|p| self.instantiate_generic_type(p, generic_params, type_args))
                    .collect(),
                ret: Box::new(self.instantiate_generic_type(&fn_ty.ret, generic_params, type_args)),
            }),
            Type::Struct(name, args) => Type::Struct(
                name.clone(),
                args.iter()
                    .map(|a| self.instantiate_generic_type(a, generic_params, type_args))
                    .collect(),
            ),
            Type::Enum(name, args) => Type::Enum(
                name.clone(),
                args.iter()
                    .map(|a| self.instantiate_generic_type(a, generic_params, type_args))
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    /// Checks that a `match` expression is exhaustive.
    ///
    /// For enum types, verifies that all variants are covered (or a wildcard/
    /// catch-all pattern is present). For non-enum types, requires a wildcard
    /// or ident catch-all.
    fn check_exhaustiveness(&mut self, scrutinee_ty: &Type, match_expr: &MatchExpr) {
        let enum_def = match scrutinee_ty {
            Type::Enum(name, _) => self.env.lookup_enum(name).cloned(),
            _ => None,
        };
        let has_wildcard = match_expr
            .arms
            .iter()
            .any(|arm| arm.guard.is_none() && self.pattern_is_catch_all(&arm.pattern, &enum_def));
        if has_wildcard {
            return;
        }
        match scrutinee_ty {
            Type::Enum(_, _) => {
                if let Some(def) = &enum_def {
                    let covered = match_expr
                        .arms
                        .iter()
                        .filter_map(|arm| self.extract_variant_name(&arm.pattern, def))
                        .collect::<std::collections::HashSet<&str>>();
                    let missing = def
                        .variants
                        .iter()
                        .map(|v| v.name.as_str())
                        .filter(|name| !covered.contains(name))
                        .collect::<Vec<&str>>();
                    if !missing.is_empty() {
                        self.errors.push(
                            TypeErrorKind::NonExhaustiveMatch {
                                missing: missing.join(", "),
                            }
                            .at(match_expr.span),
                        );
                    }
                }
            }
            Type::Bool => {
                let has_true = match_expr.arms.iter().any(|arm| {
                    matches!(&arm.pattern, Pattern::Literal(e) if matches!(e.as_ref(), Expr::Bool(Bool { value: true, .. })))
                });
                let has_false = match_expr.arms.iter().any(|arm| {
                    matches!(&arm.pattern, Pattern::Literal(e) if matches!(e.as_ref(), Expr::Bool(Bool { value: false, .. })))
                });
                if !has_true || !has_false {
                    self.errors.push(
                        TypeErrorKind::NonExhaustiveMatch {
                            missing: "not all boolean values covered".to_string(),
                        }
                        .at(match_expr.span),
                    );
                }
            }
            _ => {
                // For non-enum, non-bool types, a catch-all is required.
                self.errors.push(
                    TypeErrorKind::NonExhaustiveMatch {
                        missing: "missing wildcard `_` or catch-all pattern".to_string(),
                    }
                    .at(match_expr.span),
                );
            }
        }
    }

    /// Returns `true` if the pattern catches all values unconditionally.
    ///
    /// An ident pattern is a catch-all unless it matches a known enum variant
    /// name in the scrutinee's enum type.
    fn pattern_is_catch_all(&self, p: &Pattern, enum_def: &Option<EnumDef>) -> bool {
        match p {
            Pattern::Wildcard(_) => true,
            Pattern::Ident(name, _) => !enum_def
                .as_ref()
                .is_some_and(|def| def.variants.iter().any(|v| v.name == *name)),
            _ => false,
        }
    }

    /// Extracts the variant name from a pattern if it is a constructor pattern.
    fn extract_variant_name<'a>(&self, p: &'a Pattern, enum_def: &EnumDef) -> Option<&'a str> {
        match p {
            Pattern::TupleStruct { path, .. } => Some(path.as_str()),
            Pattern::Struct { path, .. } => Some(path.as_str()),
            Pattern::Ident(name, _) => {
                // Check if this identifier is actually a unit variant name.
                if enum_def.variants.iter().any(|v| v.name == *name) {
                    Some(name.as_str())
                } else {
                    None
                }
            }
            Pattern::Or(pats, _) => pats
                .iter()
                .find_map(|p| self.extract_variant_name(p, enum_def)),
            _ => None,
        }
    }

    /// Checks an infix expression, inserting promotion casts if needed.
    fn check_infix(&mut self, infix: &mut InfixExpr) -> Type {
        if infix.operator == "&&" || infix.operator == "||" {
            let lhs_ty = self.infer_expression(&mut infix.lhs);
            let lhs_resolved = self.subst.apply(&lhs_ty);
            self.require_bool(&lhs_resolved, infix.lhs.span());

            let rhs_ty = self.infer_expression(&mut infix.rhs);
            let rhs_resolved = self.subst.apply(&rhs_ty);
            self.require_bool(&rhs_resolved, infix.rhs.span());

            return Type::Bool;
        }

        let lhs_ty = self.infer_expression(&mut infix.lhs);
        let rhs_ty = self.infer_expression(&mut infix.rhs);
        let lhs_resolved = self.subst.apply(&lhs_ty);
        let rhs_resolved = self.subst.apply(&rhs_ty);

        let is_comparison = matches!(
            infix.operator.as_str(),
            "<" | ">" | "<=" | ">=" | "==" | "!="
        );
        // String concatenation
        if infix.operator == "+" && lhs_resolved == Type::String && rhs_resolved == Type::String {
            return Type::String;
        }
        // Boolean equality
        if (infix.operator == "==" || infix.operator == "!=")
            && lhs_resolved == Type::Bool
            && rhs_resolved == Type::Bool
        {
            return Type::Bool;
        }
        // Numeric operations
        if lhs_resolved.is_integer() && rhs_resolved.is_integer() {
            if lhs_resolved == rhs_resolved {
                return if is_comparison {
                    Type::Bool
                } else {
                    lhs_resolved
                };
            }
            match promote::common_numeric_type(&lhs_resolved, &rhs_resolved) {
                Ok(promoted) => {
                    self.maybe_insert_cast(&mut infix.lhs, &lhs_resolved, &promoted);
                    self.maybe_insert_cast(&mut infix.rhs, &rhs_resolved, &promoted);
                    if is_comparison { Type::Bool } else { promoted }
                }
                Err(PromotionError::NonNumeric(ty)) => {
                    self.errors.push(
                        TypeErrorKind::Mismatch {
                            expected: "numeric".to_string(),
                            found: ty.to_string(),
                        }
                        .at(infix.span),
                    );
                    self.env.fresh_var()
                }
            }
        } else if matches!(lhs_resolved, Type::Var(_)) || matches!(rhs_resolved, Type::Var(_)) {
            if let Err(e) = self.subst.unify(&lhs_resolved, &rhs_resolved) {
                self.report_unify_error(e, infix.span);
            }
            if is_comparison {
                Type::Bool
            } else {
                self.subst.apply(&lhs_resolved)
            }
        } else {
            if let Err(e) = self.subst.unify(&lhs_resolved, &rhs_resolved) {
                self.report_unify_error(e, infix.span);
            }
            if is_comparison {
                Type::Bool
            } else {
                lhs_resolved
            }
        }
    }

    /// Wraps an expression in a `Cast` node if it needs promotion.
    fn maybe_insert_cast(&self, expr: &mut Box<Expr>, current: &Type, target: &Type) {
        if current == target {
            return;
        }
        if let Some(ann) = target.to_number_kind() {
            let span = expr.span();
            let inner = std::mem::replace(
                expr.as_mut(),
                Expr::Bool(Bool {
                    value: false,
                    span: Span::ZERO,
                }),
            );
            *expr.as_mut() = Expr::Cast(CastExpr {
                expr: Box::new(inner),
                target: ann,
                span,
            });
        }
    }

    /// Infers the type of a block (the type of its last expression statement).
    fn infer_block(&mut self, block: &mut BlockStmt) -> Type {
        let mut last = Type::Null;
        for stmt in &mut block.statements {
            match stmt {
                Stmt::Expr(es) => {
                    last = self.infer_expression(&mut es.value);
                }
                Stmt::Return(ret) => {
                    self.infer_expression(&mut ret.value);
                    last = Type::Never;
                }
                _ => {
                    self.check_statement(stmt);
                    last = Type::Null;
                }
            }
        }
        last
    }

    /// Checks that a literal value fits within the declared type's range.
    fn check_literal_range(&mut self, expr: &Expr, expected: &Type, span: Span) {
        let Some(val) = expr.extract_integer_value() else {
            return;
        };

        macro_rules! check_int_range {
            ($target:ty, $target_name:expr) => {
                if <$target>::try_from(val).is_err() {
                    self.errors.push(
                        TypeErrorKind::NumericOverflow {
                            value: val.to_string(),
                            target: $target_name.to_string(),
                        }
                        .at(span),
                    );
                }
            };
        }

        match expected {
            Type::I8 => check_int_range!(i8, "i8"),
            Type::I16 => check_int_range!(i16, "i16"),
            Type::I32 => check_int_range!(i32, "i32"),
            Type::I64 => check_int_range!(i64, "i64"),
            Type::I128 => {}
            Type::Isize => check_int_range!(i64, "isize"),
            Type::U8 => check_int_range!(u8, "u8"),
            Type::U16 => check_int_range!(u16, "u16"),
            Type::U32 => check_int_range!(u32, "u32"),
            Type::U64 => check_int_range!(u64, "u64"),
            Type::U128 => check_int_range!(u128, "u128"),
            Type::Usize => check_int_range!(u64, "usize"),
            _ => {}
        }
    }

    /// Ensures a resolved type is `Bool`, reporting a mismatch if not.
    fn require_bool(&mut self, resolved: &Type, span: Span) {
        if !matches!(resolved, Type::Bool | Type::Var(_)) {
            self.errors.push(
                TypeErrorKind::Mismatch {
                    expected: "bool".to_string(),
                    found: resolved.to_string(),
                }
                .at(span),
            );
        }
        if let Type::Var(_) = resolved
            && let Err(e) = self.subst.unify(resolved, &Type::Bool)
        {
            self.report_unify_error(e, span);
        }
    }

    /// Maps a resolved type to the dispatch prefix used in builtin qualified names.
    ///
    /// Returns `Some("Vector")` for vector types, `Some("str")` for strings,
    /// `Some("Map")` for map types, `Some("Set")` for set types, and
    /// `Some(name)` for user-defined structs/enums. Returns `None` for
    /// unresolved type variables or primitive types that have no inherent methods.
    fn receiver_type_name(ty: &Type) -> Option<String> {
        match ty {
            Type::Vector(_) => Some("Vector".to_string()),
            Type::String => Some("str".to_string()),
            Type::Map(..) => Some("Map".to_string()),
            Type::Set(_) => Some("Set".to_string()),
            Type::Struct(name, _) | Type::Enum(name, _) => Some(name.to_string()),
            _ => None,
        }
    }

    fn report_unify_error(&mut self, err: UnifyError, span: Span) {
        let kind = match err {
            UnifyError::Mismatch(a, b) => TypeErrorKind::Mismatch {
                expected: a.to_string(),
                found: b.to_string(),
            },
            UnifyError::OccursCheck(id, ty) => {
                TypeErrorKind::OccursCheck(format!("?T{id} in {ty}"))
            }
        };
        self.errors.push(kind.at(span));
    }
}
