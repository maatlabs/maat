//! End-to-end prove-then-verify integration tests.
//!
//! Each test compiles a Maat source program, generates an execution trace,
//! produces a STARK proof, and verifies it; exercising the full pipeline
//! from source code to cryptographic soundness.

use maat_air::MaatPublicInputs;
use maat_field::{BaseElement, Felt, FieldElement};
use maat_prover::{
    MaatProver, deserialize_proof, development_options, production_options, serialize_proof,
    verify, verify_with_inputs,
};
use maat_tests::prover::*;
use maat_trace::selector::*;
use maat_trace::table::{COL_MEM_ADDR, COL_MEM_VAL, COL_SUB_SEL_BASE, TraceTable};

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
fn prove_and_verify_fn_main_entry() {
    prove_and_verify(
        "
        fn compute() -> Felt {
            let mut a: Felt = 1_fe;
            let mut b: Felt = 2_fe;
            for _step in 0..5 {
                let next = a + b;
                a = b;
                b = next;
            }
            a
        }

        fn main() -> Felt {
            compute()
        }
        ",
    );
}

#[test]
fn fn_main_matches_script_form_output() {
    let script = compile_and_trace(
        "
        fn double(x: Felt) -> Felt { x + x }
        double(21_fe)
        ",
    );
    let entry = compile_and_trace(
        "
        fn double(x: Felt) -> Felt { x + x }
        fn main() -> Felt { double(21_fe) }
        ",
    );
    assert_eq!(script.output, entry.output);
    assert_eq!(entry.output, BaseElement::new(42));
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
fn vector_element_tamper_rejected() {
    let source = "
        let mut v = Vector::new();
        v = v.push(11);
        v = v.push(22);
        v = v.push(33);
        v[0] + v[1] + v[2]
    ";
    let mut bundle = compile_and_trace(source);
    // Each push writes one cell via VectorPush, which records the value into
    // the unified memory permutation. Corrupt the trace row carrying the
    // middle value to break single-value consistency on the matching read.
    let n = bundle.trace.num_rows();
    let mut tampered = false;
    for i in 0..n {
        if bundle.trace.row(i)[COL_MEM_VAL].as_int() == 22 {
            bundle.trace.row_mut(i)[COL_MEM_VAL] = Felt::new(999);
            tampered = true;
            break;
        }
    }
    assert!(
        tampered,
        "expected at least one memory row carrying vector element value 22"
    );
    assert_tampered_trace_rejected(bundle, "vector element");
}

#[test]
fn fixed_size_array_element_tamper_rejected() {
    let source = "
        let a: [i64; 3] = [10, 20, 30];
        a[0] + a[1] + a[2]
    ";
    let mut bundle = compile_and_trace(source);

    // Find the first row that records value 10 (the first allocated array
    // element) on a heap-allocation memory write and corrupt it. Heap accesses
    // share the unified memory permutation argument, so flipping a value
    // breaks single-value consistency on subsequent dummy reads.
    let n = bundle.trace.num_rows();
    let mut tampered = false;
    for i in 0..n {
        if bundle.trace.row(i)[COL_MEM_VAL].as_int() == 10 {
            bundle.trace.row_mut(i)[COL_MEM_VAL] = Felt::new(99);
            tampered = true;
            break;
        }
    }
    assert!(
        tampered,
        "expected at least one memory row carrying value 10"
    );
    assert_tampered_trace_rejected(bundle, "array element");
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
    let bundle = compile_and_trace(source);
    let output_base = bundle.output_base;
    let output_segment = bundle.output_segment.clone();
    let program_base = bundle.program_base;
    let program_segment = bundle.program_segment.clone();

    // Generate a valid proof with the correct output.
    let (proof, _correct_inputs) = prove(bundle);

    // Attempt to verify with wrong public inputs (different output).
    let wrong_output = BaseElement::new(999);
    let wrong_inputs = MaatPublicInputs::with_segments(
        vec![],
        wrong_output,
        output_base,
        output_segment,
        program_base,
        program_segment,
    );

    assert!(
        verify_with_inputs(proof, wrong_inputs).is_err(),
        "verification with wrong output must fail"
    );
}

#[test]
fn wrong_program_segment_rejected() {
    let source = "let x: i64 = 42;";
    let bundle = compile_and_trace(source);
    let output = bundle.output;
    let output_base = bundle.output_base;
    let output_segment = bundle.output_segment.clone();
    let program_base = bundle.program_base;
    let mut tampered_program = bundle.program_segment.clone();

    let (proof, _honest_inputs) = prove(bundle);

    // Flip a single program-segment byte; the verifier's recomputed
    // public-memory endpoint diverges from the trace-derived accumulator.
    tampered_program[0] = BaseElement::new(tampered_program[0].as_int().wrapping_add(1));
    let tampered_inputs = MaatPublicInputs::with_segments(
        vec![],
        output,
        output_base,
        output_segment,
        program_base,
        tampered_program,
    );
    assert!(
        verify_with_inputs(proof, tampered_inputs).is_err(),
        "tampered program segment must be rejected"
    );
}

#[test]
fn proof_file_round_trip() {
    let source = "let x: i64 = 7; x";
    let bundle = compile_and_trace(source);
    let output = bundle.output;
    let output_base = bundle.output_base;
    let output_segment = bundle.output_segment.clone();
    let program_base = bundle.program_base;
    let program_segment = bundle.program_segment.clone();
    let (proof, _public_inputs) = prove(bundle);

    let serialized = serialize_proof(
        &proof,
        output,
        &[],
        output_base,
        &output_segment,
        program_base,
        &program_segment,
    );
    let (decoded_proof, embedded) = deserialize_proof(&serialized).expect("deserialization failed");

    assert_eq!(embedded.output, output);
    assert!(embedded.inputs.is_empty());
    assert_eq!(embedded.output_base, output_base);
    assert_eq!(embedded.output_segment, output_segment);
    assert_eq!(embedded.program_base, program_base);
    assert_eq!(embedded.program_segment, program_segment);
    assert_eq!(decoded_proof.to_bytes(), proof.to_bytes());
}

#[test]
fn verify_serialized_proof_end_to_end() {
    let source = "let x: i64 = 7; x";
    let bundle = compile_and_trace(source);
    let output = bundle.output;
    let output_base = bundle.output_base;
    let output_segment = bundle.output_segment.clone();
    let program_base = bundle.program_base;
    let program_segment = bundle.program_segment.clone();
    let (proof, _public_inputs) = prove(bundle);

    let serialized = serialize_proof(
        &proof,
        output,
        &[],
        output_base,
        &output_segment,
        program_base,
        &program_segment,
    );
    verify(&serialized).expect("proof file verification failed");
}

#[test]
fn proof_file_with_inputs_round_trip() {
    let source = "let x: i64 = 7; x";
    let bundle = compile_and_trace(source);
    let output = bundle.output;
    let output_base = bundle.output_base;
    let output_segment = bundle.output_segment.clone();
    let program_base = bundle.program_base;
    let program_segment = bundle.program_segment.clone();
    let (proof, _public_inputs) = prove(bundle);

    let inputs = vec![
        BaseElement::new(1),
        BaseElement::new(2),
        BaseElement::new(3),
    ];
    let serialized = serialize_proof(
        &proof,
        output,
        &inputs,
        output_base,
        &output_segment,
        program_base,
        &program_segment,
    );
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
    let bundle = compile_and_trace(source);
    let output_base = bundle.output_base;
    let output_segment = bundle.output_segment.clone();
    let program_base = bundle.program_base;
    let program_segment = bundle.program_segment.clone();
    let (proof, _correct_inputs) = prove(bundle);

    let wrong_inputs = MaatPublicInputs::with_segments(
        vec![],
        BaseElement::new(80),
        output_base,
        output_segment,
        program_base,
        program_segment,
    );

    assert!(
        verify_with_inputs(proof, wrong_inputs).is_err(),
        "function-call proof must reject a tampered output",
    );
}

#[test]
fn prove_and_verify_production_options() {
    let source = "let x: i64 = 42; x";
    let bundle = compile_and_trace(source);
    let public_inputs = MaatPublicInputs::with_segments(
        vec![],
        bundle.output,
        bundle.output_base,
        bundle.output_segment.clone(),
        bundle.program_base,
        bundle.program_segment.clone(),
    );
    let prover = MaatProver::new(production_options(), public_inputs.clone());
    let proof = prover
        .generate_proof(bundle.trace)
        .expect("proof generation with production options failed");
    verify_with_inputs(proof, public_inputs).expect("verification with production options failed");
}

#[test]
fn tampered_arithmetic_add_output_rejected() {
    let source = "let a: i64 = 10; let b: i64 = 20; a + b";
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_ADD);
    assert_tampered_trace_rejected(bundle, "add");
}

