use std::path::Path;
use std::process;

use maat_ast::Node;
use maat_ast::fold::fold_constants;
use maat_bytecode::Bytecode;
use maat_codegen::Compiler;
use maat_eval::{define_macros, expand_macros};
use maat_lexer::Lexer;
use maat_parser::Parser;
use maat_runtime::Env;
use maat_types::TypeChecker;
use maat_vm::VM;

use crate::diagnostic;

/// Bytecode compilation for the `maat build` command.
///
/// Compiles a source file and writes serialized bytecode to disk.
/// If `output_path` is `None`, the output file is derived from the
/// source path by replacing its extension with `.mtc`.
pub fn build(source_path: &Path, output_path: Option<&Path>) {
    require_extension(source_path, "mt", "build");

    let bytecode = compile_source(source_path);

    let bytes = match bytecode.serialize() {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "error: failed to serialize bytecode for '{}': {e}",
                source_path.display()
            );
            process::exit(1);
        }
    };

    let default_output = source_path.with_extension("mtc");
    let out = output_path.unwrap_or(&default_output);

    if let Err(e) = std::fs::write(out, bytes) {
        eprintln!("error: cannot write '{}': {e}", out.display());
        process::exit(1);
    }

    eprintln!("compiled {} -> {}", source_path.display(), out.display());
}

/// Pre-compiled bytecode execution for the `maat exec` command.
///
/// Reads a `.mtc` bytecode file, deserializes into [`Bytecode`], and runs it
/// on the VM. The result of the last expression is printed to stdout.
///
/// Since no original source is available, error diagnostics fall back to
/// plain messages without source snippets.
pub fn execute(path: &Path) {
    require_extension(path, "mtc", "exec");

    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read '{}': {e}", path.display());
            process::exit(1);
        }
    };

    let bytecode = match Bytecode::deserialize(&bytes) {
        Ok(bc) => bc,
        Err(e) => {
            eprintln!("error: failed to deserialize '{}': {e}", path.display());
            process::exit(1);
        }
    };

    let mut vm = VM::new(bytecode);
    if let Err(e) = vm.run() {
        eprintln!("{}: vm error: {e}", path.display());
        process::exit(1);
    }

    if let Some(result) = vm.last_popped_stack_elem()
        && !matches!(result, maat_runtime::Object::Null)
    {
        println!("{result}");
    }
}

/// File execution for the `maat run` command.
///
/// Reads, compiles, and executes a Maat source file.
/// The execution pipeline:
/// 1. Compile the source file to bytecode via [`compile_source`]
/// 2. Execute the bytecode on the VM
/// 3. Print the result of the last expression (if non-null)
pub fn run(path: &Path) {
    require_extension(path, "mt", "run");

    let bytecode = compile_source(path);

    let mut vm = VM::new(bytecode);
    if let Err(e) = vm.run() {
        eprintln!("{}: vm error: {}", path.display(), e);
        process::exit(1);
    }

    if let Some(result) = vm.last_popped_stack_elem()
        && !matches!(result, maat_runtime::Object::Null)
    {
        println!("{result}");
    }
}

/// Validates that a file path has the expected extension, exiting with a
/// diagnostic message if it does not.
fn require_extension(path: &Path, expected: &str, command: &str) {
    let actual = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if actual != expected {
        eprintln!(
            "error: `maat {command}` expects a `.{expected}` file, got '{}'",
            path.display(),
        );
        std::process::exit(1);
    }
}

/// Shared utils for the `run` and `build` commands.
/// Compiles a `.mt` source file to [`Bytecode`].
///
/// Runs the full pipeline: read -> parse -> macro expand -> compile.
/// Prints rich diagnostics and exits the process on any error.
fn compile_source(path: &Path) -> Bytecode {
    let source = read_source_file(path);
    let filename = path.display().to_string();

    let mut parser = Parser::new(Lexer::new(&source));
    let program = parser.parse();

    if !parser.errors().is_empty() {
        for err in parser.errors() {
            diagnostic::report_parse_error(&filename, &source, err);
        }
        process::exit(1);
    }

    let macro_env = Env::default();
    let program = define_macros(program, &macro_env);
    let expanded = expand_macros(Node::Program(program), &macro_env);
    let mut program = match expanded {
        Node::Program(p) => p,
        _ => unreachable!("expand_macros preserves Program variant"),
    };

    let type_errors = TypeChecker::new().check_program(&mut program);
    if !type_errors.is_empty() {
        for err in &type_errors {
            diagnostic::report_type_error(&filename, &source, err);
        }
        process::exit(1);
    }

    let fold_errors = fold_constants(&mut program);
    if !fold_errors.is_empty() {
        for err in &fold_errors {
            diagnostic::report_type_error(&filename, &source, err);
        }
        process::exit(1);
    }

    let mut compiler = Compiler::new();
    if let Err(e) = compiler.compile(&Node::Program(program)) {
        diagnostic::report_error(&filename, &source, &e);
        process::exit(1);
    }

    match compiler.bytecode() {
        Ok(bc) => bc,
        Err(e) => {
            diagnostic::report_error(&filename, &source, &e);
            process::exit(1);
        }
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
