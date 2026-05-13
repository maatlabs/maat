//! Execution trace generation for the Maat STARK prover.

#![forbid(unsafe_code)]

pub mod holes;
pub mod main_segment;
pub mod recorder;
pub mod relocation;
pub mod selector;
pub mod table;

pub use holes::fill_memory_holes;
use maat_bytecode::Bytecode;
use maat_errors::{Result, VmError};
use maat_field::{Felt, FieldElement};
use maat_runtime::Value;
use maat_vm::VM;
pub use recorder::TraceRecorder;
pub use relocation::{Relocator, relocate_trace};
use table::TraceTable;

/// Executes bytecode and returns the padded, relocated execution trace
/// alongside the program's result value (if any).
pub fn run(bytecode: Bytecode) -> Result<(TraceTable, Option<Value>)> {
    let mut recorder = TraceRecorder::new();
    let mut vm = VM::new(bytecode);
    vm.run_with_recorder(&mut recorder)?;
    let result = vm.last_popped_stack_elem().cloned();

    let segment_sizes = vm
        .segments()
        .compute_sizes()
        .map_err(|e| VmError::new(format!("segment size computation failed: {e}")))?;
    let (mut trace, plans, heap_base) = recorder.finish();
    let relocator = Relocator::new(&segment_sizes, heap_base)
        .map_err(|e| VmError::new(format!("relocation table build failed: {e}")))?;
    relocate_trace(&mut trace, &plans, &relocator)?;

    let output_felt = match result.as_ref() {
        Some(Value::Relocatable(r)) => relocator
            .flatten(*r)
            .map_err(|e| VmError::new(format!("output relocation failed: {e}")))?,
        Some(v) => v.to_felt(),
        None => Felt::ZERO,
    };
    trace.stamp_output(output_felt);
    fill_memory_holes(&mut trace, vm.segments(), &relocator)?;
    trace.pad_to_power_of_two();
    Ok((trace, result))
}
