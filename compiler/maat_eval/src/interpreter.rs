//! Implements a tree-walking interpreter that evaluates the AST nodes into runtime values.

use indexmap::IndexMap;
use maat_ast::*;
use maat_errors::{EvalError, Result};
use maat_runtime::{
    Env, FALSE, Felt, Function, Hashable, Integer, Macro, Map, Quote, TRUE, UNIT, Value, WideInt,
    get_builtin,
};

use crate::{QUOTE, UNQUOTE};

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
pub fn eval(node: Node, env: &Env) -> Result<Value> {
    match node {
        Node::Program(prog) => eval_program(prog, env),
        Node::Stmt(stmt) => match stmt {
            Stmt::Let(ls) => {
                let val = eval(Node::Expr(ls.value), env)?;
                env.set(ls.ident, &val);
                Ok(val)
            }
            Stmt::ReAssign(assign) => {
                let val = eval(Node::Expr(assign.value), env)?;
                env.update(assign.ident, &val);
                Ok(val)
            }
            Stmt::Return(rs) => {
                let val = eval(Node::Expr(rs.value), env)?;
                Ok(Value::ReturnValue(Box::new(val)))
            }
            Stmt::Expr(es) => eval(Node::Expr(es.value), env),
            Stmt::Block(bs) => eval_block_statement(&bs, env),
            Stmt::FuncDef(fn_item) => {
                let val = Value::Function(Function {
                    params: fn_item.param_names().map(String::from).collect(),
                    body: fn_item.body,
                    env: env.clone(),
                });
                env.set(fn_item.name, &val);
                Ok(val)
            }
            Stmt::Loop(loop_stmt) => eval_loop_statement(loop_stmt, env),
            Stmt::While(while_stmt) => eval_while_statement(while_stmt, env),
            Stmt::For(for_stmt) => eval_for_statement(for_stmt, env),
            Stmt::StructDecl(_)
            | Stmt::EnumDecl(_)
            | Stmt::TraitDecl(_)
            | Stmt::ImplBlock(_)
            | Stmt::Use(_)
            | Stmt::Mod(_) => Ok(UNIT),
        },
        Node::Expr(expr) => match expr {
            Expr::Number(v) => Ok(Value::from_number_literal(&v).map_err(EvalError::Number)?),
            Expr::Bool(b) => Ok(Value::Bool(b.value)),
            Expr::Str(s) => Ok(Value::Str(maat_ast::unescape_string(&s.value))),
            Expr::Char(c) => Ok(Value::Char(c.value)),
            Expr::Tuple(tuple) => {
                let elements = tuple
                    .elements
                    .iter()
                    .map(|e| eval(Node::Expr(e.clone()), env))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::Tuple(elements))
            }
            Expr::Vector(vector) => {
                let elements = eval_expressions(&vector.elements, env)?;
                Ok(Value::Vector(elements))
            }
            Expr::Array(arr) => {
                let elements = eval_expressions(&arr.elements, env)?;
                Ok(Value::Array(elements))
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
            Expr::Lambda(lambda) => Ok(Value::Function(Function {
                params: lambda.param_names().map(String::from).collect(),
                body: lambda.body,
                env: env.clone(),
            })),
            Expr::MacroLit(macro_lit) => Ok(Value::Macro(Macro {
                params: macro_lit.params,
                body: macro_lit.body,
                env: env.clone(),
            })),
            Expr::Break(break_expr) => {
                let value = break_expr
                    .value
                    .map(|v| eval(Node::Expr(*v), env))
                    .transpose()?
                    .unwrap_or(UNIT);
                Ok(Value::Break(Box::new(value)))
            }
            Expr::Continue(_) => Ok(Value::Continue),
            Expr::Cast(cast_expr) => eval_cast_expression(cast_expr, env),
            Expr::Range(range) => {
                let start = eval(Node::Expr(*range.start), env)?;
                let end = eval(Node::Expr(*range.end), env)?;
                match (start, end) {
                    (Value::Integer(s), Value::Integer(e)) => {
                        if range.inclusive {
                            Ok(Value::RangeInclusive(s, e))
                        } else {
                            Ok(Value::Range(s, e))
                        }
                    }
                    _ => {
                        Err(EvalError::Builtin("range bounds must be integers".to_string()).into())
                    }
                }
            }
            Expr::MacroCall(_)
            | Expr::Match(_)
            | Expr::Try(_)
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
                    return Ok(Value::Quote(Box::new(Quote { node })));
                }
                eval_function_call(call_expr, env)
            }
        },
    }
}

