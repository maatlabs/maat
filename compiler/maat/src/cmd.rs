use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::process;
use std::time::Instant;

use maat_air::{MaatPublicInputs, PublicMemory, PublicSegment, program_hash};
use maat_bytecode::Bytecode;
use maat_field::{BaseElement, FieldElement, from_i64};
use maat_module::{MainArity, check_and_compile, main_entry_arity, resolve_module_graph};
use maat_prover::{
    MaatProver, PublicIo, deserialize_proof, development_options, format_program_hash,
    parse_program_hash, production_options, serialize_proof,
};
use maat_runtime::Value;
use maat_vm::VM;

use crate::diagnostic;
use crate::public_io::{load_bundle, load_expected_inputs, parse_expected_felt, write_bundle};

/// Print an `error: ...` line and exit the process with a non-zero status.
fn die(msg: impl AsRef<str>) -> ! {
    eprintln!("error: {}", msg.as_ref());
    process::exit(1);
}

/// Bytecode compilation for the `maat build` command.
pub fn build(source_path: &Path, output_path: Option<&Path>) {
    require_extension(source_path, "maat", "build");

    let bytecode = compile_source(source_path);
    let bytes = match bytecode.serialize() {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "error: failed to serialize bytecode for '{}': {e}",
                source_path.display()
            );
            process::exit(1);
        }
    };
    let default_output = source_path.with_extension("mtc");
    let out = output_path.unwrap_or(&default_output);
    if let Err(e) = std::fs::write(out, bytes) {
        eprintln!("error: cannot write '{}': {e}", out.display());
        process::exit(1);
    }
    eprintln!("compiled {} -> {}", source_path.display(), out.display());
}

/// Pre-compiled bytecode execution for the `maat exec` command.
pub fn execute(path: &Path) {
    require_extension(path, "mtc", "exec");

    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read '{}': {e}", path.display());
            process::exit(1);
        }
    };
    let bytecode = match Bytecode::deserialize(&bytes) {
        Ok(bc) => bc,
        Err(e) => {
            eprintln!("error: failed to deserialize '{}': {e}", path.display());
            process::exit(1);
        }
    };
    let mut vm = VM::new(bytecode);
    if let Err(e) = vm.run() {
        eprintln!("error: {}: {e}", path.display());
        process::exit(1);
    }
    if let Some(result) = vm.last_popped_stack_elem()
        && !matches!(result, Value::Unit)
    {
        println!("{result}");
    }
}

/// Source file execution for the `maat run` command.
pub fn run(path: &Path) {
    require_extension(path, "maat", "run");

    let bytecode = compile_source(path);
    let mut vm = VM::new(bytecode);
    if let Err(e) = vm.run() {
        eprintln!("error: {}: {}", path.display(), e);
        process::exit(1);
    }
    if let Some(result) = vm.last_popped_stack_elem()
        && !matches!(result, Value::Unit)
    {
        println!("{result}");
    }
}

/// Trace generation for the `maat trace` command.
pub fn trace(path: &Path, output_path: Option<&Path>) {
    require_extension(path, "maat", "trace");

    let bytecode = compile_source(path);
    let (trace, result) = match maat_trace::run(bytecode) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("error: {}: {e}", path.display());
            process::exit(1);
        }
    };

    match output_path {
        Some(out) => {
            let file = match File::create(out) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("error: cannot write '{}': {e}", out.display());
                    process::exit(1);
                }
            };
            if let Err(e) = trace.write_csv(BufWriter::new(file)) {
                eprintln!("error: failed to write trace CSV: {e}");
                process::exit(1);
            }
            eprintln!("trace: {} rows -> {}", trace.num_rows(), out.display());
        }
        None => {
            let stdout = io::stdout();
            if let Err(e) = trace.write_csv(BufWriter::new(stdout.lock())) {
                eprintln!("error: failed to write trace CSV: {e}");
                process::exit(1);
            }
        }
    }

    if let Some(val) = result
        && !matches!(val, Value::Unit)
    {
        eprintln!("result: {val}");
    }
}