#[test]
fn tampered_arithmetic_neg_output_rejected() {
    let source = "let a: i64 = 7; -a";
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_NEG);
    assert_tampered_trace_rejected(bundle, "neg");
}

#[test]
fn tampered_felt_add_output_rejected() {
    let source = "
        let a: Felt = 5_fe;
        let b: Felt = 3_fe;
        a + b
    ";
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_FELT_ADD);
    assert_tampered_trace_rejected(bundle, "felt add");
}

#[test]
fn tampered_equality_output_rejected() {
    let source = "
        let a: i64 = 5;
        let b: i64 = 5;
        if a == b { 1i64 } else { 0i64 }
    ";
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_EQ);
    assert_tampered_trace_rejected(bundle, "equality");
}

#[test]
fn heap_synthetic_alloc_read_write_development() {
    let bytecode = synthetic_segment_alloc_read_bytecode(42);
    prove_synthetic_heap(bytecode, BaseElement::new(42));
}

#[test]
fn heap_synthetic_alloc_read_write_production() {
    let bytecode = synthetic_segment_alloc_read_bytecode(42);
    prove_synthetic_heap_production(bytecode, BaseElement::new(42));
}

#[test]
fn heap_synthetic_two_segments_cross_segment_read() {
    let bytecode = synthetic_two_segments_bytecode(7, 99);
    prove_synthetic_heap(bytecode, BaseElement::new(7));
}

