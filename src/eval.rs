//! Evaluation engine.
//!
//! This module implements a tree-walking interpreter that evaluates the AST nodes
//! into runtime objects. It supports integers, booleans, functions, conditionals,
//! and lexically-scoped environments.

mod builtins;
mod env;
mod object;

use std::collections::HashMap;

use builtins::get_builtin;
pub use env::Env;
pub use object::{BuiltinFn, FALSE, Function, HashObject, Hashable, NULL, Object, TRUE};

use crate::ast::*;
use crate::{EvalError, Result};

/// Evaluates an AST node in the given environment.
///
/// This is the main entry point for the interpreter. It recursively traverses
/// the AST, evaluating expressions and executing statements, producing runtime
/// objects as results.
///
/// # Examples
///
/// ```
/// use maat::{Lexer, Parser, Env};
/// use maat::{eval, Object};
/// use maat::ast::Node;
///
/// let input = "5 + 10";
/// let lexer = Lexer::new(input);
/// let mut parser = Parser::new(lexer);
/// let program = parser.parse_program();
/// let env = Env::default();
///
/// let result = eval(Node::Program(program), &env).unwrap();
/// assert_eq!(result, Object::I64(15));
/// ```
pub fn eval(node: Node, env: &Env) -> Result<Object> {
    match node {
        Node::Program(prog) => eval_program(prog, env),

        Node::Statement(stmt) => match stmt {
            Statement::Let(ls) => {
                let obj = eval(Node::Expression(ls.value), env)?;
                env.set(ls.ident, &obj);
                Ok(obj)
            }
            Statement::Return(rs) => {
                let obj = eval(Node::Expression(rs.value), env)?;
                Ok(Object::ReturnValue(Box::new(obj)))
            }
            Statement::Expression(es) => eval(Node::Expression(es.value), env),
            Statement::Block(bs) => eval_block_statement(bs, env),
        },

        Node::Expression(expr) => match expr {
            Expression::I8(v) => Ok(Object::I8(v.value)),
            Expression::I16(v) => Ok(Object::I16(v.value)),
            Expression::I32(v) => Ok(Object::I32(v.value)),
            Expression::I64(v) => Ok(Object::I64(v.value)),
            Expression::I128(v) => Ok(Object::I128(v.value)),
            Expression::Isize(v) => Ok(Object::Isize(v.value)),

            Expression::U8(v) => Ok(Object::U8(v.value)),
            Expression::U16(v) => Ok(Object::U16(v.value)),
            Expression::U32(v) => Ok(Object::U32(v.value)),
            Expression::U64(v) => Ok(Object::U64(v.value)),
            Expression::U128(v) => Ok(Object::U128(v.value)),
            Expression::Usize(v) => Ok(Object::Usize(v.value)),

            Expression::F32(v) => Ok(Object::F32(v.into())),
            Expression::F64(v) => Ok(Object::F64(v.into())),

            Expression::Boolean(boolean) => Ok(Object::Boolean(boolean)),
            Expression::String(string) => Ok(Object::String(string)),
            Expression::Array(array_lit) => {
                let elements = eval_expressions(&array_lit.elements, env)?;
                Ok(Object::Array(elements))
            }
            Expression::Index(index_expr) => eval_index_expression(index_expr, env),
            Expression::Hash(hash_literal) => eval_hash_literal(hash_literal, env),
            Expression::Prefix(prefix_expr) => eval_prefix_expression(prefix_expr, env),
            Expression::Infix(infix_expr) => eval_infix_expression(infix_expr, env),
            Expression::Conditional(cond_expr) => eval_conditional_expression(cond_expr, env),
            Expression::Identifier(ident) => eval_identifier(ident, env),
            Expression::Function(func_lit) => Ok(Object::Function(Function {
                params: func_lit.params,
                body: func_lit.body,
                env: env.clone(),
            })),
            Expression::Call(call_expr) => eval_function_call(call_expr, env),
        },
    }
}

fn eval_program(prog: Program, env: &Env) -> Result<Object> {
    let mut result = NULL;

    for stmt in &prog.statements {
        result = eval(Node::Statement(stmt.clone()), env)?;
        // handle early return statements, "unwrapping" the inner value
        // and terminating the program.
        if let Object::ReturnValue(val) = result {
            return Ok(*val);
        }
    }
    Ok(result)
}

fn eval_block_statement(block: BlockStatement, env: &Env) -> Result<Object> {
    let mut result = NULL;

    for stmt in &block.statements {
        result = eval(Node::Statement(stmt.clone()), env)?;
        // handle early return statements by terminating the block,
        // not the entire program.
        if let Object::ReturnValue(_) = result {
            return Ok(result);
        }
    }
    Ok(result)
}

