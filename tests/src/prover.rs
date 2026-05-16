//! Utilities used by the `prove_verify.rs` integration tests

use maat_air::{MaatPublicInputs, Proof};
use maat_bytecode::{Bytecode, Instructions, Opcode, encode};
use maat_field::{BaseElement, Felt, FieldElement};
use maat_prover::{
    MaatProver, compute_program_hash, development_options, production_options, verify_with_inputs,
};
use maat_runtime::{Integer, Relocatable, SEG_PUBLIC_OUTPUT, Value};
use maat_span::SourceMap;
use maat_trace::table::{COL_OUT, COL_SUB_SEL_BASE, TraceTable};

pub fn prove_and_verify(source: &str) {
    let (bytecode, trace, output) = compile_and_trace(source);
    let (proof, public_inputs) = prove(&bytecode, trace, output);
    verify_with_inputs(proof, public_inputs).expect("verification failed");
}

pub fn compile_and_trace(source: &str) -> (Bytecode, TraceTable, BaseElement) {
    let bytecode = crate::compile(source);
    let (trace, result) = maat_trace::run(bytecode.clone()).expect("trace execution failed");
    let output = result.map(|v| v.to_felt()).unwrap_or(BaseElement::ZERO);
    (bytecode, trace, output)
}

pub fn prove(
    bytecode: &Bytecode,
    trace: TraceTable,
    output: BaseElement,
) -> (Proof, MaatPublicInputs) {
    let program_hash = compute_program_hash(bytecode).expect("program hash failed");
    let public_inputs = MaatPublicInputs::new(program_hash, vec![], output);
    let prover = MaatProver::new(development_options(), public_inputs.clone());
    let proof = prover
        .generate_proof(trace)
        .expect("proof generation failed");
    (proof, public_inputs)
}

pub fn tamper_output_on_sub_sel(trace: &mut TraceTable, sub_selector: usize) {
    let n = trace.num_rows();
    for i in 0..n {
        if trace.row(i)[COL_SUB_SEL_BASE + sub_selector] == Felt::ONE {
            let cur = trace.row(i)[COL_OUT].as_int();
            trace.row_mut(i)[COL_OUT] = Felt::new(cur.wrapping_add(1));
            return;
        }
    }
    panic!("no row with sub_selector offset {sub_selector} found in trace");
}

pub fn assert_tampered_trace_rejected(
    bytecode: Bytecode,
    trace: TraceTable,
    output: BaseElement,
    label: &str,
) {
    let program_hash = compute_program_hash(&bytecode).expect("program hash failed");
    let public_inputs = MaatPublicInputs::new(program_hash, vec![], output);
    let prover = MaatProver::new(development_options(), public_inputs.clone());

    let prove_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prover.generate_proof(trace)
    }));
    match prove_result {
        Err(_) => {}
        Ok(proof) => {
            let proof = proof.expect("proof generation failed");
            assert!(
                verify_with_inputs(proof, public_inputs).is_err(),
                "tampered {label} output must be rejected by the verifier",
            );
        }
    }
}

/// Bytecode that allocates a fresh segment, appends a single value, reads it
/// back, and discards the readback so the program output is the original value.
pub fn synthetic_segment_alloc_read_bytecode(initial_value: i64) -> Bytecode {
    let mut instructions = Instructions::new();
    instructions.extend_from_bytes(&encode(Opcode::SegmentNew, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::HeapAlloc, &[]));
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    Bytecode {
        instructions,
        constants: vec![Value::Integer(Integer::I64(initial_value))],
        source_map: SourceMap::new(),
        type_registry: vec![],
    }
}

