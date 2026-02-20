//! Shared utilities for integration tests.

use maat_ast::{Node, Program};
use maat_bytecode::Bytecode;
use maat_codegen::Compiler;
use maat_lexer::Lexer;
use maat_parser::Parser;

/// Parses the given source string into an AST [`Program`].
///
/// # Panics
///
/// Panics if the parser encounters any errors.
pub fn parse(input: &str) -> Program {
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(
        parser.errors().is_empty(),
        "parser errors: {:?}",
        parser.errors()
    );
    program
}

/// Compiles the given source string into [`Bytecode`].
///
/// # Panics
///
/// Panics if parsing or compilation fails.
pub fn compile(input: &str) -> Bytecode {
    let program = parse(input);
    let mut compiler = Compiler::new();
    compiler
        .compile(&Node::Program(program))
        .expect("compilation failed");
    compiler.bytecode().expect("bytecode extraction failed")
}