fn eval_program(prog: Program, env: &Env) -> Result<Value> {
    let mut result = UNIT;
    for stmt in &prog.statements {
        result = eval(Node::Stmt(stmt.clone()), env)?;
        match result {
            // Unwrap early returns at program level.
            Value::ReturnValue(val) => return Ok(*val),
            // Break/Continue outside a loop is a semantic error.
            Value::Break(_) | Value::Continue => {
                return Err(
                    EvalError::Ident("break/continue outside of a loop".to_string()).into(),
                );
            }
            _ => {}
        }
    }
    Ok(result)
}

pub fn eval_block_statement(block: &BlockStmt, env: &Env) -> Result<Value> {
    let block_env = Env::new_enclosed(env);
    let mut result = UNIT;
    for stmt in &block.statements {
        result = eval(Node::Stmt(stmt.clone()), &block_env)?;
        // Propagate control flow signals up to the enclosing loop or function.
        if matches!(
            result,
            Value::ReturnValue(_) | Value::Break(_) | Value::Continue
        ) {
            return Ok(result);
        }
    }
    Ok(result)
}

fn eval_loop_statement(stmt: LoopStmt, env: &Env) -> Result<Value> {
    let bound = stmt.bound;
    let mut counter = 0u64;
    loop {
        if counter >= bound {
            return Err(EvalError::BoundExceeded(bound).into());
        }
        let result = eval_block_statement(&stmt.body, env)?;
        match result {
            Value::Break(val) => return Ok(*val),
            Value::ReturnValue(_) => return Ok(result),
            Value::Continue => {
                counter += 1;
                continue;
            }
            _ => {}
        }
        counter += 1;
    }
}

fn eval_while_statement(stmt: WhileStmt, env: &Env) -> Result<Value> {
    let bound = stmt.bound;
    let mut counter = 0u64;
    loop {
        let condition = eval(Node::Expr(*stmt.condition.clone()), env)?;
        if !condition.is_truthy() {
            break;
        }
        if counter >= bound {
            return Err(EvalError::BoundExceeded(bound).into());
        }
        let result = eval_block_statement(&stmt.body, env)?;
        match result {
            Value::Break(val) => return Ok(*val),
            Value::ReturnValue(_) => return Ok(result),
            Value::Continue => {
                counter += 1;
                continue;
            }
            _ => {}
        }
        counter += 1;
    }
    Ok(UNIT)
}