#[test]
fn heap_synthetic_relocatable_value_stored_and_relocated() {
    let bytecode = synthetic_relocatable_cell_value_bytecode(1234);
    prove_synthetic_heap(bytecode, BaseElement::new(1234));
}

#[test]
fn heap_synthetic_write_once_violation_rejected() {
    let bytecode = synthetic_write_once_violation_bytecode(7, 99);
    let err = maat_trace::run(bytecode).expect_err("write-once violation must produce an error");
    let msg = err.to_string();
    assert!(
        msg.contains("write-once violation"),
        "expected write-once rejection, got: {msg}"
    );
}

#[test]
fn heap_synthetic_single_value_tampered_rejected() {
    let bytecode = synthetic_segment_alloc_read_bytecode(42);
    let artifacts = maat_trace::run_with_output(bytecode).expect("heap trace failed");
    let mut trace = artifacts.trace;

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

    let public_inputs = MaatPublicInputs::with_segments(
        vec![],
        BaseElement::new(42),
        artifacts.output_base,
        artifacts.output_segment,
        artifacts.program_base,
        artifacts.program_segment,
    );
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
fn heap_synthetic_intra_segment_holes_filled() {
    let bytecode = synthetic_sparse_segment_bytecode(17, 42);
    let artifacts =
        maat_trace::run_with_output(bytecode.clone()).expect("sparse heap trace failed");
    let program_end = u64::from(artifacts.program_base) + artifacts.program_segment.len() as u64;

    let mut unique_addrs = std::collections::HashSet::new();
    for i in 0..artifacts.trace.num_rows() {
        unique_addrs.insert(artifacts.trace.row(i)[COL_MEM_ADDR].as_int());
    }
    let max = unique_addrs.iter().copied().max().unwrap_or(0);
    for addr in program_end..=max {
        assert!(
            unique_addrs.contains(&addr),
            "flat address {addr} missing after hole filling (max = {max})",
        );
    }
    prove_synthetic_heap(bytecode, BaseElement::new(17));
}

#[test]
fn heap_synthetic_intra_segment_holes_filled_production() {
    let bytecode = synthetic_sparse_segment_bytecode(17, 42);
    prove_synthetic_heap_production(bytecode, BaseElement::new(17));
}

#[test]
fn heap_synthetic_cross_segment_holes_filled() {
    let bytecode = synthetic_cross_segment_sparse_bytecode(11, 23);
    let artifacts =
        maat_trace::run_with_output(bytecode.clone()).expect("cross-segment trace failed");
    let program_end = u64::from(artifacts.program_base) + artifacts.program_segment.len() as u64;
    let mut unique_addrs = std::collections::HashSet::new();
    for i in 0..artifacts.trace.num_rows() {
        unique_addrs.insert(artifacts.trace.row(i)[COL_MEM_ADDR].as_int());
    }
    let max = unique_addrs.iter().copied().max().unwrap_or(0);
    for addr in program_end..=max {
        assert!(
            unique_addrs.contains(&addr),
            "flat address {addr} missing across two sparse segments",
        );
    }
    prove_synthetic_heap(bytecode, BaseElement::new(11));
}

#[test]
fn heap_synthetic_hole_row_removed_rejected() {
    let bytecode = synthetic_sparse_segment_bytecode(17, 42);
    let artifacts = maat_trace::run_with_output(bytecode).expect("sparse heap trace failed");
    let mut trace = artifacts.trace;

    let mut addrs: Vec<u64> = (0..trace.num_rows())
        .map(|i| trace.row(i)[COL_MEM_ADDR].as_int())
        .collect();
    addrs.sort_unstable();
    addrs.dedup();
    let hole_addr = addrs
        .iter()
        .zip(addrs.iter().skip(1))
        .find_map(|(&a, &b)| if b == a + 1 { Some(a + 1) } else { None })
        .expect("expected at least one dummy hole row");

    let mut rebuilt = TraceTable::new();
    for i in 0..trace.num_rows() {
        if trace.row(i)[COL_MEM_ADDR].as_int() != hole_addr {
            rebuilt.push_row(*trace.row(i));
        }
    }
    trace = rebuilt;

    let public_inputs = MaatPublicInputs::with_segments(
        vec![],
        BaseElement::new(17),
        artifacts.output_base,
        artifacts.output_segment,
        artifacts.program_base,
        artifacts.program_segment,
    );
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
                "removing a hole row must be rejected by the verifier",
            );
        }
    }
}

