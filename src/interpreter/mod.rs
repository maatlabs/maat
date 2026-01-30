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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lexer, Parser};

    fn test_eval(input: &str) -> Result<Object> {
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program();
        assert!(
            parser.errors().is_empty(),
            "parser errors: {:?}",
            parser.errors()
        );
        let env = Env::default();
        eval(Node::Program(program), &env)
    }

    #[test]
    fn eval_int64_expression() {
        [
            ("5", 5),
            ("10", 10),
            ("-5", -5),
            ("-10", -10),
            ("5 + 5 + 5 + 5 - 10", 10),
            ("2 * 2 * 2 * 2 * 2", 32),
            ("-50 + 100 + -50", 0),
            ("5 * 2 + 10", 20),
            ("5 + 2 * 10", 25),
            ("20 + 2 * -10", 0),
            ("50 / 2 * 2 + 10", 60),
            ("2 * (5 + 10)", 30),
            ("3 * 3 * 3 + 10", 37),
            ("3 * (3 * 3) + 10", 37),
            ("(5 + 10 * 2 + 15 / 3) * 2 + -10", 50),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::Int64(*expected), "input: {}", input);
        });
    }

    #[test]
    fn eval_boolean_expression() {
        [
            ("true", true),
            ("false", false),
            ("1 < 2", true),
            ("1 > 2", false),
            ("1 < 1", false),
            ("1 > 1", false),
            ("1 == 1", true),
            ("1 != 1", false),
            ("1 == 2", false),
            ("1 != 2", true),
            ("true == true", true),
            ("false == false", true),
            ("true == false", false),
            ("true != false", true),
            ("false != true", true),
            ("(1 < 2) == true", true),
            ("(1 < 2) == false", false),
            ("(1 > 2) == true", false),
            ("(1 > 2) == false", true),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::Boolean(*expected), "input: {}", input);
        });
    }

    #[test]
    fn eval_bang_operator() {
        [
            ("!true", false),
            ("!false", true),
            ("!5", false),
            ("!!true", true),
            ("!!false", false),
            ("!!5", true),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::Boolean(*expected), "input: {}", input);
        });
    }

    #[test]
    fn eval_if_else_expressions() {
        [
            ("if (true) { 10 }", Some(10)),
            ("if (false) { 10 }", None),
            ("if (1) { 10 }", Some(10)),
            ("if (1 < 2) { 10 }", Some(10)),
            ("if (1 > 2) { 10 }", None),
            ("if (1 > 2) { 10 } else { 20 }", Some(20)),
            ("if (1 < 2) { 10 } else { 20 }", Some(10)),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            match expected {
                Some(val) => assert_eq!(result, Object::Int64(*val), "input: {}", input),
                None => assert_eq!(result, Object::Null, "input: {}", input),
            }
        });
    }

    #[test]
    fn eval_return_statements() {
        [
            ("return 10;", 10),
            ("return 10; 9;", 10),
            ("return 2 * 5; 9;", 10),
            ("9; return 2 * 5; 9;", 10),
            ("if (10 > 1) { return 10; }", 10),
            ("if (10 > 1) { if (10 > 1) { return 10; } return 1; }", 10),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::Int64(*expected), "input: {}", input);
        });
    }

    #[test]
    fn eval_let_statements() {
        [
            ("let a = 5; a;", 5),
            ("let a = 5 * 5; a;", 25),
            ("let a = 5; let b = a; b;", 5),
            ("let a = 5; let b = a; let c = a + b + 5; c;", 15),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::Int64(*expected), "input: {}", input);
        });
    }

    #[test]
    fn eval_function_object() {
        let input = "fn(x) { x + 2; };";
        let result = test_eval(input).unwrap();
        match result {
            Object::Function(func) => {
                assert_eq!(func.params.len(), 1);
                assert_eq!(func.params[0], "x");
                assert_eq!(func.body.statements.len(), 1);
            }
            _ => panic!("expected Function object, got {:?}", result),
        }
    }

    #[test]
    fn eval_function_application() {
        [
            ("let identity = fn(x) { x; }; identity(5);", 5),
            ("let identity = fn(x) { return x; }; identity(5);", 5),
            ("let double = fn(x) { x * 2; }; double(5);", 10),
            ("let add = fn(x, y) { x + y; }; add(5, 5);", 10),
            ("let add = fn(x, y) { x + y; }; add(5 + 5, add(5, 5));", 20),
            ("fn(x) { x; }(5)", 5),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::Int64(*expected), "input: {}", input);
        });
    }

    #[test]
    fn eval_closures() {
        let input = "
            let newAdder = fn(x) {
                fn(y) { x + y };
            };
            let addTwo = newAdder(2);
            addTwo(2);
        ";
        let result = test_eval(input).unwrap();
        assert_eq!(result, Object::Int64(4));
    }

    #[test]
    fn eval_string_literals() {
        [
            (r#""Hello World!""#, "Hello World!"),
            (r#""foo bar""#, "foo bar"),
            (r#""""#, ""),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(
                result,
                Object::String(expected.to_string()),
                "input: {}",
                input
            );
        });
    }

    #[test]
    fn eval_string_concatenation() {
        [
            (r#""Hello" + " " + "World!""#, "Hello World!"),
            (r#""foo" + "bar""#, "foobar"),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(
                result,
                Object::String(expected.to_string()),
                "input: {}",
                input
            );
        });
    }

    #[test]
    fn eval_array_literals() {
        let result = test_eval("[1, 2 * 2, 3 + 3]").unwrap();
        match result {
            Object::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0], Object::Int64(1));
                assert_eq!(arr[1], Object::Int64(4));
                assert_eq!(arr[2], Object::Int64(6));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn eval_array_index_expressions() {
        [
            ("[1, 2, 3][0]", Some(1)),
            ("[1, 2, 3][1]", Some(2)),
            ("[1, 2, 3][2]", Some(3)),
            ("let i = 0; [1][i];", Some(1)),
            ("[1, 2, 3][1 + 1];", Some(3)),
            ("let myArray = [1, 2, 3]; myArray[2];", Some(3)),
            (
                "let myArray = [1, 2, 3]; myArray[0] + myArray[1] + myArray[2];",
                Some(6),
            ),
            (
                "let myArray = [1, 2, 3]; let i = myArray[0]; myArray[i]",
                Some(2),
            ),
            ("[1, 2, 3][3]", None),
            ("[1, 2, 3][-1]", None),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            match expected {
                Some(val) => assert_eq!(result, Object::Int64(*val), "input: {}", input),
                None => assert_eq!(result, NULL, "input: {}", input),
            }
        });
    }

    #[test]
    fn eval_hash_literals() {
        let input = r#"
            let two = "two";
            {
                "one": 10 - 9,
                two: 1 + 1,
                "thr" + "ee": 6 / 2,
                4: 4,
                true: 5,
                false: 6
            }
        "#;
        let result = test_eval(input).unwrap();
        match result {
            Object::Hash(hash) => {
                assert_eq!(hash.pairs.len(), 6);

                let expected = [
                    (Hashable::String("one".to_string()), Object::Int64(1)),
                    (Hashable::String("two".to_string()), Object::Int64(2)),
                    (Hashable::String("three".to_string()), Object::Int64(3)),
                    (Hashable::Int64(4), Object::Int64(4)),
                    (Hashable::Boolean(true), Object::Int64(5)),
                    (Hashable::Boolean(false), Object::Int64(6)),
                ];

                for (key, value) in expected {
                    assert_eq!(hash.pairs.get(&key), Some(&value));
                }
            }
            _ => panic!("expected hash"),
        }
    }

    #[test]
    fn eval_hash_index_expressions() {
        [
            (r#"{"foo": 5}["foo"]"#, Some(5)),
            (r#"{"foo": 5}["bar"]"#, None),
            (r#"let key = "foo"; {"foo": 5}[key]"#, Some(5)),
            (r#"{}["foo"]"#, None),
            (r#"{5: 5}[5]"#, Some(5)),
            (r#"{true: 5}[true]"#, Some(5)),
            (r#"{false: 5}[false]"#, Some(5)),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            match expected {
                Some(val) => assert_eq!(result, Object::Int64(*val), "input: {}", input),
                None => assert_eq!(result, NULL, "input: {}", input),
            }
        });
    }

    #[test]
    fn eval_builtin_len() {
        [
            (r#"len("")"#, Some(0)),
            (r#"len("four")"#, Some(4)),
            (r#"len("hello world")"#, Some(11)),
            ("len([1, 2, 3])", Some(3)),
            ("len([])", Some(0)),
            ("len(1)", None),
        ]
        .iter()
        .for_each(|(input, expected)| match expected {
            Some(val) => {
                let result = test_eval(input).unwrap();
                assert_eq!(result, Object::Int64(*val), "input: {}", input);
            }
            None => {
                assert!(test_eval(input).is_err(), "expected error for: {}", input);
            }
        });
    }

    #[test]
    fn eval_builtin_first() {
        [
            ("first([1, 2, 3])", Some(1)),
            ("first([10, 20])", Some(10)),
            ("first([])", None),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            match expected {
                Some(val) => assert_eq!(result, Object::Int64(*val), "input: {}", input),
                None => assert_eq!(result, NULL, "input: {}", input),
            }
        });
    }

    #[test]
    fn eval_builtin_last() {
        [
            ("last([1, 2, 3])", Some(3)),
            ("last([10, 20])", Some(20)),
            ("last([])", None),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            match expected {
                Some(val) => assert_eq!(result, Object::Int64(*val), "input: {}", input),
                None => assert_eq!(result, NULL, "input: {}", input),
            }
        });
    }

    #[test]
    fn eval_builtin_rest() {
        [
            ("rest([1, 2, 3])", Some(vec![2, 3])),
            ("rest([10, 20])", Some(vec![20])),
            ("rest([])", None),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            match expected {
                Some(vals) => match result {
                    Object::Array(arr) => {
                        assert_eq!(arr.len(), vals.len());
                        for (i, val) in vals.iter().enumerate() {
                            assert_eq!(arr[i], Object::Int64(*val));
                        }
                    }
                    _ => panic!("expected array for: {}", input),
                },
                None => assert_eq!(result, NULL, "input: {}", input),
            }
        });
    }

    #[test]
    fn eval_builtin_push() {
        let result = test_eval("push([1, 2], 3)").unwrap();
        match result {
            Object::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0], Object::Int64(1));
                assert_eq!(arr[1], Object::Int64(2));
                assert_eq!(arr[2], Object::Int64(3));
            }
            _ => panic!("expected array"),
        }

        let result = test_eval("push([], 1)").unwrap();
        match result {
            Object::Array(arr) => {
                assert_eq!(arr.len(), 1);
                assert_eq!(arr[0], Object::Int64(1));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn eval_float_arithmetic() {
        [
            ("3.14 + 2.86", 6.0),
            ("10.5 - 5.5", 5.0),
            ("2.5 * 4.0", 10.0),
            ("10.0 / 2.0", 5.0),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            match result {
                Object::Float64(val) => {
                    assert!((val - expected).abs() < 1e-10, "input: {}", input);
                }
                _ => panic!("expected float for: {}", input),
            }
        });
    }

    #[test]
    fn eval_binary_literals() {
        [
            ("0b1010", 10),
            ("0B1111", 15),
            ("0b0", 0),
            ("0b11111111", 255),
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::Int64(*expected), "input: {}", input);
        });
    }

    #[test]
    fn eval_octal_literals() {
        [("0o755", 493), ("0O644", 420), ("0o0", 0), ("0o10", 8)]
            .iter()
            .for_each(|(input, expected)| {
                let result = test_eval(input).unwrap();
                assert_eq!(result, Object::Int64(*expected), "input: {}", input);
            });
    }

    #[test]
    fn eval_hex_literals() {
        [("0xff", 255), ("0xFF", 255), ("0xDEAD", 57005), ("0x0", 0)]
            .iter()
            .for_each(|(input, expected)| {
                let result = test_eval(input).unwrap();
                assert_eq!(result, Object::Int64(*expected), "input: {}", input);
            });
    }

    #[test]
    fn eval_rust_style_suffixes() {
        [("123i64", 123), ("456i64", 456), ("0i64", 0)]
            .iter()
            .for_each(|(input, expected)| {
                let result = test_eval(input).unwrap();
                assert_eq!(result, Object::Int64(*expected), "input: {}", input);
            });

        [("3.14f64", 3.14), ("0.5f64", 0.5)]
            .iter()
            .for_each(|(input, expected)| {
                let result = test_eval(input).unwrap();
                match result {
                    Object::Float64(val) => {
                        assert!((val - expected).abs() < 1e-10, "input: {}", input);
                    }
                    _ => panic!("expected float for: {}", input),
                }
            });
    }

    #[test]
    fn eval_radix_arithmetic() {
        [
            ("0b1010 + 0o12", 20),    // 10 + 10 = 20
            ("0xff - 0b11111111", 0), // 255 - 255 = 0
            ("0x10 * 2", 32),         // 16 * 2 = 32
        ]
        .iter()
        .for_each(|(input, expected)| {
            let result = test_eval(input).unwrap();
            assert_eq!(result, Object::Int64(*expected), "input: {}", input);
        });
    }

    #[test]
    fn error_handling() {
        [
            ("5 + true;", "invalid infix expression"),
            ("5 + true; 5;", "invalid infix expression"),
            ("-true", "cannot be negated"),
            ("true + false;", "invalid boolean operation"),
            ("5; true + false; 5", "invalid boolean operation"),
            ("if (10 > 1) { true + false; }", "invalid boolean operation"),
            (
                "if (10 > 1) { if (10 > 1) { return true + false; } return 1; }",
                "invalid boolean operation",
            ),
            ("foobar", "unknown identifier"),
        ]
        .iter()
        .for_each(|(input, expected_msg)| {
            let result = test_eval(input);
            assert!(result.is_err(), "expected error for input: {}", input);
            let err = result.unwrap_err();
            assert!(
                err.to_string().contains(expected_msg),
                "expected error containing '{}', got '{}'",
                expected_msg,
                err
            );
        });
    }
}
