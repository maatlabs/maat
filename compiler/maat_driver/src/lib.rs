//! This crate is effectively the "main" function for the Maat compiler.
//! It orchestrates the compilation process and "knits together" the code from the other crates within maatc.
//! This crate itself does not contain any of the "main logic" of the compiler. It purely re-exports.

pub use maat_ast::ast::*;
pub use maat_ast::{Node, Program};
pub use maat_errors::{Error, EvalError, ParseError, Result};
pub use maat_eval::{Env, FALSE, Hashable, NULL, Object, TRUE, define_macros, eval, expand_macros};
pub use maat_lexer::{Lexer, Span, Token, TokenKind};
pub use maat_parse::{Parser, TransformFn, transform};
pub use {maat_ast, maat_errors, maat_eval, maat_lexer, maat_parse};
