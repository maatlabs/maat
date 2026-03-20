//! Implements a tree-walking interpreter that evaluates the AST nodes
//! into runtime objects. It supports integers, booleans, functions, conditionals,
//! and lexically-scoped environments.

use indexmap::IndexMap;
use maat_ast::*;
use maat_errors::{EvalError, Result};
use maat_runtime::{
    Env, FALSE, Function, Hashable, Macro, MapObject, NULL, Object, QUOTE, Quote, TRUE, UNQUOTE,
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
        Node::Stmt(stmt) => match stmt {
            Stmt::Let(ls) => {
                let obj = eval(Node::Expr(ls.value), env)?;
                env.set(ls.ident, &obj);
                Ok(obj)
            }
            Stmt::ReAssign(assign) => {
                let obj = eval(Node::Expr(assign.value), env)?;
                env.update(assign.ident, &obj);
                Ok(obj)
            }
            Stmt::Return(rs) => {
                let obj = eval(Node::Expr(rs.value), env)?;
                Ok(Object::ReturnValue(Box::new(obj)))
            }
            Stmt::Expr(es) => eval(Node::Expr(es.value), env),
            Stmt::Block(bs) => eval_block_statement(&bs, env),
            Stmt::FuncDef(fn_item) => {
                let obj = Object::Function(Function {
                    params: fn_item.param_names().map(String::from).collect(),
                    body: fn_item.body,
                    env: env.clone(),
                });
                env.set(fn_item.name, &obj);
                Ok(obj)
            }
            Stmt::Loop(loop_stmt) => eval_loop_statement(loop_stmt, env),
            Stmt::While(while_stmt) => eval_while_statement(while_stmt, env),
            Stmt::For(for_stmt) => eval_for_statement(for_stmt, env),
            Stmt::StructDecl(_)
            | Stmt::EnumDecl(_)
            | Stmt::TraitDecl(_)
            | Stmt::ImplBlock(_)
            | Stmt::Use(_)
            | Stmt::Mod(_) => Ok(NULL),
        },
        Node::Expr(expr) => match expr {
            Expr::Number(v) => Ok(match v.kind {
                NumberKind::I8 => Object::I8(v.value as i8),
                NumberKind::I16 => Object::I16(v.value as i16),
                NumberKind::I32 => Object::I32(v.value as i32),
                NumberKind::I64 => Object::I64(v.value as i64),
                NumberKind::I128 => Object::I128(v.value),
                NumberKind::Isize => Object::Isize(v.value as isize),
                NumberKind::U8 => Object::U8(v.value as u8),
                NumberKind::U16 => Object::U16(v.value as u16),
                NumberKind::U32 => Object::U32(v.value as u32),
                NumberKind::U64 => Object::U64(v.value as u64),
                NumberKind::U128 => Object::U128(v.value as u128),
                NumberKind::Usize => Object::Usize(v.value as usize),
            }),

            Expr::Bool(b) => Ok(Object::Bool(b.value)),
            Expr::Str(s) => Ok(Object::Str(maat_ast::unescape_string(&s.value))),
            Expr::Vector(vector) => {
                let elements = eval_expressions(&vector.elements, env)?;
                Ok(Object::Vector(elements))
            }
            Expr::Index(index_expr) => eval_index_expression(index_expr, env),
            Expr::Map(map) => eval_map_literal(map, env),
            Expr::Prefix(prefix_expr) => eval_prefix_expression(prefix_expr, env),
            Expr::Infix(infix_expr)
                if infix_expr.operator == "&&" || infix_expr.operator == "||" =>
            {
                eval_logical_expression(infix_expr, env)
            }
            Expr::Infix(infix_expr) => eval_infix_expression(infix_expr, env),
            Expr::Cond(cond_expr) => eval_conditional_expression(cond_expr, env),
            Expr::Ident(ident) => eval_identifier(ident.value, env),
            Expr::Lambda(lambda) => Ok(Object::Function(Function {
                params: lambda.param_names().map(String::from).collect(),
                body: lambda.body,
                env: env.clone(),
            })),
            Expr::Macro(macro_lit) => Ok(Object::Macro(Macro {
                params: macro_lit.params,
                body: macro_lit.body,
                env: env.clone(),
            })),
            Expr::Break(break_expr) => {
                let value = break_expr
                    .value
                    .map(|v| eval(Node::Expr(*v), env))
                    .transpose()?
                    .unwrap_or(NULL);
                Ok(Object::Break(Box::new(value)))
            }
            Expr::Continue(_) => Ok(Object::Continue),
            Expr::Cast(cast_expr) => eval_cast_expression(cast_expr, env),
            Expr::Range(range) => {
                let start = eval(Node::Expr(*range.start), env)?;
                let end = eval(Node::Expr(*range.end), env)?;
                match (start, end) {
                    (Object::I64(s), Object::I64(e)) => {
                        if range.inclusive {
                            Ok(Object::RangeInclusive(s, e))
                        } else {
                            Ok(Object::Range(s, e))
                        }
                    }
                    _ => Err(EvalError::Builtin("range bounds must be i64".to_string()).into()),
                }
            }
            Expr::Match(_)
            | Expr::FieldAccess(_)
            | Expr::MethodCall(_)
            | Expr::StructLit(_)
            | Expr::PathExpr(_) => Err(EvalError::Builtin(
                "custom type expressions are not yet supported in the tree-walking interpreter"
                    .to_string(),
            )
            .into()),
            Expr::Call(call_expr) => {
                // Handle special `quote` builtin
                if let Expr::Ident(ref ident) = *call_expr.function
                    && ident.value == QUOTE
                {
                    if call_expr.arguments.len() != 1 {
                        return Err(EvalError::Builtin(format!(
                            "{QUOTE} expects exactly 1 argument"
                        ))
                        .into());
                    }
                    let node = Node::Expr(call_expr.arguments[0].clone());
                    let node = eval_unquote_calls(node, env);
                    return Ok(Object::Quote(Box::new(Quote { node })));
                }
                eval_function_call(call_expr, env)
            }
        },
    }
}

