//! File execution for the `maat run` command.
//!
//! Reads a `.mt` source file, compiles it to bytecode, and executes it
//! on the VM. The result of the last expression is printed to stdout.

use std::path::Path;
use std::process;

use maat_vm::VM;

use crate::pipeline;

/// Reads, compiles, and executes a Maat source file.
///
/// The execution pipeline:
/// 1. Compile the source file to bytecode via [`pipeline::compile_source`]
/// 2. Execute the bytecode on the VM
/// 3. Print the result of the last expression (if non-null)
pub fn compile_and_run(path: &Path) {
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
