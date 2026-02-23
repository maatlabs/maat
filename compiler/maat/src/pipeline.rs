//! Shared utils for the `run` and `build` commands.
//!
//! Provides a single function that reads a `.mt` source file and compiles
//! it through every phase (parse -> macro expansion -> codegen) into
//! [`Bytecode`].

use std::path::Path;
use std::process;

use maat_ast::Node;
use maat_bytecode::Bytecode;
use maat_codegen::Compiler;
use maat_errors::Error;
use maat_eval::{define_macros, expand_macros};
use maat_lexer::Lexer;
use maat_parser::Parser;
use maat_runtime::Env;

use crate::diagnostic;

/// Compiles a `.mt` source file to [`Bytecode`].
///
/// Runs the full pipeline: read -> parse -> macro expand -> compile.
/// Prints rich diagnostics and exits the process on any error.
pub fn compile_source(path: &Path) -> Bytecode {
    let source = read_source_file(path);
    let filename = path.display().to_string();

    let mut parser = Parser::new(Lexer::new(&source));
    let program = parser.parse_program();

    if !parser.errors().is_empty() {
        for err in parser.errors() {
            diagnostic::report_parse_error(&filename, &source, err);
        }
        process::exit(1);
    }

    let macro_env = Env::default();
    let program = define_macros(program, &macro_env);
    let expanded = expand_macros(Node::Program(program), &macro_env);
    let program = match expanded {
        Node::Program(p) => p,
        _ => unreachable!("expand_macros preserves Program variant"),
    };

    let mut compiler = Compiler::new();
    if let Err(e) = compiler.compile(&Node::Program(program)) {
        report_error(&filename, &source, &e);
        process::exit(1);
    }

    match compiler.bytecode() {
        Ok(bc) => bc,
        Err(e) => {
            report_error(&filename, &source, &e);
            process::exit(1);
        }
    }
}

/// Routes an [`Error`] to the appropriate diagnostic reporter.
fn report_error(path: &str, source: &str, error: &Error) {
    match error {
        Error::Parse(e) => diagnostic::report_parse_error(path, source, e),
        Error::Compile(e) => diagnostic::report_compile_error(path, source, e),
        Error::Vm(e) => diagnostic::report_vm_error(path, source, e),
        _ => eprintln!("{path}: {error}"),
    }
}

/// Reads a source file with UTF-8 validation and BOM rejection.
///
/// Exits with an error message if the file cannot be read, contains
/// invalid UTF-8, or starts with a byte-order mark.
fn read_source_file(path: &Path) -> String {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read '{}': {e}", path.display());
            process::exit(1);
        }
    };

    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        eprintln!(
            "error: '{}' starts with a UTF-8 BOM. Maat source files must not contain a byte-order mark",
            path.display()
        );
        process::exit(1);
    }

    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("error: '{}' is not valid UTF-8", path.display());
            process::exit(1);
        }
    }
}
