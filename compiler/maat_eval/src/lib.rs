//! Evaluation engine for Maat.
//!
//! This crate provides two levels of evaluation:
//!
//! - [`eval`] — Pure tree-walking evaluation of an AST node.
//! - [`eval_program`] — Full evaluation pipeline that processes macro definitions,
//!   expands macro calls, and then evaluates the resulting program.
//!
//! Most callers should use [`eval_program`] for top-level program evaluation,
//! while [`eval`] is available for evaluating individual nodes without macro processing.

mod interpreter;
mod macros;

pub use interpreter::eval;
use maat_ast::{Node, Program};
use maat_errors::Result;
use maat_runtime::{Env, Object};
pub use macros::{define_macros, expand_macros};

/// Evaluates a program through the full pipeline: macro definition, expansion, and evaluation.
///
/// This is the primary entry point for evaluating Maat programs. It performs three steps:
///
/// 1. Extracts and registers macro definitions from the program
/// 2. Expands all macro calls in the AST
/// 3. Evaluates the resulting program via the tree-walking interpreter
pub fn eval_program(program: Program, env: &Env) -> Result<Object> {
    let program = define_macros(program, env);
    let node = expand_macros(Node::Program(program), env);
    eval(node, env)
}
