//! Constant folding pass over the AST.
//!
//! Replaces `Infix(Literal, op, Literal)` nodes with a single literal
//! when both operands are compile-time constants. Uses checked arithmetic;
//! overflow is reported as a compile error.

use maat_ast::*;
use maat_errors::{TypeError, TypeErrorKind};

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

fn fold_statement(stmt: &mut Statement, errors: &mut Vec<TypeError>) {
    match stmt {
        Statement::Let(let_stmt) => fold_expression(&mut let_stmt.value, errors),
        Statement::Return(ret_stmt) => fold_expression(&mut ret_stmt.value, errors),
        Statement::Expression(expr_stmt) => fold_expression(&mut expr_stmt.value, errors),
        Statement::Block(block) => fold_block(block, errors),
        Statement::Loop(loop_stmt) => fold_block(&mut loop_stmt.body, errors),
        Statement::While(while_stmt) => {
            fold_expression(&mut while_stmt.condition, errors);
            fold_block(&mut while_stmt.body, errors);
        }
        Statement::For(for_stmt) => {
            fold_expression(&mut for_stmt.iterable, errors);
            fold_block(&mut for_stmt.body, errors);
        }
    }
}

fn fold_block(block: &mut BlockStatement, errors: &mut Vec<TypeError>) {
    for stmt in &mut block.statements {
        fold_statement(stmt, errors);
    }
}

fn fold_expression(expr: &mut Expression, errors: &mut Vec<TypeError>) {
    // Post-order: fold children first so `1 + 2 + 3` cascades.
    match expr {
        Expression::Prefix(prefix) => {
            fold_expression(&mut prefix.operand, errors);
            if let Some(folded) = try_fold_prefix(prefix, errors) {
                *expr = folded;
            }
        }
        Expression::Infix(infix) => {
            fold_expression(&mut infix.lhs, errors);
            fold_expression(&mut infix.rhs, errors);
            if let Some(folded) = try_fold_infix(infix, errors) {
                *expr = folded;
            }
        }
        Expression::Array(array) => {
            for elem in &mut array.elements {
                fold_expression(elem, errors);
            }
        }
        Expression::Hash(hash) => {
            for (k, v) in &mut hash.pairs {
                fold_expression(k, errors);
                fold_expression(v, errors);
            }
        }
        Expression::Index(idx) => {
            fold_expression(&mut idx.expr, errors);
            fold_expression(&mut idx.index, errors);
        }
        Expression::Conditional(cond) => {
            fold_expression(&mut cond.condition, errors);
            fold_block(&mut cond.consequence, errors);
            if let Some(alt) = &mut cond.alternative {
                fold_block(alt, errors);
            }
        }
        Expression::Function(func) => fold_block(&mut func.body, errors),
        Expression::Macro(m) => fold_block(&mut m.body, errors),
        Expression::Call(call) => {
            fold_expression(&mut call.function, errors);
            for arg in &mut call.arguments {
                fold_expression(arg, errors);
            }
        }
        Expression::Cast(cast) => fold_expression(&mut cast.expr, errors),
        Expression::Break(break_expr) => {
            if let Some(val) = &mut break_expr.value {
                fold_expression(val, errors);
            }
        }
        _ => {}
    }
}

