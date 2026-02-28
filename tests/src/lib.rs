//! Shared utilities for integration tests.

use maat_ast::{Node, Program};
use maat_bytecode::Bytecode;
use maat_codegen::Compiler;
use maat_lexer::Lexer;
use maat_parser::Parser;
use maat_types::{TypeChecker, fold_constants};

/// Parses the given source string into an AST [`Program`].
///
/// # Panics
///
/// Panics if the parser encounters any errors.
pub fn parse(input: &str) -> Program {
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer);
    let program = parser.parse();
    assert!(
        parser.errors().is_empty(),
        "parser errors: {:?}",
        parser.errors()
    );
    program
}

/// Compiles the given source string into [`Bytecode`].
///
/// Runs the full pipeline: parse -> type check -> constant fold -> compile.
///
/// # Panics
///
/// Panics if parsing, type checking, or compilation fails.
pub fn compile(input: &str) -> Bytecode {
    let mut program = parse(input);

    let type_errors = TypeChecker::new().check_program(&mut program);
    assert!(type_errors.is_empty(), "type errors: {:?}", type_errors);

    let fold_errors = fold_constants(&mut program);
    assert!(
        fold_errors.is_empty(),
        "constant folding errors: {:?}",
        fold_errors
    );

    let mut compiler = Compiler::new();
    compiler
        .compile(&Node::Program(program))
        .expect("compilation failed");
    compiler.bytecode().expect("bytecode extraction failed")
}

/// Compiles the given source string into [`Bytecode`] without type checking
/// or constant folding.
///
/// This is used by compiler tests that assert on exact bytecode layout,
/// where constant folding would alter the expected instruction sequences.
///
/// # Panics
///
/// Panics if parsing or compilation fails.
pub fn compile_raw(input: &str) -> Bytecode {
    let program = parse(input);
    let mut compiler = Compiler::new();
    compiler
        .compile(&Node::Program(program))
        .expect("compilation failed");
    compiler.bytecode().expect("bytecode extraction failed")
}

/// Compiles the given source string, expecting type errors.
///
/// Returns the type error messages for assertion.
///
/// # Panics
///
/// Panics if parsing fails.
pub fn compile_type_errors(input: &str) -> Vec<String> {
    let mut program = parse(input);
    let type_errors = TypeChecker::new().check_program(&mut program);
    type_errors.iter().map(|e| e.kind.to_string()).collect()
}

/// Compiles the given source string, serializes the bytecode, deserializes
/// it, and returns the restored [`Bytecode`].
///
/// This exercises the full round-trip through the binary format, ensuring
/// that execution from deserialized bytecode produces the same results as
/// direct compilation.
///
/// # Panics
///
/// Panics if parsing, compilation, serialization, or deserialization fails.
pub fn roundtrip(input: &str) -> Bytecode {
    let bytecode = compile(input);
    let bytes = bytecode.serialize().expect("serialization failed");
    Bytecode::deserialize(&bytes).expect("deserialization failed")
}
