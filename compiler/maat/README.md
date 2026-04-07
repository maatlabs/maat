# maat

The Maat programming language -- Rust-native syntax for writing zero-knowledge proofs.

## Overview

Maat is a Turing-complete ZK programming language. It accepts Rust-like syntax and rejects any construct illegal in zero-knowledge protocols (floating-point
arithmetic, global mutable state, unbounded side effects). All programs are executed over the Goldilocks prime field and produce execution traces suitable for STARK proving and verification.

## Subcommands

```txt
maat run   <file.maat>                      Compile and execute a `.maat` source file
maat build <file.maat> -o <out.mtc>         Compile a `.maat` file to `.mtc` bytecode
maat exec  <file.mtc>                       Execute a pre-compiled `.mtc` bytecode file
maat trace <file.maat> [-o out.csv]         Execute and emit the ZK execution trace
maat repl                                   Start an interactive REPL session
```

## Quick Start

```sh
cargo install maat

echo 'fn main() { println!("hello from maat"); }' > hello.maat
maat run hello.maat
```

## Documentation

Main project repository and README: [github.com/maatlabs/maat](https://github.com/maatlabs/maat)