fn eval_program(prog: Program, env: &Env) -> Result<Object> {
    let mut result = NULL;
    for stmt in &prog.statements {
        result = eval(Node::Stmt(stmt.clone()), env)?;
        match result {
            // Unwrap early returns at program level.
            Object::ReturnValue(val) => return Ok(*val),
            // Break/Continue outside a loop is a semantic error.
            Object::Break(_) | Object::Continue => {
                return Err(
                    EvalError::Ident("break/continue outside of a loop".to_string()).into(),
                );
            }
            _ => {}
        }
    }
    Ok(result)
}

pub(crate) fn eval_block_statement(block: &BlockStmt, env: &Env) -> Result<Object> {
    let block_env = Env::new_enclosed(env);
    let mut result = NULL;
    for stmt in &block.statements {
        result = eval(Node::Stmt(stmt.clone()), &block_env)?;
        // Propagate control flow signals up to the enclosing loop or function.
        if matches!(
            result,
            Object::ReturnValue(_) | Object::Break(_) | Object::Continue
        ) {
            return Ok(result);
        }
    }
    Ok(result)
}

fn eval_loop_statement(stmt: LoopStmt, env: &Env) -> Result<Object> {
    loop {
        let result = eval_block_statement(&stmt.body, env)?;
        match result {
            Object::Break(val) => return Ok(*val),
            Object::ReturnValue(_) => return Ok(result),
            Object::Continue => continue,
            _ => {}
        }
    }
}

fn eval_while_statement(stmt: WhileStmt, env: &Env) -> Result<Object> {
    loop {
        let condition = eval(Node::Expr(*stmt.condition.clone()), env)?;
        if !condition.is_truthy() {
            break;
        }
        let result = eval_block_statement(&stmt.body, env)?;
        match result {
            Object::Break(val) => return Ok(*val),
            Object::ReturnValue(_) => return Ok(result),
            Object::Continue => continue,
            _ => {}
        }
    }
    Ok(NULL)
}