/// Bytecode that allocates two independent segments, writes one cell into
/// each, then reads both back.
pub fn synthetic_two_segments_bytecode(seg_a_value: i64, seg_b_value: i64) -> Bytecode {
    let mut instructions = Instructions::new();
    // Segment A: SegmentNew, push value, HeapAlloc; drop the cell address but
    // keep the segment-base pointer live.
    instructions.extend_from_bytes(&encode(Opcode::SegmentNew, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::HeapAlloc, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    // Segment B: SegmentNew, push value, HeapAlloc; drop the cell address.
    instructions.extend_from_bytes(&encode(Opcode::SegmentNew, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::HeapAlloc, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    // Stack now: `[Rel(A, 0), Rel(B, 0)]`. Read segment B, discard the value,
    // then read segment A so the final `last_popped` is `seg_a_value`.
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    Bytecode {
        instructions,
        constants: vec![
            Value::Integer(Integer::I64(seg_a_value)),
            Value::Integer(Integer::I64(seg_b_value)),
        ],
        source_map: SourceMap::new(),
        type_registry: vec![],
    }
}

/// Bytecode that writes one value to a cell, then attempts to overwrite it
/// with a different value. The VM rejects the second write; the prover never runs.
pub fn synthetic_write_once_violation_bytecode(initial: i64, conflict: i64) -> Bytecode {
    let mut instructions = Instructions::new();
    instructions.extend_from_bytes(&encode(Opcode::SegmentNew, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::HeapAlloc, &[]));
    // Stash the just-allocated cell address.
    instructions.extend_from_bytes(&encode(Opcode::SetLocal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    // Push the address and the conflicting value, then attempt HeapWrite.
    instructions.extend_from_bytes(&encode(Opcode::GetLocal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::HeapWrite, &[]));
    Bytecode {
        instructions,
        constants: vec![
            Value::Integer(Integer::I64(initial)),
            Value::Integer(Integer::I64(conflict)),
        ],
        source_map: SourceMap::new(),
        type_registry: vec![],
    }
}

/// Bytecode that stores a `Relocatable` as a heap cell value, reads it back,
/// dereferences it, and returns the dereferenced integer.
pub fn synthetic_relocatable_cell_value_bytecode(payload: i64) -> Bytecode {
    let mut instructions = Instructions::new();
    // Build segment A and write `payload` at offset 0.
    instructions.extend_from_bytes(&encode(Opcode::SegmentNew, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::HeapAlloc, &[]));
    // Drop the cell address; keep the segment-A base pointer.
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    // Stash the segment-A base in global 0 (operand stack reuse-safe).
    instructions.extend_from_bytes(&encode(Opcode::SetGlobal, &[0]));
    // Build segment B and write `Rel(A, 0)` (the stashed pointer) into it.
    instructions.extend_from_bytes(&encode(Opcode::SegmentNew, &[]));
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::HeapAlloc, &[]));
    // Stack: `[Rel(B, 0)]`. Read `Rel(B, 0)` -- the cell value is the stored
    // pointer, which the relocator must rewrite to the flat address of seg A.
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    // Stack: `[pointer-to-seg-A]`. Dereference it to read the payload.
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    Bytecode {
        instructions,
        constants: vec![Value::Integer(Integer::I64(payload))],
        source_map: SourceMap::new(),
        type_registry: vec![],
    }
}

/// Bytecode that writes two values into the same segment at offsets `0` and
/// `5`. The resulting effective segment size is `6`, with offsets `1..=4`
/// left as memory holes that the trace runner must dummy-read.
pub fn synthetic_sparse_segment_bytecode(low_value: i64, high_value: i64) -> Bytecode {
    let mut instructions = Instructions::new();
    // Allocate a fresh segment and stash its base in global 0.
    instructions.extend_from_bytes(&encode(Opcode::SegmentNew, &[]));
    instructions.extend_from_bytes(&encode(Opcode::SetGlobal, &[0]));
    // Write `low_value` at `Rel(seg, 0)`.
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::HeapWrite, &[]));
    // Write `high_value` at `Rel(seg, 5)`, leaving offsets 1..=4 unwritten.
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::Add, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[2]));
    instructions.extend_from_bytes(&encode(Opcode::HeapWrite, &[]));
    // Read `Rel(seg, 5)` and discard, then read `Rel(seg, 0)` so the program
    // output is `low_value`.
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::Add, &[]));
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    Bytecode {
        instructions,
        constants: vec![
            Value::Integer(Integer::I64(low_value)),
            Value::Integer(Integer::I64(5)),
            Value::Integer(Integer::I64(high_value)),
        ],
        source_map: SourceMap::new(),
        type_registry: vec![],
    }
}

