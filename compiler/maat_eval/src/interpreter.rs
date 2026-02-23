//! Implements a tree-walking interpreter that evaluates the AST nodes
//! into runtime objects. It supports integers, booleans, functions, conditionals,
//! and lexically-scoped environments.

use std::collections::HashMap;

use maat_ast::*;
use maat_errors::{EvalError, Result};
use maat_runtime::{
    Env, FALSE, Function, HashObject, Hashable, Macro, NULL, Object, QUOTE, Quote, TRUE, UNQUOTE,
    get_builtin,
};

/// Evaluates an AST node in the given environment.
///
/// This function performs pure tree-walking evaluation without macro processing.
///
/// # Examples
///
/// ```no_run
/// # use maat_eval::eval;
/// # use maat_runtime::Env;
/// # use maat_ast::Node;
/// # let program = maat_ast::Program { statements: vec![] };
/// let env = Env::default();
/// let result = eval(Node::Program(program), &env).unwrap();
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
            Statement::Block(bs) => eval_block_statement(&bs, env),
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

            Expression::Boolean(b) => Ok(Object::Boolean(b.value)),
            Expression::String(s) => Ok(Object::String(unescape_string(&s.value))),
            Expression::Array(array_lit) => {
                let elements = eval_expressions(&array_lit.elements, env)?;
                Ok(Object::Array(elements))
            }
            Expression::Index(index_expr) => eval_index_expression(index_expr, env),
            Expression::Hash(hash_literal) => eval_hash_literal(hash_literal, env),
            Expression::Prefix(prefix_expr) => eval_prefix_expression(prefix_expr, env),
            Expression::Infix(infix_expr) => eval_infix_expression(infix_expr, env),
            Expression::Conditional(cond_expr) => eval_conditional_expression(cond_expr, env),
            Expression::Identifier(ident) => eval_identifier(ident.value, env),
            Expression::Function(func_lit) => Ok(Object::Function(Function {
                params: func_lit.params,
                body: func_lit.body,
                env: env.clone(),
            })),
            Expression::Macro(macro_lit) => Ok(Object::Macro(Macro {
                params: macro_lit.params,
                body: macro_lit.body,
                env: env.clone(),
            })),
            Expression::Cast(cast_expr) => eval_cast_expression(cast_expr, env),
            Expression::Call(call_expr) => {
                // Handle special `quote` builtin
                if let Expression::Identifier(ref ident) = *call_expr.function
                    && ident.value == QUOTE
                {
                    if call_expr.arguments.len() != 1 {
                        return Err(EvalError::Builtin(format!(
                            "{QUOTE} expects exactly 1 argument"
                        ))
                        .into());
                    }
                    let node = Node::Expression(call_expr.arguments[0].clone());
                    let node = eval_unquote_calls(node, env);
                    return Ok(Object::Quote(Quote { node }));
                }
                eval_function_call(call_expr, env)
            }
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