fn eval_for_statement(stmt: ForStmt, env: &Env) -> Result<Object> {
    let iterable = eval(Node::Expr(*stmt.iterable), env)?;
    let elements = match iterable {
        Object::Vector(elems) => elems,
        other => {
            return Err(EvalError::Ident(format!(
                "for..in requires a vector, got {}",
                other.type_name()
            ))
            .into());
        }
    };
    let loop_env = Env::new_enclosed(env);
    for elem in elements {
        loop_env.set(stmt.ident.clone(), &elem);
        let result = eval_block_statement(&stmt.body, &loop_env)?;
        match result {
            Object::Break(val) => return Ok(*val),
            Object::ReturnValue(_) => return Ok(result),
            Object::Continue => continue,
            _ => {}
        }
    }
    Ok(NULL)
}

/// Evaluates an expression in the given environment.
///
/// This is a convenience wrapper around `eval` for macro system use.
pub(crate) fn eval_expression(expr: &Expr, env: &Env) -> Result<Object> {
    eval(Node::Expr(expr.clone()), env)
}

fn eval_expressions(exprs: &[Expr], env: &Env) -> Result<Vec<Object>> {
    let mut result = Vec::new();
    for expr in exprs {
        let evaluated = eval(Node::Expr(expr.to_owned()), env)?;
        result.push(evaluated);
    }
    Ok(result)
}

fn eval_index_expression(idx_expr: IndexExpr, env: &Env) -> Result<Object> {
    let expr = eval(Node::Expr(*idx_expr.expr), env)?;
    let expr_type = expr.type_name();
    let index = eval(Node::Expr(*idx_expr.index), env)?;

    match expr {
        Object::Vector(arr) => {
            if index.is_integer() {
                match index.to_vector_index() {
                    Some(idx) if idx < arr.len() => Ok(arr[idx].clone()),
                    _ => Ok(NULL),
                }
            } else {
                Err(EvalError::IndexExpr(format!(
                    "vector index must be an integer, got {}",
                    index.type_name()
                ))
                .into())
            }
        }
        Object::Map(map) => {
            let key_hash = Hashable::try_from(index)?;
            Ok(map.pairs.get(&key_hash).cloned().unwrap_or(NULL))
        }
        _ => Err(
            EvalError::IndexExpr(format!("index expression not supported for {expr_type}")).into(),
        ),
    }
}

fn eval_map_literal(expr: Map, env: &Env) -> Result<Object> {
    let mut pairs = IndexMap::new();
    for (key_expr, val_expr) in &expr.pairs {
        let key = eval(Node::Expr(key_expr.clone()), env)?;
        let key = Hashable::try_from(key)?;
        let value = eval(Node::Expr(val_expr.clone()), env)?;
        pairs.insert(key, value);
    }
    Ok(Object::Map(MapObject { pairs }))
}

fn eval_prefix_expression(expr: PrefixExpr, env: &Env) -> Result<Object> {
    let operand = eval(Node::Expr(*expr.operand), env)?;
    let op = &expr.operator;

    match op.as_str() {
        "!" => match operand {
            obj if !obj.is_truthy() => Ok(TRUE),
            _ => Ok(FALSE),
        },
        "-" => {
            match operand {
                Object::I8(v) => v.checked_neg().map(Object::I8).ok_or_else(|| {
                    EvalError::PrefixExpr(format!("negation overflow: -{v}")).into()
                }),
                Object::I16(v) => v.checked_neg().map(Object::I16).ok_or_else(|| {
                    EvalError::PrefixExpr(format!("negation overflow: -{v}")).into()
                }),
                Object::I32(v) => v.checked_neg().map(Object::I32).ok_or_else(|| {
                    EvalError::PrefixExpr(format!("negation overflow: -{v}")).into()
                }),
                Object::I64(v) => v.checked_neg().map(Object::I64).ok_or_else(|| {
                    EvalError::PrefixExpr(format!("negation overflow: -{v}")).into()
                }),
                Object::I128(v) => v.checked_neg().map(Object::I128).ok_or_else(|| {
                    EvalError::PrefixExpr(format!("negation overflow: -{v}")).into()
                }),
                Object::Isize(v) => v.checked_neg().map(Object::Isize).ok_or_else(|| {
                    EvalError::PrefixExpr(format!("negation overflow: -{v}")).into()
                }),
                Object::U8(_)
                | Object::U16(_)
                | Object::U32(_)
                | Object::U64(_)
                | Object::U128(_)
                | Object::Usize(_) => Err(EvalError::PrefixExpr(format!(
                    "{} cannot be negated (unsigned type)",
                    operand.type_name()
                ))
                .into()),

                _ => Err(EvalError::PrefixExpr(format!(
                    "{} cannot be negated",
                    operand.type_name()
                ))
                .into()),
            }
        }
        _ => {
            Err(EvalError::PrefixExpr(format!("invalid prefix expression: `{op}{operand}`")).into())
        }
    }
}

