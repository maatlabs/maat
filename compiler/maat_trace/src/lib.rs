//! Trace-generating virtual machine for the Maat ZK backend.
//!
//! This crate provides a VM that executes Maat bytecode while recording a
//! 36-column execution trace suitable for STARK proving. Each instruction
//! step produces one row capturing the program counter, stack state, memory
//! accesses, a one-hot opcode selector, and range-check witness data.
//!
//! # Architecture
//!
//! - **[`TraceTable`]** stores the trace matrix and handles power-of-two
//!   padding required by the Winterfell FRI prover.
//! - **[`TraceVM`]** mirrors `maat_vm::VM` but instruments every instruction
//!   to emit a trace row.
//! - **[`selector`]** maps each opcode to one of 17 constraint classes.
//! - **[`encode`]** converts runtime values to Goldilocks field elements.
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

pub mod encode;
pub mod selector;
pub mod table;
pub mod vm;

use maat_bytecode::Bytecode;
use maat_errors::Result;
use maat_runtime::Value;
pub use table::{
    COL_CMP_INV, COL_DIV_AUX, COL_FP, COL_IS_READ, COL_MEM_ADDR, COL_MEM_VAL, COL_NONZERO_INV,
    COL_OP_WIDTH, COL_OPCODE, COL_OPERAND_0, COL_OPERAND_1, COL_OUT, COL_PC, COL_RC_L0, COL_RC_L1,
    COL_RC_L2, COL_RC_L3, COL_RC_VAL, COL_S0, COL_S1, COL_S2, COL_SEL_BASE, COL_SP,
    COL_SUB_SEL_BASE, COLUMN_NAMES, NUM_SUB_SELECTORS, SUB_SEL_ADD, SUB_SEL_DIV, SUB_SEL_EQ,
    SUB_SEL_FELT_ADD, SUB_SEL_FELT_MUL, SUB_SEL_FELT_SUB, SUB_SEL_NEG, SUB_SEL_NEQ, SUB_SEL_SUB,
    TRACE_WIDTH, TraceRow, TraceTable,
};
pub use vm::TraceVM;

/// Executes bytecode and returns the padded execution trace alongside
/// the program's result value (if any).
///
/// This is the primary entry point for trace generation. The returned
/// [`TraceTable`] is padded to a power-of-two length (minimum 8 rows)
/// as required by the STARK prover.
pub fn run_trace(bytecode: Bytecode) -> Result<(TraceTable, Option<Value>)> {
    let mut vm = TraceVM::new(bytecode);
    vm.run()?;
    let result = vm.last_popped_stack_elem().cloned();
    let mut trace = vm.into_trace();

    // Stamp the program output onto the last execution row so the boundary
    // assertion `out[last] = public_output` holds after NOP padding.
    let output_felt = result
        .as_ref()
        .map(encode::value_to_felt)
        .unwrap_or(maat_field::Felt::ZERO);
    trace.stamp_output(output_felt);

    trace.pad_to_power_of_two();
    Ok((trace, result))
}
