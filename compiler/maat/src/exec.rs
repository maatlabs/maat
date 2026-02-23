//! Pre-compiled bytecode execution for the `maat exec` command.
//!
//! Reads a `.mtc` bytecode file, deserializes it, and executes it on the VM.
//! Since no original source is available, error diagnostics fall back to
//! plain messages without source snippets.

use std::path::Path;
use std::process;

use maat_bytecode::Bytecode;
use maat_vm::VM;

/// Loads and executes a pre-compiled `.mtc` bytecode file.
///
/// Reads the binary from disk, deserializes into [`Bytecode`], and runs it
/// on the VM. The result of the last expression is printed to stdout.
pub fn execute_bytecode(path: &Path) {
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
