//! Integration tests for the verifier-side public-I/O bundle
//! and the assertion flags on `maat prove` / `maat verify`.
//!
//! These tests drive the built `maat` binary directly via
//! `CARGO_BIN_EXE_maat`, so the assertion layer is exercised across the
//! real process boundary---exit codes, stderr diagnostics, JSON-bundle
//! round-trips, and all.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

const VDF_SOURCE: &str = r#"
fn vdf(seed: Felt, steps: i64) -> Felt {
    let mut x: Felt = seed;
    for _step in 0..steps {
        x = x * x * x + 42_fe;
    }
    x
}

fn main(seed: pub Felt) -> Felt {
    vdf(seed, 8)
}
"#;

const LAMPORT_SOURCE: &str = r#"
fn main(sig0: Felt, sig1: Felt, challenge: pub Felt) -> Felt {
    let in0: [Felt; 2] = [sig0, challenge];
    let d0 = hash::rescue_2(in0);
    let in1: [Felt; 2] = [sig1, challenge];
    let d1 = hash::rescue_2(in1);
    let absorbed: [Felt; 8] = [
        d0[0], d0[1], d1[0], d1[1],
        0_fe, 0_fe, 0_fe, 0_fe,
    ];
    let pk = hash::rescue_8(absorbed);
    pk[0]
}
"#;

struct ProveOutcome {
    proof: PathBuf,
    bundle: Option<PathBuf>,
    output: u64,
}

fn maat_bin() -> &'static str {
    env!("CARGO_BIN_EXE_maat")
}

fn write_source(dir: &Path, name: &str, source: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("failed to write fixture source");
    path
}

fn prove(args: &[&str], source: &Path) -> std::process::Output {
    let mut cmd = Command::new(maat_bin());
    cmd.arg("prove").arg(source);
    for a in args {
        cmd.arg(a);
    }
    cmd.output().expect("failed to spawn maat prove")
}

fn verify(args: &[&str], proof: &Path) -> std::process::Output {
    let mut cmd = Command::new(maat_bin());
    cmd.arg("verify").arg(proof);
    for a in args {
        cmd.arg(a);
    }
    cmd.output().expect("failed to spawn maat verify")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn parse_output_line(stderr: &str) -> u64 {
    let line = stderr
        .lines()
        .find(|l| l.starts_with("output: "))
        .expect("`output: ...` line missing in prover stderr");
    line.trim_start_matches("output: ")
        .trim()
        .parse()
        .expect("output is not a u64")
}

fn prove_default(dir: &Path) -> ProveOutcome {
    let source = write_source(dir, "vdf.maat", VDF_SOURCE);
    let out = prove(&[], &source);
    assert!(
        out.status.success(),
        "default prove failed: {}",
        stderr(&out)
    );
    ProveOutcome {
        proof: source.with_extension("proof.bin"),
        bundle: None,
        output: parse_output_line(&stderr(&out)),
    }
}

fn prove_with_bundle(dir: &Path) -> ProveOutcome {
    let source = write_source(dir, "vdf.maat", VDF_SOURCE);
    let bundle_path = dir.join("vdf.pubio.json");
    let bundle_arg = bundle_path.to_str().unwrap().to_string();
    let out = prove(&["--input", "3", "--write-public-io", &bundle_arg], &source);
    assert!(
        out.status.success(),
        "prove with bundle failed: {}",
        stderr(&out)
    );
    ProveOutcome {
        proof: source.with_extension("proof.bin"),
        bundle: Some(bundle_path),
        output: parse_output_line(&stderr(&out)),
    }
}

#[test]
fn verify_with_no_assertions_produce_expected_behaviour() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_default(dir.path());
    let out = verify(&[], &outcome.proof);
    assert!(out.status.success(), "{}", stderr(&out));
    let stderr = stderr(&out);
    assert!(stderr.contains("VERIFIED"));
    assert!(
        !stderr.contains("assertions:"),
        "no assertions ==> no `assertions:` line"
    );
}

