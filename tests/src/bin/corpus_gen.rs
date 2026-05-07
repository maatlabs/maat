//! Seed corpus generator for proof-system fuzz targets.
//!
//! Run from the workspace root:
//!
//! ```
//! cargo run -p maat_tests --bin corpus_gen
//! ```
//!
//! Corpus layout produced:
//!   fuzz/corpus/fuzz_proof_deserializer/  -- serialized STARK proofs
//!   fuzz/corpus/fuzz_verifier/            -- serialized STARK proofs
//!   fuzz/corpus/fuzz_trace_recorder/      -- well-typed Maat source programs
//!   fuzz/corpus/fuzz_air_constraints/     -- structured (row, col, delta) seeds

use std::path::{Path, PathBuf};

use maat_air::MaatPublicInputs;
use maat_ast::{MaatAst, fold_constants};
use maat_codegen::Compiler;
use maat_lexer::MaatLexer;
use maat_parser::MaatParser;
use maat_prover::{
    MaatProver, compute_program_hash, compute_program_hash_bytes, development_options,
    serialize_proof,
};
use maat_trace::table::COL_OUT;
use maat_types::TypeChecker;

/// Seed programs for `fuzz_trace_recorder`. Each must parse, type-check, and prove.
const SEED_PROGRAMS: &[(&str, &str)] = &[
    (
        "arithmetic",
        "fn main() -> i64 { let x: i64 = 5; let y: i64 = 3; x + y }",
    ),
    (
        "nested_arith",
        "fn main() -> i64 { let a: i64 = 10; let b: i64 = 4; (a - b) * 2 }",
    ),
    (
        "function_call",
        "fn square(n: i64) -> i64 { n * n }\nfn main() -> i64 { square(7) }",
    ),
    (
        "loop_accumulator",
        "fn main() -> i64 { let mut s: i64 = 0; for i in 0..5 { s = s + i; } s }",
    ),
    (
        "comparison",
        "fn main() -> i64 { let x: i64 = 3; if x > 2 { 1 } else { 0 } }",
    ),
];

/// Seed mutation inputs for `fuzz_air_constraints`.
/// Each entry encodes `(row_idx: u32 LE, col_idx: u8, delta: u64 LE)`.
const AIR_SEEDS: &[([u8; 13], &str)] = &[
    // Tamper the result output column (COL_OUT = 7).
    (
        [0, 0, 0, 0, 7, 1, 0, 0, 0, 0, 0, 0, 0],
        "row0_col_out_delta1",
    ),
    // Tamper the stack top (COL_S0 = 4).
    (
        [0, 0, 0, 0, 4, 1, 0, 0, 0, 0, 0, 0, 0],
        "row0_col_s0_delta1",
    ),
    // Tamper the memory address (COL_MEM_ADDR = 8).
    (
        [0, 0, 0, 0, 8, 1, 0, 0, 0, 0, 0, 0, 0],
        "row0_col_mem_addr_delta1",
    ),
    // Tamper the first opcode selector (COL_SEL_BASE = 11).
    (
        [0, 0, 0, 0, 11, 1, 0, 0, 0, 0, 0, 0, 0],
        "row0_col_sel0_delta1",
    ),
    // Row 1, stack second element (COL_S1 = 5).
    (
        [1, 0, 0, 0, 5, 2, 0, 0, 0, 0, 0, 0, 0],
        "row1_col_s1_delta2",
    ),
    // Row 2, range-check value (COL_RC_VAL = 31).
    (
        [2, 0, 0, 0, 31, 1, 0, 0, 0, 0, 0, 0, 0],
        "row2_col_rc_val_delta1",
    ),
];

fn compile_and_prove(source: &str) -> Vec<u8> {
    let mut parser = MaatParser::new(MaatLexer::new(source));
    let mut program = parser.parse();
    assert!(
        parser.errors().is_empty(),
        "seed program has parse errors: {:?}",
        parser.errors()
    );

    let type_errors = TypeChecker::new().check_program(&mut program);
    assert!(
        type_errors.is_empty(),
        "seed program has type errors: {:?}",
        type_errors
    );

    let fold_errors = fold_constants(&mut program);
    assert!(
        fold_errors.is_empty(),
        "seed program has fold errors: {:?}",
        fold_errors
    );

    let mut compiler = Compiler::new();
    compiler
        .compile(&MaatAst::Program(program))
        .expect("seed program failed to compile");
    let bytecode = compiler
        .bytecode()
        .expect("seed program failed to produce bytecode");

    let (trace, _) = maat_trace::run(bytecode.clone()).expect("seed program failed to trace");
    let output = trace.row(trace.num_rows() - 1)[COL_OUT];

    let program_hash = compute_program_hash(&bytecode).expect("seed program failed to hash");
    let program_hash_bytes =
        compute_program_hash_bytes(&bytecode).expect("seed program failed to hash bytes");

    let public_inputs = MaatPublicInputs::new(program_hash, vec![], output);
    let prover = MaatProver::new(development_options(), public_inputs);
    let proof = prover
        .generate_proof(trace)
        .expect("seed program failed to prove");

    serialize_proof(&proof, &program_hash_bytes, output, &[])
}

fn ensure_dir(path: &Path) {
    std::fs::create_dir_all(path)
        .unwrap_or_else(|e| panic!("failed to create {}: {}", path.display(), e));
}

fn write_file(path: &Path, data: &[u8]) {
    std::fs::write(path, data)
        .unwrap_or_else(|e| panic!("failed to write {}: {}", path.display(), e));
    println!("  wrote {} ({} bytes)", path.display(), data.len());
}

fn corpus_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .expect("maat_tests manifest must have a parent (workspace root)")
        .join("fuzz")
        .join("corpus")
}

fn main() {
    let corpus_root_buf = corpus_root();
    let corpus_root = corpus_root_buf.as_path();
    println!("corpus root: {}", corpus_root.display());

    // fuzz_proof_deserializer and fuzz_verifier: seed with genuine proof bytes.
    for target in ["fuzz_proof_deserializer", "fuzz_verifier"] {
        let dir = corpus_root.join(target);
        ensure_dir(&dir);
        for (i, (_, source)) in SEED_PROGRAMS.iter().enumerate() {
            print!("[{target}] proving seed {i}...");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            let proof_bytes = compile_and_prove(source);
            write_file(&dir.join(format!("seed_{i}")), &proof_bytes);
        }
    }

    // fuzz_trace_recorder: seed with well-typed source programs.
    {
        let dir = corpus_root.join("fuzz_trace_recorder");
        ensure_dir(&dir);
        for (name, source) in SEED_PROGRAMS {
            write_file(&dir.join(name), source.as_bytes());
        }
    }

    // fuzz_air_constraints: seed with structured (row_idx, col_idx, delta) inputs.
    {
        let dir = corpus_root.join("fuzz_air_constraints");
        ensure_dir(&dir);
        for (bytes, name) in AIR_SEEDS {
            write_file(&dir.join(name), bytes);
        }
    }

    println!("\nCorpus generation complete.");
}
