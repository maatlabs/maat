# maat_prover

Zero knowledge STARK prover and verifier for the Maat programming language.

## Role

`maat_prover` wires together the Maat AIR constraint system (`maat_air`), the execution trace (`maat_trace`), and Winterfell's proving infrastructure to produce and verify cryptographic proofs of correct program execution. It implements Winterfell's `Prover` trait via `MaatProver`, handles proof serialization with a self-contained binary format, and provides a thin verification wrapper around Winterfell's verifier. The AIR declares main-segment transition degrees via static arrays and aux-segment degrees through the registered `BuiltinSet`'s runtime `Layout`; `winter_air::TraceInfo::meta` is empty and the verifier reconstructs the same `AirContext` from the same constants and the same registered builtin set.

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

## Provability Scope (v0.18.0)

End-to-end proving is supported for programs with a top-level `fn main(<params>) -> T` entry point that operate on **primitive types, fixed-size arrays, `Vector<T>`, closures, and Rescue-Prime hashing**: integers (`i8`..`i64`, `u8`..`u64`, `usize`, `isize`), `bool`, `Felt` (Goldilocks field element), `[T; N]` for primitive `T`, `Vector<T>` for primitive `T`, closures, `hash::rescue_2` / `rescue_4` / `rescue_8`, and user-defined functions over those types (parameters, return values, nested calls, bounded recursion). `pub` parameters bind to a boundary-constrained public-memory cell; bare parameters bind to a prover-supplied uncommitted witness cell. All `examples/*.maat` programs prove and verify end-to-end under `development_options`.

The remaining surface composite types (`Map<K, V>`, `Set<T>`, `str`, `struct`, `enum`) continue to execute under inline `Value` variants. The inline forms prove and verify cleanly for every program currently in the corpus because their cells never enter the heap and never produce multi-cell-per-dispatch trace rows. The verifier accepts a primitive-rooted program that contains these types, including programs that pattern-match on `Option<T>` / `Result<T, E>` or use `Map`/`Set` builtins. Segment-backed migration is deferred until a real consumer (recursive proofs, STARK-to-SNARK wrapping, in-AIR composite-cell content for Map keys) demonstrates that AIR-level cell coverage is load-bearing.

## Proof File Format (wire v6)

```text
PROOF_MAGIC:        b"MATP"       (4 bytes)
PROOF_VERSION:      u16 BE        (2 bytes, currently 6)
OUTPUT:             u64 LE        (8 bytes, claimed program output)
INPUT_COUNT:        u16 BE        (2 bytes, number of public inputs)
INPUTS:             [u64; N] LE   (8 * N bytes, public input values)
INPUT_BASE:         u32 BE        (4 bytes, flat base of the public-input segment)
OUTPUT_BASE:        u32 BE        (4 bytes, flat base of the output segment)
OUTPUT_SEG_LEN:     u32 BE        (4 bytes, number of public-output cells)
OUTPUT_SEG:         [u64; L] LE   (8 * L bytes, public-output cell values)
PROGRAM_BASE:       u32 BE        (4 bytes, flat base of the program segment)
PROGRAM_SEG_LEN:    u32 BE        (4 bytes, number of program-memory cells)
PROGRAM_SEG:        [u64; P] LE   (8 * P bytes, program-memory cell values)
PAYLOAD:            Winterfell    (variable, native Proof encoding)
```

Minimum header: 36 bytes (with zero inputs, zero output cells, zero program cells). The bytecode is pinned cell-by-cell into the AIR's public-memory accumulator via `PROGRAM_BASE` / `PROGRAM_SEG`; public-input cells are bound via `INPUT_BASE` / `INPUTS`. Output and program segments are capped at `MAX_PUBLIC_SEGMENT_CELLS = 2^20` cells each; public-input count is capped at `MAX_INPUT_COUNT = 1024`. The embedded output, public-input cells, output segment, and program segment are absorbed into the Fiat-Shamir transcript so a verifier disagreeing with the prover on any header field derives different challenges and rejects.

## Verifier-Side Public-I/O Bundle

`PublicIo` is the JSON bundle exchanged across the proof boundary. `maat_prover::extract_public_io(&[u8]) -> Result<PublicIo, BundleError>` extracts it directly from a proof byte stream so external tooling (e.g., recursive provers, on-chain verifiers, audit harnesses, etc.) can pin embedded public values without re-implementing the wire layout. The CLI exposes `maat verify --public-io <path>` / `--input` / `--expect-output` / `--expect-program{,-hash}` for the same purpose; mismatches print a structured `error: ...` line and exit non-zero. See [SECURITY.md](../../SECURITY.md#attacker-3-malicious-proofbin-stark-proof) for the assertion layer's order of operations.

## Usage

```rust
use maat_prover::{MaatProver, development_options, verify_with_inputs};
use maat_air::{MaatPublicInputs, PublicMemory, PublicSegment};

let artifacts = maat_trace::run_with_io(bytecode, &public_inputs, &private_inputs)?;
let output = artifacts
    .result
    .as_ref()
    .map(|v| v.to_felt())
    .unwrap_or(maat_field::BaseElement::ZERO);
let public_inputs = MaatPublicInputs::new(
    output,
    PublicMemory {
        input: PublicSegment::new(artifacts.memory.input.base, public_inputs),
        output: artifacts.memory.output,
        program: artifacts.memory.program,
    },
);

let prover = MaatProver::new(development_options(), public_inputs.clone());
let proof = prover.generate_proof(artifacts.trace)?;
verify_with_inputs(proof, public_inputs)?;
```

## API Docs

[docs.rs/maat_prover](https://docs.rs/maat_prover/latest/maat_prover/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
