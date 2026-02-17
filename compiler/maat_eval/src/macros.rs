//! Macro system implementation for runtime metaprogramming.
//!
//! This module implements a Lisp-style macro system with:
//! - Macro definition extraction from programs
//! - Macro expansion using AST transformation
//! - Quote/unquote builtins for AST manipulation

use maat_ast::{self as ast, Expression, Node, Program, Statement, transform};
use maat_runtime::{Env, Macro, Object, Quote, UNQUOTE};

use crate::eval::{eval_block_statement, eval_expression};

/// Extracts macro definitions from a program and stores them in the environment.
///
/// This function traverses the program's statements, identifies `let` bindings that
/// define macros, stores them in the environment, and returns a modified program
/// with the macro definitions removed.
///
/// # Parameters
///
/// * `program` - The AST program to extract macros from
/// * `env` - The environment to store macro definitions in
///
/// # Returns
///
/// A new program with macro definitions removed
pub fn define_macros(mut program: Program, env: &Env) -> Program {
    let mut defs = Vec::new();

    for (i, stmt) in program.statements.iter().enumerate() {
        if let Statement::Let(let_stmt) = stmt
            && let Expression::Macro(macro_lit) = &let_stmt.value
        {
            let macro_obj = Object::Macro(Macro {
                params: macro_lit.params.clone(),
                body: macro_lit.body.clone(),
                env: env.clone(),
            });
            env.set(let_stmt.ident.clone(), &macro_obj);
            defs.push(i);
        }
    }

    // Remove macro definitions in reverse order to maintain correct indices
    for &i in defs.iter().rev() {
        program.statements.remove(i);
    }

    program
}

/// Expands macro calls in the AST using the macro definitions in the environment.
///
/// This function traverses the entire AST and replaces macro calls with their
/// expanded forms. Macro expansion happens before evaluation.
///
/// # Parameters
///
/// * `program` - The AST node to expand macros in
/// * `env` - The environment containing macro definitions
///
/// # Returns
///
/// A new AST node with all macro calls expanded
pub fn expand_macros(program: Node, env: &Env) -> Node {
    transform(program, &mut |node| {
        if let Node::Expression(Expression::Call(call_expr)) = &node
            && let Expression::Identifier(ident) = &*call_expr.function
            && let Some(Object::Macro(obj)) = env.get(ident)
        {
            let args = call_expr
                .arguments
                .iter()
                .map(|arg| {
                    Object::Quote(Quote {
                        node: Node::Expression(arg.clone()),
                    })
                })
                .collect::<Vec<_>>();

            if args.len() != obj.params.len() {
                return node;
            }

            // Create extended environment for macro evaluation
            let ext_env = Env::new_enclosed(&obj.env);
            for (param, arg) in obj.params.iter().zip(args.iter()) {
                ext_env.set(param.clone(), arg);
            }

            let evaluated = match eval_block_statement(&obj.body, &ext_env) {
                Ok(obj) => obj,
                Err(_) => return node,
            };

            if let Object::Quote(obj) = evaluated {
                return obj.node;
            }
        }
        node
    })
}

/// Evaluates unquote calls within a quoted AST node.
///
/// This function traverses the AST and replaces `unquote` calls with
/// their evaluated results.
pub(crate) fn eval_unquote_calls(quoted: Node, env: &Env) -> Node {
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

            match object_to_node(&unquoted) {
                Some(ast_node) => ast_node,
                None => node,
            }
        } else {
            node
        }
    })
}

/// Checks if a node is a call to the `unquote` builtin.
fn is_unquote_call(node: &Node) -> bool {
    if let Node::Expression(Expression::Call(call)) = node
        && let Expression::Identifier(ident) = &*call.function
    {
        return ident == UNQUOTE;
    }
    false
}

/// Converts a runtime object back to an AST node.
///
/// This is used to splice evaluated values back into quoted code.
fn object_to_node(obj: &Object) -> Option<Node> {
    use ast::Radix;

    macro_rules! convert_int {
        ($($obj:ident => $ast_name:ident($ast_type:ident)),* $(,)?) => {
            match obj {
                $(
                    Object::$obj(v) => Some(Node::Expression(Expression::$ast_name(ast::$ast_type {
                        radix: Radix::Dec,
                        value: *v,
                    }))),
                )*
                Object::Boolean(b) => Some(Node::Expression(Expression::Boolean(*b))),
                Object::Quote(q) => Some(q.node.clone()),
                _ => None,
            }
        };
    }

    convert_int!(
        I8 => I8(I8),
        I16 => I16(I16),
        I32 => I32(I32),
        I64 => I64(I64),
        I128 => I128(I128),
        Isize => Isize(Isize),
        U8 => U8(U8),
        U16 => U16(U16),
        U32 => U32(U32),
        U64 => U64(U64),
        U128 => U128(U128),
        Usize => Usize(Usize),
    )
}
