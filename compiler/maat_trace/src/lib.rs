//! Trace-generating virtual machine for the Maat ZK backend.
//!
//! This crate provides a VM that executes Maat bytecode while recording a
//! 29-column execution trace suitable for STARK proving. Each instruction
//! step produces one row capturing the program counter, stack state, memory
//! accesses, and a one-hot opcode selector.
//!
//! # Architecture
//!
//! - **[`TraceTable`]** stores the trace matrix and handles power-of-two
//!   padding required by the Winterfell FRI prover.
//! - **[`TraceVM`]** mirrors `maat_vm::VM` but instruments every instruction
//!   to emit a trace row.
//! - **[`selector`]** maps each opcode to one of 16 constraint classes.
//! - **[`encode`]** converts runtime values to Goldilocks field elements.
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
    COL_FP, COL_IS_READ, COL_MEM_ADDR, COL_MEM_VAL, COL_OPCODE, COL_OPERAND_0, COL_OPERAND_1,
    COL_OUT, COL_PC, COL_S0, COL_S1, COL_S2, COL_SEL_BASE, COL_SP, COLUMN_NAMES, TRACE_WIDTH,
    TraceRow, TraceTable,
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
    trace.pad_to_power_of_two();
    Ok((trace, result))
}