fn eval_expressions(exprs: &[Expression], env: &Env) -> Result<Vec<Object>> {
    let mut result = Vec::new();

    for expr in exprs {
        let evaluated = eval(Node::Expression(expr.to_owned()), env)?;
        result.push(evaluated);
    }
    Ok(result)
}

fn eval_index_expression(idx_expr: IndexExpr, env: &Env) -> Result<Object> {
    let expr = eval(Node::Expression(*idx_expr.expr), env)?;
    let expr_type = expr.type_name();
    let index = eval(Node::Expression(*idx_expr.index), env)?;

    match expr {
        Object::Array(arr) => {
            let idx = match index {
                Object::Usize(v) => v,
                _ => {
                    return Err(EvalError::IndexExpression(format!(
                        "array index must be of type Usize, got {}",
                        index.type_name()
                    ))
                    .into());
                }
            };

            if arr.is_empty() || idx >= arr.len() {
                return Ok(NULL);
            }
            Ok(arr[idx].clone())
        }
        Object::Hash(hash) => {
            let key_hash = Hashable::try_from(index)?;
            Ok(hash.pairs.get(&key_hash).cloned().unwrap_or(NULL))
        }
        _ => Err(EvalError::IndexExpression(format!(
            "index expression not supported for {expr_type}"
        ))
        .into()),
    }
}

fn eval_hash_literal(expr: HashLiteral, env: &Env) -> Result<Object> {
    let mut pairs = HashMap::new();

    for (key_expr, val_expr) in &expr.pairs {
        let key = eval(Node::Expression(key_expr.clone()), env)?;
        let key = Hashable::try_from(key)?;
        let value = eval(Node::Expression(val_expr.clone()), env)?;
        pairs.insert(key, value);
    }

    Ok(Object::Hash(HashObject { pairs }))
}

fn eval_prefix_expression(expr: PrefixExpr, env: &Env) -> Result<Object> {
    let operand = eval(Node::Expression(*expr.operand), env)?;
    let operator = &expr.operator;

    match operator.as_str() {
        "!" => match operand {
            obj if !is_truthy(&obj) => Ok(TRUE),
            _ => Ok(FALSE),
        },
        "-" => match operand {
            Object::I8(v) => v.checked_neg().map(Object::I8).ok_or_else(|| {
                EvalError::PrefixExpression(format!("negation overflow: -{v}")).into()
            }),
            Object::I16(v) => v.checked_neg().map(Object::I16).ok_or_else(|| {
                EvalError::PrefixExpression(format!("negation overflow: -{v}")).into()
            }),
            Object::I32(v) => v.checked_neg().map(Object::I32).ok_or_else(|| {
                EvalError::PrefixExpression(format!("negation overflow: -{v}")).into()
            }),
            Object::I64(v) => v.checked_neg().map(Object::I64).ok_or_else(|| {
                EvalError::PrefixExpression(format!("negation overflow: -{v}")).into()
            }),
            Object::I128(v) => v.checked_neg().map(Object::I128).ok_or_else(|| {
                EvalError::PrefixExpression(format!("negation overflow: -{v}")).into()
            }),
            Object::Isize(v) => v.checked_neg().map(Object::Isize).ok_or_else(|| {
                EvalError::PrefixExpression(format!("negation overflow: -{v}")).into()
            }),
            Object::F32(v) => Ok(Object::F32(-v)),
            Object::F64(v) => Ok(Object::F64(-v)),

            Object::U8(_)
            | Object::U16(_)
            | Object::U32(_)
            | Object::U64(_)
            | Object::U128(_)
            | Object::Usize(_) => Err(EvalError::PrefixExpression(format!(
                "{} cannot be negated (unsigned type)",
                operand.type_name()
            ))
            .into()),

            _ => Err(EvalError::PrefixExpression(format!(
                "{} cannot be negated",
                operand.type_name()
            ))
            .into()),
        },
        _ => Err(EvalError::PrefixExpression(format!(
            "invalid prefix expression: `{operator}{operand}`"
        ))
        .into()),
    }
}

