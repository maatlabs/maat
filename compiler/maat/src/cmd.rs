use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::process;
use std::time::Instant;

use maat_air::MaatPublicInputs;
use maat_bytecode::Bytecode;
use maat_field::{BaseElement, FieldElement, from_i64};
use maat_module::{check_and_compile, resolve_module_graph};
use maat_prover::{
    MaatProver, compute_program_hash, compute_program_hash_bytes, deserialize_proof,
    development_options, production_options, serialize_proof,
};
use maat_runtime::Value;
use maat_vm::VM;

use crate::diagnostic;

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

/// STARK proof generation for the `maat prove` command.
pub fn prove(
    path: &Path,
    input: Option<&str>,
    inputs_file: Option<&Path>,
    output_path: Option<&Path>,
    trace_path: Option<&Path>,
    production: bool,
) {
    require_extension(path, "maat", "prove");

    let inputs = load_inputs(input, inputs_file);
    let bytecode = compile_source(path);

    let (trace, result) = match maat_trace::run(bytecode.clone()) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("error: trace generation failed: {e}");
            process::exit(1);
        }
    };

    if let Some(tp) = trace_path {
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

    let program_hash = match compute_program_hash(&bytecode) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("error: failed to compute program hash: {e}");
            process::exit(1);
        }
    };
    let program_hash_bytes = match compute_program_hash_bytes(&bytecode) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("error: failed to compute program hash bytes: {e}");
            process::exit(1);
        }
    };

    let public_inputs = MaatPublicInputs::new(program_hash, inputs.clone(), output);
    let options = if production {
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

    let proof_bytes = serialize_proof(&proof, &program_hash_bytes, output, &inputs);
    let default_output = path.with_extension("proof.bin");
    let out = output_path.unwrap_or(&default_output);
    if let Err(e) = std::fs::write(out, &proof_bytes) {
        eprintln!("error: cannot write '{}': {e}", out.display());
        process::exit(1);
    }

    let queries = options.num_queries();
    let blowup = options.blowup_factor();
    let grinding = options.grinding_factor();
    let fri_bits = queries as u32 * blowup.ilog2();
    let security_bits = fri_bits + grinding;

    eprintln!(
        "proved: {} -> {} ({} bytes, ~{} bits, {:.2?})",
        path.display(),
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

/// STARK proof verification for the `maat verify` command.
pub fn verify(path: &Path) {
    require_extension(path, "bin", "verify");

    let proof_bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read '{}': {e}", path.display());
            process::exit(1);
        }
    };
    let (_, embedded) = match deserialize_proof(&proof_bytes) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("error: failed to parse proof file: {e}");
            process::exit(1);
        }
    };

    let start = Instant::now();
    match maat_prover::verify(&proof_bytes) {
        Ok(()) => {
            let elapsed = start.elapsed();
            eprintln!(
                "VERIFIED (output: {}, inputs: {}, {:.2?})",
                embedded.output.as_int(),
                embedded.inputs.len(),
                elapsed
            );
        }
        Err(e) => {
            eprintln!("REJECTED: {e}");
            process::exit(1);
        }
    }
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

/// Loads public inputs from either command-line arguments or a JSON file.
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
