//! Constant folding pass over the AST.
//!
//! Replaces `Infix(Literal, op, Literal)` nodes with a single literal
//! when both operands are compile-time constants. Uses checked arithmetic;
//! overflow is reported as a compile error.

use maat_errors::{TypeError, TypeErrorKind};

use super::*;

/// Folds constant expressions in a program.
///
/// Walks the AST in post-order and replaces binary operations on two
/// literal operands with the computed result. Returns any overflow errors.
pub fn fold_constants(program: &mut Program) -> Vec<TypeError> {
    let mut errors = Vec::new();
    for stmt in &mut program.statements {
        fold_statement(stmt, &mut errors);
    }
    errors
}

fn fold_statement(stmt: &mut Stmt, errors: &mut Vec<TypeError>) {
    match stmt {
        Stmt::Let(let_stmt) => fold_expression(&mut let_stmt.value, errors),
        Stmt::ReAssign(assign_stmt) => fold_expression(&mut assign_stmt.value, errors),
        Stmt::Return(ret_stmt) => fold_expression(&mut ret_stmt.value, errors),
        Stmt::Expr(expr_stmt) => fold_expression(&mut expr_stmt.value, errors),
        Stmt::Block(block) => fold_block(block, errors),
        Stmt::FuncDef(fn_item) => fold_block(&mut fn_item.body, errors),
        Stmt::Loop(loop_stmt) => fold_block(&mut loop_stmt.body, errors),
        Stmt::While(while_stmt) => {
            fold_expression(&mut while_stmt.condition, errors);
            fold_block(&mut while_stmt.body, errors);
        }
        Stmt::For(for_stmt) => {
            fold_expression(&mut for_stmt.iterable, errors);
            fold_block(&mut for_stmt.body, errors);
        }
        Stmt::StructDecl(_) | Stmt::EnumDecl(_) | Stmt::Use(_) => {}
        Stmt::TraitDecl(decl) => {
            for method in &mut decl.methods {
                if let Some(body) = &mut method.default_body {
                    fold_block(body, errors);
                }
            }
        }
        Stmt::ImplBlock(impl_block) => {
            for method in &mut impl_block.methods {
                fold_block(&mut method.body, errors);
            }
        }
        Stmt::Mod(mod_stmt) => {
            if let Some(body) = &mut mod_stmt.body {
                for stmt in body {
                    fold_statement(stmt, errors);
                }
            }
        }
    }
}

fn fold_block(block: &mut BlockStmt, errors: &mut Vec<TypeError>) {
    for stmt in &mut block.statements {
        fold_statement(stmt, errors);
    }
}

fn fold_expression(expr: &mut Expr, errors: &mut Vec<TypeError>) {
    // Post-order: fold children first so `1 + 2 + 3` cascades.
    match expr {
        Expr::Prefix(prefix) => {
            fold_expression(&mut prefix.operand, errors);
            if let Some(folded) = try_fold_prefix(prefix, errors) {
                *expr = folded;
            }
        }
        Expr::Infix(infix) => {
            fold_expression(&mut infix.lhs, errors);
            fold_expression(&mut infix.rhs, errors);
            if let Some(folded) = try_fold_infix(infix, errors) {
                *expr = folded;
            }
        }
        Expr::Array(array) => {
            for elem in &mut array.elements {
                fold_expression(elem, errors);
            }
        }
        Expr::Map(map) => {
            for (k, v) in &mut map.pairs {
                fold_expression(k, errors);
                fold_expression(v, errors);
            }
        }
        Expr::Index(idx) => {
            fold_expression(&mut idx.expr, errors);
            fold_expression(&mut idx.index, errors);
        }
        Expr::Cond(cond) => {
            fold_expression(&mut cond.condition, errors);
            fold_block(&mut cond.consequence, errors);
            if let Some(alt) = &mut cond.alternative {
                fold_block(alt, errors);
            }
        }
        Expr::Lambda(lambda) => fold_block(&mut lambda.body, errors),
        Expr::Macro(m) => fold_block(&mut m.body, errors),
        Expr::Call(call) => {
            fold_expression(&mut call.function, errors);
            for arg in &mut call.arguments {
                fold_expression(arg, errors);
            }
        }
        Expr::Cast(cast) => fold_expression(&mut cast.expr, errors),
        Expr::Break(break_expr) => {
            if let Some(val) = &mut break_expr.value {
                fold_expression(val, errors);
            }
        }
        Expr::Match(match_expr) => {
            fold_expression(&mut match_expr.scrutinee, errors);
            for arm in &mut match_expr.arms {
                fold_expression(&mut arm.body, errors);
            }
        }
        Expr::FieldAccess(fa) => fold_expression(&mut fa.object, errors),
        Expr::MethodCall(mc) => {
            fold_expression(&mut mc.object, errors);
            for arg in &mut mc.arguments {
                fold_expression(arg, errors);
            }
        }
        Expr::StructLit(sl) => {
            for (_, val) in &mut sl.fields {
                fold_expression(val, errors);
            }
        }
        _ => {}
    }
}

