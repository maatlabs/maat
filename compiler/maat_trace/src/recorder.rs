//! Trace recorder: a [`Tracer`] implementation that materialises the
//! execution trace consumed by the STARK prover.

use std::collections::HashMap;

use maat_bytecode::selector::SEL_NOP;
use maat_bytecode::{MAX_GLOBALS, Opcode};
use maat_errors::{Result, VmError};
use maat_field::{Felt, FieldElement, try_inv};
use maat_vm::trace::{CallCtx, DispatchCtx, Tracer};

use crate::table::*;

/// Logical-address offset that lifts heap accesses out of the locals/globals
/// region of the unified memory segment.
const HEAP_LOGICAL_BASE: usize = 1usize << 32;

/// Decomposes a 64-bit value into four 16-bit limbs `[l0, l1, l2, l3]` such
/// that `val = l0 + 2^16 l1 + 2^32 l2 + 2^48 l3`.
fn decompose_limbs(val: u64) -> [Felt; 4] {
    [
        Felt::new(val & 0xFFFF),
        Felt::new((val >> 16) & 0xFFFF),
        Felt::new((val >> 32) & 0xFFFF),
        Felt::new((val >> 48) & 0xFFFF),
    ]
}

pub struct TraceRecorder {
    trace: TraceTable,
    current: TraceRow,
    alloc_ptr: usize,
    addr_map: HashMap<usize, usize>,
    fp: usize,
    fp_stack: Vec<usize>,
    last_mem_addr: Felt,
    last_mem_val: Felt,
}

impl TraceRecorder {
    pub fn new() -> Self {
        Self {
            trace: TraceTable::new(),
            current: [Felt::ZERO; TRACE_WIDTH],
            alloc_ptr: 1,
            addr_map: HashMap::new(),
            fp: MAX_GLOBALS,
            fp_stack: Vec::new(),
            last_mem_addr: Felt::ZERO,
            last_mem_val: Felt::ZERO,
        }
    }

    pub fn into_trace(self) -> TraceTable {
        self.trace
    }

    fn alloc_physical(&mut self) -> Result<usize> {
        let physical = self.alloc_ptr;
        self.alloc_ptr = self
            .alloc_ptr
            .checked_add(1)
            .ok_or_else(|| VmError::new("memory allocator overflow"))?;
        Ok(physical)
    }

    fn record_mem_write(&mut self, logical: usize, value: Felt) -> Result<()> {
        let physical = self.alloc_physical()?;
        self.addr_map.insert(logical, physical);
        let addr_felt = Felt::new(physical as u64);
        self.current[COL_MEM_ADDR] = addr_felt;
        self.current[COL_MEM_VAL] = value;
        self.current[COL_IS_READ] = Felt::ZERO;
        self.last_mem_addr = addr_felt;
        self.last_mem_val = value;
        Ok(())
    }

    fn record_mem_read(&mut self, logical: usize, value: Felt) -> Result<()> {
        let physical = *self.addr_map.get(&logical).ok_or_else(|| {
            VmError::new(format!(
                "memory read of unallocated logical address {logical}"
            ))
        })?;
        let addr_felt = Felt::new(physical as u64);
        self.current[COL_MEM_ADDR] = addr_felt;
        self.current[COL_MEM_VAL] = value;
        self.current[COL_IS_READ] = Felt::ONE;
        self.last_mem_addr = addr_felt;
        self.last_mem_val = value;
        Ok(())
    }

    fn emit_parameter_writes(
        &mut self,
        call_ip: usize,
        sp_at_call: usize,
        caller_fp: usize,
        new_fp: usize,
        args: &[Felt],
    ) -> Result<()> {
        let pc = Felt::new(call_ip as u64);
        let sp = Felt::new(sp_at_call as u64);
        let fp = Felt::new(caller_fp as u64);
        let out = Felt::new(new_fp as u64);
        for (i, &arg_felt) in args.iter().enumerate() {
            let physical = self.alloc_physical()?;
            let logical = new_fp
                .checked_add(i)
                .ok_or_else(|| VmError::new("frame pointer overflow"))?;
            self.addr_map.insert(logical, physical);

            let mem_addr = Felt::new(physical as u64);
            let mut row = [Felt::ZERO; TRACE_WIDTH];
            row[COL_PC] = pc;
            row[COL_SP] = sp;
            row[COL_FP] = fp;
            row[COL_OUT] = out;
            row[COL_SEL_BASE + SEL_NOP] = Felt::ONE;
            row[COL_MEM_ADDR] = mem_addr;
            row[COL_MEM_VAL] = arg_felt;
            row[COL_IS_READ] = Felt::ZERO;

            self.last_mem_addr = mem_addr;
            self.last_mem_val = arg_felt;
            self.trace.push_row(row);
        }
        Ok(())
    }
}