fn eval_for_statement(stmt: ForStmt, env: &Env) -> Result<Value> {
    let iterable = eval(Node::Expr(*stmt.iterable), env)?;
    let elements = match iterable {
        Value::Vector(elems) | Value::Array(elems) => elems,
        other => {
            return Err(EvalError::Ident(format!(
                "for..in requires a vector or array, got {}",
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
            Value::Break(val) => return Ok(*val),
            Value::ReturnValue(_) => return Ok(result),
            Value::Continue => continue,
            _ => {}
        }
    }
    Ok(UNIT)
}

/// Evaluates an expression in the given environment.
///
/// This is a convenience wrapper around `eval` for macro system use.
fn eval_expression(expr: &Expr, env: &Env) -> Result<Value> {
    eval(Node::Expr(expr.clone()), env)
}

fn eval_expressions(exprs: &[Expr], env: &Env) -> Result<Vec<Value>> {
    let mut result = Vec::new();
    for expr in exprs {
        let evaluated = eval(Node::Expr(expr.to_owned()), env)?;
        result.push(evaluated);
    }
    Ok(result)
}

fn eval_index_expression(idx_expr: IndexExpr, env: &Env) -> Result<Value> {
    let expr = eval(Node::Expr(*idx_expr.expr), env)?;
    let expr_type = expr.type_name();
    let index = eval(Node::Expr(*idx_expr.index), env)?;

    match expr {
        Value::Vector(arr) | Value::Array(arr) => {
            if index.is_integer() {
                match index.to_vector_index() {
                    Some(idx) if idx < arr.len() => Ok(arr[idx].clone()),
                    _ => Ok(UNIT),
                }
            } else {
                Err(EvalError::IndexExpr(format!(
                    "vector index must be an integer, got {}",
                    index.type_name()
                ))
                .into())
            }
        }
        Value::Map(map) => {
            let key_hash = Hashable::try_from(index)?;
            Ok(map.pairs.get(&key_hash).cloned().unwrap_or(UNIT))
        }
        _ => Err(
            EvalError::IndexExpr(format!("index expression not supported for {expr_type}")).into(),
        ),
    }
}

fn eval_map_literal(expr: MapLit, env: &Env) -> Result<Value> {
    let mut pairs = IndexMap::new();
    for (key_expr, val_expr) in &expr.pairs {
        let key = eval(Node::Expr(key_expr.clone()), env)?;
        let key = Hashable::try_from(key)?;
        let value = eval(Node::Expr(val_expr.clone()), env)?;
        pairs.insert(key, value);
    }
    Ok(Value::Map(Map { pairs }))
}

fn eval_prefix_expression(expr: PrefixExpr, env: &Env) -> Result<Value> {
    let operand = eval(Node::Expr(*expr.operand), env)?;
    let op = &expr.operator;

    match op.as_str() {
        "!" => match operand {
            val if !val.is_truthy() => Ok(TRUE),
            _ => Ok(FALSE),
        },
        "-" => match operand {
            Value::Integer(v) => v
                .checked_neg()
                .map(Value::Integer)
                .ok_or_else(|| EvalError::PrefixExpr(format!("negation overflow: -{v}")).into()),
            _ => Err(
                EvalError::PrefixExpr(format!("{} cannot be negated", operand.type_name())).into(),
            ),
        },
        _ => {
            Err(EvalError::PrefixExpr(format!("invalid prefix expression: `{op}{operand}`")).into())
        }
    }
}

/// Evaluates short-circuit logical operators `&&` and `||`.
///
/// `&&` returns the left operand if falsy, otherwise the right operand.
/// `||` returns the left operand if truthy, otherwise the right operand.
fn eval_logical_expression(expr: InfixExpr, env: &Env) -> Result<Value> {
    let lhs = eval(Node::Expr(*expr.lhs), env)?;
    let Value::Bool(left_val) = &lhs else {
        return Err(EvalError::InfixExpr(format!(
            "expected bool in `{}` expression, got `{lhs}`",
            expr.operator
        ))
        .into());
    };
    match expr.operator.as_str() {
        "&&" => {
            if !left_val {
                return Ok(Value::Bool(false));
            }
            eval(Node::Expr(*expr.rhs), env)
        }
        "||" => {
            if *left_val {
                return Ok(Value::Bool(true));
            }
            eval(Node::Expr(*expr.rhs), env)
        }
        _ => unreachable!(),
    }
}

fn eval_infix_expression(expr: InfixExpr, env: &Env) -> Result<Value> {
    let lhs = eval(Node::Expr(*expr.lhs), env)?;
    let rhs = eval(Node::Expr(*expr.rhs), env)?;
    let op = &expr.operator;

    match (&lhs, &rhs) {
        (Value::Integer(l), Value::Integer(r)) => {
            let result = match op.as_str() {
                "+" => l.checked_add(*r).ok_or_else(|| {
                    EvalError::Number(format!(
                        "arithmetic overflow: {l} + {r} exceeds {} bounds",
                        l.type_name()
                    ))
                })?,
                "-" => l.checked_sub(*r).ok_or_else(|| {
                    EvalError::Number(format!(
                        "arithmetic overflow: {l} - {r} exceeds {} bounds",
                        l.type_name()
                    ))
                })?,
                "*" => l.checked_mul(*r).ok_or_else(|| {
                    EvalError::Number(format!(
                        "arithmetic overflow: {l} * {r} exceeds {} bounds",
                        l.type_name()
                    ))
                })?,
                "/" => l.checked_div(*r).ok_or_else(|| {
                    EvalError::Number(format!(
                        "division error: {l} / {r} (division by zero or overflow)"
                    ))
                })?,
                "%" => l.checked_rem_euclid(*r).ok_or_else(|| {
                    EvalError::Number(format!(
                        "modulo error: {l} % {r} (division by zero or overflow)"
                    ))
                })?,
                "<" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(Value::Bool(ordering.is_lt()));
                }
                ">" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(Value::Bool(ordering.is_gt()));
                }
                "<=" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(Value::Bool(ordering.is_le()));
                }
                ">=" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(Value::Bool(ordering.is_ge()));
                }
                "==" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(Value::Bool(ordering.is_eq()));
                }
                "!=" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(Value::Bool(!ordering.is_eq()));
                }
                _ => {
                    return Err(EvalError::Number(format!(
                        "invalid integer operation: `{l} {op} {r}`"
                    ))
                    .into());
                }
            };
            Ok(Value::Integer(result))
        }
        (Value::Felt(l), Value::Felt(r)) => eval_infix_felt(op, *l, *r),
        (Value::Bool(l), Value::Bool(r)) => eval_infix_bool(op, *l, *r),
        (Value::Str(l), Value::Str(r)) => eval_infix_string(op, l, r),
        _ => Err(
            EvalError::InfixExpr(format!("invalid infix expression: `{lhs} {op} {rhs}`")).into(),
        ),
    }
}