/// Bytecode that builds two segments, each with an internal hole, so the
/// relocator lays both segments into flat space and the hole filler must
/// cover offsets in *both* segments to keep flat continuity intact.
pub fn synthetic_cross_segment_sparse_bytecode(seg_a_value: i64, seg_b_value: i64) -> Bytecode {
    let mut instructions = Instructions::new();
    // Segment A: write at offsets 0 and 3 (holes at 1, 2).
    instructions.extend_from_bytes(&encode(Opcode::SegmentNew, &[]));
    instructions.extend_from_bytes(&encode(Opcode::SetGlobal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::HeapWrite, &[]));
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::Add, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[2]));
    instructions.extend_from_bytes(&encode(Opcode::HeapWrite, &[]));
    // Segment B: write at offsets 0 and 2 (hole at 1).
    instructions.extend_from_bytes(&encode(Opcode::SegmentNew, &[]));
    instructions.extend_from_bytes(&encode(Opcode::SetGlobal, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[3]));
    instructions.extend_from_bytes(&encode(Opcode::HeapWrite, &[]));
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[4]));
    instructions.extend_from_bytes(&encode(Opcode::Add, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[5]));
    instructions.extend_from_bytes(&encode(Opcode::HeapWrite, &[]));
    // Read segment B's high cell, discard, then read segment A's low cell.
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[4]));
    instructions.extend_from_bytes(&encode(Opcode::Add, &[]));
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    Bytecode {
        instructions,
        constants: vec![
            Value::Integer(Integer::I64(seg_a_value)),
            Value::Integer(Integer::I64(3)),
            Value::Integer(Integer::I64(seg_a_value.wrapping_add(100))),
            Value::Integer(Integer::I64(seg_b_value)),
            Value::Integer(Integer::I64(2)),
            Value::Integer(Integer::I64(seg_b_value.wrapping_add(100))),
        ],
        source_map: SourceMap::new(),
        type_registry: vec![],
    }
}

/// Bytecode that writes `cells.len()` values directly into the pre-allocated
/// public-output segment ([`SEG_PUBLIC_OUTPUT`]) at offsets `0..cells.len()`
/// and leaves the segment base pointer as the program's last-popped value.
pub fn synthetic_output_segment_bytecode(cells: &[i64]) -> Bytecode {
    let mut instructions = Instructions::new();
    let mut constants: Vec<Value> = Vec::with_capacity(cells.len() * 2 + 1);

    let pubmem_base_idx = constants.len();
    constants.push(Value::Relocatable(Relocatable::new(SEG_PUBLIC_OUTPUT, 0)));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[pubmem_base_idx]));
    instructions.extend_from_bytes(&encode(Opcode::SetGlobal, &[0]));

    for (off, &val) in cells.iter().enumerate() {
        // Push the cell address: `base` for offset 0, `base + off` otherwise.
        instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
        if off > 0 {
            let off_const_idx = constants.len();
            constants.push(Value::Integer(Integer::I64(off as i64)));
            instructions.extend_from_bytes(&encode(Opcode::Constant, &[off_const_idx]));
            instructions.extend_from_bytes(&encode(Opcode::Add, &[]));
        }
        // Push the cell value, then HeapWrite.
        let val_const_idx = constants.len();
        constants.push(Value::Integer(Integer::I64(val)));
        instructions.extend_from_bytes(&encode(Opcode::Constant, &[val_const_idx]));
        instructions.extend_from_bytes(&encode(Opcode::HeapWrite, &[]));
    }

    // Push the segment base as the program's return value.
    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    Bytecode {
        instructions,
        constants,
        source_map: SourceMap::new(),
        type_registry: vec![],
    }
}

/// Runs the bytecode, extracts the public-output segment from the reserved
/// [`SEG_PUBLIC_OUTPUT`] slot, builds `MaatPublicInputs::with_output_segment`,
/// and verifies the proof end-to-end.
pub fn prove_and_verify_pubmem(bytecode: Bytecode) {
    let artifacts =
        maat_trace::run_with_output(bytecode.clone()).expect("trace with public output failed");
    let output_felt = BaseElement::new(u64::from(artifacts.output_base));
    let program_hash = compute_program_hash(&bytecode).expect("program hash failed");
    let public_inputs = MaatPublicInputs::with_output_segment(
        program_hash,
        vec![],
        output_felt,
        artifacts.output_base,
        artifacts.output_segment.clone(),
    );
    let prover = MaatProver::new(development_options(), public_inputs.clone());
    let proof = prover
        .generate_proof(artifacts.trace)
        .expect("pubmem proof generation failed");
    verify_with_inputs(proof, public_inputs).expect("pubmem verification failed");
}