impl Default for TraceRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Tracer for TraceRecorder {
    fn before_dispatch(&mut self, ctx: DispatchCtx) {
        let info = ctx.op.info();
        let mut row = [Felt::ZERO; TRACE_WIDTH];
        row[COL_PC] = Felt::new(ctx.ip as u64);
        row[COL_SP] = Felt::new(ctx.sp as u64);
        row[COL_FP] = Felt::new(self.fp as u64);
        row[COL_OPERAND_0] = Felt::new(ctx.operand0 as u64);
        row[COL_S0] = ctx.s0;
        row[COL_S1] = ctx.s1;
        row[COL_S2] = ctx.s2;
        row[COL_OP_WIDTH] = Felt::new(info.instruction_width() as u64);
        row[COL_SEL_BASE + info.selector] = Felt::ONE;
        if let Some(sub) = info.sub_selector {
            row[COL_SUB_SEL_BASE + sub] = Felt::ONE;
        }
        // Default to a dummy memory read; real accesses overwrite these
        // fields in the corresponding `record_*` methods.
        row[COL_MEM_ADDR] = self.last_mem_addr;
        row[COL_MEM_VAL] = self.last_mem_val;
        row[COL_IS_READ] = Felt::ONE;
        self.current = row;
    }

    fn record_out(&mut self, value: Felt) {
        self.current[COL_OUT] = value;
    }

    fn record_global_access(&mut self, index: usize, value: Felt, is_read: bool) {
        let result = if is_read {
            self.record_mem_read(index, value)
        } else {
            self.record_mem_write(index, value)
        };
        if let Err(e) = result {
            // Memory access failure indicates an internal trace-recorder
            // invariant violation; surface it loudly rather than masking it.
            panic!("trace recorder: global access failed: {e}");
        }
    }

    fn record_local_access(&mut self, local_index: usize, value: Felt, is_read: bool) {
        let logical = self.fp.wrapping_add(local_index);
        let result = if is_read {
            self.record_mem_read(logical, value)
        } else {
            self.record_mem_write(logical, value)
        };
        if let Err(e) = result {
            panic!("trace recorder: local access failed: {e}");
        }
    }

    fn record_heap_access(&mut self, heap_id: usize, value: Felt, is_read: bool) {
        let logical = HEAP_LOGICAL_BASE.wrapping_add(heap_id);
        let result = if is_read {
            self.record_mem_read(logical, value)
        } else {
            self.record_mem_write(logical, value)
        };
        if let Err(e) = result {
            panic!("trace recorder: heap access failed: {e}");
        }
    }

    fn record_call_closure(&mut self, ctx: CallCtx<'_>) -> Result<()> {
        let caller_fp = self.fp;
        let new_fp = caller_fp
            .checked_add(ctx.caller_num_locals)
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| VmError::new("frame pointer overflow"))?;

        self.emit_parameter_writes(ctx.call_ip, ctx.sp_at_call, caller_fp, new_fp, ctx.args)?;

        let saved_fp_physical = self.alloc_physical()?;
        let saved_fp_logical = new_fp
            .checked_sub(1)
            .ok_or_else(|| VmError::new("frame pointer underflow allocating saved FP"))?;
        self.addr_map.insert(saved_fp_logical, saved_fp_physical);
        let mem_addr = Felt::new(saved_fp_physical as u64);
        let mem_val = Felt::new(caller_fp as u64);
        self.current[COL_MEM_ADDR] = mem_addr;
        self.current[COL_MEM_VAL] = mem_val;
        self.current[COL_IS_READ] = Felt::ZERO;
        self.last_mem_addr = mem_addr;
        self.last_mem_val = mem_val;

