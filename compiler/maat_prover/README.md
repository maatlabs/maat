# maat_prover

Zero knowledge STARK prover and verifier for the Maat programming language.

## Role

`maat_prover` wires together the Maat AIR constraint system (`maat_air`), the execution trace (`maat_trace`), and Winterfell's proving infrastructure to produce and verify cryptographic proofs of correct program execution. It implements Winterfell's `Prover` trait via `MaatProver`, handles proof serialization with a self-contained binary format, and provides a thin verification wrapper around Winterfell's verifier. The AIR declares static upper-bound transition degrees via `pub const` arrays, so `winter_air::TraceInfo::meta` is empty and the verifier reconstructs the same `AirContext` directly from the AIR constants.

## Architecture

```text
Bytecode --> VM + TraceRecorder --> TraceTable --> MaatProver --> Proof
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

## Provability Scope (v0.15.0)

End-to-end proving is supported for programs that operate on **primitive types, fixed-size arrays, `Vector<T>`, and closures**: integers (`i8`..`i64`, `u8`..`u64`, `usize`), `bool`, `Felt` (Goldilocks field element), `[T; N]` for primitive `T`, `Vector<T>` for primitive `T` (segment-backed, with builtin-allocated cells covered by the memory permutation argument), closures (segment-backed captures, tamper-detected via aux constraint 1), and user-defined functions over those types (parameters, return values, nested calls, bounded recursion). All twelve `examples/*.maat` programs prove and verify end-to-end under `development_options`.

The remaining surface composite types (`Map<K, V>`, `Set<T>`, `str`, `struct`, `enum`) continue to execute under inline `Value` variants. The inline forms prove and verify cleanly for every program currently in the corpus because their cells never enter the heap and never produce multi-cell-per-dispatch trace rows. The verifier accepts a primitive-rooted program that contains these types, including programs that pattern-match on `Option<T>` / `Result<T, E>` or use `Map`/`Set` builtins. Segment-backed migration is deferred until a real consumer (recursive proofs, STARK-to-SNARK wrapping, in-AIR composite-cell content for Map keys) demonstrates that AIR-level cell coverage is load-bearing.

## Proof File Format

```text
PROOF_MAGIC:        b"MATP"       (4 bytes)
PROOF_VERSION:      u16 BE        (2 bytes, currently 3)
PROGRAM_HASH:       [u8; 32]      (32 bytes, raw Blake3 digest)
OUTPUT:             u64 LE        (8 bytes, claimed program output)
INPUT_COUNT:        u16 BE        (2 bytes, number of public inputs)
INPUTS:             [u64; N] LE   (8 * N bytes, public input values)
PAYLOAD:            Winterfell    (variable, native Proof encoding)
```

Minimum header: 48 bytes (with zero inputs). The program hash binds each proof to the exact bytecode that produced the execution trace; the embedded output and inputs are absorbed into the Fiat-Shamir transcript so a verifier disagreeing with the prover on any header field derives different challenges and rejects.

## Usage

```rust
use maat_prover::{MaatProver, development_options, verify, compute_program_hash};
use maat_air::MaatPublicInputs;

let (trace, result) = maat_trace::run(bytecode.clone())?;
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
