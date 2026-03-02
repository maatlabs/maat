//! Compile-time type checker for the AST.
//!
//! Walks the AST, infers types for all expressions, unifies constraints,
//! and reports errors. Inserts implicit numeric promotion casts where needed.

use maat_ast::*;
use maat_errors::{TypeError, TypeErrorKind};
use maat_span::Span;

use crate::convert::resolve_type_expr;
use crate::env::TypeEnv;
use crate::promote::{self, PromotionError};
use crate::ty::{FnType, Type, TypeScheme};
use crate::unify::{Substitution, UnifyError};

/// Registers builtin function signatures in the type environment.
///
/// Each builtin with type variables is stored as a generalized `TypeScheme`
/// so that each call site receives fresh inference variables.
///
/// `print` is variadic at runtime and is not registered
/// here. Unknown identifiers fall back to fresh type variables, which
/// allows any number of arguments without arity errors.
fn register_builtins(env: &mut TypeEnv) {
    /// Helper: creates a fresh type variable, builds the function type via
    /// the provided closure, then generalizes over the variable.
    fn register_generic_1(env: &mut TypeEnv, name: &str, build: impl FnOnce(Type) -> Type) {
        let var = env.fresh_var();
        let var_id = match var {
            Type::Var(id) => id,
            _ => unreachable!(),
        };
        let ty = build(var);
        env.define_scheme(
            name,
            TypeScheme {
                forall: vec![var_id],
                ty,
            },
        );
    }

    // len(collection) -> usize
    register_generic_1(env, "len", |t| {
        Type::Function(FnType {
            params: vec![t],
            ret: Box::new(Type::Usize),
        })
    });

    // first([T]) -> T
    register_generic_1(env, "first", |t| {
        Type::Function(FnType {
            params: vec![Type::Array(Box::new(t.clone()))],
            ret: Box::new(t),
        })
    });

    // last([T]) -> T
    register_generic_1(env, "last", |t| {
        Type::Function(FnType {
            params: vec![Type::Array(Box::new(t.clone()))],
            ret: Box::new(t),
        })
    });

    // rest([T]) -> [T]
    register_generic_1(env, "rest", |t| {
        Type::Function(FnType {
            params: vec![Type::Array(Box::new(t.clone()))],
            ret: Box::new(Type::Array(Box::new(t))),
        })
    });

    // push([T], T) -> [T]
    register_generic_1(env, "push", |t| {
        Type::Function(FnType {
            params: vec![Type::Array(Box::new(t.clone())), t.clone()],
            ret: Box::new(Type::Array(Box::new(t))),
        })
    });
}

