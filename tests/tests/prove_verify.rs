//! End-to-end prove-then-verify integration tests.
//!
//! Each test compiles a Maat source program, generates an execution trace,
//! produces a STARK proof, and verifies it; exercising the full pipeline
//! from source code to cryptographic soundness.

use maat_air::{MaatPublicInputs, Proof};
use maat_bytecode::selector::*;
use maat_bytecode::{Bytecode, Instructions, Opcode, encode};
use maat_field::{BaseElement, Felt, FieldElement};
use maat_prover::{
    MaatProver, compute_program_hash, compute_program_hash_bytes, deserialize_proof,
    development_options, production_options, serialize_proof, verify, verify_with_inputs,
};
use maat_runtime::{Integer, Value};
use maat_span::SourceMap;
use maat_trace::table::{COL_MEM_ADDR, COL_MEM_VAL, COL_OUT, COL_SUB_SEL_BASE, TraceTable};

/// Compiles source, runs the trace, and returns everything needed for proving.
fn compile_and_trace(source: &str) -> (Bytecode, TraceTable, BaseElement) {
    let bytecode = maat_tests::compile(source);
    let (trace, result) = maat_trace::run(bytecode.clone()).expect("trace execution failed");
    let output = result.map(|v| v.to_felt()).unwrap_or(BaseElement::ZERO);
    (bytecode, trace, output)
}

/// Builds public inputs and proves, returning the proof and public inputs.
fn prove(bytecode: &Bytecode, trace: TraceTable, output: BaseElement) -> (Proof, MaatPublicInputs) {
    let program_hash = compute_program_hash(bytecode).expect("program hash failed");
    let public_inputs = MaatPublicInputs::new(program_hash, vec![], output);
    let prover = MaatProver::new(development_options(), public_inputs.clone());
    let proof = prover
        .generate_proof(trace)
        .expect("proof generation failed");
    (proof, public_inputs)
}

/// Proves and verifies in one step.
fn prove_and_verify(source: &str) {
    let (bytecode, trace, output) = compile_and_trace(source);
    let (proof, public_inputs) = prove(&bytecode, trace, output);
    verify_with_inputs(proof, public_inputs).expect("verification failed");
}

/// Tampers with the `out` column on the first row where `sub_selector` fires.
///
/// Returns the original (correct) output field element so the caller can
/// build public inputs with the true output while submitting a fraudulent
/// trace. If no row with the given sub-selector is found the test panics,
/// ensuring the source program actually exercises the targeted operation.
fn tamper_output_on_sub_sel(trace: &mut TraceTable, sub_selector: usize) {
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

/// Asserts that proving (in release mode) followed by verifying a tampered trace
/// rejects the proof, accepting that debug builds may panic earlier inside
/// Winterfell's per-row constraint validation.
fn assert_tampered_trace_rejected(
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

/// Builds bytecode that allocates a heap cell, reads it back, and produces the
/// stored value as the program output. Used by synthetic tests; the
/// heap opcodes are internal-only and not yet emitted by the surface compiler.
fn synthetic_heap_alloc_read_bytecode(initial_value: i64) -> Bytecode {
    let mut instructions = Instructions::new();
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

/// Builds bytecode that allocates a heap cell, overwrites it with a new value,
/// reads the updated value, and produces it as the program output.
///
/// The trace VM keys its logical-->physical heap map by allocation address, so
/// the literal `1` in the constant pool matches the first physical heap
/// address (`heap_alloc_ptr` starts at 1).
fn synthetic_heap_write_then_read_bytecode(initial: i64, updated: i64) -> Bytecode {
    let mut instructions = Instructions::new();
    // Constants: 0 = initial, 1 = updated, 2 = logical heap address (1).
    // alloc: push initial, HeapAlloc -> push allocated address
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[0]));
    instructions.extend_from_bytes(&encode(Opcode::HeapAlloc, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    // write: push logical addr, push updated value, HeapWrite
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[2]));
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[1]));
    instructions.extend_from_bytes(&encode(Opcode::HeapWrite, &[]));
    // read: push logical addr, HeapRead -> push value
    instructions.extend_from_bytes(&encode(Opcode::Constant, &[2]));
    instructions.extend_from_bytes(&encode(Opcode::HeapRead, &[]));
    instructions.extend_from_bytes(&encode(Opcode::Pop, &[]));
    Bytecode {
        instructions,
        constants: vec![
            Value::Integer(Integer::I64(initial)),
            Value::Integer(Integer::I64(updated)),
            Value::Integer(Integer::U64(1)),
        ],
        source_map: SourceMap::new(),
        type_registry: vec![],
    }
}