pub struct ProveArgs<'a> {
    pub source: &'a Path,
    pub input: Option<&'a str>,
    pub inputs_file: Option<&'a Path>,
    pub private_input: Option<&'a str>,
    pub private_inputs_file: Option<&'a Path>,
    pub expect_output: Option<&'a str>,
    pub public_io: Option<&'a Path>,
    pub write_public_io: Option<&'a Path>,
    pub output: Option<&'a Path>,
    pub trace: Option<&'a Path>,
    pub production: bool,
}

/// STARK proof generation for the `maat prove` command.
pub fn prove(args: ProveArgs<'_>) {
    require_extension(args.source, "maat", "prove");

    let (bytecode, arity) = compile_provable_source(args.source);
    let (inputs_supplied, private_supplied, expected_output) = resolve_prove_io(&args);
    let inputs = reconcile_inputs(inputs_supplied, arity.public, "public");
    let private_inputs = reconcile_inputs(private_supplied, arity.private, "private");

    let maat_trace::TraceArtifacts {
        trace,
        result,
        memory,
    } = match maat_trace::run_with_io(bytecode, &inputs, &private_inputs) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: trace generation failed: {e}");
            process::exit(1);
        }
    };

    if let Some(tp) = args.trace {
        let file = match File::create(tp) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("error: cannot write '{}': {e}", tp.display());
                process::exit(1);
            }
        };
        if let Err(e) = trace.write_csv(BufWriter::new(file)) {
            eprintln!("error: failed to write trace CSV: {e}");
            process::exit(1);
        }
        eprintln!("trace: {} rows -> {}", trace.num_rows(), tp.display());
    }

    let output = result
        .as_ref()
        .map(|v| v.to_felt())
        .unwrap_or(BaseElement::ZERO);

    if let Some(expected) = expected_output
        && expected != output
    {
        eprintln!(
            "error: prover output mismatch: trace produced {}, --expect-output asserted {}",
            output.as_int(),
            expected.as_int()
        );
        process::exit(1);
    }

    let memory = PublicMemory {
        input: PublicSegment::new(memory.input.base, inputs),
        output: memory.output,
        program: memory.program,
    };
    let public_inputs = MaatPublicInputs::new(output, memory.clone());
    let options = if args.production {
        production_options()
    } else {
        development_options()
    };

    let start = Instant::now();
    let prover = MaatProver::new(options.clone(), public_inputs);
    let proof = match prover.generate_proof(trace) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: proof generation failed: {e}");
            process::exit(1);
        }
    };
    let elapsed = start.elapsed();

    let proof_bytes = serialize_proof(&proof, output, &memory);
    let default_output = args.source.with_extension("proof.bin");
    let out = args.output.unwrap_or(&default_output);
    if let Err(e) = std::fs::write(out, &proof_bytes) {
        eprintln!("error: cannot write '{}': {e}", out.display());
        process::exit(1);
    }

    if let Some(bundle_path) = args.write_public_io {
        let bundle = PublicIo::new(
            memory.input.cells.clone(),
            output,
            program_hash(&memory.program),
        );
        write_bundle(&bundle, bundle_path);
    }

    let queries = options.num_queries();
    let blowup = options.blowup_factor();
    let grinding = options.grinding_factor();
    let fri_bits = queries as u32 * blowup.ilog2();
    let security_bits = fri_bits + grinding;

    eprintln!(
        "proved: {} -> {} ({} bytes, ~{} bits, {:.2?})",
        args.source.display(),
        out.display(),
        proof_bytes.len(),
        security_bits,
        elapsed
    );
    if let Some(val) = result
        && !matches!(val, Value::Unit)
    {
        eprintln!("output: {val}");
    }
}

