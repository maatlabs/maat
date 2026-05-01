//! Execution trace generation for the Maat STARK prover.

#![forbid(unsafe_code)]

pub mod recorder;
pub mod table;

use maat_bytecode::Bytecode;
use maat_errors::Result;
use maat_field::{Felt, FieldElement};
use maat_runtime::Value;
use maat_vm::VM;
pub use recorder::TraceRecorder;
pub use table::{
    COL_CMP_INV, COL_DIV_AUX, COL_FP, COL_IS_READ, COL_MEM_ADDR, COL_MEM_VAL, COL_NONZERO_INV,
    COL_OP_WIDTH, COL_OPERAND_0, COL_OUT, COL_PC, COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3,
    COL_RC_VAL, COL_S0, COL_S1, COL_S2, COL_SEL_BASE, COL_SP, COL_SUB_SEL_BASE, COLUMN_NAMES,
    TRACE_WIDTH, TraceRow, TraceTable,
};

/// Executes bytecode and returns the padded execution trace alongside
/// the program's result value (if any).
///
/// This is the primary entry point for trace generation. The returned
/// [`TraceTable`] is padded to a power-of-two length (minimum 8 rows) as
/// required by the Winterfell prover.
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
