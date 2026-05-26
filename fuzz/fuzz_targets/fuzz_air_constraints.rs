#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use maat_air::MaatPublicInputs;
use maat_ast::MaatAst;
use maat_bytecode::Bytecode;
use maat_codegen::Compiler;
use maat_field::{Felt, MODULUS};
use maat_lexer::MaatLexer;
use maat_parser::MaatParser;
use maat_prover::{development_options, verify_with_inputs, MaatProver};
use maat_trace::table::{COL_OUT, TRACE_WIDTH};
use maat_types::TypeChecker;

/// A simple arithmetic program whose trace exercises the common constraint
/// classes (push, arithmetic, store/load, compare, return).
const SEED_SOURCE: &str = "fn add(a: i64, b: i64) -> i64 { a + b }\n\
                            fn main() -> i64 { let x: i64 = 7; let y: i64 = 3; add(x, y) }";

/// Pre-compiled seed state cached across fuzz iterations.
///
/// `bytecode` is stored as serialized bytes because `Bytecode` contains
/// `Vec<Value>` which holds `Rc<[u8]>` (closure bytecodes) and is therefore
/// not `Sync`. Deserializing on each iteration is cheap for a small program.
struct SeedState {
    bytecode_bytes: Vec<u8>,
    public_inputs: MaatPublicInputs,
}

static SEED: OnceLock<SeedState> = OnceLock::new();

fn seed() -> &'static SeedState {
    SEED.get_or_init(|| {
        let mut parser = MaatParser::new(MaatLexer::new(SEED_SOURCE));
        let mut program = parser.parse();
        assert!(
            parser.errors().is_empty(),
            "seed program must parse cleanly"
        );

        TypeChecker::new().check_program(&mut program);

        let mut compiler = Compiler::new();
        compiler
            .compile(&MaatAst::Program(program))
            .expect("seed program must compile");
        let bytecode = compiler
            .bytecode()
            .expect("seed program must produce bytecode");

        let artifacts =
            maat_trace::run_with_output(bytecode.clone()).expect("seed program must trace");
        let output = artifacts.trace.row(artifacts.trace.num_rows() - 1)[COL_OUT];

        let public_inputs = MaatPublicInputs::with_segments(
            vec![],
            output,
            artifacts.output_base,
            artifacts.output_segment.clone(),
            artifacts.program_base,
            artifacts.program_segment.clone(),
        );

        let bytecode_bytes = bytecode.serialize().expect("seed bytecode must serialize");
        SeedState {
            bytecode_bytes,
            public_inputs,
        }
    })
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 13 {
        return;
    }

    let row_idx = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let col_idx = data[4] as usize % TRACE_WIDTH;
    let delta = u64::from_le_bytes(data[5..13].try_into().unwrap());
    if delta == 0 {
        return;
    }

    let state = seed();

    let bytecode = match Bytecode::deserialize(&state.bytecode_bytes) {
        Ok(b) => b,
        Err(_) => return,
    };
    let artifacts = match maat_trace::run_with_output(bytecode) {
        Ok(a) => a,
        Err(_) => return,
    };
    let mut trace = artifacts.trace;

    let num_rows = trace.num_rows();
    let row_idx = row_idx % num_rows;

    let orig = trace.row(row_idx)[col_idx].as_int();
    let new_val = orig.wrapping_add(delta) % MODULUS;
    if new_val == orig {
        return;
    }
    trace.row_mut(row_idx)[col_idx] = Felt::new(new_val);

    let prover = MaatProver::new(development_options(), state.public_inputs.clone());

    let libfuzzer_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let prove_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prover.generate_proof(trace)
    }));
    std::panic::set_hook(libfuzzer_hook);

    match prove_result {
        // Prover panicked: debug-mode constraint check fired; expected for
        // constrained cells.
        Err(_) => {}
        // Prover returned a structured error; expected for constrained cells.
        Ok(Err(_)) => {}
        // Prover succeeded. The verifier must still reject the tampered proof.
        Ok(Ok(proof)) => {
            assert!(
                verify_with_inputs(proof, state.public_inputs.clone()).is_err(),
                "soundness gap: tampered trace (row={row_idx}, col={col_idx}, \
                 delta={delta}) produced a proof that the verifier accepted"
            );
        }
    }
});