fn resolve_prove_io(
    args: &ProveArgs<'_>,
) -> (Vec<BaseElement>, Vec<BaseElement>, Option<BaseElement>) {
    let Some(bundle_path) = args.public_io else {
        return (
            load_inputs(args.input, args.inputs_file),
            load_inputs(args.private_input, args.private_inputs_file),
            args.expect_output
                .map(|s| parse_expected_felt(s, "--expect-output")),
        );
    };
    if args.input.is_some() || args.inputs_file.is_some() {
        die("--public-io is mutually exclusive with --input / --inputs-file");
    }
    if args.private_input.is_some() || args.private_inputs_file.is_some() {
        die("--public-io is mutually exclusive with --private-input / --private-inputs-file");
    }
    if args.expect_output.is_some() {
        die("--public-io is mutually exclusive with --expect-output");
    }
    let bundle = load_bundle(bundle_path);
    let bail =
        |e: maat_errors::BundleError| -> ! { die(format!("'{}': {e}", bundle_path.display())) };
    (
        bundle.inputs_felt().unwrap_or_else(|e| bail(e)),
        bundle.private_inputs_felt().unwrap_or_else(|e| bail(e)),
        Some(bundle.output_felt().unwrap_or_else(|e| bail(e))),
    )
}

pub struct VerifyArgs<'a> {
    pub proof: &'a Path,
    pub input: Option<&'a str>,
    pub inputs_file: Option<&'a Path>,
    pub expect_output: Option<&'a str>,
    pub public_io: Option<&'a Path>,
    pub expect_program: Option<&'a Path>,
    pub expect_program_hash: Option<&'a str>,
}

struct VerifyExpectations {
    inputs: Option<Vec<BaseElement>>,
    output: Option<BaseElement>,
    program_hash: Option<[u8; 32]>,
}

/// STARK proof verification for the `maat verify` command.
pub fn verify(args: VerifyArgs<'_>) {
    require_extension(args.proof, "bin", "verify");

    let expectations = resolve_verify_expectations(&args);

    let proof_bytes = std::fs::read(args.proof)
        .unwrap_or_else(|e| die(format!("cannot read '{}': {e}", args.proof.display())));
    let (_, embedded) = deserialize_proof(&proof_bytes)
        .unwrap_or_else(|e| die(format!("failed to parse proof file: {e}")));

    let start = Instant::now();
    if let Err(e) = maat_prover::verify(&proof_bytes) {
        eprintln!("REJECTED: {e}");
        process::exit(1);
    }
    let elapsed = start.elapsed();

    let assertions_matched = apply_expectations(&expectations, &embedded);

    eprintln!(
        "VERIFIED (output: {}, inputs: {}, {:.2?})",
        embedded.output.as_int(),
        embedded.memory.input.cells.len(),
        elapsed
    );
    if assertions_matched > 0 {
        eprintln!("assertions: {assertions_matched} matched");
    }
}

fn resolve_verify_expectations(args: &VerifyArgs<'_>) -> VerifyExpectations {
    let (inputs, output, bundle_program_hash) = match args.public_io {
        Some(bundle_path) => load_verify_bundle(bundle_path, args),
        None => (
            resolve_expect_inputs(args.input, args.inputs_file),
            args.expect_output
                .map(|s| parse_expected_felt(s, "--expect-output")),
            None,
        ),
    };

    let cli_program_hash = match (args.expect_program, args.expect_program_hash) {
        (Some(_), Some(_)) => {
            die("--expect-program is mutually exclusive with --expect-program-hash")
        }
        (Some(source), None) => Some(program_hash_from_source(source)),
        (None, Some(hex)) => Some(
            parse_program_hash(hex).unwrap_or_else(|e| die(format!("--expect-program-hash: {e}"))),
        ),
        (None, None) => None,
    };

    if bundle_program_hash.is_some() && cli_program_hash.is_some() {
        die(
            "--public-io already binds program_hash; remove --expect-program / --expect-program-hash",
        );
    }

    VerifyExpectations {
        inputs,
        output,
        program_hash: cli_program_hash.or(bundle_program_hash),
    }
}