#[test]
fn matching_expect_output_emits_assertions_line() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_default(dir.path());
    let expect = outcome.output.to_string();
    let out = verify(&["--expect-output", &expect], &outcome.proof);
    assert!(out.status.success(), "{}", stderr(&out));
    let stderr = stderr(&out);
    assert!(stderr.contains("VERIFIED"));
    assert!(stderr.contains("assertions: 1 matched"));
}

#[test]
fn mismatched_expect_output_rejects_with_diagnostic() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_default(dir.path());
    let wrong = outcome.output.wrapping_add(1).to_string();
    let out = verify(&["--expect-output", &wrong], &outcome.proof);
    assert!(!out.status.success());
    let stderr = stderr(&out);
    assert!(
        stderr.starts_with("error: output mismatch: proof binds "),
        "got: {stderr}"
    );
    assert!(stderr.contains(&outcome.output.to_string()));
    assert!(stderr.contains(&wrong));
}

#[test]
fn mismatched_input_rejects_with_index_in_diagnostic() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_with_bundle(dir.path());
    let out = verify(&["--input", "99"], &outcome.proof);
    assert!(!out.status.success());
    let stderr = stderr(&out);
    assert!(
        stderr.starts_with("error: public-input mismatch at index 0: proof binds "),
        "got: {stderr}"
    );
    assert!(stderr.contains("--input asserted 99"));
}

#[test]
fn mismatched_input_arity_rejects() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_with_bundle(dir.path());
    let out = verify(&["--input", "3,4"], &outcome.proof);
    assert!(!out.status.success());
    let stderr = stderr(&out);
    assert!(
        stderr.starts_with("error: public-input arity mismatch: "),
        "got: {stderr}"
    );
}

#[test]
fn mismatched_program_hash_rejects() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_default(dir.path());
    let bogus = format!("0x{}", "ab".repeat(32));
    let out = verify(&["--expect-program-hash", &bogus], &outcome.proof);
    assert!(!out.status.success());
    let stderr = stderr(&out);
    assert!(
        stderr.starts_with("error: program-hash mismatch: proof binds "),
        "got: {stderr}"
    );
}

#[test]
fn matching_program_pins_via_source_recompilation() {
    let dir = TempDir::new().unwrap();
    let source = write_source(dir.path(), "vdf.maat", VDF_SOURCE);
    let out = prove(&[], &source);
    assert!(out.status.success(), "{}", stderr(&out));
    let proof = source.with_extension("proof.bin");
    let out = verify(&["--expect-program", source.to_str().unwrap()], &proof);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains("assertions: 1 matched"));
}

#[test]
fn mismatched_program_recompilation_rejects() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_default(dir.path());
    let other = write_source(
        dir.path(),
        "other.maat",
        "fn main() -> Felt { 7_fe + 11_fe }",
    );
    let out = verify(
        &["--expect-program", other.to_str().unwrap()],
        &outcome.proof,
    );
    assert!(!out.status.success());
    assert!(stderr(&out).starts_with("error: program-hash mismatch: "));
}

#[test]
fn expect_program_and_hash_are_mutually_exclusive() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_default(dir.path());
    let dummy_hash = format!("0x{}", "00".repeat(32));
    let out = verify(
        &[
            "--expect-program",
            dir.path().join("vdf.maat").to_str().unwrap(),
            "--expect-program-hash",
            &dummy_hash,
        ],
        &outcome.proof,
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("mutually exclusive"));
}

#[test]
fn all_three_assertions_match_emits_assertion_count() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_with_bundle(dir.path());
    let expect = outcome.output.to_string();
    let out = verify(
        &[
            "--input",
            "3",
            "--expect-output",
            &expect,
            "--expect-program",
            dir.path().join("vdf.maat").to_str().unwrap(),
        ],
        &outcome.proof,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains("assertions: 3 matched"));
}

#[test]
fn vdf_bundle_round_trips_byte_identically() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_with_bundle(dir.path());
    let bundle = outcome.bundle.unwrap();

    let out = verify(&["--public-io", bundle.to_str().unwrap()], &outcome.proof);
    assert!(out.status.success(), "{}", stderr(&out));
    let stderr = stderr(&out);
    assert!(stderr.contains("VERIFIED"));
    assert!(
        stderr.contains("assertions: 3 matched"),
        "bundle pins inputs + output + program_hash: got {stderr}"
    );
}