pub fn prove_synthetic_heap(bytecode: Bytecode, expected_output: BaseElement) {
    let (trace, _) = maat_trace::run(bytecode.clone()).expect("heap trace failed");
    let program_hash = compute_program_hash(&bytecode).expect("program hash failed");
    let public_inputs = MaatPublicInputs::new(program_hash, vec![], expected_output);
    let prover = MaatProver::new(development_options(), public_inputs.clone());
    let proof = prover
        .generate_proof(trace)
        .expect("heap synthetic proof generation failed");
    verify_with_inputs(proof, public_inputs).expect("heap synthetic verification failed");
}

pub fn prove_synthetic_heap_production(bytecode: Bytecode, expected_output: BaseElement) {
    let (trace, _) = maat_trace::run(bytecode.clone()).expect("heap trace failed");
    let program_hash = compute_program_hash(&bytecode).expect("program hash failed");
    let public_inputs = MaatPublicInputs::new(program_hash, vec![], expected_output);
    let prover = MaatProver::new(production_options(), public_inputs.clone());
    let proof = prover
        .generate_proof(trace)
        .expect("heap synthetic proof generation (production) failed");
    verify_with_inputs(proof, public_inputs)
        .expect("heap synthetic verification (production) failed");
}

/// Bytecode that creates a `SegmentArena`, allocates `payloads.len()`
/// segments through it, writes a single sentinel cell into each allocated
/// segment, finalizes each one, then reads back the first segment's cell so
/// the program's last-popped value is `payloads[0]`.
///
/// Globals layout: slot 0 is the arena base; slots `1..=payloads.len()`
/// hold each allocated segment's base.
pub fn synthetic_arena_alloc_finalize_bytecode(payloads: &[i64]) -> Bytecode {
    assert!(
        !payloads.is_empty(),
        "arena test needs at least one payload"
    );

    let mut instructions = Instructions::new();

    instructions.extend_from_bytes(&encode(Opcode::SegmentNew, &[]));
    instructions.extend_from_bytes(&encode(Opcode::SetGlobal, &[0]));

    let mut constants: Vec<Value> = Vec::with_capacity(payloads.len());

    for (i, &payload) in payloads.iter().enumerate() {
        let alloc_slot = i + 1;
        instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
        instructions.extend_from_bytes(&encode(Opcode::ArenaNew, &[]));
        instructions.extend_from_bytes(&encode(Opcode::SetGlobal, &[alloc_slot]));

        instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[alloc_slot]));
        let const_idx = constants.len();
        constants.push(Value::Integer(Integer::I64(payload)));
        instructions.extend_from_bytes(&encode(Opcode::Constant, &[const_idx]));
        instructions.extend_from_bytes(&encode(Opcode::HeapWrite, &[]));
    }

    for i in 0..payloads.len() {
        let alloc_slot = i + 1;
        instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[0]));
        instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[alloc_slot]));
        instructions.extend_from_bytes(&encode(Opcode::ArenaFinalize, &[]));
    }

    instructions.extend_from_bytes(&encode(Opcode::GetGlobal, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));

    Bytecode {
        instructions,
        constants,
        source_map: SourceMap::new(),
        type_registry: vec![],
    }
}

/// Prove honestly, then verify against a tampered public input.
pub fn honest_prover_dishonest_verifier(
    bytecode: Bytecode,
    tamper: impl FnOnce(&mut MaatPublicInputs),
    label: &str,
) {
    let artifacts = maat_trace::run_with_output(bytecode.clone()).expect("trace failed");
    let output_felt = BaseElement::new(u64::from(artifacts.output_base));
    let program_hash = compute_program_hash(&bytecode).expect("hash");
    let honest_inputs = MaatPublicInputs::with_output_segment(
        program_hash,
        vec![],
        output_felt,
        artifacts.output_base,
        artifacts.output_segment.clone(),
    );
    let prover = MaatProver::new(development_options(), honest_inputs.clone());
    let proof = prover
        .generate_proof(artifacts.trace)
        .expect("honest proof generation must succeed");

    let mut lying_inputs = honest_inputs;
    tamper(&mut lying_inputs);
    assert!(
        verify_with_inputs(proof, lying_inputs).is_err(),
        "{label}: tampered public input must be rejected",
    );
}