/// Evaluates short-circuit logical operators `&&` and `||`.
///
/// `&&` returns the left operand if falsy, otherwise the right operand.
/// `||` returns the left operand if truthy, otherwise the right operand.
fn eval_logical_expression(expr: InfixExpr, env: &Env) -> Result<Object> {
    let lhs = eval(Node::Expr(*expr.lhs), env)?;
    let Object::Bool(left_val) = &lhs else {
        return Err(EvalError::InfixExpr(format!(
            "expected bool in `{}` expression, got `{lhs}`",
            expr.operator
        ))
        .into());
    };
    match expr.operator.as_str() {
        "&&" => {
            if !left_val {
                return Ok(Object::Bool(false));
            }
            eval(Node::Expr(*expr.rhs), env)
        }
        "||" => {
            if *left_val {
                return Ok(Object::Bool(true));
            }
            eval(Node::Expr(*expr.rhs), env)
        }
        _ => unreachable!(),
    }
}

fn eval_infix_expression(expr: InfixExpr, env: &Env) -> Result<Object> {
    let lhs = eval(Node::Expr(*expr.lhs), env)?;
    let rhs = eval(Node::Expr(*expr.rhs), env)?;
    let op = &expr.operator;

    match (&lhs, &rhs) {
        (Object::I8(l), Object::I8(r)) => eval_infix_op(op, *l, *r),
        (Object::I16(l), Object::I16(r)) => eval_infix_op(op, *l, *r),
        (Object::I32(l), Object::I32(r)) => eval_infix_op(op, *l, *r),
        (Object::I64(l), Object::I64(r)) => eval_infix_op(op, *l, *r),
        (Object::I128(l), Object::I128(r)) => eval_infix_op(op, *l, *r),
        (Object::Isize(l), Object::Isize(r)) => eval_infix_op(op, *l, *r),
        (Object::U8(l), Object::U8(r)) => eval_infix_op(op, *l, *r),
        (Object::U16(l), Object::U16(r)) => eval_infix_op(op, *l, *r),
        (Object::U32(l), Object::U32(r)) => eval_infix_op(op, *l, *r),
        (Object::U64(l), Object::U64(r)) => eval_infix_op(op, *l, *r),
        (Object::U128(l), Object::U128(r)) => eval_infix_op(op, *l, *r),
        (Object::Usize(l), Object::Usize(r)) => eval_infix_op(op, *l, *r),
        (Object::Bool(l), Object::Bool(r)) => eval_infix_bool(op, *l, *r),
        (Object::Str(l), Object::Str(r)) => eval_infix_string(op, l, r),
        _ => Err(
            EvalError::InfixExpr(format!("invalid infix expression: `{lhs} {op} {rhs}`")).into(),
        ),
    }
}