#[test]
fn lamport_bundle_round_trips_mixed_public_private() {
    let dir = TempDir::new().unwrap();
    let source = write_source(dir.path(), "lamport.maat", LAMPORT_SOURCE);
    let bundle = dir.path().join("lamport.pubio.json");
    let prover_io = dir.path().join("lamport.proverio.json");
    std::fs::write(
        &prover_io,
        r#"{
            "inputs": ["7"],
            "private_inputs": ["3", "5"],
            "output": "0",
            "program_hash": "0x0000000000000000000000000000000000000000000000000000000000000000"
        }"#,
    )
    .unwrap();
    let out = prove(
        &[
            "--input",
            "7",
            "--private-input",
            "3,5",
            "--write-public-io",
            bundle.to_str().unwrap(),
        ],
        &source,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let proof = source.with_extension("proof.bin");

    // The written bundle must omit private_inputs entirely.
    let written = std::fs::read_to_string(&bundle).unwrap();
    assert!(
        !written.contains("private_inputs"),
        "--write-public-io must not leak private_inputs into the bundle: got {written}"
    );

    let out = verify(&["--public-io", bundle.to_str().unwrap()], &proof);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains("assertions: 3 matched"));
}

#[test]
fn prove_public_io_loads_inputs_and_pins_output() {
    let dir = TempDir::new().unwrap();
    let source = write_source(dir.path(), "vdf.maat", VDF_SOURCE);

    // First prove with --input 3 and capture the canonical output.
    let out = prove(&["--input", "3"], &source);
    assert!(out.status.success(), "{}", stderr(&out));
    let expected_output = parse_output_line(&stderr(&out));

    let prover_io = dir.path().join("vdf.proverio.json");
    std::fs::write(
        &prover_io,
        format!(
            r#"{{
                "inputs": ["3"],
                "output": "{expected_output}",
                "program_hash": "0x0000000000000000000000000000000000000000000000000000000000000000"
            }}"#
        ),
    )
    .unwrap();
    let out = prove(&["--public-io", prover_io.to_str().unwrap()], &source);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains(&format!("output: {expected_output}")));
}

#[test]
fn prove_expect_output_mismatch_rejects_before_proof_generation() {
    let dir = TempDir::new().unwrap();
    let source = write_source(dir.path(), "vdf.maat", VDF_SOURCE);
    let out = prove(&["--input", "3", "--expect-output", "999"], &source);
    assert!(!out.status.success());
    assert!(
        stderr(&out).starts_with("error: prover output mismatch: trace produced "),
        "got: {}",
        stderr(&out)
    );
}

#[test]
fn unknown_bundle_field_rejected_at_parse_time() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_default(dir.path());
    let bundle = dir.path().join("bogus.pubio.json");
    std::fs::write(
        &bundle,
        r#"{
            "inputs": [],
            "output": "0",
            "program_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "stray": "x"
        }"#,
    )
    .unwrap();
    let out = verify(&["--public-io", bundle.to_str().unwrap()], &outcome.proof);
    assert!(!out.status.success());
    let stderr = stderr(&out);
    assert!(
        stderr.starts_with("error: ") && stderr.contains("unknown field"),
        "got: {stderr}"
    );
}

#[test]
fn public_io_mutually_exclusive_with_individual_flags() {
    let dir = TempDir::new().unwrap();
    let outcome = prove_default(dir.path());
    let bundle = dir.path().join("vdf.pubio.json");
    std::fs::write(
        &bundle,
        r#"{
            "inputs": [],
            "output": "0",
            "program_hash": "0x0000000000000000000000000000000000000000000000000000000000000000"
        }"#,
    )
    .unwrap();
    let out = verify(
        &[
            "--public-io",
            bundle.to_str().unwrap(),
            "--expect-output",
            "0",
        ],
        &outcome.proof,
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("mutually exclusive"),
        "got: {}",
        stderr(&out)
    );
}