fn eval_infix_expression(expr: InfixExpr, env: &Env) -> Result<Object> {
    let lhs = eval(Node::Expression(*expr.lhs), env)?;
    let rhs = eval(Node::Expression(*expr.rhs), env)?;
    let operator = &expr.operator;

    match (&lhs, &rhs) {
        (Object::I8(left), Object::I8(right)) => eval_infix_op(operator, *left, *right),
        (Object::I16(left), Object::I16(right)) => eval_infix_op(operator, *left, *right),
        (Object::I32(left), Object::I32(right)) => eval_infix_op(operator, *left, *right),
        (Object::I64(left), Object::I64(right)) => eval_infix_op(operator, *left, *right),
        (Object::I128(left), Object::I128(right)) => eval_infix_op(operator, *left, *right),
        (Object::Isize(left), Object::Isize(right)) => eval_infix_op(operator, *left, *right),
        (Object::U8(left), Object::U8(right)) => eval_infix_op(operator, *left, *right),
        (Object::U16(left), Object::U16(right)) => eval_infix_op(operator, *left, *right),
        (Object::U32(left), Object::U32(right)) => eval_infix_op(operator, *left, *right),
        (Object::U64(left), Object::U64(right)) => eval_infix_op(operator, *left, *right),
        (Object::U128(left), Object::U128(right)) => eval_infix_op(operator, *left, *right),
        (Object::Usize(left), Object::Usize(right)) => eval_infix_op(operator, *left, *right),
        (Object::F32(left), Object::F32(right)) => eval_infix_op(operator, *left, *right),
        (Object::F64(left), Object::F64(right)) => eval_infix_op(operator, *left, *right),

        (Object::Boolean(left), Object::Boolean(right)) => eval_infix_bool(operator, *left, *right),
        (Object::String(left), Object::String(right)) => eval_infix_string(operator, left, right),

        _ => Err(EvalError::InfixExpression(format!(
            "invalid infix expression: `{lhs} {operator} {rhs}`"
        ))
        .into()),
    }
}

/// Trait for types that support infix numeric operations, both arithmetic and comparison.
///
/// Provides checked arithmetic operations to prevent overflow panics. Integer types return `None` on overflow,
/// while floating-point types allow infinity and NaN per IEEE 754 standard.
trait InfixOp: Copy + PartialOrd + PartialEq + core::fmt::Display {
    fn into_object(self) -> Object;
    fn type_name() -> &'static str;

    fn checked_add(self, rhs: Self) -> Option<Self>;
    fn checked_sub(self, rhs: Self) -> Option<Self>;
    fn checked_mul(self, rhs: Self) -> Option<Self>;
    fn checked_div(self, rhs: Self) -> Option<Self>;
}

macro_rules! impl_infix_op_int {
    ($($t:ty => $variant:ident, $name:literal),* $(,)?) => {
        $(
            impl InfixOp for $t {
                #[inline]
                fn into_object(self) -> Object {
                    Object::$variant(self)
                }

                #[inline]
                fn type_name() -> &'static str {
                    $name
                }

                #[inline]
                fn checked_add(self, rhs: Self) -> Option<Self> {
                    <$t>::checked_add(self, rhs)
                }

                #[inline]
                fn checked_sub(self, rhs: Self) -> Option<Self> {
                    <$t>::checked_sub(self, rhs)
                }

                #[inline]
                fn checked_mul(self, rhs: Self) -> Option<Self> {
                    <$t>::checked_mul(self, rhs)
                }

                #[inline]
                fn checked_div(self, rhs: Self) -> Option<Self> {
                    <$t>::checked_div(self, rhs)
                }
            }
        )*
    };
}

macro_rules! impl_infix_op_float {
    ($($t:ty => $variant:ident, $name:literal),* $(,)?) => {
        $(
            impl InfixOp for $t {
                #[inline]
                fn into_object(self) -> Object {
                    Object::$variant(self)
                }

                #[inline]
                fn type_name() -> &'static str {
                    $name
                }

                #[inline]
                fn checked_add(self, rhs: Self) -> Option<Self> {
                    Some(self + rhs)
                }

                #[inline]
                fn checked_sub(self, rhs: Self) -> Option<Self> {
                    Some(self - rhs)
                }

                #[inline]
                fn checked_mul(self, rhs: Self) -> Option<Self> {
                    Some(self * rhs)
                }

                #[inline]
                fn checked_div(self, rhs: Self) -> Option<Self> {
                    Some(self / rhs)
                }
            }
        )*
    };
}

impl_infix_op_int! {
    i8 => I8, "i8",
    i16 => I16, "i16",
    i32 => I32, "i32",
    i64 => I64, "i64",
    i128 => I128, "i128",
    isize => Isize, "isize",
    u8 => U8, "u8",
    u16 => U16, "u16",
    u32 => U32, "u32",
    u64 => U64, "u64",
    u128 => U128, "u128",
    usize => Usize, "usize",
}

impl_infix_op_float! {
    f32 => F32, "f32",
    f64 => F64, "f64",
}

