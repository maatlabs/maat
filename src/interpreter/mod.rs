//! Evaluation engine.
//!
//! This module implements a tree-walking interpreter that evaluates the AST nodes
//! into runtime objects. It supports integers, booleans, functions, conditionals,
//! and lexically-scoped environments.

pub mod builtins;
pub mod env;
pub mod object;
pub mod repl;

use std::collections::HashMap;

use builtins::get_builtin;
pub use env::Env;
pub use object::{BuiltinFn, FALSE, Function, HashObject, Hashable, NULL, Object, TRUE};

use crate::Result;
use crate::error::EvalError;
use crate::parser::ast::*;

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
/// use maat::interpreter::{eval, Object};
/// use maat::parser::ast::Node;
///
/// let input = "5 + 10";
/// let lexer = Lexer::new(input);
/// let mut parser = Parser::new(lexer);
/// let program = parser.parse_program();
/// let env = Env::default();
///
/// let result = eval(Node::Program(program), &env).unwrap();
/// assert_eq!(result, Object::Int64(15));
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
            Expression::Int64(int64) => Ok(Object::Int64(int64.value)),
            Expression::Float64(float64) => Ok(Object::Float64(float64.into())),
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

    match (expr, index) {
        (Object::Array(arr), Object::Int64(idx)) => {
            if arr.is_empty() || idx < 0 || idx as usize >= arr.len() {
                return Ok(NULL);
            }
            Ok(arr[idx as usize].clone())
        }
        (Object::Hash(hash), key) => {
            let key_hash = Hashable::try_from(key)?;
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
            Object::Int64(int64) => Ok(Object::Int64(-int64)),
            Object::Float64(float64) => Ok(Object::Float64(-float64)),

            _ => Err(EvalError::PrefixExpression(format!(
                "{operand} is of type that cannot be negated"
            ))
            .into()),
        },

        _ => Err(EvalError::PrefixExpression(format!("unknown operator: {operator}")).into()),
    }
}

fn eval_infix_expression(expr: InfixExpr, env: &Env) -> Result<Object> {
    let lhs = eval(Node::Expression(*expr.lhs), env)?;
    let rhs = eval(Node::Expression(*expr.rhs), env)?;
    let operator = &expr.operator;

    match (&lhs, &rhs) {
        (Object::Int64(left), Object::Int64(right)) => eval_infix_int64(operator, *left, *right),
        (Object::Float64(left), Object::Float64(right)) => {
            eval_infix_float64(operator, *left, *right)
        }
        (Object::Boolean(left), Object::Boolean(right)) => eval_infix_bool(operator, *left, *right),
        (Object::String(left), Object::String(right)) => eval_infix_string(operator, left, right),
        _ => Err(EvalError::InfixExpression(format!(
            "invalid infix expression: `{lhs} {operator} {rhs}`"
        ))
        .into()),
    }
}

fn eval_infix_int64(operator: &str, lhs: i64, rhs: i64) -> Result<Object> {
    match operator {
        "+" => Ok(Object::Int64(lhs + rhs)),
        "-" => Ok(Object::Int64(lhs - rhs)),
        "*" => Ok(Object::Int64(lhs * rhs)),
        "/" => Ok(Object::Int64(lhs / rhs)),

        "<" => Ok(Object::Boolean(lhs < rhs)),
        ">" => Ok(Object::Boolean(lhs > rhs)),
        "==" => Ok(Object::Boolean(lhs == rhs)),
        "!=" => Ok(Object::Boolean(lhs != rhs)),

        _ => Err(
            EvalError::Number(format!("invalid i64 operation: `{lhs} {operator} {rhs}`")).into(),
        ),
    }
}

fn eval_infix_float64(operator: &str, lhs: f64, rhs: f64) -> Result<Object> {
    match operator {
        "+" => Ok(Object::Float64(lhs + rhs)),
        "-" => Ok(Object::Float64(lhs - rhs)),
        "*" => Ok(Object::Float64(lhs * rhs)),
        "/" => Ok(Object::Float64(lhs / rhs)),

        "<" => Ok(Object::Boolean(lhs < rhs)),
        ">" => Ok(Object::Boolean(lhs > rhs)),
        "==" => Ok(Object::Boolean(lhs == rhs)),
        "!=" => Ok(Object::Boolean(lhs != rhs)),

        _ => Err(
            EvalError::Number(format!("invalid f64 operation: `{lhs} {operator} {rhs}`")).into(),
        ),
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