fn load_verify_bundle(
    bundle_path: &Path,
    args: &VerifyArgs<'_>,
) -> (
    Option<Vec<BaseElement>>,
    Option<BaseElement>,
    Option<[u8; 32]>,
) {
    if args.input.is_some() || args.inputs_file.is_some() {
        die("--public-io is mutually exclusive with --input / --inputs-file");
    }
    if args.expect_output.is_some() {
        die("--public-io is mutually exclusive with --expect-output");
    }
    let bundle = load_bundle(bundle_path);
    let bail =
        |e: maat_errors::BundleError| -> ! { die(format!("'{}': {e}", bundle_path.display())) };
    let inputs = bundle.inputs_felt().unwrap_or_else(|e| bail(e));
    let output = bundle.output_felt().unwrap_or_else(|e| bail(e));
    let program_hash = (!bundle.program_hash.is_empty())
        .then(|| bundle.program_hash_bytes().unwrap_or_else(|e| bail(e)));
    (Some(inputs), Some(output), program_hash)
}

fn resolve_expect_inputs(inline: Option<&str>, file: Option<&Path>) -> Option<Vec<BaseElement>> {
    match (inline, file) {
        (Some(_), Some(_)) => die("cannot specify both --input and --inputs-file"),
        (Some(s), None) => Some(parse_input_values(s)),
        (None, Some(path)) => Some(load_expected_inputs(path)),
        (None, None) => None,
    }
}

/// Hash a source file by serializing its bytecode and pinning it directly,
/// without executing the VM, avoiding stdout side effects from any
/// `println!` in the user program.
fn program_hash_from_source(source: &Path) -> [u8; 32] {
    require_extension(source, "maat", "verify");
    let (bytecode, _) = compile_provable_source(source);
    let program_bytes = bytecode.serialize().unwrap_or_else(|e| {
        die(format!(
            "--expect-program: bytecode serialization failed for '{}': {e}",
            source.display()
        ))
    });
    program_hash(&maat_trace::program_image_from_bytes(&program_bytes))
}

fn apply_expectations(
    expectations: &VerifyExpectations,
    embedded: &maat_prover::ProofPublicInputs,
) -> usize {
    let mut matched = 0;

    if let Some(expected_hash) = expectations.program_hash {
        let embedded_hash = program_hash(&embedded.memory.program);
        if expected_hash != embedded_hash {
            eprintln!(
                "error: program-hash mismatch: proof binds {}, expectation asserted {}",
                format_program_hash(&embedded_hash),
                format_program_hash(&expected_hash),
            );
            process::exit(1);
        }
        matched += 1;
    }

    if let Some(expected_inputs) = &expectations.inputs {
        let embedded_inputs = &embedded.memory.input.cells;
        if expected_inputs.len() != embedded_inputs.len() {
            eprintln!(
                "error: public-input arity mismatch: proof binds {} cell(s), --input asserted {}",
                embedded_inputs.len(),
                expected_inputs.len()
            );
            process::exit(1);
        }
        for (i, (got, want)) in embedded_inputs
            .iter()
            .zip(expected_inputs.iter())
            .enumerate()
        {
            if got != want {
                eprintln!(
                    "error: public-input mismatch at index {i}: proof binds {}, --input asserted {}",
                    got.as_int(),
                    want.as_int()
                );
                process::exit(1);
            }
        }
        matched += 1;
    }

    if let Some(expected_output) = expectations.output {
        if embedded.output != expected_output {
            eprintln!(
                "error: output mismatch: proof binds {}, --expect-output asserted {}",
                embedded.output.as_int(),
                expected_output.as_int()
            );
            process::exit(1);
        }
        matched += 1;
    }

    matched
}

/// Validates that a file path has the expected extension, exiting with a
/// diagnostic message if it does not.
fn require_extension(path: &Path, expected: &str, command: &str) {
    let actual = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if actual != expected {
        eprintln!(
            "error: `maat {command}` expects a `.{expected}` file, got '{}'",
            path.display(),
        );
        std::process::exit(1);
    }
}

/// Compiles a `.maat` source file (and all its module dependencies) to
/// linked [`Bytecode`].
fn compile_source(path: &Path) -> Bytecode {
    let mut graph = match resolve_module_graph(path) {
        Ok(g) => g,
        Err(e) => {
            diagnostic::report_module_error(&e);
            process::exit(1);
        }
    };
    match check_and_compile(&mut graph) {
        Ok(bc) => bc,
        Err(e) => {
            diagnostic::report_module_error(&e);
            process::exit(1);
        }
    }
}

