use std::path::Path;
use std::process;

use maat_bytecode::Bytecode;
use maat_vm::VM;

use crate::pipeline;

/// Bytecode compilation for the `maat build` command.
///
/// Compiles a source file and writes serialized bytecode to disk.
/// If `output_path` is `None`, the output file is derived from the
/// source path by replacing its extension with `.mtc`.
pub fn build(source_path: &Path, output_path: Option<&Path>) {
    require_extension(source_path, "mt", "build");

    let bytecode = pipeline::compile_source(source_path);

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
/// 1. Compile the source file to bytecode via [`pipeline::compile_source`]
/// 2. Execute the bytecode on the VM
/// 3. Print the result of the last expression (if non-null)
pub fn run(path: &Path) {
    require_extension(path, "mt", "run");

    let bytecode = pipeline::compile_source(path);

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
