pub mod env;
pub mod object;
pub mod repl;

pub use env::Env;
pub use object::{Function, Object};

use crate::Result;
use crate::error::EvalError;
use crate::parser::ast::{
    BlockStatement, CallExpr, ConditionalExpr, Expression, InfixExpr, Node, PrefixExpr, Program,
    Statement,
};

const TRUE: Object = Object::Boolean(true);
const FALSE: Object = Object::Boolean(false);
const NULL: Object = Object::Null;

pub fn eval(node: Node, env: &mut Env) -> Result<Object> {
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
            Expression::Boolean(boolean) => Ok(Object::Boolean(boolean)),
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

fn eval_program(prog: Program, env: &mut Env) -> Result<Object> {
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

fn eval_block_statement(block: BlockStatement, env: &mut Env) -> Result<Object> {
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

fn eval_expressions(exprs: &[Expression], env: &mut Env) -> Result<Vec<Object>> {
    let mut result = Vec::new();

    for expr in exprs {
        let evaluated = eval(Node::Expression(expr.to_owned()), env)?;
        result.push(evaluated);
    }
    Ok(result)
}

fn eval_prefix_expression(expr: PrefixExpr, env: &mut Env) -> Result<Object> {
    let operand = eval(Node::Expression(*expr.operand), env)?;
    let operator = &expr.operator;

    match operator.as_str() {
        "!" => match operand {
            obj if obj == NULL || obj == FALSE => Ok(TRUE),
            _ => Ok(FALSE),
        },

        "-" => match operand {
            Object::Int64(int64) => Ok(Object::Int64(-int64)),

            _ => Err(EvalError::PrefixExpression(format!(
                "{operand} is of type that cannot be negated"
            ))
            .into()),
        },

        _ => Err(EvalError::PrefixExpression(format!("unknown operator: {operator}")).into()),
    }
}

fn eval_infix_expression(expr: InfixExpr, env: &mut Env) -> Result<Object> {
    let lhs = eval(Node::Expression(*expr.lhs), env)?;
    let rhs = eval(Node::Expression(*expr.rhs), env)?;
    let operator = &expr.operator;

    match (&lhs, &rhs) {
        (Object::Int64(left), Object::Int64(right)) => eval_infix_int64(operator, *left, *right),
        (Object::Boolean(left), Object::Boolean(right)) => eval_infix_bool(operator, *left, *right),
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

        _ => {
            Err(EvalError::Number(format!("invalid i64 operation: {lhs} {operator} {rhs}")).into())
        }
    }
}

fn eval_infix_bool(operator: &str, lhs: bool, rhs: bool) -> Result<Object> {
    match operator {
        "==" => Ok(Object::Boolean(lhs == rhs)),
        "!=" => Ok(Object::Boolean(lhs != rhs)),
        _ => Err(
            EvalError::Boolean(format!("invalid boolean operation: {lhs} {operator} {rhs}")).into(),
        ),
    }
}

fn eval_conditional_expression(expr: ConditionalExpr, env: &mut Env) -> Result<Object> {
    let condition = eval(Node::Expression(*expr.condition), env)?;

    if is_truthy(condition) {
        eval(Node::Statement(Statement::Block(expr.consequence)), env)
    } else if let Some(alt) = expr.alternative {
        eval(Node::Statement(Statement::Block(alt)), env)
    } else {
        Ok(NULL)
    }
}

fn is_truthy(obj: Object) -> bool {
    !(obj == NULL || obj == FALSE)
}

fn eval_identifier(ident: String, env: &mut Env) -> Result<Object> {
    match env.get(&ident) {
        Some(obj) => Ok(obj.clone()),
        None => Err(EvalError::Identifier(format!("unknown identifier: {ident}")).into()),
    }
}

fn eval_function_call(expr: CallExpr, env: &mut Env) -> Result<Object> {
    let function = eval(Node::Expression(*expr.function), env)?;
    let arguments = eval_expressions(&expr.arguments, env)?;
    apply_function(function, arguments)
}

fn apply_function(f: Object, args: Vec<Object>) -> Result<Object> {
    match f {
        Object::Function(func) => {
            let mut env = Env::new_enclosed(&func.env);

            func.params.iter().enumerate().for_each(|(i, param)| {
                env.set(param.to_owned(), &args[i]);
            });

            let evaluated = eval(Node::Statement(Statement::Block(func.body)), &mut env)?;

            if let Object::ReturnValue(val) = evaluated {
                Ok(*val)
            } else {
                Ok(evaluated)
            }
        }
        obj => Err(EvalError::NotAFunction(format!("expected {obj} to be a function")).into()),
    }
}