fn prove_synthetic_heap(bytecode: Bytecode, expected_output: BaseElement) {
    let (trace, _) = maat_trace::run(bytecode.clone()).expect("heap trace failed");
    let program_hash = compute_program_hash(&bytecode).expect("program hash failed");
    let public_inputs = MaatPublicInputs::new(program_hash, vec![], expected_output);
    let prover = MaatProver::new(development_options(), public_inputs.clone());
    let proof = prover
        .generate_proof(trace)
        .expect("heap synthetic proof generation failed");
    verify_with_inputs(proof, public_inputs).expect("heap synthetic verification failed");
}

fn prove_synthetic_heap_production(bytecode: Bytecode, expected_output: BaseElement) {
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

#[test]
fn prove_and_verify_arithmetic() {
    prove_and_verify(
        "
        let a: i64 = 1 + 2;
        let b: i64 = a * 3;
        let c: i64 = b - a;
        let d: i64 = c / 2;
        d
        ",
    );
}

#[test]
fn prove_and_verify_modular_arithmetic() {
    prove_and_verify(
        "
        let a: i64 = 17 % 5;
        let b: i64 = a + 10;
        b
        ",
    );
}

#[test]
fn prove_and_verify_nested_arithmetic() {
    prove_and_verify(
        "
        let x: i64 = (3 + 4) * (10 - 2);
        let y: i64 = x / 7 + x % 7;
        y
        ",
    );
}

#[test]
fn prove_and_verify_boolean_logic() {
    prove_and_verify(
        "
        let a: bool = true;
        let b: bool = false;
        let c: bool = !b;
        c
        ",
    );
}

#[test]
fn prove_and_verify_comparison_operators() {
    prove_and_verify(
        "
        let a: i64 = 10;
        let b: i64 = 20;
        let lt: bool = a < b;
        let gt: bool = a > b;
        let eq: bool = a == a;
        eq
        ",
    );
}

#[test]
fn prove_and_verify_if_else() {
    prove_and_verify(
        "
        let x: i64 = 5;
        let result: i64 = if x > 3 { x * 2 } else { x + 1 };
        result
        ",
    );
}

#[test]
fn prove_and_verify_if_else_false_branch() {
    prove_and_verify(
        "
        let x: i64 = 1;
        let result: i64 = if x > 3 { x * 2 } else { x + 1 };
        result
        ",
    );
}

#[test]
fn prove_and_verify_nested_if() {
    prove_and_verify(
        "
        let x: i64 = 10;
        let y: i64 = if x > 5 {
            if x > 8 { 100 } else { 50 }
        } else {
            0
        };
        y
        ",
    );
}

#[test]
fn prove_and_verify_fixed_size_array_literal_and_index() {
    prove_and_verify(
        "
        let a: [i64; 3] = [10, 20, 30];
        a[0] + a[1] + a[2]
        ",
    );
}

#[test]
fn prove_and_verify_fixed_size_array_sum_loop_unrolled() {
    prove_and_verify(
        "
        let b: [i64; 4] = [1, 2, 3, 4];
        b[0] + b[1] + b[2] + b[3]
        ",
    );
}

#[test]
fn prove_and_verify_fixed_size_array_length_method() {
    prove_and_verify(
        "
        let a: [i64; 5] = [1, 2, 3, 4, 5];
        a.len() as i64
        ",
    );
}

#[test]
fn prove_and_verify_fixed_size_array_equality_true() {
    prove_and_verify(
        "
        let c: [i64; 2] = [42, 99];
        let d: [i64; 2] = [42, 99];
        if c == d { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_fixed_size_array_equality_false() {
    prove_and_verify(
        "
        let c: [i64; 2] = [42, 99];
        let e: [i64; 2] = [42, 100];
        if c == e { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_fixed_size_array_inequality() {
    prove_and_verify(
        "
        let c: [i64; 2] = [42, 99];
        let e: [i64; 2] = [42, 100];
        if c != e { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_fixed_size_array_function_param_and_return() {
    prove_and_verify(
        "
        fn dot(x: [i64; 3], y: [i64; 3]) -> i64 {
            x[0] * y[0] + x[1] * y[1] + x[2] * y[2]
        }
        let v1: [i64; 3] = [1, 2, 3];
        let v2: [i64; 3] = [4, 5, 6];
        dot(v1, v2)
        ",
    );
}

#[test]
fn fixed_size_array_element_tamper_rejected() {
    let source = "
        let a: [i64; 3] = [10, 20, 30];
        a[0] + a[1] + a[2]
    ";
    let (bytecode, mut trace, output) = compile_and_trace(source);

    // Find the first row that records value 10 (the first allocated array
    // element) on a heap-allocation memory write and corrupt it. Heap accesses
    // share the unified memory permutation argument, so flipping a value
    // breaks single-value consistency on subsequent dummy reads.
    let n = trace.num_rows();
    let mut tampered = false;
    for i in 0..n {
        if trace.row(i)[COL_MEM_VAL].as_int() == 10 {
            trace.row_mut(i)[COL_MEM_VAL] = Felt::new(99);
            tampered = true;
            break;
        }
    }
    assert!(
        tampered,
        "expected at least one memory row carrying value 10"
    );
    assert_tampered_trace_rejected(bytecode, trace, output, "array element");
}

#[test]
fn prove_and_verify_range_loop() {
    prove_and_verify(
        "
        let mut acc: i64 = 0;
        for i in 0..10 {
            acc = acc + i;
        }
        acc
        ",
    );
}

#[test]
fn prove_and_verify_range_loop_single_iteration() {
    prove_and_verify(
        "
        let mut x: i64 = 0;
        for i in 0..1 {
            x = x + 1;
        }
        x
        ",
    );
}

#[test]
fn prove_and_verify_nested_loops() {
    prove_and_verify(
        "
        let mut total: i64 = 0;
        for i in 0..3 {
            for j in 0..3 {
                total = total + 1;
            }
        }
        total
        ",
    );
}

#[test]
fn prove_and_verify_loop_with_conditional() {
    prove_and_verify(
        "
        let mut even_sum: i64 = 0;
        for i in 0..10 {
            if i % 2 == 0 {
                even_sum = even_sum + i;
            }
        }
        even_sum
        ",
    );
}

#[test]
fn prove_and_verify_mutable_reassignment() {
    prove_and_verify(
        "
        let mut x: i64 = 1;
        x = x + 1;
        x = x * 3;
        x = x - 2;
        x
        ",
    );
}

#[test]
fn prove_and_verify_multiple_globals() {
    prove_and_verify(
        "
        let a: i64 = 10;
        let b: i64 = 20;
        let c: i64 = 30;
        let d: i64 = a + b + c;
        d
        ",
    );
}

#[test]
fn prove_and_verify_global_reuse() {
    prove_and_verify(
        "
        let x: i64 = 5;
        let y: i64 = x + x + x;
        y
        ",
    );
}

#[test]
fn prove_and_verify_felt_arithmetic() {
    prove_and_verify(
        "
        let a: Felt = 42_fe;
        let b: Felt = 7_fe;
        let c: Felt = a + b;
        let d: Felt = c * 2_fe;
        d
        ",
    );
}

#[test]
fn prove_and_verify_felt_subtraction() {
    prove_and_verify(
        "
        let a: Felt = 100_fe;
        let b: Felt = 30_fe;
        let c: Felt = a - b;
        c
        ",
    );
}

#[test]
fn prove_and_verify_integer_conversion() {
    prove_and_verify(
        "
        let a: i64 = 42;
        let b: u8 = a as u8;
        let c: i64 = b as i64;
        c
        ",
    );
}

#[test]
fn prove_and_verify_integer_to_felt() {
    prove_and_verify(
        "
        let n: u64 = 99;
        let f: Felt = n as Felt;
        f
        ",
    );
}

#[test]
fn prove_and_verify_empty_program() {
    prove_and_verify("let x: i64 = 0;");
}

#[test]
fn prove_and_verify_single_literal() {
    prove_and_verify("42");
}

#[test]
fn prove_and_verify_unit_result() {
    prove_and_verify("let x: i64 = 1;");
}

#[test]
fn prove_and_verify_zero_loop() {
    prove_and_verify(
        "
        let mut x: i64 = 99;
        for i in 0..0 {
            x = 0;
        }
        x
        ",
    );
}

#[test]
fn prove_and_verify_large_accumulator() {
    prove_and_verify(
        "
        let mut acc: i64 = 0;
        for i in 0..50 {
            acc = acc + i;
        }
        acc
        ",
    );
}

#[test]
fn prove_and_verify_division_and_modulo() {
    prove_and_verify(
        "
        let a: i64 = 100;
        let b: i64 = 7;
        let q: i64 = a / b;
        let r: i64 = a % b;
        q + r
        ",
    );
}

#[test]
fn wrong_output_rejected() {
    let source = "let x: i64 = 42;";
    let (bytecode, trace, output) = compile_and_trace(source);

    // Generate a valid proof with the correct output.
    let (proof, _correct_inputs) = prove(&bytecode, trace, output);

    // Attempt to verify with wrong public inputs (different output).
    let program_hash = compute_program_hash(&bytecode).expect("program hash failed");
    let wrong_output = BaseElement::new(999);
    let wrong_inputs = MaatPublicInputs::new(program_hash, vec![], wrong_output);

    assert!(
        verify_with_inputs(proof, wrong_inputs).is_err(),
        "verification with wrong output must fail"
    );
}

#[test]
fn wrong_program_hash_rejected() {
    let source = "let x: i64 = 42;";
    let (bytecode, trace, output) = compile_and_trace(source);

    // Use a valid proof but with tampered program hash.
    let mut wrong_hash = compute_program_hash(&bytecode).expect("hash failed");
    wrong_hash[0] = BaseElement::new(wrong_hash[0].as_int().wrapping_add(1));

    let real_hash = compute_program_hash(&bytecode).expect("hash failed");
    let real_inputs = MaatPublicInputs::new(real_hash, vec![], output);
    let prover = MaatProver::new(development_options(), real_inputs);
    let proof = prover
        .generate_proof(trace)
        .expect("proof generation failed");

    let tampered_inputs = MaatPublicInputs::new(wrong_hash, vec![], output);
    assert!(
        verify_with_inputs(proof, tampered_inputs).is_err(),
        "tampered program hash must be rejected"
    );
}

#[test]
fn proof_file_round_trip() {
    let source = "let x: i64 = 7; x";
    let (bytecode, trace, output) = compile_and_trace(source);
    let program_hash_bytes =
        compute_program_hash_bytes(&bytecode).expect("program hash bytes failed");
    let (proof, _public_inputs) = prove(&bytecode, trace, output);

    let serialized = serialize_proof(&proof, &program_hash_bytes, output, &[]);
    let (decoded_proof, embedded) = deserialize_proof(&serialized).expect("deserialization failed");

    assert_eq!(embedded.program_hash, program_hash_bytes);
    assert_eq!(embedded.output, output);
    assert!(embedded.inputs.is_empty());
    assert_eq!(decoded_proof.to_bytes(), proof.to_bytes());
}

#[test]
fn verify_serialized_proof_end_to_end() {
    let source = "let x: i64 = 7; x";
    let (bytecode, trace, output) = compile_and_trace(source);
    let program_hash_bytes =
        compute_program_hash_bytes(&bytecode).expect("program hash bytes failed");
    let (proof, _public_inputs) = prove(&bytecode, trace, output);

    let serialized = serialize_proof(&proof, &program_hash_bytes, output, &[]);
    verify(&serialized).expect("proof file verification failed");
}

#[test]
fn proof_file_with_inputs_round_trip() {
    let source = "let x: i64 = 7; x";
    let (bytecode, trace, output) = compile_and_trace(source);
    let program_hash_bytes =
        compute_program_hash_bytes(&bytecode).expect("program hash bytes failed");
    let (proof, _public_inputs) = prove(&bytecode, trace, output);

    let inputs = vec![
        BaseElement::new(1),
        BaseElement::new(2),
        BaseElement::new(3),
    ];
    let serialized = serialize_proof(&proof, &program_hash_bytes, output, &inputs);
    let (_, embedded) = deserialize_proof(&serialized).expect("deserialization failed");

    assert_eq!(embedded.inputs.len(), 3);
    assert_eq!(embedded.inputs[0], BaseElement::new(1));
    assert_eq!(embedded.inputs[1], BaseElement::new(2));
    assert_eq!(embedded.inputs[2], BaseElement::new(3));
}

#[test]
fn prove_and_verify_single_param_function() {
    prove_and_verify(
        "
        fn inc(x: i64) -> i64 {
            x + 1
        }
        inc(41)
        ",
    );
}

#[test]
fn prove_and_verify_multi_param_function() {
    prove_and_verify(
        "
        fn add3(a: i64, b: i64, c: i64) -> i64 {
            a + b + c
        }
        add3(10, 20, 12)
        ",
    );
}

#[test]
fn prove_and_verify_nested_function_calls() {
    prove_and_verify(
        "
        fn double(x: i64) -> i64 {
            x * 2
        }
        fn quadruple(x: i64) -> i64 {
            double(double(x))
        }
        quadruple(5)
        ",
    );
}

#[test]
fn prove_and_verify_function_with_local_then_call() {
    prove_and_verify(
        "
        fn compute(a: i64, b: i64) -> i64 {
            let s: i64 = a + b;
            let p: i64 = a * b;
            s + p
        }
        compute(3, 4)
        ",
    );
}

#[test]
fn prove_and_verify_bounded_recursion() {
    prove_and_verify(
        "
        fn fact(n: i64) -> i64 {
            if n <= 1 { 1 } else { n * fact(n - 1) }
        }
        fact(5)
        ",
    );
}

#[test]
fn wrong_function_output_rejected() {
    let source = "
        fn square(x: i64) -> i64 { x * x }
        square(9)
    ";
    let (bytecode, trace, output) = compile_and_trace(source);
    let (proof, _correct_inputs) = prove(&bytecode, trace, output);

    let program_hash = compute_program_hash(&bytecode).expect("program hash failed");
    let wrong_output = BaseElement::new(80);
    let wrong_inputs = MaatPublicInputs::new(program_hash, vec![], wrong_output);

    assert!(
        verify_with_inputs(proof, wrong_inputs).is_err(),
        "function-call proof must reject a tampered output",
    );
}

#[test]
fn prove_and_verify_production_options() {
    let source = "let x: i64 = 42; x";
    let (bytecode, trace, output) = compile_and_trace(source);
    let program_hash = compute_program_hash(&bytecode).expect("program hash failed");
    let public_inputs = MaatPublicInputs::new(program_hash, vec![], output);
    let prover = MaatProver::new(production_options(), public_inputs.clone());
    let proof = prover
        .generate_proof(trace)
        .expect("proof generation with production options failed");
    verify_with_inputs(proof, public_inputs).expect("verification with production options failed");
}

#[test]
fn tampered_arithmetic_add_output_rejected() {
    let source = "let a: i64 = 10; let b: i64 = 20; a + b";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_ADD);
    assert_tampered_trace_rejected(bytecode, trace, output, "add");
}

#[test]
fn tampered_arithmetic_neg_output_rejected() {
    let source = "let a: i64 = 7; -a";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_NEG);
    assert_tampered_trace_rejected(bytecode, trace, output, "neg");
}

#[test]
fn tampered_felt_add_output_rejected() {
    let source = "
        let a: Felt = 5_fe;
        let b: Felt = 3_fe;
        a + b
    ";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_FELT_ADD);
    assert_tampered_trace_rejected(bytecode, trace, output, "felt add");
}

#[test]
fn tampered_equality_output_rejected() {
    let source = "
        let a: i64 = 5;
        let b: i64 = 5;
        if a == b { 1i64 } else { 0i64 }
    ";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_EQ);
    assert_tampered_trace_rejected(bytecode, trace, output, "equality");
}

#[test]
fn heap_synthetic_alloc_read_write_development() {
    let bytecode = synthetic_heap_alloc_read_bytecode(42);
    prove_synthetic_heap(bytecode, BaseElement::new(42));
}

#[test]
fn heap_synthetic_alloc_read_write_production() {
    let bytecode = synthetic_heap_alloc_read_bytecode(42);
    prove_synthetic_heap_production(bytecode, BaseElement::new(42));
}

#[test]
fn heap_synthetic_write_then_read_development() {
    let bytecode = synthetic_heap_write_then_read_bytecode(7, 99);
    prove_synthetic_heap(bytecode, BaseElement::new(99));
}

#[test]
fn heap_synthetic_single_value_tampered_rejected() {
    let bytecode = synthetic_heap_alloc_read_bytecode(42);
    let (mut trace, _) = maat_trace::run(bytecode.clone()).expect("heap trace failed");

    // Find the first row that records the heap-allocated value 42 in the
    // unified memory column and corrupt it. The memory permutation argument
    // sees the same address with two different values and the verifier
    // rejects the proof.
    let n = trace.num_rows();
    let mut tampered = false;
    for i in 0..n {
        if trace.row(i)[COL_MEM_VAL].as_int() == 42 {
            let cur = trace.row(i)[COL_MEM_VAL].as_int();
            trace.row_mut(i)[COL_MEM_VAL] = Felt::new(cur.wrapping_add(1));
            tampered = true;
            break;
        }
    }
    assert!(tampered, "expected at least one heap row carrying value 42");

    let program_hash = compute_program_hash(&bytecode).expect("hash failed");
    let public_inputs = MaatPublicInputs::new(program_hash, vec![], BaseElement::new(42));
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
                "heap single-value violation must be rejected by the verifier",
            );
        }
    }
}

#[test]
fn physical_address_gap_rejected() {
    let source = "let a: i64 = 5; a";
    let (bytecode, mut trace, _) = compile_and_trace(source);

    // Shift every non-sentinel address up by 1 so address 1 is skipped,
    // leaving the unique sorted address set as {0, 2, ...} instead of {0, 1, ...}.
    let n = trace.num_rows();
    for i in 0..n {
        let addr = trace.row(i)[COL_MEM_ADDR].as_int();
        if addr > 0 {
            trace.row_mut(i)[COL_MEM_ADDR] = Felt::new(addr + 1);
        }
    }

    let program_hash = compute_program_hash(&bytecode).expect("hash failed");
    let public_inputs = MaatPublicInputs::new(program_hash, vec![], BaseElement::new(5));
    let prover = MaatProver::new(development_options(), public_inputs.clone());

    // The AIR address-continuity constraint (aux constraint 0:
    // addr_delta * (addr_delta - 1) = 0) rejects the proof on the gap.
    let prove_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prover.generate_proof(trace)
    }));
    match prove_result {
        Err(_) => {}
        Ok(proof) => {
            let proof = proof.expect("proof generation failed");
            assert!(
                verify_with_inputs(proof, public_inputs).is_err(),
                "physical address gap must be rejected by the verifier",
            );
        }
    }
}

#[test]
fn prove_and_verify_bitwise_and() {
    prove_and_verify(
        "
        let a: u64 = 0xCAFEBABEDEADBEEF;
        let b: u64 = 0x0F0F0F0F0F0F0F0F;
        a & b
        ",
    );
}

#[test]
fn tampered_bitwise_and_output_rejected() {
    let source = "
        let a: u64 = 0xCAFEBABEDEADBEEF;
        let b: u64 = 0x0F0F0F0F0F0F0F0F;
        a & b
    ";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_AND);
    assert_tampered_trace_rejected(bytecode, trace, output, "bitwise and");
}

#[test]
fn prove_and_verify_bitwise_or() {
    prove_and_verify(
        "
        let a: u64 = 0xAAAAAAAAAAAAAAAA;
        let b: u64 = 0x5555555555555555;
        a | b
        ",
    );
}

#[test]
fn tampered_bitwise_or_output_rejected() {
    let source = "
        let a: u64 = 0xAAAAAAAAAAAAAAAA;
        let b: u64 = 0x5555555555555555;
        a | b
    ";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_OR);
    assert_tampered_trace_rejected(bytecode, trace, output, "bitwise or");
}

#[test]
fn prove_and_verify_bitwise_xor() {
    prove_and_verify(
        "
        let a: u64 = 0x123456789ABCDEF0;
        let b: u64 = 0xFEDCBA9876543210;
        a ^ b
        ",
    );
}

#[test]
fn tampered_bitwise_xor_output_rejected() {
    let source = "
        let a: u64 = 0x123456789ABCDEF0;
        let b: u64 = 0xFEDCBA9876543210;
        a ^ b
    ";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_XOR);
    assert_tampered_trace_rejected(bytecode, trace, output, "bitwise xor");
}

#[test]
fn prove_and_verify_bitwise_shl() {
    prove_and_verify(
        "
        let a: u64 = 0xCAFE;
        let b: u64 = 16;
        a << b
        ",
    );
}

#[test]
fn tampered_bitwise_shl_output_rejected() {
    let source = "
        let a: u64 = 0xCAFE;
        let b: u64 = 16;
        a << b
    ";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_SHL);
    assert_tampered_trace_rejected(bytecode, trace, output, "bitwise shl");
}

#[test]
fn prove_and_verify_bitwise_shr() {
    prove_and_verify(
        "
        let a: u64 = 0xDEADBEEF00000000;
        let b: u64 = 32;
        a >> b
        ",
    );
}

#[test]
fn tampered_bitwise_shr_output_rejected() {
    let source = "
        let a: u64 = 0xDEADBEEF00000000;
        let b: u64 = 32;
        a >> b
    ";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_SHR);
    assert_tampered_trace_rejected(bytecode, trace, output, "bitwise shr");
}

#[test]
fn prove_and_verify_ordering_lt_true() {
    prove_and_verify(
        "
        let a: u32 = 7u32;
        let b: u32 = 42u32;
        if a < b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_lt_false() {
    prove_and_verify(
        "
        let a: u32 = 50u32;
        let b: u32 = 42u32;
        if a < b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_lt_equal() {
    prove_and_verify(
        "
        let a: u32 = 42u32;
        let b: u32 = 42u32;
        if a < b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_gt_true() {
    prove_and_verify(
        "
        let a: u32 = 99u32;
        let b: u32 = 42u32;
        if a > b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_gt_false() {
    prove_and_verify(
        "
        let a: u32 = 7u32;
        let b: u32 = 42u32;
        if a > b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_le_boundary() {
    prove_and_verify(
        "
        let a: u32 = 42u32;
        let b: u32 = 42u32;
        if a <= b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_ge_boundary() {
    prove_and_verify(
        "
        let a: u32 = 42u32;
        let b: u32 = 42u32;
        if a >= b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_signed_negatives() {
    prove_and_verify(
        "
        let a: i32 = -7i32;
        let b: i32 = -3i32;
        if a < b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn tampered_lt_output_rejected() {
    let source = "
        let a: u32 = 7u32;
        let b: u32 = 42u32;
        if a < b { 1i64 } else { 0i64 }
    ";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_LT);
    assert_tampered_trace_rejected(bytecode, trace, output, "ordering lt");
}

#[test]
fn tampered_gt_output_rejected() {
    let source = "
        let a: u32 = 99u32;
        let b: u32 = 42u32;
        if a > b { 1i64 } else { 0i64 }
    ";
    let (bytecode, mut trace, output) = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut trace, SUB_SEL_GT);
    assert_tampered_trace_rejected(bytecode, trace, output, "ordering gt");
}