fn eval_infix_op<T: InfixOp>(operator: &str, lhs: T, rhs: T) -> Result<Object> {
    match operator {
        "+" => lhs
            .checked_add(rhs)
            .map(|v| v.into_object())
            .ok_or_else(|| {
                EvalError::Number(format!(
                    "arithmetic overflow: {} + {} exceeds {} bounds",
                    lhs,
                    rhs,
                    T::type_name()
                ))
                .into()
            }),
        "-" => lhs
            .checked_sub(rhs)
            .map(|v| v.into_object())
            .ok_or_else(|| {
                EvalError::Number(format!(
                    "arithmetic overflow: {} - {} exceeds {} bounds",
                    lhs,
                    rhs,
                    T::type_name()
                ))
                .into()
            }),
        "*" => lhs
            .checked_mul(rhs)
            .map(|v| v.into_object())
            .ok_or_else(|| {
                EvalError::Number(format!(
                    "arithmetic overflow: {} * {} exceeds {} bounds",
                    lhs,
                    rhs,
                    T::type_name()
                ))
                .into()
            }),
        "/" => lhs
            .checked_div(rhs)
            .map(|v| v.into_object())
            .ok_or_else(|| {
                EvalError::Number(format!(
                    "division error: {} / {} (division by zero or overflow)",
                    lhs, rhs
                ))
                .into()
            }),

        "<" => Ok(Object::Boolean(lhs < rhs)),
        ">" => Ok(Object::Boolean(lhs > rhs)),
        "<=" => Ok(Object::Boolean(lhs <= rhs)),
        ">=" => Ok(Object::Boolean(lhs >= rhs)),
        "==" => Ok(Object::Boolean(lhs == rhs)),
        "!=" => Ok(Object::Boolean(lhs != rhs)),

        _ => Err(EvalError::Number(format!(
            "invalid {} operation: `{lhs} {operator} {rhs}`",
            T::type_name()
        ))
        .into()),
    }
}

fn eval_infix_bool(operator: &str, lhs: bool, rhs: bool) -> Result<Object> {
    match operator {
        "==" => Ok(Object::Boolean(lhs == rhs)),
        "!=" => Ok(Object::Boolean(lhs != rhs)),
        _ => Err(EvalError::Boolean(format!(
            "invalid boolean operation: `{lhs} {operator} {rhs}`"
        ))
        .into()),
    }
}

fn eval_infix_string(operator: &str, lhs: &str, rhs: &str) -> Result<Object> {
    if operator != "+" {
        return Err(EvalError::InfixExpression(format!(
            "invalid concat operation: `{lhs} {operator} {rhs}`"
        ))
        .into());
    }
    Ok(Object::String(format!("{lhs}{rhs}")))
}

fn eval_conditional_expression(expr: ConditionalExpr, env: &Env) -> Result<Object> {
    let condition = eval(Node::Expression(*expr.condition), env)?;

    if is_truthy(&condition) {
        eval(Node::Statement(Statement::Block(expr.consequence)), env)
    } else if let Some(alt) = expr.alternative {
        eval(Node::Statement(Statement::Block(alt)), env)
    } else {
        Ok(NULL)
    }
}

fn is_truthy(obj: &Object) -> bool {
    !(*obj == NULL || *obj == FALSE)
}

fn eval_identifier(ident: String, env: &Env) -> Result<Object> {
    match env.get(&ident) {
        Some(obj) => Ok(obj.clone()),
        None => match get_builtin(&ident) {
            Some(func) => Ok(Object::Builtin(func)),
            None => Err(EvalError::Identifier(format!("unknown identifier: {ident}")).into()),
        },
    }
}

fn eval_function_call(expr: CallExpr, env: &Env) -> Result<Object> {
    let function = eval(Node::Expression(*expr.function), env)?;
    let arguments = eval_expressions(&expr.arguments, env)?;
    apply_function(function, arguments)
}

fn apply_function(f: Object, args: Vec<Object>) -> Result<Object> {
    match f {
        Object::Function(func) => {
            let env = Env::new_enclosed(&func.env);

            func.params.iter().enumerate().for_each(|(i, param)| {
                env.set(param.to_owned(), &args[i]);
            });

            let evaluated = eval(Node::Statement(Statement::Block(func.body)), &env)?;

            if let Object::ReturnValue(val) = evaluated {
                Ok(*val)
            } else {
                Ok(evaluated)
            }
        }
        Object::Builtin(builtin_fn) => builtin_fn(&args),
        obj => Err(EvalError::NotAFunction(format!("expected {obj} to be a function")).into()),
    }
}
