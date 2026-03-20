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
            if let Some(base) = &mut e.base {
                fold_expression(base, errors);
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
        "-" => match prefix.operand.as_ref() {
            Expr::Number(lit) if lit.kind.is_signed() => match lit.value.checked_neg() {
                Some(value) if lit.kind.fits(value) => Some(Expr::Number(Number {
                    kind: lit.kind,
                    value,
                    radix: lit.radix,
                    span,
                })),
                _ => {
                    errors.push(
                        TypeErrorKind::NumericOverflow {
                            value: format!("-{}", lit.value),
                            target: lit.kind.as_str().to_string(),
                        }
                        .at(span),
                    );
                    None
                }
            },
            _ => None,
        },
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

    match (infix.lhs.as_ref(), infix.rhs.as_ref()) {
        (Expr::Number(l), Expr::Number(r)) if l.kind == r.kind => {
            let result = match op {
                "+" => l.value.checked_add(r.value),
                "-" => l.value.checked_sub(r.value),
                "*" => l.value.checked_mul(r.value),
                "/" => l.value.checked_div(r.value),
                "%" => l.value.checked_rem_euclid(r.value),
                _ => return try_fold_comparison(infix),
            };
            match result {
                Some(value) if l.kind.fits(value) => Some(Expr::Number(Number {
                    kind: l.kind,
                    value,
                    radix: Radix::Dec,
                    span,
                })),
                _ => {
                    errors.push(
                        TypeErrorKind::NumericOverflow {
                            value: format!("{} {op} {}", l.value, r.value),
                            target: l.kind.as_str().to_string(),
                        }
                        .at(span),
                    );
                    None
                }
            }
        }

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

    match (infix.lhs.as_ref(), infix.rhs.as_ref()) {
        (Expr::Number(l), Expr::Number(r)) if l.kind == r.kind => {
            let value = match op {
                "==" => l.value == r.value,
                "!=" => l.value != r.value,
                "<" => l.value < r.value,
                ">" => l.value > r.value,
                "<=" => l.value <= r.value,
                ">=" => l.value >= r.value,
                _ => return None,
            };
            Some(Expr::Bool(Bool { value, span }))
        }
        _ => None,
    }
}
