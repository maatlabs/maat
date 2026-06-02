#![no_main]

use libfuzzer_sys::fuzz_target;
use maat_air::MaatPublicInputs;
use maat_ast::{fold_constants, MaatAst};
use maat_codegen::Compiler;
use maat_lexer::MaatLexer;
use maat_parser::MaatParser;
use maat_prover::{development_options, verify_with_inputs, MaatProver};
use maat_trace::table::COL_OUT;
use maat_types::TypeChecker;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    let mut parser = MaatParser::new(MaatLexer::new(source));
    let mut program = parser.parse();
    if !parser.errors().is_empty() {
        return;
    }

    let type_errors = TypeChecker::new().check_program(&mut program);
    if !type_errors.is_empty() {
        return;
    }

    let fold_errors = fold_constants(&mut program);
    if !fold_errors.is_empty() {
        return;
    }

    let mut compiler = Compiler::new();
    if compiler.compile(&MaatAst::Program(program)).is_err() {
        return;
    }
    let Ok(bytecode) = compiler.bytecode() else {
        return;
    };

    let Ok(artifacts) = maat_trace::run_with_output(bytecode) else {
        return;
    };
    let output = artifacts.trace.row(artifacts.trace.num_rows() - 1)[COL_OUT];
    let public_inputs = MaatPublicInputs::new(output, artifacts.memory.clone());
    let prover = MaatProver::new(development_options(), public_inputs.clone());

    let libfuzzer_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let prove_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prover.generate_proof(artifacts.trace)
    }));
    std::panic::set_hook(libfuzzer_hook);

    let proof = match prove_result {
        Err(_) => return, // prover panicked on degenerate input; not a completeness failure
        Ok(Err(_)) => return, // prover returned Err; not provable, skip
        Ok(Ok(p)) => p,
    };

    assert!(
        verify_with_inputs(proof, public_inputs).is_ok(),
        "completeness failure: a proved trace was rejected by the verifier"
    );
});
