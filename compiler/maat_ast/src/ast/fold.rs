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
        Stmt::Let(s) => fold_expression(&mut s.value, errors),
        Stmt::ReAssign(s) => fold_expression(&mut s.value, errors),
        Stmt::Return(s) => fold_expression(&mut s.value, errors),
        Stmt::Expr(s) => fold_expression(&mut s.value, errors),
        Stmt::Block(s) => fold_block(s, errors),
        Stmt::FuncDef(s) => fold_block(&mut s.body, errors),
        Stmt::Loop(s) => fold_block(&mut s.body, errors),
        Stmt::While(s) => {
            fold_expression(&mut s.condition, errors);
            fold_block(&mut s.body, errors);
        }
        Stmt::For(s) => {
            fold_expression(&mut s.iterable, errors);
            fold_block(&mut s.body, errors);
        }
        Stmt::StructDecl(_) | Stmt::EnumDecl(_) | Stmt::Use(_) => {}
        Stmt::TraitDecl(s) => {
            for method in &mut s.methods {
                if let Some(body) = &mut method.default_body {
                    fold_block(body, errors);
                }
            }
        }
        Stmt::ImplBlock(s) => {
            for method in &mut s.methods {
                fold_block(&mut method.body, errors);
            }
        }
        Stmt::Mod(s) => {
            if let Some(body) = &mut s.body {
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
        Expr::Prefix(e) => {
            fold_expression(&mut e.operand, errors);
            if let Some(folded) = try_fold_prefix(e, errors) {
                *expr = folded;
            }
        }
        Expr::Infix(e) => {
            fold_expression(&mut e.lhs, errors);
            fold_expression(&mut e.rhs, errors);
            if let Some(folded) = try_fold_infix(e, errors) {
                *expr = folded;
            }
        }
        Expr::Array(e) => {
            for elem in &mut e.elements {
                fold_expression(elem, errors);
            }
        }
        Expr::Map(e) => {
            for (k, v) in &mut e.pairs {
                fold_expression(k, errors);
                fold_expression(v, errors);
            }
        }
        Expr::Index(e) => {
            fold_expression(&mut e.expr, errors);
            fold_expression(&mut e.index, errors);
        }
        Expr::Cond(e) => {
            fold_expression(&mut e.condition, errors);
            fold_block(&mut e.consequence, errors);
            if let Some(alt) = &mut e.alternative {
                fold_block(alt, errors);
            }
        }
        Expr::Lambda(e) => fold_block(&mut e.body, errors),
        Expr::Macro(e) => fold_block(&mut e.body, errors),
        Expr::Call(e) => {
            fold_expression(&mut e.function, errors);
            for arg in &mut e.arguments {
                fold_expression(arg, errors);
            }
        }
        Expr::Cast(e) => fold_expression(&mut e.expr, errors),
        Expr::Break(e) => {
            if let Some(val) = &mut e.value {
                fold_expression(val, errors);
            }
        }
        Expr::Match(e) => {
            fold_expression(&mut e.scrutinee, errors);
            for arm in &mut e.arms {
                fold_expression(&mut arm.body, errors);
            }
        }
        Expr::FieldAccess(e) => fold_expression(&mut e.object, errors),
        Expr::MethodCall(e) => {
            fold_expression(&mut e.object, errors);
            for arg in &mut e.arguments {
                fold_expression(arg, errors);
            }
        }
        Expr::StructLit(e) => {
            for (_, val) in &mut e.fields {
                fold_expression(val, errors);
            }
        }
        Expr::Range(e) => {
            fold_expression(&mut e.start, errors);
            fold_expression(&mut e.end, errors);
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
                Expr::I8(v) => negate_int!(v, I8, "i8"),
                Expr::I16(v) => negate_int!(v, I16, "i16"),
                Expr::I32(v) => negate_int!(v, I32, "i32"),
                Expr::I64(v) => negate_int!(v, I64, "i64"),
                Expr::I128(v) => negate_int!(v, I128, "i128"),
                Expr::Isize(v) => negate_int!(v, Isize, "isize"),
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
                            value: format!("{} {op} {}", $lhs.value, $rhs.value),
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
