//! Tree-walking interpreter for macro expansion.
//!
//! Operates on [`EvalValue`] / [`EvalEnv`] from `crate::value`; the runtime
//! [`Value`] enum is only touched at the builtin call boundary
//! (where the interpreter passes [`BuiltinArg`]) and at the unquote reification
//! site (where AST nodes flow out).

use indexmap::IndexMap;
use maat_ast::*;
use maat_errors::{EvalError, Result};
use maat_runtime::{
    BuiltinArg, BuiltinFn, Felt, Hashable, Integer, Map, Set, Value, WideInt, from_i64,
    get_builtin, try_div,
};

use crate::value::{
    EvalEnv, EvalFunction, EvalMacro, EvalMap, EvalQuote, EvalValue, eval_value_to_builtin_arg,
    eval_value_to_value, return_to_eval_value,
};
use crate::{QUOTE, UNQUOTE};

const UNIT: EvalValue = EvalValue::Unit;
const TRUE: EvalValue = EvalValue::Bool(true);
const FALSE: EvalValue = EvalValue::Bool(false);

pub fn eval(node: MaatAst, env: &EvalEnv) -> Result<EvalValue> {
    match node {
        MaatAst::Program(prog) => eval_program(prog, env),
        MaatAst::Stmt(stmt) => match stmt {
            Stmt::Let(ls) => {
                let val = eval(MaatAst::Expr(ls.value), env)?;
                env.set(ls.ident, &val);
                Ok(val)
            }
            Stmt::ReAssign(assign) => {
                let val = eval(MaatAst::Expr(assign.value), env)?;
                env.update(assign.ident, &val);
                Ok(val)
            }
            Stmt::Return(rs) => {
                let val = eval(MaatAst::Expr(rs.value), env)?;
                Ok(EvalValue::ReturnValue(Box::new(val)))
            }
            Stmt::Expr(es) => eval(MaatAst::Expr(es.value), env),
            Stmt::Block(bs) => eval_block_statement(&bs, env),
            Stmt::FuncDef(fn_item) => {
                let val = EvalValue::Function(EvalFunction {
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
        MaatAst::Expr(expr) => match expr {
            Expr::Number(v) => {
                EvalValue::from_number_literal(&v).map_err(|e| EvalError::Number(e).into())
            }
            Expr::Bool(b) => Ok(EvalValue::Bool(b.value)),
            Expr::Str(s) => Ok(EvalValue::Str(maat_ast::unescape_string(&s.value))),
            Expr::Char(c) => Ok(EvalValue::Char(c.value)),
            Expr::Tuple(tuple) => {
                let elements = tuple
                    .elements
                    .iter()
                    .map(|e| eval(MaatAst::Expr(e.clone()), env))
                    .collect::<Result<Vec<_>>>()?;
                Ok(EvalValue::Tuple(elements))
            }
            Expr::Vector(vector) => {
                let elements = eval_expressions(&vector.elements, env)?;
                Ok(EvalValue::Vector(elements))
            }
            Expr::Array(arr) => {
                let elements = eval_expressions(&arr.elements, env)?;
                Ok(EvalValue::Array(elements))
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
            Expr::Lambda(lambda) => Ok(EvalValue::Function(EvalFunction {
                params: lambda.param_names().map(String::from).collect(),
                body: lambda.body,
                env: env.clone(),
            })),
            Expr::MacroLit(macro_lit) => Ok(EvalValue::Macro(EvalMacro {
                params: macro_lit.params,
                body: macro_lit.body,
                env: env.clone(),
            })),
            Expr::Break(break_expr) => {
                let value = break_expr
                    .value
                    .map(|v| eval(MaatAst::Expr(*v), env))
                    .transpose()?
                    .unwrap_or(UNIT);
                Ok(EvalValue::Break(Box::new(value)))
            }
            Expr::Continue(_) => Ok(EvalValue::Continue),
            Expr::Cast(cast_expr) => eval_cast_expression(cast_expr, env),
            Expr::Range(range) => {
                let start = eval(MaatAst::Expr(*range.start), env)?;
                let end = eval(MaatAst::Expr(*range.end), env)?;
                match (start, end) {
                    (EvalValue::Integer(s), EvalValue::Integer(e)) => {
                        if range.inclusive {
                            Ok(EvalValue::RangeInclusive(s, e))
                        } else {
                            Ok(EvalValue::Range(s, e))
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
                    let node = MaatAst::Expr(call_expr.arguments[0].clone());
                    let node = eval_unquote_calls(node, env);
                    return Ok(EvalValue::Quote(Box::new(EvalQuote { node })));
                }
                eval_function_call(call_expr, env)
            }
        },
    }
}

fn eval_program(prog: Program, env: &EvalEnv) -> Result<EvalValue> {
    let mut result = UNIT;
    for stmt in &prog.statements {
        result = eval(MaatAst::Stmt(stmt.clone()), env)?;
        match result {
            EvalValue::ReturnValue(val) => return Ok(*val),
            EvalValue::Break(_) | EvalValue::Continue => {
                return Err(
                    EvalError::Ident("break/continue outside of a loop".to_string()).into(),
                );
            }
            _ => {}
        }
    }
    Ok(result)
}

pub fn eval_block_statement(block: &BlockStmt, env: &EvalEnv) -> Result<EvalValue> {
    let block_env = EvalEnv::new_enclosed(env);
    let mut result = UNIT;
    for stmt in &block.statements {
        result = eval(MaatAst::Stmt(stmt.clone()), &block_env)?;
        if matches!(
            result,
            EvalValue::ReturnValue(_) | EvalValue::Break(_) | EvalValue::Continue
        ) {
            return Ok(result);
        }
    }
    Ok(result)
}

fn eval_loop_statement(stmt: LoopStmt, env: &EvalEnv) -> Result<EvalValue> {
    let bound = stmt.bound;
    let mut counter = 0u64;
    loop {
        if counter >= bound {
            return Err(EvalError::BoundExceeded(bound).into());
        }
        let result = eval_block_statement(&stmt.body, env)?;
        match result {
            EvalValue::Break(val) => return Ok(*val),
            EvalValue::ReturnValue(_) => return Ok(result),
            EvalValue::Continue => {
                counter += 1;
                continue;
            }
            _ => {}
        }
        counter += 1;
    }
}

fn eval_while_statement(stmt: WhileStmt, env: &EvalEnv) -> Result<EvalValue> {
    let bound = stmt.bound;
    let mut counter = 0u64;
    loop {
        let condition = eval(MaatAst::Expr(*stmt.condition.clone()), env)?;
        if !condition.is_truthy() {
            break;
        }
        if counter >= bound {
            return Err(EvalError::BoundExceeded(bound).into());
        }
        let result = eval_block_statement(&stmt.body, env)?;
        match result {
            EvalValue::Break(val) => return Ok(*val),
            EvalValue::ReturnValue(_) => return Ok(result),
            EvalValue::Continue => {
                counter += 1;
                continue;
            }
            _ => {}
        }
        counter += 1;
    }
    Ok(UNIT)
}

fn eval_for_statement(stmt: ForStmt, env: &EvalEnv) -> Result<EvalValue> {
    let iterable = eval(MaatAst::Expr(*stmt.iterable), env)?;
    let elements = match iterable {
        EvalValue::Vector(elems) | EvalValue::Array(elems) => elems,
        other => {
            return Err(EvalError::Ident(format!(
                "for..in requires a vector or array, got {}",
                other.type_name()
            ))
            .into());
        }
    };
    let loop_env = EvalEnv::new_enclosed(env);
    for elem in elements {
        loop_env.set(stmt.ident.clone(), &elem);
        let result = eval_block_statement(&stmt.body, &loop_env)?;
        match result {
            EvalValue::Break(val) => return Ok(*val),
            EvalValue::ReturnValue(_) => return Ok(result),
            EvalValue::Continue => continue,
            _ => {}
        }
    }
    Ok(UNIT)
}

fn eval_expression(expr: &Expr, env: &EvalEnv) -> Result<EvalValue> {
    eval(MaatAst::Expr(expr.clone()), env)
}

fn eval_expressions(exprs: &[Expr], env: &EvalEnv) -> Result<Vec<EvalValue>> {
    exprs
        .iter()
        .map(|expr| eval(MaatAst::Expr(expr.to_owned()), env))
        .collect()
}

fn eval_index_expression(idx_expr: IndexExpr, env: &EvalEnv) -> Result<EvalValue> {
    let expr = eval(MaatAst::Expr(*idx_expr.expr), env)?;
    let expr_type = expr.type_name();
    let index = eval(MaatAst::Expr(*idx_expr.index), env)?;

    match expr {
        EvalValue::Vector(arr) | EvalValue::Array(arr) => {
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
        EvalValue::Map(map) => {
            let key_hash = Hashable::try_from(index)?;
            Ok(map.pairs.get(&key_hash).cloned().unwrap_or(UNIT))
        }
        _ => Err(
            EvalError::IndexExpr(format!("index expression not supported for {expr_type}")).into(),
        ),
    }
}

fn eval_map_literal(expr: MapLit, env: &EvalEnv) -> Result<EvalValue> {
    let mut pairs = IndexMap::new();
    for (key_expr, val_expr) in &expr.pairs {
        let key = eval(MaatAst::Expr(key_expr.clone()), env)?;
        let key = Hashable::try_from(key)?;
        let value = eval(MaatAst::Expr(val_expr.clone()), env)?;
        pairs.insert(key, value);
    }
    Ok(EvalValue::Map(EvalMap { pairs }))
}

fn eval_prefix_expression(expr: PrefixExpr, env: &EvalEnv) -> Result<EvalValue> {
    let operand = eval(MaatAst::Expr(*expr.operand), env)?;
    let op = &expr.operator;

    match op.as_str() {
        "!" => match operand {
            val if !val.is_truthy() => Ok(TRUE),
            _ => Ok(FALSE),
        },
        "-" => match operand {
            EvalValue::Integer(v) => v
                .checked_neg()
                .map(EvalValue::Integer)
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

fn eval_logical_expression(expr: InfixExpr, env: &EvalEnv) -> Result<EvalValue> {
    let lhs = eval(MaatAst::Expr(*expr.lhs), env)?;
    let EvalValue::Bool(left_val) = &lhs else {
        return Err(EvalError::InfixExpr(format!(
            "expected bool in `{}` expression, got `{lhs}`",
            expr.operator
        ))
        .into());
    };
    match expr.operator.as_str() {
        "&&" => {
            if !left_val {
                return Ok(EvalValue::Bool(false));
            }
            eval(MaatAst::Expr(*expr.rhs), env)
        }
        "||" => {
            if *left_val {
                return Ok(EvalValue::Bool(true));
            }
            eval(MaatAst::Expr(*expr.rhs), env)
        }
        _ => unreachable!(),
    }
}

fn eval_infix_expression(expr: InfixExpr, env: &EvalEnv) -> Result<EvalValue> {
    let lhs = eval(MaatAst::Expr(*expr.lhs), env)?;
    let rhs = eval(MaatAst::Expr(*expr.rhs), env)?;
    let op = &expr.operator;

    match (&lhs, &rhs) {
        (EvalValue::Integer(l), EvalValue::Integer(r)) => {
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
                    return Ok(EvalValue::Bool(ordering.is_lt()));
                }
                ">" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(EvalValue::Bool(ordering.is_gt()));
                }
                "<=" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(EvalValue::Bool(ordering.is_le()));
                }
                ">=" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(EvalValue::Bool(ordering.is_ge()));
                }
                "==" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(EvalValue::Bool(ordering.is_eq()));
                }
                "!=" => {
                    let ordering = l.partial_cmp(r).ok_or_else(|| {
                        EvalError::Number(format!(
                            "cannot compare {} and {}",
                            l.type_name(),
                            r.type_name()
                        ))
                    })?;
                    return Ok(EvalValue::Bool(!ordering.is_eq()));
                }
                _ => {
                    return Err(EvalError::Number(format!(
                        "invalid integer operation: `{l} {op} {r}`"
                    ))
                    .into());
                }
            };
            Ok(EvalValue::Integer(result))
        }
        (EvalValue::Felt(l), EvalValue::Felt(r)) => eval_infix_felt(op, *l, *r),
        (EvalValue::Bool(l), EvalValue::Bool(r)) => eval_infix_bool(op, *l, *r),
        (EvalValue::Str(l), EvalValue::Str(r)) => eval_infix_string(op, l, r),
        _ => Err(
            EvalError::InfixExpr(format!("invalid infix expression: `{lhs} {op} {rhs}`")).into(),
        ),
    }
}

fn eval_infix_felt(op: &str, lhs: Felt, rhs: Felt) -> Result<EvalValue> {
    match op {
        "+" => Ok(EvalValue::Felt(lhs + rhs)),
        "-" => Ok(EvalValue::Felt(lhs - rhs)),
        "*" => Ok(EvalValue::Felt(lhs * rhs)),
        "/" => try_div(lhs, rhs)
            .map(EvalValue::Felt)
            .map_err(|e| EvalError::Number(format!("Felt division error: {e}")).into()),
        "==" => Ok(EvalValue::Bool(lhs == rhs)),
        "!=" => Ok(EvalValue::Bool(lhs != rhs)),
        _ => Err(EvalError::Number(format!(
            "operator `{op}` is not defined on Felt; field elements are unordered"
        ))
        .into()),
    }
}

fn eval_infix_bool(op: &str, lhs: bool, rhs: bool) -> Result<EvalValue> {
    match op {
        "==" => Ok(EvalValue::Bool(lhs == rhs)),
        "!=" => Ok(EvalValue::Bool(lhs != rhs)),
        "&&" => Ok(EvalValue::Bool(lhs && rhs)),
        "||" => Ok(EvalValue::Bool(lhs || rhs)),
        _ => {
            Err(EvalError::Boolean(format!("invalid boolean operation: `{lhs} {op} {rhs}`")).into())
        }
    }
}

fn eval_infix_string(op: &str, lhs: &str, rhs: &str) -> Result<EvalValue> {
    if op != "+" {
        return Err(
            EvalError::InfixExpr(format!("invalid concat operation: `{lhs} {op} {rhs}`")).into(),
        );
    }
    Ok(EvalValue::Str(format!("{lhs}{rhs}")))
}

fn eval_conditional_expression(expr: CondExpr, env: &EvalEnv) -> Result<EvalValue> {
    let condition = eval(MaatAst::Expr(*expr.condition), env)?;
    if condition.is_truthy() {
        eval(MaatAst::Stmt(Stmt::Block(expr.consequence)), env)
    } else if let Some(alt) = expr.alternative {
        eval(MaatAst::Stmt(Stmt::Block(alt)), env)
    } else {
        Ok(UNIT)
    }
}

fn eval_identifier(ident: String, env: &EvalEnv) -> Result<EvalValue> {
    match env.get(&ident) {
        Some(val) => Ok(val),
        None => match get_builtin(&ident) {
            Some(func) => Ok(EvalValue::Builtin(func)),
            None => Err(EvalError::Ident(format!("unknown identifier: {ident}")).into()),
        },
    }
}

fn eval_function_call(expr: CallExpr, env: &EvalEnv) -> Result<EvalValue> {
    let value = eval(MaatAst::Expr(*expr.function), env)?;
    let expressions = eval_expressions(&expr.arguments, env)?;

    match value {
        EvalValue::Function(func) => {
            let env = EvalEnv::new_enclosed(&func.env);
            func.params.iter().enumerate().for_each(|(i, param)| {
                env.set(param.to_owned(), &expressions[i]);
            });
            let evaluated = eval(MaatAst::Stmt(Stmt::Block(func.body)), &env)?;
            match evaluated {
                EvalValue::ReturnValue(val) => Ok(*val),
                EvalValue::Break(_) | EvalValue::Continue => {
                    Err(EvalError::Ident("break/continue outside of a loop".to_string()).into())
                }
                other => Ok(other),
            }
        }
        EvalValue::Builtin(builtin_fn) => call_builtin(builtin_fn, &expressions),
        val => Err(EvalError::NotAFunction(format!("expected {val} to be a function")).into()),
    }
}

fn eval_unquote_calls(quoted: MaatAst, env: &EvalEnv) -> MaatAst {
    transform(quoted, &mut |node| {
        if !is_unquote_call(&node) {
            return node;
        }
        if let MaatAst::Expr(Expr::Call(call)) = &node {
            if call.arguments.len() != 1 {
                return node;
            }
            let unquoted = match eval_expression(&call.arguments[0], env) {
                Ok(val) => val,
                Err(_) => return node,
            };
            match EvalValue::to_ast_node(&unquoted) {
                Some(ast_node) => ast_node,
                None => node,
            }
        } else {
            node
        }
    })
}

fn eval_cast_expression(expr: CastExpr, env: &EvalEnv) -> Result<EvalValue> {
    let value = eval(MaatAst::Expr(*expr.expr), env)?;
    let target = expr.target;

    match target {
        CastTarget::Char => match value {
            EvalValue::Integer(val) => {
                let scalar = match val.to_wide() {
                    WideInt::Signed(v) => u32::try_from(v).ok().and_then(char::from_u32),
                    WideInt::Unsigned(v) => u32::try_from(v).ok().and_then(char::from_u32),
                };
                scalar.map(EvalValue::Char).ok_or_else(|| {
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
            EvalValue::Char(ch) => {
                Integer::from_wide(WideInt::Unsigned(u128::from(ch as u32)), num_kind)
                    .map(EvalValue::Integer)
                    .map_err(|e| EvalError::Number(e).into())
            }
            EvalValue::Integer(val) => val
                .cast_to(num_kind)
                .map(EvalValue::Integer)
                .map_err(|e| EvalError::Number(e).into()),
            EvalValue::Felt(_) => Err(EvalError::Number(format!(
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

fn cast_to_felt(value: EvalValue) -> Result<EvalValue> {
    use maat_runtime::Integer as I;

    let felt = match value {
        EvalValue::Felt(f) => return Ok(EvalValue::Felt(f)),
        EvalValue::Integer(I::I8(v)) => from_i64(v as i64),
        EvalValue::Integer(I::I16(v)) => from_i64(v as i64),
        EvalValue::Integer(I::I32(v)) => from_i64(v as i64),
        EvalValue::Integer(I::I64(v)) => from_i64(v),
        EvalValue::Integer(I::Isize(v)) => from_i64(v as i64),
        EvalValue::Integer(I::U8(v)) => Felt::new(u64::from(v)),
        EvalValue::Integer(I::U16(v)) => Felt::new(u64::from(v)),
        EvalValue::Integer(I::U32(v)) => Felt::new(u64::from(v)),
        EvalValue::Integer(I::U64(v)) => Felt::new(v),
        EvalValue::Integer(I::Usize(v)) => Felt::new(v as u64),
        EvalValue::Integer(I::I128(_)) | EvalValue::Integer(I::U128(_)) => {
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
    Ok(EvalValue::Felt(felt))
}

fn is_unquote_call(node: &MaatAst) -> bool {
    if let MaatAst::Expr(Expr::Call(call)) = node
        && let Expr::Ident(ident) = &*call.function
    {
        return ident.value == UNQUOTE;
    }
    false
}

/// Bridges an [`EvalValue`]-shaped argument list into the
/// [`BuiltinArg`]-shaped slice that builtins consume.
fn call_builtin(func: BuiltinFn, args: &[EvalValue]) -> Result<EvalValue> {
    let mut base_vecs: Vec<Vec<Value>> = Vec::with_capacity(args.len());
    let mut base_maps: Vec<Map> = Vec::with_capacity(args.len());
    let mut base_sets: Vec<Set> = Vec::with_capacity(args.len());

    for arg in args {
        let (vb, mb, sb) = match arg {
            EvalValue::Vector(items) | EvalValue::Tuple(items) | EvalValue::Array(items) => {
                let projected = items.iter().cloned().map(eval_value_to_value).collect();
                (projected, Map::default(), Set::default())
            }
            EvalValue::Map(m) => {
                let pairs = m
                    .pairs
                    .iter()
                    .map(|(k, v)| (k.clone(), eval_value_to_value(v.clone())))
                    .collect();
                (Vec::new(), Map { pairs }, Set::default())
            }
            EvalValue::Set(s) => (Vec::new(), Map::default(), Set(s.0.clone())),
            _ => (Vec::new(), Map::default(), Set::default()),
        };
        base_vecs.push(vb);
        base_maps.push(mb);
        base_sets.push(sb);
    }

    let args_view: Vec<BuiltinArg<'_>> = args
        .iter()
        .enumerate()
        .map(|(i, arg)| eval_value_to_builtin_arg(arg, &base_vecs[i], &base_maps[i], &base_sets[i]))
        .collect();
    let ret = func(&args_view)?;
    drop(args_view);
    drop(base_vecs);
    drop(base_maps);
    drop(base_sets);
    Ok(return_to_eval_value(ret))
}
