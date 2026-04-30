//! Execution trace callback surface.

use maat_bytecode::Opcode;
use maat_errors::Result;
use maat_field::Felt;

/// Callback surface the VM dispatch loop consults at each instrumentation point.
///
/// The VM pays no overhead beyond the call-site itself: every method has a
/// default no-op body and the dispatch loop is generic over the recorder type, so a
/// [`NoOpRecorder`] disappears under monomorphization. The trace-generating crate
/// (`maat_trace`) supplies a real implementation that owns the trace row
/// buffer and the trace-only state.
pub trait Tracer {
    /// Called once per instruction, before the opcode handler runs.
    #[inline(always)]
    fn before_dispatch(&mut self, _ctx: DispatchCtx) {}

    /// Records the output value for the current row.
    #[inline(always)]
    fn record_out(&mut self, _value: Felt) {}

    /// Records a read or write of a global variable slot.
    #[inline(always)]
    fn record_global_access(&mut self, _index: usize, _value: Felt, _is_read: bool) {}

    /// Records a read or write of a local variable slot in the active frame.
    #[inline(always)]
    fn record_local_access(&mut self, _local_index: usize, _value: Felt, _is_read: bool) {}

    /// Records a heap access (alloc, read, or write) against the unified
    /// memory permutation argument. `heap_id` is a unique-per-event identifier
    /// allocated by the VM; the recorder lifts it out of the locals/globals
    /// logical address space before threading it through the same code path
    /// used for globals and locals.
    #[inline(always)]
    fn record_heap_access(&mut self, _heap_id: usize, _value: Felt, _is_read: bool) {}

    /// Records entry into a closure frame: emits one synthetic per-parameter
    /// memory-write row plus the saved-frame-pointer write that goes on the
    /// current `Call` row, and updates the recorder's logical frame-pointer.
    #[inline(always)]
    fn record_call_closure(&mut self, _ctx: CallCtx<'_>) -> Result<()> {
        Ok(())
    }

    /// Records a builtin call (no frame change, no memory writes).
    #[inline(always)]
    fn record_call_builtin(&mut self) {}

    /// Records a function return: emits the saved-frame-pointer read into the
    /// current row and pops the recorder's logical frame-pointer.
    #[inline(always)]
    fn record_return(&mut self) -> Result<()> {
        Ok(())
    }

    /// Supplies the witness data needed by the division/modulo AIR rule.
    #[inline(always)]
    fn record_div_mod_witness(&mut self, _op: Opcode, _s0: Felt, _s1: Felt, _result: Felt) {}

    /// Supplies the witness data needed by the equality/inequality AIR rule.
    #[inline(always)]
    fn record_cmp_witness(&mut self, _s0: Felt, _s1: Felt) {}

    /// Supplies the witness data needed by the type-conversion range check.
    #[inline(always)]
    fn record_convert_witness(&mut self, _result: Felt) {}

    /// Marks the current row complete; the recorder commits it to its buffer.
    #[inline(always)]
    fn end_row(&mut self) {}

    /// Called once after the dispatch loop terminates, with the post-execution
    /// program counter and stack pointer. Used to emit a final NOP row.
    #[inline(always)]
    fn finalize(&mut self, _final_pc: usize, _final_sp: usize) {}
}

/// Snapshot of the pre-execution machine state for one instruction.
#[derive(Debug, Clone, Copy)]
pub struct DispatchCtx {
    /// Instruction pointer (within the active frame's instruction stream).
    pub ip: usize,
    /// The decoded opcode.
    pub op: Opcode,
    /// First operand (zero if absent).
    pub operand0: usize,
    /// Second operand (zero if absent).
    pub operand1: usize,
    /// Operand stack pointer (depth of the operand stack).
    pub sp: usize,
    /// Stack top encoded as a Goldilocks field element (or zero if empty).
    pub s0: Felt,
    /// Stack second element encoded as a field element (or zero if absent).
    pub s1: Felt,
    /// Stack third element encoded as a field element (or zero if absent).
    pub s2: Felt,
}

/// Pre-frame-push call site context, supplied to [`Tracer::record_call_closure`].
#[derive(Debug, Clone, Copy)]
pub struct CallCtx<'a> {
    /// Instruction pointer of the `Call` opcode.
    pub call_ip: usize,
    /// Stack pointer at the moment of the call (before frame setup).
    pub sp_at_call: usize,
    /// Number of locals declared by the caller's frame.
    pub caller_num_locals: usize,
    /// Argument values flowing into the callee, encoded as field elements.
    pub args: &'a [Felt],
}

/// A recorder that drops every event. Used by the VM run path so
/// the dispatch loop monomorphises into the same instructions as the
/// pre-instrumentation implementation.
pub struct NoOpRecorder;

impl Tracer for NoOpRecorder {}