        self.fp_stack.push(caller_fp);
        self.fp = new_fp;
        self.current[COL_OUT] = Felt::new(new_fp as u64);
        Ok(())
    }

    fn record_call_builtin(&mut self) {
        self.current[COL_OUT] = Felt::new(self.fp as u64);
    }

    fn record_return(&mut self) -> Result<()> {
        let saved_fp_logical = self
            .fp
            .checked_sub(1)
            .ok_or_else(|| VmError::new("frame pointer underflow on return"))?;
        let saved_fp_physical = *self
            .addr_map
            .get(&saved_fp_logical)
            .ok_or_else(|| VmError::new("saved FP slot not allocated on return"))?;
        let caller_fp = self.fp_stack.pop().unwrap_or(MAX_GLOBALS);
        let mem_addr = Felt::new(saved_fp_physical as u64);
        let mem_val = Felt::new(caller_fp as u64);
        self.current[COL_MEM_ADDR] = mem_addr;
        self.current[COL_MEM_VAL] = mem_val;
        self.current[COL_IS_READ] = Felt::ONE;
        self.last_mem_addr = mem_addr;
        self.last_mem_val = mem_val;
        self.fp = caller_fp;
        Ok(())
    }

    fn record_div_mod_witness(&mut self, op: Opcode, divisor: Felt, dividend: Felt, result: Felt) {
        if divisor == Felt::ZERO {
            return;
        }
        let inv = try_inv(divisor).expect("non-zero field element has inverse");
        self.current[COL_NONZERO_INV] = inv;
        self.current[COL_DIV_AUX] = match op {
            Opcode::Div => dividend - divisor * result,
            Opcode::Mod => inv * (dividend - result),
            _ => Felt::ZERO,
        };
    }

    fn record_cmp_witness(&mut self, s0: Felt, s1: Felt) {
        let diff = s0 - s1;
        if diff != Felt::ZERO {
            self.current[COL_CMP_INV] = try_inv(diff).expect("non-zero field element has inverse");
        }
    }

    fn record_lt_gt_witness(&mut self, op: Opcode, s0: Felt, s1: Felt, result: Felt) {
        let one = Felt::ONE;
        let one_minus_result = one - result;
        let c = match op {
            Opcode::LessThan => result * (s0 - s1 - one) + one_minus_result * (s1 - s0),
            Opcode::GreaterThan => result * (s1 - s0 - one) + one_minus_result * (s0 - s1),
            _ => return,
        };
        self.current[COL_RC_VAL] = c;
        let limbs = decompose_limbs(c.as_int());
        self.current[COL_RC_L0] = limbs[0];
        self.current[COL_RC_L1] = limbs[1];
        self.current[COL_RC_L2] = limbs[2];
        self.current[COL_RC_L3] = limbs[3];
    }

    fn record_convert_witness(&mut self, result: Felt) {
        let raw = result.as_int();
        self.current[COL_RC_VAL] = result;
        let limbs = decompose_limbs(raw);
        self.current[COL_RC_L0] = limbs[0];
        self.current[COL_RC_L1] = limbs[1];
        self.current[COL_RC_L2] = limbs[2];
        self.current[COL_RC_L3] = limbs[3];
    }

    fn end_row(&mut self) {
        let row = std::mem::replace(&mut self.current, [Felt::ZERO; TRACE_WIDTH]);
        self.trace.push_row(row);
    }

    fn finalize(&mut self, final_pc: usize, final_sp: usize) {
        let mut row = [Felt::ZERO; TRACE_WIDTH];
        row[COL_PC] = Felt::new(final_pc as u64);
        row[COL_SP] = Felt::new(final_sp as u64);
        row[COL_FP] = Felt::new(self.fp as u64);
        row[COL_SEL_BASE + SEL_NOP] = Felt::ONE;
        row[COL_MEM_ADDR] = self.last_mem_addr;
        row[COL_MEM_VAL] = self.last_mem_val;
        row[COL_IS_READ] = Felt::ONE;
        self.trace.push_row(row);
    }
}
