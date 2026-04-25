# maat_prover

Zero knowledge STARK prover and verifier for the Maat programming language.

## Role

`maat_prover` wires together the Maat AIR constraint system (`maat_air`), the execution trace (`maat_trace`), and Winterfell's proving infrastructure to produce and verify cryptographic proofs of correct program execution. It implements Winterfell's `Prover` trait via `MaatProver`, handles proof serialization with a self-contained binary format, and provides a thin verification wrapper around Winterfell's verifier. `MaatTrace::from_trace_table` runs `maat_air::encode_degrees` on the main trace and ships the per-constraint tight degrees through `winter_air::TraceInfo::meta`, so the verifier reconstructs the same `AirContext` without any out-of-band coordination.

## Architecture

```text
Bytecode --> TraceVM --> TraceTable --> MaatProver --> Proof
                                           |            |
                                           v            v
                                    MaatPublicInputs    |
                                           |            |
                                     verify(proof) <----+
```

## Proof Options

| Preset        | Queries | Blowup | Grinding | Security (conjectural) |
| ------------- | ------- | ------ | -------- | ---------------------- |
| `development` | 4       | 8      | 0        | Minimal (fast)         |
| `production`  | 27      | 8      | 16       | ~97 bits               |

Both presets require `FieldExtension::Quadratic` because the auxiliary trace segment evaluates constraints over `QuadExtension<BaseElement>`.

## Provability Scope (v0.13.0)

End-to-end proving is supported for programs that operate on **primitive types only**: integers (`i8`..`i64`, `u8`..`u64`, `usize`), `bool`, `Felt` (Goldilocks field element), and user-defined functions over those types (including parameters, return values, nested calls, and bounded recursion). Composite types (`Vector<T>`, `Map<K, V>`, `Set<T>`, `str`, `struct`, `enum`, fixed-size arrays `[T; N]`, closures) execute correctly under the standard VM but are **not yet trace-VM-complete**: `prove` will emit a proof that the verifier rejects. Composite-type tracing requires heap-allocated segment memory model, planned for a future release.

## Proof File Format

```text
PROOF_MAGIC:        b"MATP"    (4 bytes)
PROOF_VERSION:      u16 BE     (2 bytes, currently 1)
PROGRAM_HASH:       [u8; 32]   (32 bytes, Blake3 digest of serialized bytecode)
PAYLOAD:            Winterfell  (variable, native Proof encoding)
```

Total header: 38 bytes. The program hash binds each proof to the exact bytecode that produced the execution trace.

## Usage

```rust
use maat_prover::{MaatProver, development_options, verify, compute_program_hash};
use maat_air::MaatPublicInputs;

let (trace, result) = maat_trace::run_trace(bytecode.clone())?;
let output = /* encode result as BaseElement */;
let program_hash = compute_program_hash(&bytecode)?;
let public_inputs = MaatPublicInputs::new(program_hash, vec![], output);

let prover = MaatProver::new(development_options(), public_inputs.clone());
let proof = prover.generate_proof(trace)?;
verify(proof, public_inputs)?;
```

## API Docs

[docs.rs/maat_prover](https://docs.rs/maat_prover/latest/maat_prover/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