fn try_fold_prefix(prefix: &PrefixExpr, errors: &mut Vec<TypeError>) -> Option<Expression> {
    let span = prefix.span;

    match prefix.operator.as_str() {
        "-" => {
            macro_rules! negate_int {
                ($lit:expr, $variant:ident, $name:expr) => {{
                    match $lit.value.checked_neg() {
                        Some(v) => Some(Expression::$variant($variant {
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
                Expression::I8(lit) => negate_int!(lit, I8, "i8"),
                Expression::I16(lit) => negate_int!(lit, I16, "i16"),
                Expression::I32(lit) => negate_int!(lit, I32, "i32"),
                Expression::I64(lit) => negate_int!(lit, I64, "i64"),
                Expression::I128(lit) => negate_int!(lit, I128, "i128"),
                Expression::Isize(lit) => negate_int!(lit, Isize, "isize"),
                _ => None,
            }
        }
        "!" => match prefix.operand.as_ref() {
            Expression::Boolean(b) => Some(Expression::Boolean(BooleanLiteral {
                value: !b.value,
                span,
            })),
            _ => None,
        },
        _ => None,
    }
}

fn try_fold_infix(infix: &InfixExpr, errors: &mut Vec<TypeError>) -> Option<Expression> {
    let span = infix.span;
    let op = infix.operator.as_str();

    macro_rules! fold_int_op {
        ($lhs:expr, $rhs:expr, $variant:ident, $name:expr) => {{
            let result = match op {
                "+" => $lhs.value.checked_add($rhs.value),
                "-" => $lhs.value.checked_sub($rhs.value),
                "*" => $lhs.value.checked_mul($rhs.value),
                "/" => $lhs.value.checked_div($rhs.value),
                _ => return try_fold_comparison(infix),
            };
            match result {
                Some(v) => Some(Expression::$variant($variant {
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
        (Expression::I8(l), Expression::I8(r)) => fold_int_op!(l, r, I8, "i8"),
        (Expression::I16(l), Expression::I16(r)) => fold_int_op!(l, r, I16, "i16"),
        (Expression::I32(l), Expression::I32(r)) => fold_int_op!(l, r, I32, "i32"),
        (Expression::I64(l), Expression::I64(r)) => fold_int_op!(l, r, I64, "i64"),
        (Expression::I128(l), Expression::I128(r)) => fold_int_op!(l, r, I128, "i128"),
        (Expression::Isize(l), Expression::Isize(r)) => fold_int_op!(l, r, Isize, "isize"),
        (Expression::U8(l), Expression::U8(r)) => fold_int_op!(l, r, U8, "u8"),
        (Expression::U16(l), Expression::U16(r)) => fold_int_op!(l, r, U16, "u16"),
        (Expression::U32(l), Expression::U32(r)) => fold_int_op!(l, r, U32, "u32"),
        (Expression::U64(l), Expression::U64(r)) => fold_int_op!(l, r, U64, "u64"),
        (Expression::U128(l), Expression::U128(r)) => fold_int_op!(l, r, U128, "u128"),
        (Expression::Usize(l), Expression::Usize(r)) => fold_int_op!(l, r, Usize, "usize"),

        (Expression::Boolean(l), Expression::Boolean(r)) => match op {
            "==" => Some(Expression::Boolean(BooleanLiteral {
                value: l.value == r.value,
                span,
            })),
            "!=" => Some(Expression::Boolean(BooleanLiteral {
                value: l.value != r.value,
                span,
            })),
            _ => None,
        },

        _ => try_fold_comparison(infix),
    }
}

fn try_fold_comparison(infix: &InfixExpr) -> Option<Expression> {
    let span = infix.span;
    let op = infix.operator.as_str();

    macro_rules! fold_cmp {
        ($lhs:expr, $rhs:expr) => {
            match op {
                "==" => Some(Expression::Boolean(BooleanLiteral {
                    value: $lhs.value == $rhs.value,
                    span,
                })),
                "!=" => Some(Expression::Boolean(BooleanLiteral {
                    value: $lhs.value != $rhs.value,
                    span,
                })),
                "<" => Some(Expression::Boolean(BooleanLiteral {
                    value: $lhs.value < $rhs.value,
                    span,
                })),
                ">" => Some(Expression::Boolean(BooleanLiteral {
                    value: $lhs.value > $rhs.value,
                    span,
                })),
                "<=" => Some(Expression::Boolean(BooleanLiteral {
                    value: $lhs.value <= $rhs.value,
                    span,
                })),
                ">=" => Some(Expression::Boolean(BooleanLiteral {
                    value: $lhs.value >= $rhs.value,
                    span,
                })),
                _ => None,
            }
        };
    }

    match (infix.lhs.as_ref(), infix.rhs.as_ref()) {
        (Expression::I8(l), Expression::I8(r)) => fold_cmp!(l, r),
        (Expression::I16(l), Expression::I16(r)) => fold_cmp!(l, r),
        (Expression::I32(l), Expression::I32(r)) => fold_cmp!(l, r),
        (Expression::I64(l), Expression::I64(r)) => fold_cmp!(l, r),
        (Expression::I128(l), Expression::I128(r)) => fold_cmp!(l, r),
        (Expression::Isize(l), Expression::Isize(r)) => fold_cmp!(l, r),
        (Expression::U8(l), Expression::U8(r)) => fold_cmp!(l, r),
        (Expression::U16(l), Expression::U16(r)) => fold_cmp!(l, r),
        (Expression::U32(l), Expression::U32(r)) => fold_cmp!(l, r),
        (Expression::U64(l), Expression::U64(r)) => fold_cmp!(l, r),
        (Expression::U128(l), Expression::U128(r)) => fold_cmp!(l, r),
        (Expression::Usize(l), Expression::Usize(r)) => fold_cmp!(l, r),
        _ => None,
    }
}
