//! Macro expansion engine for Maat.
//!
//! This crate provides the macro system that powers Maat's metaprogramming
//! capabilities. It exposes two primary operations:
//!
//! - [`extract_macros`] -- Extracts macro definitions from a program and stores
//!   them in the environment.
//! - [`expand_macros`] -- Replaces macro calls in the AST with their expanded forms.
//!
//! Internally, macro bodies are evaluated using a tree-walking interpreter
//! ([`eval`]) that is not exposed as a general-purpose execution API. Program
//! execution is handled exclusively by the bytecode VM (`maat_vm`).
#![forbid(unsafe_code)]

mod interpreter;

pub use interpreter::{eval, eval_block_statement};
use maat_ast::{Expr, Node, Program, Stmt, transform};
use maat_runtime::{Env, Macro, Quote, Value};

/// The name of the `quote` special form for AST quoting.
///
/// Used to capture AST nodes without evaluation, enabling metaprogramming.
/// This is a special form handled directly in the evaluator, not a regular builtin.
pub const QUOTE: &str = "quote";

/// The name of the `unquote` special form for splicing evaluated expressions into quotes.
///
/// Used within `quote` to evaluate and splice expressions into the quoted AST.
/// This is a special form handled during quote evaluation, not a regular builtin.
pub const UNQUOTE: &str = "unquote";

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
pub fn extract_macros(mut program: Program, env: &Env) -> Program {
    let mut defs = Vec::new();

    for (i, stmt) in program.statements.iter().enumerate() {
        if let Stmt::Let(l) = stmt
            && let Expr::MacroLit(m) = &l.value
        {
            let val = Value::Macro(Macro {
                params: m.params.clone(),
                body: m.body.clone(),
                env: env.clone(),
            });
            env.set(l.ident.clone(), &val);
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
            && let Some(Value::Macro(val)) = env.get(&ident.value)
        {
            let args = call_expr
                .arguments
                .iter()
                .map(|arg| {
                    Value::Quote(Box::new(Quote {
                        node: Node::Expr(arg.clone()),
                    }))
                })
                .collect::<Vec<_>>();

            if args.len() != val.params.len() {
                return node;
            }

            // Create extended environment for macro evaluation
            let ext_env = Env::new_enclosed(&val.env);
            for (param, arg) in val.params.iter().zip(args.iter()) {
                ext_env.set(param.clone(), arg);
            }

            let evaluated = match eval_block_statement(&val.body, &ext_env) {
                Ok(val) => val,
                Err(_) => return node,
            };

            if let Value::Quote(val) = evaluated {
                return val.node;
            }
        }
        node
    })
}