/// Evaluates an infix operator on two Goldilocks field elements.
fn eval_infix_felt(op: &str, lhs: Felt, rhs: Felt) -> Result<Value> {
    match op {
        "+" => Ok(Value::Felt(lhs + rhs)),
        "-" => Ok(Value::Felt(lhs - rhs)),
        "*" => Ok(Value::Felt(lhs * rhs)),
        "/" => (lhs / rhs)
            .map(Value::Felt)
            .map_err(|e| EvalError::Number(format!("Felt division error: {e}")).into()),
        "==" => Ok(Value::Bool(lhs == rhs)),
        "!=" => Ok(Value::Bool(lhs != rhs)),
        _ => Err(EvalError::Number(format!(
            "operator `{op}` is not defined on Felt; field elements are unordered"
        ))
        .into()),
    }
}

fn eval_infix_bool(op: &str, lhs: bool, rhs: bool) -> Result<Value> {
    match op {
        "==" => Ok(Value::Bool(lhs == rhs)),
        "!=" => Ok(Value::Bool(lhs != rhs)),
        "&&" => Ok(Value::Bool(lhs && rhs)),
        "||" => Ok(Value::Bool(lhs || rhs)),
        _ => {
            Err(EvalError::Boolean(format!("invalid boolean operation: `{lhs} {op} {rhs}`")).into())
        }
    }
}

fn eval_infix_string(op: &str, lhs: &str, rhs: &str) -> Result<Value> {
    if op != "+" {
        return Err(
            EvalError::InfixExpr(format!("invalid concat operation: `{lhs} {op} {rhs}`")).into(),
        );
    }
    Ok(Value::Str(format!("{lhs}{rhs}")))
}

fn eval_conditional_expression(expr: CondExpr, env: &Env) -> Result<Value> {
    let condition = eval(Node::Expr(*expr.condition), env)?;
    if condition.is_truthy() {
        eval(Node::Stmt(Stmt::Block(expr.consequence)), env)
    } else if let Some(alt) = expr.alternative {
        eval(Node::Stmt(Stmt::Block(alt)), env)
    } else {
        Ok(UNIT)
    }
}

fn eval_identifier(ident: String, env: &Env) -> Result<Value> {
    match env.get(&ident) {
        Some(val) => Ok(val.clone()),
        None => match get_builtin(&ident) {
            Some(func) => Ok(Value::Builtin(func)),
            None => Err(EvalError::Ident(format!("unknown identifier: {ident}")).into()),
        },
    }
}