fn try_fold_prefix(prefix: &PrefixExpr, errors: &mut Vec<TypeError>) -> Option<Expr> {
    let span = prefix.span;

    match prefix.operator.as_str() {
        "-" => {
            macro_rules! negate_int {
                ($lit:expr, $variant:ident, $name:expr) => {{
                    match $lit.value.checked_neg() {
                        Some(v) => Some(Expr::$variant($variant {
                            radix: $lit.radix,
                            value: v,
                            span,
                        })),
                        None => {
                            errors.push(
                                TypeErrorKind::NumericOverflow {
                                    value: format!("-{}", $lit.value),
                                    target: $name.to_string(),
                                }
                                .at(span),
                            );
                            None
                        }
                    }
                }};
            }

            match prefix.operand.as_ref() {
                Expr::I8(lit) => negate_int!(lit, I8, "i8"),
                Expr::I16(lit) => negate_int!(lit, I16, "i16"),
                Expr::I32(lit) => negate_int!(lit, I32, "i32"),
                Expr::I64(lit) => negate_int!(lit, I64, "i64"),
                Expr::I128(lit) => negate_int!(lit, I128, "i128"),
                Expr::Isize(lit) => negate_int!(lit, Isize, "isize"),
                _ => None,
            }
        }
        "!" => match prefix.operand.as_ref() {
            Expr::Bool(b) => Some(Expr::Bool(Bool {
                value: !b.value,
                span,
            })),
            _ => None,
        },
        _ => None,
    }
}

