# maat

The Maat programming language -- Rust-native syntax for writing zero-knowledge proofs.

## Overview

Maat is a Turing-complete ZK programming language. It accepts Rust-like syntax and rejects any construct illegal in zero-knowledge protocols (floating-point arithmetic, global mutable state, unbounded side effects, etc.). All programs are executed over the Goldilocks prime field and produce execution traces suitable for STARK proving and verification.

## Subcommands

```txt
maat run    <file.maat>                     Compile and execute a `.maat` source file
maat build  <file.maat> -o <out.mtc>        Compile a `.maat` file to `.mtc` bytecode
maat exec   <file.mtc>                      Execute a pre-compiled `.mtc` bytecode file
maat trace  <file.maat> -o <out.csv>        Execute and emit the ZK execution trace
maat prove  <file.maat> [options]           Generate a STARK proof of correct execution
maat verify <proof.bin>                     Verify a STARK proof file
maat repl                                   Start an interactive REPL session
```

### Proving and Verification

Generate a STARK proof:

```sh
maat prove program.maat                     # Development mode (~12 bits)
maat prove program.maat --production        # Production mode (~97 bits)
maat prove program.maat -o out.proof.bin    # Custom output path
maat prove program.maat -t trace.csv        # Also dump execution trace
maat prove program.maat --input "1,2,3"     # Public inputs (bind `pub` parameters)
maat prove program.maat --private-input "4,5"            # Private witness inputs
maat prove program.maat --inputs-file in.json            # Public inputs from JSON
maat prove program.maat --expect-output 42               # Fail fast on wrong witness
maat prove program.maat --write-public-io io.json        # Emit verifier-side bundle
```

`maat prove` requires a top-level `fn main(<params>) -> T`; script-form programs are `run` / `exec`-only. `pub` parameters bind to a boundary-constrained public-memory cell; bare parameters bind to a prover-supplied uncommitted witness cell.

Verify a proof:

```sh
maat verify program.proof.bin                            # Cryptographic verify only
maat verify program.proof.bin --public-io io.json        # + bundle
maat verify program.proof.bin --input "1,2,3" \
                              --expect-output 42 \
                              --expect-program program.maat
```

The cryptographic verify runs first; assertion mismatches are only reported on proofs that are themselves valid. Mismatches print a structured `error: ...` line and exit non-zero; a successful run with at least one assertion prints `assertions: K matched`.

> **Note:** `println!` is for debugging only and does not affect the proof. The provable output is the program's return value.
> **Provability scope:** v0.17.0 produces verifiable proofs for programs with a top-level `fn main(<params>) -> T` entry point that operate on primitive types (integers, `bool`, `Felt`), fixed-size arrays `[T; N]`, `Vector<T>` for primitive `T`, closures, Rescue-Prime hashing (`hash::rescue_2` / `rescue_4` / `rescue_8`), and user-defined functions over those types. All `examples/*.maat` programs prove and verify end-to-end. `Map<K, V>`, `Set<T>`, `str`, `struct`, and `enum` continue to execute via inline `Value` variants---they prove and verify cleanly for current programs (including `Option<T>` / `Result<T, E>` pattern matching) but their cells are not yet covered by the AIR's memory permutation argument. Segment-backed migration is deferred.

## Quick Start

```sh
cargo install maat

echo 'fn main() -> i64 { let a = 1; let b = 2; a + b }' > hello.maat
maat run hello.maat
maat prove hello.maat
maat verify hello.proof.bin --expect-output 3
```

## Documentation

Main project repository and README: [github.com/maatlabs/maat](https://github.com/maatlabs/maat)
