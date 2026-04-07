use std::fs::File;
use std::io::{self, BufWriter};
use std::path::Path;
use std::process;

use maat_bytecode::Bytecode;
use maat_module::{check_and_compile, resolve_module_graph};
use maat_runtime::Value;
use maat_vm::VM;

use crate::diagnostic;

/// Bytecode compilation for the `maat build` command.
///
/// Resolves module imports relative to the source file, builds the
/// module dependency graph, type-checks and compiles all modules into
/// linked bytecode, and writes the serialized result to disk.
///
/// If `output_path` is `None`, the output file is derived from the
/// source path by replacing its extension with `.mtc`.
pub fn build(source_path: &Path, output_path: Option<&Path>) {
    require_extension(source_path, "maat", "build");

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
        eprintln!("error: {}: {e}", path.display());
        process::exit(1);
    }
    if let Some(result) = vm.last_popped_stack_elem()
        && !matches!(result, Value::Unit)
    {
        println!("{result}");
    }
}

/// File execution for the `maat run` command.
///
/// Resolves module imports relative to the source file, builds the
/// module dependency graph, type-checks and compiles all modules into
/// linked bytecode, then executes it on the VM. The result of the last
/// expression (if non-unit) is printed to stdout.
pub fn run(path: &Path) {
    require_extension(path, "maat", "run");

    let bytecode = compile_source(path);
    let mut vm = VM::new(bytecode);
    if let Err(e) = vm.run() {
        eprintln!("error: {}: {}", path.display(), e);
        process::exit(1);
    }
    if let Some(result) = vm.last_popped_stack_elem()
        && !matches!(result, Value::Unit)
    {
        println!("{result}");
    }
}

/// Trace execution for the `maat trace` command.
///
/// Compiles the source file and runs it through the trace-generating VM,
/// producing a CSV execution trace. If `output_path` is `None`, the trace
/// is written to stdout; otherwise it is written to the specified file.
pub fn trace(path: &Path, output_path: Option<&Path>) {
    require_extension(path, "maat", "trace");

    let bytecode = compile_source(path);
    let (trace, result) = match maat_trace::run_trace(bytecode) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("error: {}: {e}", path.display());
            process::exit(1);
        }
    };

    match output_path {
        Some(out) => {
            let file = match File::create(out) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("error: cannot write '{}': {e}", out.display());
                    process::exit(1);
                }
            };
            if let Err(e) = trace.write_csv(BufWriter::new(file)) {
                eprintln!("error: failed to write trace CSV: {e}");
                process::exit(1);
            }
            eprintln!("trace: {} rows -> {}", trace.num_rows(), out.display());
        }
        None => {
            let stdout = io::stdout();
            if let Err(e) = trace.write_csv(BufWriter::new(stdout.lock())) {
                eprintln!("error: failed to write trace CSV: {e}");
                process::exit(1);
            }
        }
    }

    if let Some(val) = result
        && !matches!(val, Value::Unit)
    {
        eprintln!("result: {val}");
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

/// Compiles a `.maat` source file (and all its module dependencies) to
/// linked [`Bytecode`].
///
/// Runs the full multi-module pipeline:
/// 1. Resolve the module dependency graph starting from the entry file
/// 2. Type-check each module independently with visibility enforcement
/// 3. Compile all modules with a shared compiler (implicit linking)
///
/// Prints diagnostics and exits the process on any error.
fn compile_source(path: &Path) -> Bytecode {
    let mut graph = match resolve_module_graph(path) {
        Ok(g) => g,
        Err(e) => {
            diagnostic::report_module_error(&e);
            process::exit(1);
        }
    };
    match check_and_compile(&mut graph) {
        Ok(bc) => bc,
        Err(e) => {
            diagnostic::report_module_error(&e);
            process::exit(1);
        }
    }
}
