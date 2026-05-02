//! Execution trace generation for the Maat STARK prover.

#![forbid(unsafe_code)]

pub mod recorder;
pub mod selector;
pub mod table;

use maat_bytecode::Bytecode;
use maat_errors::Result;
use maat_field::{Felt, FieldElement};
use maat_runtime::Value;
use maat_vm::VM;
pub use recorder::TraceRecorder;
use table::TraceTable;

/// Executes bytecode and returns the padded execution trace alongside
/// the program's result value (if any).
pub fn run(bytecode: Bytecode) -> Result<(TraceTable, Option<Value>)> {
    let mut recorder = TraceRecorder::new();
    let mut vm = VM::new(bytecode);
    vm.run_with_recorder(&mut recorder)?;
    let result = vm.last_popped_stack_elem().cloned();

    let mut trace = recorder.into_trace();

    let output_felt = result.as_ref().map(|v| v.to_felt()).unwrap_or(Felt::ZERO);
    trace.stamp_output(output_felt);
    trace.pad_to_power_of_two();
    Ok((trace, result))
}