fn eval_function_call(expr: CallExpr, env: &Env) -> Result<Value> {
    let value = eval(Node::Expr(*expr.function), env)?;
    let expressions = eval_expressions(&expr.arguments, env)?;

    match value {
        Value::Function(func) => {
            let env = Env::new_enclosed(&func.env);
            func.params.iter().enumerate().for_each(|(i, param)| {
                env.set(param.to_owned(), &expressions[i]);
            });
            let evaluated = eval(Node::Stmt(Stmt::Block(func.body)), &env)?;
            match evaluated {
                Value::ReturnValue(val) => Ok(*val),
                Value::Break(_) | Value::Continue => {
                    Err(EvalError::Ident("break/continue outside of a loop".to_string()).into())
                }
                other => Ok(other),
            }
        }
        Value::Builtin(builtin_fn) => builtin_fn(&expressions),
        val => Err(EvalError::NotAFunction(format!("expected {val} to be a function")).into()),
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
                Ok(val) => val,
                Err(_) => return node,
            };
            match Value::to_ast_node(&unquoted) {
                Some(ast_node) => ast_node,
                None => node,
            }
        } else {
            node
        }
    })
}

fn eval_cast_expression(expr: CastExpr, env: &Env) -> Result<Value> {
    let value = eval(Node::Expr(*expr.expr), env)?;
    let target = expr.target;

    match target {
        CastTarget::Char => match value {
            Value::Integer(val) => {
                let scalar = match val.to_wide() {
                    WideInt::Signed(v) => u32::try_from(v).ok().and_then(char::from_u32),
                    WideInt::Unsigned(v) => u32::try_from(v).ok().and_then(char::from_u32),
                };
                scalar.map(Value::Char).ok_or_else(|| {
                    EvalError::Number(format!("value {val} is not a valid Unicode scalar value",))
                        .into()
                })
            }
            other => {
                Err(EvalError::Number(format!("cannot cast {} as char", other.type_name(),)).into())
            }
        },
        CastTarget::Num(NumKind::Fe) => cast_to_felt(value),
        CastTarget::Num(num_kind) => match value {
            Value::Char(ch) => {
                Integer::from_wide(WideInt::Unsigned(u128::from(ch as u32)), num_kind)
                    .map(Value::Integer)
                    .map_err(|e| EvalError::Number(e).into())
            }
            Value::Integer(val) => val
                .cast_to(num_kind)
                .map(Value::Integer)
                .map_err(|e| EvalError::Number(e).into()),
            Value::Felt(_) => Err(EvalError::Number(format!(
                "cannot cast Felt to {}; field elements are non-narrowing",
                num_kind.as_str(),
            ))
            .into()),
            other => Err(EvalError::Number(format!(
                "cannot cast {} to {}",
                other.type_name(),
                num_kind.as_str(),
            ))
            .into()),
        },
    }
}

/// Casts an integer-typed [`Value`] into a Goldilocks field element.
fn cast_to_felt(value: Value) -> Result<Value> {
    use maat_runtime::Integer as I;

    let felt = match value {
        Value::Felt(f) => return Ok(Value::Felt(f)),
        Value::Integer(I::I8(v)) => Felt::from_i64(v as i64),
        Value::Integer(I::I16(v)) => Felt::from_i64(v as i64),
        Value::Integer(I::I32(v)) => Felt::from_i64(v as i64),
        Value::Integer(I::I64(v)) => Felt::from_i64(v),
        Value::Integer(I::Isize(v)) => Felt::from_i64(v as i64),
        Value::Integer(I::U8(v)) => Felt::new(u64::from(v)),
        Value::Integer(I::U16(v)) => Felt::new(u64::from(v)),
        Value::Integer(I::U32(v)) => Felt::new(u64::from(v)),
        Value::Integer(I::U64(v)) => Felt::new(v),
        Value::Integer(I::Usize(v)) => Felt::new(v as u64),
        Value::Integer(I::I128(_)) | Value::Integer(I::U128(_)) => {
            return Err(EvalError::Number(
                "cannot cast 128-bit integer to Felt; use explicit `Felt::new`".to_string(),
            )
            .into());
        }
        other => {
            return Err(
                EvalError::Number(format!("cannot cast {} to Felt", other.type_name(),)).into(),
            );
        }
    };
    Ok(Value::Felt(felt))
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
