//! Macro system implementation for runtime metaprogramming.
//!
//! This module implements a Lisp-style macro system with:
//! - Macro definition extraction from programs
//! - Macro expansion using AST transformation

use maat_ast::{Expr, Node, Program, Stmt, transform};
use maat_runtime::{Env, Macro, Object, Quote};

use crate::interpreter::eval_block_statement;

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
        if let Stmt::Let(let_stmt) = stmt
            && let Expr::Macro(macro_lit) = &let_stmt.value
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
        if let Node::Expr(Expr::Call(call_expr)) = &node
            && let Expr::Ident(ident) = &*call_expr.function
            && let Some(Object::Macro(obj)) = env.get(&ident.value)
        {
            let args = call_expr
                .arguments
                .iter()
                .map(|arg| {
                    Object::Quote(Box::new(Quote {
                        node: Node::Expr(arg.clone()),
                    }))
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