/// The type checker.
///
/// Performs Hindley-Milner-style type inference with explicit annotations,
/// numeric promotion rules, and compile-time overflow checking.
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
        register_builtins(&mut env);
        Self {
            env,
            subst: Substitution::new(),
            errors: Vec::new(),
        }
    }

    /// Type-checks a program, mutating the AST to insert promotion casts.
    ///
    /// Returns accumulated type errors (empty if the program is well-typed).
    pub fn check_program(mut self, program: &mut Program) -> Vec<TypeError> {
        for stmt in &mut program.statements {
            self.check_statement(stmt);
        }
        self.errors
    }

    fn check_statement(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Let(let_stmt) => self.check_let(let_stmt),
            Stmt::Return(ret_stmt) => {
                self.infer_expression(&mut ret_stmt.value);
            }
            Stmt::Expr(expr_stmt) => {
                self.infer_expression(&mut expr_stmt.value);
            }
            Stmt::Block(block) => self.check_block(block),
            Stmt::FnItem(fn_item) => self.check_fn_item(fn_item),
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
                    Type::Array(elem) => *elem,
                    Type::Var(_) => self.env.fresh_var(),
                    _ => {
                        self.errors.push(
                            TypeErrorKind::Mismatch {
                                expected: "[T]".to_string(),
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
        }
    }

    fn check_let(&mut self, let_stmt: &mut LetStmt) {
        let inferred = self.infer_expression(&mut let_stmt.value);

        let ty = if let Some(ann) = &let_stmt.type_annotation {
            let expected = resolve_type_expr(ann);
            self.check_literal_range(&let_stmt.value, &expected, let_stmt.span);

            // Integer literals coerce to any numeric type whose range they fit.
            // E.g.: `let x: u8 = 5;` is valid because the unsuffixed
            // literal `5` adapts to the declared type. When a literal is in range,
            // we skip unification entirely. When it overflows, the range check has
            // already reported a precise error, so we also skip the redundant
            // type mismatch from unification.
            let is_coercible_literal = expected.is_integer()
                && inferred.is_integer()
                && let_stmt.value.is_integer_literal();

            if is_coercible_literal {
                // Rewrite the literal AST node to match the declared type so the
                // compiler emits the correctly-typed constant.
                coerce_literal(&mut let_stmt.value, &expected);
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
    ///
    /// The function type is generalized into a `TypeScheme` so that each
    /// call site receives a fresh instantiation (let-polymorphism).
    fn check_fn_item(&mut self, fn_item: &mut FnItem) {
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

    /// Shared inference logic for function bodies (used by both `FnItem` and `Lambda`).
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
                    .map(resolve_type_expr)
                    .unwrap_or_else(|| self.env.fresh_var());
                self.env.define_var(&p.name, ty.clone());
                ty
            })
            .collect();

        let body_ty = self.infer_block(body);

        let ret_ty = return_type
            .as_ref()
            .map(resolve_type_expr)
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

    /// Infers the type of an expression, potentially mutating it
    /// (e.g., inserting promotion casts on infix operands).
    fn infer_expression(&mut self, expr: &mut Expr) -> Type {
        match expr {
            Expr::I8(_) => Type::I8,
            Expr::I16(_) => Type::I16,
            Expr::I32(_) => Type::I32,
            Expr::I64(_) => Type::I64,
            Expr::I128(_) => Type::I128,
            Expr::Isize(_) => Type::Isize,
            Expr::U8(_) => Type::U8,
            Expr::U16(_) => Type::U16,
            Expr::U32(_) => Type::U32,
            Expr::U64(_) => Type::U64,
            Expr::U128(_) => Type::U128,
            Expr::Usize(_) => Type::Usize,
            Expr::Bool(_) => Type::Bool,
            Expr::Str(_) => Type::String,

            Expr::Ident(ident) => self
                .env
                .instantiate(&ident.value, &self.subst)
                .unwrap_or_else(|| {
                    // Don't error on unknown idents; the compiler will catch them.
                    // Return a fresh type variable to keep inference going.
                    self.env.fresh_var()
                }),

            Expr::Array(array) => {
                if array.elements.is_empty() {
                    let elem = self.env.fresh_var();
                    Type::Array(Box::new(elem))
                } else {
                    let first = self.infer_expression(&mut array.elements[0]);
                    for elem in &mut array.elements[1..] {
                        let elem_ty = self.infer_expression(elem);
                        if let Err(e) = self.subst.unify(&first, &elem_ty) {
                            self.report_unify_error(e, elem.span());
                        }
                    }
                    Type::Array(Box::new(self.subst.apply(&first)))
                }
            }

            Expr::Map(hash) => {
                if hash.pairs.is_empty() {
                    let k = self.env.fresh_var();
                    let v = self.env.fresh_var();
                    Type::Hash(Box::new(k), Box::new(v))
                } else {
                    let (first_k, first_v) = {
                        let k = self.infer_expression(&mut hash.pairs[0].0);
                        let v = self.infer_expression(&mut hash.pairs[0].1);
                        (k, v)
                    };
                    for (k_expr, v_expr) in &mut hash.pairs[1..] {
                        let k = self.infer_expression(k_expr);
                        let v = self.infer_expression(v_expr);
                        if let Err(e) = self.subst.unify(&first_k, &k) {
                            self.report_unify_error(e, k_expr.span());
                        }
                        if let Err(e) = self.subst.unify(&first_v, &v) {
                            self.report_unify_error(e, v_expr.span());
                        }
                    }
                    Type::Hash(
                        Box::new(self.subst.apply(&first_k)),
                        Box::new(self.subst.apply(&first_v)),
                    )
                }
            }

            Expr::Index(idx) => {
                let collection = self.infer_expression(&mut idx.expr);
                let _index_ty = self.infer_expression(&mut idx.index);
                let resolved = self.subst.apply(&collection);
                match resolved {
                    Type::Array(elem) => *elem,
                    Type::Hash(_, v) => *v,
                    _ => self.env.fresh_var(),
                }
            }

            Expr::Prefix(prefix) => {
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

            Expr::Infix(infix) => self.check_infix(infix),

            Expr::Cond(cond) => {
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

            Expr::Lambda(lambda) => self.infer_fn_body(
                &lambda.generic_params,
                &lambda.params,
                &lambda.return_type,
                &mut lambda.body,
                lambda.span,
            ),

            Expr::Call(call) => {
                let func_ty = self.infer_expression(&mut call.function);
                let resolved = self.subst.apply(&func_ty);

                let arg_types: Vec<Type> = call
                    .arguments
                    .iter_mut()
                    .map(|a| self.infer_expression(a))
                    .collect();

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
                                // Allow numeric promotion at call sites (e.g., passing
                                // i64 where usize is expected). The VM handles the
                                // conversion at runtime via OpConvert.
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

            Expr::Cast(cast) => {
                self.infer_expression(&mut cast.expr);
                let ann = &cast.target;
                type_annotation_to_type(ann)
            }

            Expr::Break(break_expr) => {
                if let Some(val) = &mut break_expr.value {
                    self.infer_expression(val);
                }
                Type::Never
            }

            Expr::Continue(_) => Type::Never,

            Expr::Macro(_) => self.env.fresh_var(),
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
            // If either side is a type variable, unify and return
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
        if let Some(ann) = target.to_annotation() {
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
    ///
    /// For type variables, unifies with `Bool` to propagate the constraint.
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

/// Converts a `TypeAnnotation` (for `as` casts) to an internal `Type`.
fn type_annotation_to_type(ann: &TypeAnnotation) -> Type {
    match ann {
        TypeAnnotation::I8 => Type::I8,
        TypeAnnotation::I16 => Type::I16,
        TypeAnnotation::I32 => Type::I32,
        TypeAnnotation::I64 => Type::I64,
        TypeAnnotation::I128 => Type::I128,
        TypeAnnotation::Isize => Type::Isize,
        TypeAnnotation::U8 => Type::U8,
        TypeAnnotation::U16 => Type::U16,
        TypeAnnotation::U32 => Type::U32,
        TypeAnnotation::U64 => Type::U64,
        TypeAnnotation::U128 => Type::U128,
        TypeAnnotation::Usize => Type::Usize,
    }
}

/// Rewrites a literal expression to match the target numeric type.
///
/// Called after range checking has confirmed the value fits. For negated
/// literals, the prefix is collapsed into a single signed literal node.
fn coerce_literal(expr: &mut Expr, target: &Type) {
    let Some(val) = expr.extract_integer_value() else {
        return;
    };
    let span = expr.span();

    macro_rules! rewrite {
        ($variant:ident, $ty:ty) => {
            *expr = Expr::$variant($variant {
                radix: Radix::Dec,
                value: val as $ty,
                span,
            })
        };
    }

    match target {
        Type::I8 => rewrite!(I8, i8),
        Type::I16 => rewrite!(I16, i16),
        Type::I32 => rewrite!(I32, i32),
        Type::I64 => rewrite!(I64, i64),
        Type::I128 => rewrite!(I128, i128),
        Type::Isize => rewrite!(Isize, isize),
        Type::U8 => rewrite!(U8, u8),
        Type::U16 => rewrite!(U16, u16),
        Type::U32 => rewrite!(U32, u32),
        Type::U64 => rewrite!(U64, u64),
        Type::U128 => rewrite!(U128, u128),
        Type::Usize => rewrite!(Usize, usize),
        _ => {}
    }
}