pub(crate) fn eval_block_statement(block: &BlockStatement, env: &Env) -> Result<Object> {
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

/// Evaluates an expression in the given environment.
///
/// This is a convenience wrapper around `eval` for macro system use.
pub(crate) fn eval_expression(expr: &Expression, env: &Env) -> Result<Object> {
    eval(Node::Expression(expr.clone()), env)
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
            if index.is_integer() {
                match index.to_array_index() {
                    Some(idx) if idx < arr.len() => Ok(arr[idx].clone()),
                    _ => Ok(NULL),
                }
            } else {
                Err(EvalError::IndexExpression(format!(
                    "array index must be an integer, got {}",
                    index.type_name()
                ))
                .into())
            }
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
            obj if !obj.is_truthy() => Ok(TRUE),
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

    if condition.is_truthy() {
        eval(Node::Statement(Statement::Block(expr.consequence)), env)
    } else if let Some(alt) = expr.alternative {
        eval(Node::Statement(Statement::Block(alt)), env)
    } else {
        Ok(NULL)
    }
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
    let object = eval(Node::Expression(*expr.function), env)?;
    let expressions = eval_expressions(&expr.arguments, env)?;

    match object {
        Object::Function(func) => {
            let env = Env::new_enclosed(&func.env);
            func.params.iter().enumerate().for_each(|(i, param)| {
                env.set(param.to_owned(), &expressions[i]);
            });

            let evaluated = eval(Node::Statement(Statement::Block(func.body)), &env)?;
            if let Object::ReturnValue(val) = evaluated {
                Ok(*val)
            } else {
                Ok(evaluated)
            }
        }
        Object::Builtin(builtin_fn) => builtin_fn(&expressions),
        obj => Err(EvalError::NotAFunction(format!("expected {obj} to be a function")).into()),
    }
}

/// Evaluates `unquote` calls within a quoted AST node.
///
/// Traverses the AST and replaces `unquote(expr)` calls with their evaluated
/// results, enabling selective evaluation inside quoted expressions.
fn eval_unquote_calls(quoted: Node, env: &Env) -> Node {
    transform(quoted, &mut |node| {
        if !is_unquote_call(&node) {
            return node;
        }

        if let Node::Expression(Expression::Call(call)) = &node {
            if call.arguments.len() != 1 {
                return node;
            }

            let unquoted = match eval_expression(&call.arguments[0], env) {
                Ok(obj) => obj,
                Err(_) => return node,
            };

            match Object::to_ast_node(&unquoted) {
                Some(ast_node) => ast_node,
                None => node,
            }
        } else {
            node
        }
    })
}

fn eval_cast_expression(expr: CastExpr, env: &Env) -> Result<Object> {
    /// Widened integer for cast dispatch.
    enum WideInt {
        Signed(i128),
        Unsigned(u128),
    }

    let value = eval(Node::Expression(*expr.expr), env)?;
    let target = expr.target;

    let wide = match &value {
        Object::I8(v) => WideInt::Signed(*v as i128),
        Object::I16(v) => WideInt::Signed(*v as i128),
        Object::I32(v) => WideInt::Signed(*v as i128),
        Object::I64(v) => WideInt::Signed(*v as i128),
        Object::I128(v) => WideInt::Signed(*v),
        Object::Isize(v) => WideInt::Signed(*v as i128),

        Object::U8(v) => WideInt::Unsigned(*v as u128),
        Object::U16(v) => WideInt::Unsigned(*v as u128),
        Object::U32(v) => WideInt::Unsigned(*v as u128),
        Object::U64(v) => WideInt::Unsigned(*v as u128),
        Object::U128(v) => WideInt::Unsigned(*v),
        Object::Usize(v) => WideInt::Unsigned(*v as u128),

        Object::F32(v) => return eval_cast_from_float(*v as f64, target),
        Object::F64(v) => return eval_cast_from_float(*v, target),
        _ => {
            return Err(EvalError::Number(format!(
                "cannot cast {} to {}",
                value.type_name(),
                target.as_str()
            ))
            .into());
        }
    };

    macro_rules! narrow {
        ($target_ty:ty, $variant:ident, $name:expr) => {{
            match wide {
                WideInt::Signed(v) => {
                    <$target_ty>::try_from(v)
                        .map(Object::$variant)
                        .map_err(|_| {
                            EvalError::Number(format!("value {} out of range for {}", v, $name))
                                .into()
                        })
                }
                WideInt::Unsigned(v) => {
                    <$target_ty>::try_from(v)
                        .map(Object::$variant)
                        .map_err(|_| {
                            EvalError::Number(format!("value {} out of range for {}", v, $name))
                                .into()
                        })
                }
            }
        }};
    }

    match target {
        TypeAnnotation::I8 => narrow!(i8, I8, "i8"),
        TypeAnnotation::I16 => narrow!(i16, I16, "i16"),
        TypeAnnotation::I32 => narrow!(i32, I32, "i32"),
        TypeAnnotation::I64 => narrow!(i64, I64, "i64"),
        TypeAnnotation::I128 => narrow!(i128, I128, "i128"),
        TypeAnnotation::Isize => narrow!(isize, Isize, "isize"),

        TypeAnnotation::U8 => narrow!(u8, U8, "u8"),
        TypeAnnotation::U16 => narrow!(u16, U16, "u16"),
        TypeAnnotation::U32 => narrow!(u32, U32, "u32"),
        TypeAnnotation::U64 => narrow!(u64, U64, "u64"),
        TypeAnnotation::U128 => narrow!(u128, U128, "u128"),
        TypeAnnotation::Usize => narrow!(usize, Usize, "usize"),

        TypeAnnotation::F32 => Ok(Object::F32(match wide {
            WideInt::Signed(v) => v as f32,
            WideInt::Unsigned(v) => v as f32,
        })),
        TypeAnnotation::F64 => Ok(Object::F64(match wide {
            WideInt::Signed(v) => v as f64,
            WideInt::Unsigned(v) => v as f64,
        })),
    }
}

fn eval_cast_from_float(v: f64, target: TypeAnnotation) -> Result<Object> {
    if !v.is_finite() {
        return Err(EvalError::Number(format!(
            "cannot cast non-finite float to {}",
            target.as_str()
        ))
        .into());
    }

    macro_rules! float_to_int {
        ($target_ty:ty, $variant:ident, $name:expr) => {{
            let i = v as i128;
            <$target_ty>::try_from(i)
                .map(Object::$variant)
                .map_err(|_| {
                    EvalError::Number(format!("value {} out of range for {}", v, $name)).into()
                })
        }};
    }

    match target {
        TypeAnnotation::I8 => float_to_int!(i8, I8, "i8"),
        TypeAnnotation::I16 => float_to_int!(i16, I16, "i16"),
        TypeAnnotation::I32 => float_to_int!(i32, I32, "i32"),
        TypeAnnotation::I64 => float_to_int!(i64, I64, "i64"),
        TypeAnnotation::I128 => Ok(Object::I128(v as i128)),
        TypeAnnotation::Isize => float_to_int!(isize, Isize, "isize"),

        TypeAnnotation::U8 => float_to_int!(u8, U8, "u8"),
        TypeAnnotation::U16 => float_to_int!(u16, U16, "u16"),
        TypeAnnotation::U32 => float_to_int!(u32, U32, "u32"),
        TypeAnnotation::U64 => float_to_int!(u64, U64, "u64"),
        TypeAnnotation::U128 => {
            if v < 0.0 {
                return Err(EvalError::Number(format!("value {v} out of range for u128")).into());
            }
            Ok(Object::U128(v as u128))
        }
        TypeAnnotation::Usize => float_to_int!(usize, Usize, "usize"),

        TypeAnnotation::F32 => Ok(Object::F32(v as f32)),
        TypeAnnotation::F64 => Ok(Object::F64(v)),
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

/// Unescapes a string literal by processing escape sequences.
///
/// Supports standard escape sequences:
/// - `\\` → backslash
/// - `\"` → double quote
/// - `\n` → newline
/// - `\r` → carriage return
/// - `\t` → tab
/// - `\0` → null character
///
/// Invalid escape sequences are preserved as-is.
fn unescape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('0') => result.push('\0'),
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Checks if a node is a call to the `unquote` builtin.
fn is_unquote_call(node: &Node) -> bool {
    if let Node::Expression(Expression::Call(call)) = node
        && let Expression::Identifier(ident) = &*call.function
    {
        return ident.value == UNQUOTE;
    }
    false
}
