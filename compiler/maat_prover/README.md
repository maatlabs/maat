# maat_prover

Zero knowledge STARK prover and verifier for the Maat programming language.

## Role

`maat_prover` wires together the Maat AIR constraint system (`maat_air`), the execution trace (`maat_trace`), and Winterfell's proving infrastructure to produce and verify cryptographic proofs of correct program execution. It implements Winterfell's `Prover` trait via `MaatProver`, handles proof serialization with a self-contained binary format, and provides a thin verification wrapper around Winterfell's verifier.

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