fn eval_infix_op<T: InfixOp>(op: &str, lhs: T, rhs: T) -> Result<Object> {
    match op {
        "+" => lhs
            .checked_add(rhs)
            .map(|v| v.into_object())
            .ok_or_else(|| {
                EvalError::Number(format!(
                    "arithmetic overflow: {lhs} + {rhs} exceeds {} bounds",
                    T::type_name()
                ))
                .into()
            }),
        "-" => lhs
            .checked_sub(rhs)
            .map(|v| v.into_object())
            .ok_or_else(|| {
                EvalError::Number(format!(
                    "arithmetic overflow: {lhs} - {rhs} exceeds {} bounds",
                    T::type_name()
                ))
                .into()
            }),
        "*" => lhs
            .checked_mul(rhs)
            .map(|v| v.into_object())
            .ok_or_else(|| {
                EvalError::Number(format!(
                    "arithmetic overflow: {lhs} * {rhs} exceeds {} bounds",
                    T::type_name()
                ))
                .into()
            }),
        "/" => lhs
            .checked_div(rhs)
            .map(|v| v.into_object())
            .ok_or_else(|| {
                EvalError::Number(format!(
                    "division error: {lhs} / {rhs} (division by zero or overflow)"
                ))
                .into()
            }),
        "%" => lhs
            .checked_rem_euclid(rhs)
            .map(|v| v.into_object())
            .ok_or_else(|| {
                EvalError::Number(format!(
                    "modulo error: {lhs} % {rhs} (division by zero or overflow)"
                ))
                .into()
            }),
        "<" => Ok(Object::Bool(lhs < rhs)),
        ">" => Ok(Object::Bool(lhs > rhs)),
        "<=" => Ok(Object::Bool(lhs <= rhs)),
        ">=" => Ok(Object::Bool(lhs >= rhs)),
        "==" => Ok(Object::Bool(lhs == rhs)),
        "!=" => Ok(Object::Bool(lhs != rhs)),
        _ => Err(EvalError::Number(format!(
            "invalid {} operation: `{lhs} {op} {rhs}`",
            T::type_name()
        ))
        .into()),
    }
}

fn eval_infix_bool(op: &str, lhs: bool, rhs: bool) -> Result<Object> {
    match op {
        "==" => Ok(Object::Bool(lhs == rhs)),
        "!=" => Ok(Object::Bool(lhs != rhs)),
        "&&" => Ok(Object::Bool(lhs && rhs)),
        "||" => Ok(Object::Bool(lhs || rhs)),
        _ => {
            Err(EvalError::Boolean(format!("invalid boolean operation: `{lhs} {op} {rhs}`")).into())
        }
    }
}

fn eval_infix_string(op: &str, lhs: &str, rhs: &str) -> Result<Object> {
    if op != "+" {
        return Err(
            EvalError::InfixExpr(format!("invalid concat operation: `{lhs} {op} {rhs}`")).into(),
        );
    }
    Ok(Object::Str(format!("{lhs}{rhs}")))
}

fn eval_conditional_expression(expr: CondExpr, env: &Env) -> Result<Object> {
    let condition = eval(Node::Expr(*expr.condition), env)?;
    if condition.is_truthy() {
        eval(Node::Stmt(Stmt::Block(expr.consequence)), env)
    } else if let Some(alt) = expr.alternative {
        eval(Node::Stmt(Stmt::Block(alt)), env)
    } else {
        Ok(NULL)
    }
}

fn eval_identifier(ident: String, env: &Env) -> Result<Object> {
    match env.get(&ident) {
        Some(obj) => Ok(obj.clone()),
        None => match get_builtin(&ident) {
            Some(func) => Ok(Object::Builtin(func)),
            None => Err(EvalError::Ident(format!("unknown identifier: {ident}")).into()),
        },
    }
}

fn eval_function_call(expr: CallExpr, env: &Env) -> Result<Object> {
    let object = eval(Node::Expr(*expr.function), env)?;
    let expressions = eval_expressions(&expr.arguments, env)?;

    match object {
        Object::Function(func) => {
            let env = Env::new_enclosed(&func.env);
            func.params.iter().enumerate().for_each(|(i, param)| {
                env.set(param.to_owned(), &expressions[i]);
            });
            let evaluated = eval(Node::Stmt(Stmt::Block(func.body)), &env)?;
            match evaluated {
                Object::ReturnValue(val) => Ok(*val),
                Object::Break(_) | Object::Continue => {
                    Err(EvalError::Ident("break/continue outside of a loop".to_string()).into())
                }
                other => Ok(other),
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
        if let Node::Expr(Expr::Call(call)) = &node {
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

    let value = eval(Node::Expr(*expr.expr), env)?;
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
    fn checked_rem_euclid(self, rhs: Self) -> Option<Self>;
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

                #[inline]
                fn checked_rem_euclid(self, rhs: Self) -> Option<Self> {
                    <$t>::checked_rem_euclid(self, rhs)
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

/// Checks if a node is a call to the `unquote` builtin.
fn is_unquote_call(node: &Node) -> bool {
    if let Node::Expr(Expr::Call(call)) = node
        && let Expr::Ident(ident) = &*call.function
    {
        return ident.value == UNQUOTE;
    }
    false
}