fn try_fold_infix(infix: &InfixExpr, errors: &mut Vec<TypeError>) -> Option<Expr> {
    let span = infix.span;
    let op = infix.operator.as_str();

    macro_rules! fold_int_op {
        ($lhs:expr, $rhs:expr, $variant:ident, $name:expr) => {{
            let result = match op {
                "+" => $lhs.value.checked_add($rhs.value),
                "-" => $lhs.value.checked_sub($rhs.value),
                "*" => $lhs.value.checked_mul($rhs.value),
                "/" => $lhs.value.checked_div($rhs.value),
                "%" => $lhs.value.checked_rem_euclid($rhs.value),
                _ => return try_fold_comparison(infix),
            };
            match result {
                Some(v) => Some(Expr::$variant($variant {
                    radix: Radix::Dec,
                    value: v,
                    span,
                })),
                None => {
                    errors.push(
                        TypeErrorKind::NumericOverflow {
                            value: format!("{} {} {}", $lhs.value, op, $rhs.value),
                            target: $name.to_string(),
                        }
                        .at(span),
                    );
                    None
                }
            }
        }};
    }

    match (infix.lhs.as_ref(), infix.rhs.as_ref()) {
        (Expr::I8(l), Expr::I8(r)) => fold_int_op!(l, r, I8, "i8"),
        (Expr::I16(l), Expr::I16(r)) => fold_int_op!(l, r, I16, "i16"),
        (Expr::I32(l), Expr::I32(r)) => fold_int_op!(l, r, I32, "i32"),
        (Expr::I64(l), Expr::I64(r)) => fold_int_op!(l, r, I64, "i64"),
        (Expr::I128(l), Expr::I128(r)) => fold_int_op!(l, r, I128, "i128"),
        (Expr::Isize(l), Expr::Isize(r)) => fold_int_op!(l, r, Isize, "isize"),
        (Expr::U8(l), Expr::U8(r)) => fold_int_op!(l, r, U8, "u8"),
        (Expr::U16(l), Expr::U16(r)) => fold_int_op!(l, r, U16, "u16"),
        (Expr::U32(l), Expr::U32(r)) => fold_int_op!(l, r, U32, "u32"),
        (Expr::U64(l), Expr::U64(r)) => fold_int_op!(l, r, U64, "u64"),
        (Expr::U128(l), Expr::U128(r)) => fold_int_op!(l, r, U128, "u128"),
        (Expr::Usize(l), Expr::Usize(r)) => fold_int_op!(l, r, Usize, "usize"),

        (Expr::Bool(l), Expr::Bool(r)) => match op {
            "==" => Some(Expr::Bool(Bool {
                value: l.value == r.value,
                span,
            })),
            "!=" => Some(Expr::Bool(Bool {
                value: l.value != r.value,
                span,
            })),
            _ => None,
        },

        _ => try_fold_comparison(infix),
    }
}

fn try_fold_comparison(infix: &InfixExpr) -> Option<Expr> {
    let span = infix.span;
    let op = infix.operator.as_str();

    macro_rules! fold_cmp {
        ($lhs:expr, $rhs:expr) => {
            match op {
                "==" => Some(Expr::Bool(Bool {
                    value: $lhs.value == $rhs.value,
                    span,
                })),
                "!=" => Some(Expr::Bool(Bool {
                    value: $lhs.value != $rhs.value,
                    span,
                })),
                "<" => Some(Expr::Bool(Bool {
                    value: $lhs.value < $rhs.value,
                    span,
                })),
                ">" => Some(Expr::Bool(Bool {
                    value: $lhs.value > $rhs.value,
                    span,
                })),
                "<=" => Some(Expr::Bool(Bool {
                    value: $lhs.value <= $rhs.value,
                    span,
                })),
                ">=" => Some(Expr::Bool(Bool {
                    value: $lhs.value >= $rhs.value,
                    span,
                })),
                _ => None,
            }
        };
    }

    match (infix.lhs.as_ref(), infix.rhs.as_ref()) {
        (Expr::I8(l), Expr::I8(r)) => fold_cmp!(l, r),
        (Expr::I16(l), Expr::I16(r)) => fold_cmp!(l, r),
        (Expr::I32(l), Expr::I32(r)) => fold_cmp!(l, r),
        (Expr::I64(l), Expr::I64(r)) => fold_cmp!(l, r),
        (Expr::I128(l), Expr::I128(r)) => fold_cmp!(l, r),
        (Expr::Isize(l), Expr::Isize(r)) => fold_cmp!(l, r),
        (Expr::U8(l), Expr::U8(r)) => fold_cmp!(l, r),
        (Expr::U16(l), Expr::U16(r)) => fold_cmp!(l, r),
        (Expr::U32(l), Expr::U32(r)) => fold_cmp!(l, r),
        (Expr::U64(l), Expr::U64(r)) => fold_cmp!(l, r),
        (Expr::U128(l), Expr::U128(r)) => fold_cmp!(l, r),
        (Expr::Usize(l), Expr::Usize(r)) => fold_cmp!(l, r),
        _ => None,
    }
}
