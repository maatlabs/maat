//! Execution trace generation for the Maat STARK prover.

#![forbid(unsafe_code)]

pub mod main_segment;
pub mod mem;
pub mod public_memory;
pub mod recorder;
pub mod selector;
pub mod table;

use maat_bytecode::Bytecode;
use maat_errors::{Result, VmError};
use maat_field::{Felt, FieldElement};
use maat_runtime::{
    MaybeRelocatable, MemorySegmentManager, Relocatable, SEG_PROGRAM, SEG_PUBLIC_INPUT,
    SEG_PUBLIC_OUTPUT, Value,
};
use maat_vm::VM;
pub use mem::{Relocator, append_pubmem_dummies, fill_memory_holes, relocate_trace};
pub use public_memory::{PublicMemory, PublicSegment};
pub use recorder::TraceRecorder;
use table::TraceTable;

/// Bundle returned by [`run_with_output`]
pub struct TraceArtifacts {
    pub trace: TraceTable,
    pub result: Option<Value>,
    pub memory: PublicMemory,
}

/// Executes bytecode and returns the padded, relocated execution trace
/// alongside the program's result value (if any).
pub fn run(bytecode: Bytecode) -> Result<(TraceTable, Option<Value>)> {
    let artifacts = run_with_output(bytecode)?;
    Ok((artifacts.trace, artifacts.result))
}

/// Variant of [`run`] that extracts the public-memory segments and appends one
/// `(0, 0)` dummy row per public cell for the AIR's public-memory accumulator.
/// Equivalent to [`run_with_io`] with no inputs.
pub fn run_with_output(bytecode: Bytecode) -> Result<TraceArtifacts> {
    run_with_io(bytecode, &[], &[])
}

/// Executes bytecode whose entry point binds `inputs` as `fn main`'s `pub`
/// parameters. Equivalent to [`run_with_io`] with no private inputs.
pub fn run_with_inputs(bytecode: Bytecode, inputs: &[Felt]) -> Result<TraceArtifacts> {
    run_with_io(bytecode, inputs, &[])
}

/// Executes bytecode whose entry point binds `public_inputs` as `fn main`'s
/// `pub` parameters and `private_inputs` as its bare (witness) parameters.
pub fn run_with_io(
    bytecode: Bytecode,
    public_inputs: &[Felt],
    private_inputs: &[Felt],
) -> Result<TraceArtifacts> {
    let program_bytes = bytecode
        .serialize()
        .map_err(|e| VmError::new(format!("bytecode serialization failed: {e}")))?;

    let mut recorder = TraceRecorder::new();
    let mut vm = VM::new(bytecode);
    vm.pin_program(&program_bytes)?;
    vm.seed_public_inputs(public_inputs)?;
    vm.seed_private_inputs(private_inputs)?;
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

    let (input_base, input_segment) = extract_input_segment(vm.segments(), &relocator)?;
    let (output_base, output_segment) = extract_output_segment(vm.segments(), &relocator)?;
    let (program_base, program_segment) = extract_program_segment(vm.segments(), &relocator)?;

    let output_felt = match result.as_ref() {
        Some(Value::Relocatable(r)) => relocator
            .flatten(*r)
            .map_err(|e| VmError::new(format!("output relocation failed: {e}")))?,
        Some(v) => v.to_felt(),
        None => Felt::ZERO,
    };
    trace.stamp_output(output_felt);
    fill_memory_holes(&mut trace, vm.segments(), &relocator)?;
    append_pubmem_dummies(
        &mut trace,
        input_segment.len() + output_segment.len() + program_segment.len(),
    )?;
    trace.pad_to_power_of_two();
    Ok(TraceArtifacts {
        trace,
        result,
        memory: PublicMemory {
            input: PublicSegment::new(input_base, input_segment),
            output: PublicSegment::new(output_base, output_segment),
            program: PublicSegment::new(program_base, program_segment),
        },
    })
}

fn extract_input_segment(
    segments: &MemorySegmentManager,
    relocator: &Relocator,
) -> Result<(u32, Vec<Felt>)> {
    let sizes = segments
        .compute_sizes()
        .map_err(|e| VmError::new(format!("input segment sizing failed: {e}")))?;
    let size = usize::try_from(*sizes.get(SEG_PUBLIC_INPUT as usize).unwrap_or(&0))
        .map_err(|_| VmError::new("input segment size exceeds usize"))?;
    if size == 0 {
        return Ok((0, Vec::new()));
    }
    let input_base_felt = relocator
        .flatten(Relocatable::new(SEG_PUBLIC_INPUT, 0))
        .map_err(|e| VmError::new(format!("input segment relocation failed: {e}")))?;
    let input_base = u32::try_from(input_base_felt.as_int())
        .map_err(|_| VmError::new("input segment base exceeds u32"))?;

    let cells = (0..size)
        .map(|off| {
            let off_u32 =
                u32::try_from(off).map_err(|_| VmError::new("input segment offset exceeds u32"))?;
            let addr = Relocatable::new(SEG_PUBLIC_INPUT, off_u32);
            let cell = segments
                .read(addr)
                .unwrap_or(MaybeRelocatable::Felt(Felt::ZERO));
            let felt = match cell {
                MaybeRelocatable::Felt(f) => f,
                MaybeRelocatable::Relocatable(r) => relocator
                    .flatten(r)
                    .map_err(|e| VmError::new(format!("input cell relocation failed: {e}")))?,
            };
            Ok::<Felt, maat_errors::Error>(felt)
        })
        .collect::<Result<Vec<Felt>>>()?;

    Ok((input_base, cells))
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

fn extract_program_segment(
    segments: &MemorySegmentManager,
    relocator: &Relocator,
) -> Result<(u32, Vec<Felt>)> {
    let sizes = segments
        .compute_sizes()
        .map_err(|e| VmError::new(format!("program segment sizing failed: {e}")))?;
    let size = usize::try_from(*sizes.get(SEG_PROGRAM as usize).unwrap_or(&0))
        .map_err(|_| VmError::new("program segment size exceeds usize"))?;
    if size == 0 {
        return Ok((0, Vec::new()));
    }
    let program_base_felt = relocator
        .flatten(Relocatable::new(SEG_PROGRAM, 0))
        .map_err(|e| VmError::new(format!("program segment relocation failed: {e}")))?;
    let program_base = u32::try_from(program_base_felt.as_int())
        .map_err(|_| VmError::new("program segment base exceeds u32"))?;

    let cells = (0..size)
        .map(|off| {
            let off_u32 = u32::try_from(off)
                .map_err(|_| VmError::new("program segment offset exceeds u32"))?;
            let addr = Relocatable::new(SEG_PROGRAM, off_u32);
            let cell = segments
                .read(addr)
                .unwrap_or(MaybeRelocatable::Felt(Felt::ZERO));
            let felt = match cell {
                MaybeRelocatable::Felt(f) => f,
                MaybeRelocatable::Relocatable(r) => relocator
                    .flatten(r)
                    .map_err(|e| VmError::new(format!("program cell relocation failed: {e}")))?,
            };
            Ok::<Felt, maat_errors::Error>(felt)
        })
        .collect::<Result<Vec<Felt>>>()?;

    Ok((program_base, cells))
}
