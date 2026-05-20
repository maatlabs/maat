//! Execution trace generation for the Maat STARK prover.

#![forbid(unsafe_code)]

pub mod main_segment;
pub mod mem;
pub mod recorder;
pub mod selector;
pub mod table;

use maat_bytecode::Bytecode;
use maat_errors::{Result, VmError};
use maat_field::{Felt, FieldElement};
use maat_runtime::{MaybeRelocatable, MemorySegmentManager, Relocatable, SEG_PUBLIC_OUTPUT, Value};
use maat_vm::VM;
pub use mem::{
    Relocator, append_pubmem_dummies, fill_memory_holes, fill_range_check_gaps, relocate_trace,
};
pub use recorder::TraceRecorder;
use table::TraceTable;

/// Bundle returned by [`run_with_output`]
pub struct TraceArtifacts {
    pub trace: TraceTable,
    pub result: Option<Value>,
    pub output_base: u32,
    pub output_segment: Vec<Felt>,
}

/// Executes bytecode and returns the padded, relocated execution trace
/// alongside the program's result value (if any).
pub fn run(bytecode: Bytecode) -> Result<(TraceTable, Option<Value>)> {
    let artifacts = run_with_output(bytecode)?;
    Ok((artifacts.trace, artifacts.result))
}

/// Variant of [`run`] that also extracts the public-output segment cells
/// from [`SEG_PUBLIC_OUTPUT`] and appends `l = output_segment.len()` `(0, 0)`
/// dummy rows to the trace for the AIR's public-memory accumulator.
pub fn run_with_output(bytecode: Bytecode) -> Result<TraceArtifacts> {
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

    let (output_base, output_segment) = extract_output_segment(vm.segments(), &relocator)?;

    let output_felt = match result.as_ref() {
        Some(Value::Relocatable(r)) => relocator
            .flatten(*r)
            .map_err(|e| VmError::new(format!("output relocation failed: {e}")))?,
        Some(v) => v.to_felt(),
        None => Felt::ZERO,
    };
    trace.stamp_output(output_felt);
    fill_memory_holes(&mut trace, vm.segments(), &relocator)?;
    append_pubmem_dummies(&mut trace, output_segment.len())?;
    fill_range_check_gaps(&mut trace)?;
    trace.pad_to_power_of_two();
    Ok(TraceArtifacts {
        trace,
        result,
        output_base,
        output_segment,
    })
}

fn extract_output_segment(
    segments: &MemorySegmentManager,
    relocator: &Relocator,
) -> Result<(u32, Vec<Felt>)> {
    let sizes = segments
        .compute_sizes()
        .map_err(|e| VmError::new(format!("output segment sizing failed: {e}")))?;
    let size = usize::try_from(*sizes.get(SEG_PUBLIC_OUTPUT as usize).unwrap_or(&0))
        .map_err(|_| VmError::new("output segment size exceeds usize"))?;
    if size == 0 {
        return Ok((0, Vec::new()));
    }
    let output_base_felt = relocator
        .flatten(Relocatable::new(SEG_PUBLIC_OUTPUT, 0))
        .map_err(|e| VmError::new(format!("output segment relocation failed: {e}")))?;
    let output_base = u32::try_from(output_base_felt.as_int())
        .map_err(|_| VmError::new("output segment base exceeds u32"))?;

    let cells = (0..size)
        .map(|off| {
            let off_u32 = u32::try_from(off)
                .map_err(|_| VmError::new("output segment offset exceeds u32"))?;
            let addr = Relocatable::new(SEG_PUBLIC_OUTPUT, off_u32);
            let cell = segments
                .read(addr)
                .unwrap_or(MaybeRelocatable::Felt(Felt::ZERO));
            let felt = match cell {
                MaybeRelocatable::Felt(f) => f,
                MaybeRelocatable::Relocatable(r) => relocator
                    .flatten(r)
                    .map_err(|e| VmError::new(format!("output cell relocation failed: {e}")))?,
            };
            Ok::<Felt, maat_errors::Error>(felt)
        })
        .collect::<Result<Vec<Felt>>>()?;

    Ok((output_base, cells))
}