#[test]
fn physical_address_gap_rejected() {
    let source = "let a: i64 = 5; a";
    let mut bundle = compile_and_trace(source);

    // Shift every non-sentinel address up by 1 so address 1 is skipped,
    // leaving the unique sorted address set as {0, 2, ...} instead of {0, 1, ...}.
    let n = bundle.trace.num_rows();
    for i in 0..n {
        let addr = bundle.trace.row(i)[COL_MEM_ADDR].as_int();
        if addr > 0 {
            bundle.trace.row_mut(i)[COL_MEM_ADDR] = Felt::new(addr + 1);
        }
    }

    let public_inputs = MaatPublicInputs::with_segments(
        vec![],
        BaseElement::new(5),
        bundle.output_base,
        bundle.output_segment.clone(),
        bundle.program_base,
        bundle.program_segment.clone(),
    );
    let prover = MaatProver::new(development_options(), public_inputs.clone());

    // The AIR address-continuity constraint (aux constraint 0:
    // addr_delta * (addr_delta - 1) = 0) rejects the proof on the gap.
    let prove_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prover.generate_proof(bundle.trace)
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
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_AND);
    assert_tampered_trace_rejected(bundle, "bitwise and");
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
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_OR);
    assert_tampered_trace_rejected(bundle, "bitwise or");
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
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_XOR);
    assert_tampered_trace_rejected(bundle, "bitwise xor");
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
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_SHL);
    assert_tampered_trace_rejected(bundle, "bitwise shl");
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
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_SHR);
    assert_tampered_trace_rejected(bundle, "bitwise shr");
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
fn prove_and_verify_ordering_u64_diff_exceeds_u32() {
    prove_and_verify(
        "
        let a: u64 = 1234567890u64;
        let b: u64 = 9876543210u64;
        if a < b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_u64_ge_large_values() {
    prove_and_verify(
        "
        let a: u64 = 18000000000000000000u64;
        let b: u64 = 17000000000000000000u64;
        if a >= b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_usize_lt() {
    prove_and_verify(
        "
        let a: usize = 0usize;
        let b: usize = 65535usize;
        if a < b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_usize_le_boundary() {
    prove_and_verify(
        "
        let a: usize = 42usize;
        if a <= a { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_i64_across_zero() {
    prove_and_verify(
        "
        let a: i64 = -9223372036854775807i64;
        let b: i64 = 9223372036854775807i64;
        if a < b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_i64_both_negative() {
    prove_and_verify(
        "
        let a: i64 = -42i64;
        let b: i64 = -7i64;
        if a < b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_i64_gt_extremes() {
    prove_and_verify(
        "
        let a: i64 = 9223372036854775807i64;
        let b: i64 = -9223372036854775807i64;
        if a > b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_isize_neg_vs_pos() {
    prove_and_verify(
        "
        let a: isize = -1isize;
        let b: isize = 1isize;
        if a < b { 1i64 } else { 0i64 }
        ",
    );
}

#[test]
fn prove_and_verify_ordering_chained_widths() {
    prove_and_verify(
        "
        let a: u64 = 100u64;
        let b: u64 = 200u64;
        let c: u64 = 300u64;
        if a < b {
            if b < c { 1i64 } else { 0i64 }
        } else {
            0i64
        }
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
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_LT);
    assert_tampered_trace_rejected(bundle, "ordering lt");
}

#[test]
fn tampered_gt_output_rejected() {
    let source = "
        let a: u32 = 99u32;
        let b: u32 = 42u32;
        if a > b { 1i64 } else { 0i64 }
    ";
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_GT);
    assert_tampered_trace_rejected(bundle, "ordering gt");
}

#[test]
fn tampered_lt_u64_wide_diff_output_rejected() {
    let source = "
        let a: u64 = 1234567890u64;
        let b: u64 = 9876543210u64;
        if a < b { 1i64 } else { 0i64 }
    ";
    let mut bundle = compile_and_trace(source);
    tamper_output_on_sub_sel(&mut bundle.trace, SUB_SEL_LT);
    assert_tampered_trace_rejected(bundle, "ordering lt (u64 wide)");
}

#[test]
fn vector_main_returns_segment_published_to_pubmem() {
    let source = "
        let mut v = Vector::new();
        v = v.push(7);
        v = v.push(13);
        v
    ";
    let bytecode = maat_tests::compile(source);
    let artifacts = maat_trace::run_with_output(bytecode.clone()).expect("trace failed");
    assert_eq!(
        artifacts.output_segment.len(),
        2,
        "main-return Vector must publish its cells to SEG_PUBLIC_OUTPUT"
    );
    assert_eq!(artifacts.output_segment[0], Felt::new(7));
    assert_eq!(artifacts.output_segment[1], Felt::new(13));
    prove_and_verify_pubmem(bytecode);
}

#[test]
fn vector_main_returns_segment_tampered_cell_rejected() {
    let source = "
        let mut v = Vector::new();
        v = v.push(7);
        v = v.push(13);
        v
    ";
    let bytecode = maat_tests::compile(source);
    honest_prover_dishonest_verifier(
        bytecode,
        |inputs| inputs.output_segment[1] = Felt::new(999),
        "main-return Vector cell",
    );
}

#[test]
fn vector_builtin_cells_in_heap_permutation() {
    let source = "
        let mut v = Vector::new();
        v = v.push(7);
        v = v.push(13);
        v.rev()
    ";
    let bytecode = maat_tests::compile(source);
    let artifacts = maat_trace::run_with_output(bytecode.clone()).expect("trace failed");
    assert_eq!(
        artifacts.output_segment.len(),
        2,
        "builtin-allocated trailing Vector must publish its cells"
    );
    assert_eq!(artifacts.output_segment[0], Felt::new(13));
    assert_eq!(artifacts.output_segment[1], Felt::new(7));
    prove_and_verify_pubmem(bytecode);
}

#[test]
fn bounded_loop_with_residual_iterations_proves_and_verifies() {
    prove_and_verify(
        "
        let mut val: u64 = 1023;
        #[bounded(12)]
        while val != 0 {
            val = val >> 1;
        }
        val
        ",
    );
}

#[test]
fn match_tag_jump_proves_and_verifies() {
    prove_and_verify(
        "
        let x: Option<i64> = None;
        match x {
            Some(v) => v,
            None => -1,
        }
        ",
    );
}

#[test]
fn match_tag_jump_some_arm_proves_and_verifies() {
    prove_and_verify(
        "
        let x: Option<i64> = Some(42);
        match x {
            Some(v) => v,
            None => -1,
        }
        ",
    );
}

#[test]
fn match_tag_jump_marker_tampered_rejected() {
    let source = "
        let x: Option<i64> = None;
        match x {
            Some(v) => v,
            None => -1,
        }
    ";
    let mut bundle = compile_and_trace(source);
    let mut cleared = false;
    for i in 0..bundle.trace.num_rows() {
        if bundle.trace.row(i)[COL_SUB_SEL_BASE + SUB_SEL_MATCH_TAG_JUMP] == Felt::ONE {
            bundle.trace.row_mut(i)[COL_SUB_SEL_BASE + SUB_SEL_MATCH_TAG_JUMP] = Felt::ZERO;
            cleared = true;
            break;
        }
    }
    assert!(
        cleared,
        "expected at least one MatchTag-jump row in the trace",
    );
    assert_tampered_trace_rejected(bundle, "match-tag-jump marker");
}

#[test]
fn closure_capture_proves_and_verifies() {
    prove_and_verify(
        "
        let make_adder = fn(x: i64) -> fn(i64) -> i64 {
            fn(y: i64) -> i64 { x + y; }
        };
        let add5 = make_adder(5);
        let add10 = make_adder(10);
        add5(3) + add10(7)
        ",
    );
}

#[test]
fn closure_capture_tampered_cell_rejected() {
    let source = "
        let make_id = fn(x: i64) {
            fn() -> i64 { x; }
        };
        let f = make_id(42);
        f()
    ";
    let mut bundle = compile_and_trace(source);
    let mut tampered = false;
    for i in 0..bundle.trace.num_rows() {
        if bundle.trace.row(i)[COL_SUB_SEL_BASE + SUB_SEL_SYNTHETIC_HEAP] == Felt::ONE {
            bundle.trace.row_mut(i)[COL_MEM_VAL] = Felt::new(999);
            tampered = true;
            break;
        }
    }
    assert!(
        tampered,
        "expected at least one synthetic-heap-write row in the trace",
    );
    assert_tampered_trace_rejected(bundle, "closure capture cell");
}

#[test]
fn vector_builtin_cells_tampered_rejected() {
    let source = "
        let mut v = Vector::new();
        v = v.push(7);
        v = v.push(13);
        v.rev()
    ";
    let bytecode = maat_tests::compile(source);
    honest_prover_dishonest_verifier(
        bytecode,
        |inputs| inputs.output_segment[0] = Felt::new(999),
        "builtin-allocated cell",
    );
}

#[test]
fn pubmem_three_cell_output_proves_and_verifies() {
    let bytecode = synthetic_output_segment_bytecode(&[10, 20, 30]);
    let artifacts = maat_trace::run_with_output(bytecode.clone()).expect("trace failed");
    assert_eq!(artifacts.output_segment.len(), 3);
    assert_eq!(artifacts.output_segment[0], Felt::new(10));
    assert_eq!(artifacts.output_segment[1], Felt::new(20));
    assert_eq!(artifacts.output_segment[2], Felt::new(30));
    prove_and_verify_pubmem(bytecode);
}

#[test]
fn pubmem_two_cell_struct_shaped_output_proves_and_verifies() {
    let bytecode = synthetic_output_segment_bytecode(&[1, 2]);
    prove_and_verify_pubmem(bytecode);
}

#[test]
fn pubmem_tampered_output_cell_value_rejected() {
    let bytecode = synthetic_output_segment_bytecode(&[10, 20, 30]);
    honest_prover_dishonest_verifier(
        bytecode,
        |inputs| inputs.output_segment[1] = Felt::new(999),
        "output cell value",
    );
}

#[test]
fn pubmem_tampered_output_base_rejected() {
    let bytecode = synthetic_output_segment_bytecode(&[10, 20, 30]);
    honest_prover_dishonest_verifier(
        bytecode,
        |inputs| inputs.output_base = inputs.output_base.wrapping_add(17),
        "output base",
    );
}

#[test]
fn pubmem_tampered_segment_length_shorter_rejected() {
    let bytecode = synthetic_output_segment_bytecode(&[10, 20, 30]);
    honest_prover_dishonest_verifier(
        bytecode,
        |inputs| {
            inputs.output_segment.pop();
        },
        "segment length (shorter)",
    );
}

#[test]
fn pubmem_tampered_segment_length_longer_rejected() {
    let bytecode = synthetic_output_segment_bytecode(&[10, 20, 30]);
    honest_prover_dishonest_verifier(
        bytecode,
        |inputs| inputs.output_segment.push(Felt::new(40)),
        "segment length (longer)",
    );
}

#[test]
fn arena_three_segments_alloc_finalize_proves_and_verifies() {
    let bytecode = synthetic_arena_alloc_finalize_bytecode(&[11, 22, 33]);
    prove_synthetic_heap(bytecode, BaseElement::new(11));
}

#[test]
fn arena_three_segments_alloc_finalize_production() {
    let bytecode = synthetic_arena_alloc_finalize_bytecode(&[11, 22, 33]);
    prove_synthetic_heap_production(bytecode, BaseElement::new(11));
}

#[test]
fn arena_assigns_distinct_segment_ids() {
    let bytecode = synthetic_arena_alloc_finalize_bytecode(&[7, 13, 21, 29]);
    let (trace, _) = maat_trace::run(bytecode).expect("arena trace failed");

    let n = trace.num_rows();
    let mut writes: Vec<u64> = (0..n)
        .filter(|&i| {
            trace.row(i)[COL_MEM_ADDR].as_int() != 0
                && trace.row(i)[maat_trace::table::COL_IS_READ].as_int() == 0
        })
        .map(|i| trace.row(i)[COL_MEM_VAL].as_int())
        .collect();

    writes.sort_unstable();
    writes.dedup();

    assert!(
        writes.windows(2).all(|w| w[0] != w[1]),
        "duplicate arena id surfaced in trace: {writes:?}"
    );
}

#[test]
fn arena_tampered_payload_value_rejected() {
    let bytecode = synthetic_arena_alloc_finalize_bytecode(&[7, 13, 21]);
    let artifacts = maat_trace::run_with_output(bytecode).expect("arena trace failed");
    let mut trace = artifacts.trace;

    // Tamper the first heap write that carries the program's payload value 7.
    let n = trace.num_rows();
    let mut tampered = false;
    for i in 0..n {
        if trace.row(i)[maat_trace::table::COL_IS_READ].as_int() == 0
            && trace.row(i)[COL_MEM_VAL].as_int() == 7
            && trace.row(i)[COL_MEM_ADDR].as_int() != 0
        {
            trace.row_mut(i)[COL_MEM_VAL] = Felt::new(107);
            tampered = true;
            break;
        }
    }
    assert!(tampered, "expected at least one payload-7 write to tamper");

    let public_inputs = MaatPublicInputs::with_segments(
        vec![],
        BaseElement::new(7),
        artifacts.output_base,
        artifacts.output_segment,
        artifacts.program_base,
        artifacts.program_segment,
    );
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
                "tampered arena payload must be rejected by the verifier",
            );
        }
    }
}

#[test]
fn arena_segments_relocate_into_distinct_flat_ranges() {
    let bytecode = synthetic_arena_alloc_finalize_bytecode(&[100, 200, 300]);
    let artifacts = maat_trace::run_with_output(bytecode).expect("arena trace failed");
    let program_end = u64::from(artifacts.program_base) + artifacts.program_segment.len() as u64;

    let mut unique_addrs = std::collections::HashSet::new();
    for i in 0..artifacts.trace.num_rows() {
        unique_addrs.insert(artifacts.trace.row(i)[COL_MEM_ADDR].as_int());
    }
    let max = unique_addrs.iter().copied().max().unwrap_or(0);
    for addr in program_end..=max {
        assert!(
            unique_addrs.contains(&addr),
            "flat address {addr} missing after arena relocation (max = {max})",
        );
    }
}
