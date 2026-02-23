//! File execution for the `maat run` command.
//!
//! Reads a `.mt` source file, compiles it to bytecode, and executes it
//! on the VM. The result of the last expression is printed to stdout.

use std::path::Path;
use std::{fs, process};

use maat_ast::Node;
use maat_codegen::Compiler;
use maat_errors::Error;
use maat_eval::{define_macros, expand_macros};
use maat_lexer::Lexer;
use maat_parser::Parser;
use maat_runtime::Env;
use maat_vm::VM;

use crate::diagnostic;

/// Reads, compiles, and executes a Maat source file.
///
/// The execution pipeline:
/// 1. Read the source file with UTF-8 validation and BOM rejection
/// 2. Parse the source into an AST
/// 3. Process macro definitions and expand macro calls
/// 4. Compile the expanded AST to bytecode
/// 5. Execute the bytecode on the VM
/// 6. Print the result of the last expression (if non-null)
pub fn execute(path: &Path) {
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

    let bytecode = match compiler.bytecode() {
        Ok(bc) => bc,
        Err(e) => {
            report_error(&filename, &source, &e);
            process::exit(1);
        }
    };

    let mut vm = VM::new(bytecode);
    if let Err(e) = vm.run() {
        report_error(&filename, &source, &e);
        process::exit(1);
    }

    if let Some(result) = vm.last_popped_stack_elem()
        && !matches!(result, maat_runtime::Object::Null)
    {
        println!("{result}");
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
    let bytes = match fs::read(path) {
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
