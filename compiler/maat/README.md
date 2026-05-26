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
maat prove program.maat --input "1,2,3"     # Provide public inputs
maat prove program.maat --inputs-file in.json
```

Verify a proof:

```sh
maat verify program.proof.bin
```

The proof file is self-contained: it embeds the program-segment cells, public inputs, expected output, and output segment, so verification requires no additional arguments.

> **Note:** `println!` is for debugging only and does not affect the proof. The provable output is the program's return value.
> **Provability scope:** v0.16.0 produces verifiable proofs for programs that operate on primitive types (integers, `bool`, `Felt`), fixed-size arrays `[T; N]`, `Vector<T>` for primitive `T`, closures, and user-defined functions over those types. Bitwise operators (AND/OR/XOR via the chunked-LogUp diluted-form pool, SHL/SHR via the pow2-lookup pool) and ordering across every integer width up through `u64`/`i64`/`usize`/`isize` are all proven. All `examples/*.maat` programs prove and verify end-to-end. `Map<K, V>`, `Set<T>`, `str`, `struct`, and `enum` continue to execute via inline `Value` variants---they prove and verify cleanly for current programs (including `Option<T>` / `Result<T, E>` pattern matching) but their cells are not yet covered by the AIR's memory permutation argument. Segment-backed migration is deferred.

## Quick Start

```sh
cargo install maat

echo 'fn main() { println!("hello from maat"); }' > hello.maat
maat run hello.maat
```

## Documentation

Main project repository and README: [github.com/maatlabs/maat](https://github.com/maatlabs/maat)
