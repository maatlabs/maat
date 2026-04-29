//! Execution trace generation for the Maat STARK prover.
//!
//! This crate executes Maat bytecode through [`maat_vm::VM`]
//! while recording a 36-column execution trace into a [`TraceTable`]
//! suitable for STARK proving.
//!
//! # Range-check columns
//!
//! Columns 30--35 carry the range-check sub-AIR witness:
//!
//! - `rc_val`: the value being range-checked (zero on non-trigger rows).
//! - `rc_l0`..`rc_l3`: 16-bit limb decomposition of `rc_val`.
//! - `nonzero_inv`: multiplicative inverse of the divisor on `sel_div_mod`
//!   rows, proving the divisor is non-zero.
//!
//! # Usage
//!
//! ```ignore
//! use maat_trace::run_trace;
//!
//! let (trace, result) = run_trace(bytecode)?;
//! println!("{}", trace.to_csv());
//! ```
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
    COL_CMP_INV, COL_DIV_AUX, COL_FP, COL_HEAP_ADDR, COL_HEAP_ALLOC_FLAG, COL_HEAP_IS_READ,
    COL_HEAP_VAL, COL_IS_READ, COL_MEM_ADDR, COL_MEM_VAL, COL_NONZERO_INV, COL_OP_WIDTH,
    COL_OPCODE, COL_OPERAND_0, COL_OPERAND_1, COL_OUT, COL_PC, COL_RC_L0, COL_RC_L1, COL_RC_L2,
    COL_RC_L3, COL_RC_VAL, COL_S0, COL_S1, COL_S2, COL_SEL_BASE, COL_SP, COL_SUB_SEL_BASE,
    COLUMN_NAMES, TRACE_WIDTH, TraceRow, TraceTable,
};

/// Executes bytecode and returns the padded execution trace alongside
/// the program's result value (if any).
///
/// This is the primary entry point for trace generation. The returned
/// [`TraceTable`] is padded to a power-of-two length (minimum 32 rows) as
/// required by the STARK prover.
pub fn run_trace(bytecode: Bytecode) -> Result<(TraceTable, Option<Value>)> {
    let mut recorder = TraceRecorder::new();
    let mut vm = VM::new(bytecode);
    vm.run_with_recorder(&mut recorder)?;
    let result = vm.last_popped_stack_elem().cloned();

    let mut trace = recorder.into_trace();
    trace.validate_address_contiguity()?;

    let output_felt = result.as_ref().map(|v| v.to_felt()).unwrap_or(Felt::ZERO);
    trace.stamp_output(output_felt);
    trace.pad_to_power_of_two();
    Ok((trace, result))
}