/// Compiles a `.maat` source file for proving, requiring a `fn main` entry
/// point. Script-form programs (top-level statements) are `run`/`exec`-only.
/// Returns the linked bytecode and `fn main`'s public/private input arity.
fn compile_provable_source(path: &Path) -> (Bytecode, MainArity) {
    let mut graph = match resolve_module_graph(path) {
        Ok(g) => g,
        Err(e) => {
            diagnostic::report_module_error(&e);
            process::exit(1);
        }
    };
    let Some(arity) = main_entry_arity(&graph) else {
        eprintln!(
            "error: `maat prove` requires a `fn main` entry point, but '{}' is a \
             script-form program; run it with `maat run` or compile and `maat exec` it",
            path.display()
        );
        process::exit(1);
    };
    match check_and_compile(&mut graph) {
        Ok(bc) => (bc, arity),
        Err(e) => {
            diagnostic::report_module_error(&e);
            process::exit(1);
        }
    }
}

fn reconcile_inputs(supplied: Vec<BaseElement>, arity: usize, kind: &str) -> Vec<BaseElement> {
    if supplied.is_empty() {
        return vec![BaseElement::ZERO; arity];
    }
    if supplied.len() != arity {
        eprintln!(
            "error: `fn main` declares {arity} {kind} input(s), but {} were supplied",
            supplied.len()
        );
        process::exit(1);
    }
    supplied
}

/// Loads inputs from either a command-line argument or a JSON file.
fn load_inputs(input: Option<&str>, inputs_file: Option<&Path>) -> Vec<BaseElement> {
    match (input, inputs_file) {
        (Some(_), Some(_)) => {
            eprintln!("error: cannot specify both --input and --inputs-file");
            process::exit(1);
        }
        (Some(s), None) => parse_input_values(s),
        (None, Some(path)) => parse_inputs_file(path),
        (None, None) => vec![],
    }
}

/// Parses comma-separated input values into field elements.
fn parse_input_values(input: &str) -> Vec<BaseElement> {
    if input.trim().is_empty() {
        return vec![];
    }
    input.split(',').map(|v| parse_value(v.trim())).collect()
}

/// Parses a JSON file containing an array of public input values.
fn parse_inputs_file(path: &Path) -> Vec<BaseElement> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: cannot read '{}': {e}", path.display());
            process::exit(1);
        }
    };
    let reader = BufReader::new(file);
    let values: Vec<serde_json::Value> = match serde_json::from_reader(reader) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: invalid JSON in '{}': {e}", path.display());
            process::exit(1);
        }
    };
    values
        .iter()
        .map(|v| match v {
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    from_i64(i)
                } else if let Some(u) = n.as_u64() {
                    BaseElement::new(u)
                } else {
                    eprintln!("error: number {} is too large for field element", n);
                    process::exit(1);
                }
            }
            serde_json::Value::String(s) => parse_value(s),
            _ => {
                eprintln!("error: inputs must be numbers or strings, got {:?}", v);
                process::exit(1);
            }
        })
        .collect()
}

/// Parses a single value string into a field element.
fn parse_value(s: &str) -> BaseElement {
    let s = s.trim();
    if s.ends_with("fe") || s.ends_with("_fe") {
        let num_part = s.trim_end_matches("_fe").trim_end_matches("fe");
        match num_part.parse::<u64>() {
            Ok(n) => BaseElement::new(n),
            Err(e) => {
                eprintln!("error: invalid field element literal '{}': {e}", s);
                process::exit(1);
            }
        }
    } else if s.starts_with('-') {
        match s.parse::<i64>() {
            Ok(n) => from_i64(n),
            Err(e) => {
                eprintln!("error: invalid integer literal '{}': {e}", s);
                process::exit(1);
            }
        }
    } else {
        match s.parse::<u64>() {
            Ok(n) => BaseElement::new(n),
            Err(e) => {
                eprintln!("error: invalid integer literal '{}': {e}", s);
                process::exit(1);
            }
        }
    }
}
