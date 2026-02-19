# Maat

<p align="center">
  <img src="./assets/maat-lang-transparent-logo.png" alt="Logo" width="200">
</p>

## Overview

_Maat_ is a Turing-complete programming language designed to encourage proof-driven development.

Proof-Driven Development (PDD) is software development methodology that emphasizes formal verification and mathematical proofs to ensure the correctness and reliability of code. It is an extension of test-driven development (TDD), but instead of relying solely on tests, it uses formal methods to prove properties of the code.

Source files written in Maat use the `.maat` extension.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.85 or later (with `rustup`)
- Cargo (comes with Rust)

### Installation

Clone the repository and build the project:

```bash
git clone https://github.com/maatlabs/maat.git
cd maat
cargo build --release
```

### Binaries

Maat provides two binaries:

- **`maat`** - Main compiler entry point
- **`repl`** - Interactive REPL for experimenting with Maat code

To see version information:

```bash
cargo run --release --bin maat
```

### Running the REPL

Start an interactive REPL session. The REPL compiles each line to bytecode and executes it on the VM:

```bash
cargo run --release --bin repl
```

Example session:

```txt
>> 5 + 10;
15
>> let add = fn(x, y) { x + y };
>> add(2, 3);
5
>> let newAdder = fn(x) { fn(y) { x + y } };
>> let addFive = newAdder(5);
>> addFive(10);
15
>> let fibonacci = fn(x) { if (x == 0) { 0 } else { if (x == 1) { return 1; } else { fibonacci(x - 1) + fibonacci(x - 2); } } };
>> fibonacci(15);
610
>> let map = fn(arr, f) { let iter = fn(arr, acc) { if (len(arr) == 0) { acc } else { iter(rest(arr), push(acc, f(first(arr)))); } }; iter(arr, []); };
>> map([1, 2, 3, 4], fn(x) { x * x });
[1, 4, 9, 16]
>> let unless = macro(cond, cons, alt) { quote(if (!(unquote(cond))) { unquote(cons); } else { unquote(alt); }); };
>> unless(10 > 5, "not greater", "greater");
greater
>> let double = macro(x) { quote(unquote(x) * 2) };
>> double(21);
42
```

### Running Tests

Run the full test suite:

```bash
cargo test --workspace
```

### Running Benchmarks

Maat includes a Criterion-based benchmark suite that compares the bytecode VM against the tree-walking interpreter:

```bash
# Run all benchmarks
cargo bench -p maat_tests

# Run specific benchmarks
cargo bench -p maat_tests -- fibonacci

# Save a baseline and compare after changes
cargo bench -p maat_tests -- --save-baseline before
# ... make changes ...
cargo bench -p maat_tests -- --baseline before
```

HTML reports are generated at `target/criterion/report/index.html`.

### Development

#### Code Formatting

Format code using nightly rustfmt:

```bash
cargo +nightly fmt
```

#### Linting

Run Clippy for linting (zero warnings policy):

```bash
cargo clippy --all-features --all-targets -- -D warnings
```

#### Building Documentation

Generate and view documentation:

```bash
cargo doc --all-features --no-deps --open
```

## Architecture

Maat uses a dual execution architecture:

- **Bytecode Compiler + VM**: Source code is parsed into an AST, macro-expanded via `maat_eval`, compiled to bytecode by `maat_codegen`, and executed on the stack-based `maat_vm`. This is the primary execution path used by the REPL.
- **Tree-Walking Evaluator**: The `maat_eval` crate provides a direct AST interpreter with support for the macro system (`quote`/`unquote`).

### Crate Organization

| Crate           | Description                                           |
| --------------- | ----------------------------------------------------- |
| `maat_span`     | Source location tracking and span management          |
| `maat_errors`   | Unified error handling with `Result` type alias       |
| `maat_lexer`    | Tokenization and lexical analysis                     |
| `maat_ast`      | Abstract Syntax Tree definitions and transformations  |
| `maat_parser`   | Pratt parser with operator precedence                 |
| `maat_eval`     | Tree-walking interpreter with macro system            |
| `maat_runtime`  | Object system, built-in functions, and compiled types |
| `maat_bytecode` | Instruction set encoding/decoding (31 opcodes)        |
| `maat_codegen`  | AST-to-bytecode compiler with scope analysis          |
| `maat_vm`       | Stack-based virtual machine                           |

## Contributing

Thank you for your interest in contributing to this project! All contributions large and small are actively accepted. To get started, please read the [contribution guidelines](./CONTRIBUTING.md). A good place to start would be [Good First Issues](https://github.com/maatlabs/maat/labels/good%20first%20issue).

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT license](./LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this codebase by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Status

Maat is currently at version 0.4 and is still going through several improvements in order to deliver the best-in-class experience as a fully-fledged Turing-complete PDD programming language.

## Disclaimer

Early adopters should be aware that Maat 0.4 is a transient accomplishment towards Maat 1.0, for which a formal audit process is expected.
In the meantime, we invite you to know and experiment with Maat, but we don't recommend using it to build mission-critical systems.

## Acknowledgments

Maat v0.4.0 is based on the following excellent sources:

1. [Writing An Interpreter In Go (WAIIG)](https://interpreterbook.com), which implements the `Monkey` programming language.
2. [The Lost Chapter: A Macro System for Monkey](https://interpreterbook.com/lost/), a follow-up to `WAIIG`.
3. [Writing A Compiler In Go (WACIG)](https://compilerbook.com), the compiler and virtual machine sequel to `WAIIG`.

The Maat implementation translates `Monkey`'s tree-walking interpreter, macro system, bytecode compiler, and stack-based virtual machine from Go to Rust, with significant enhancements including comprehensive numeric type support, string escape sequences, AST transformation infrastructure, runtime metaprogramming capabilities, closure compilation with free variable tracking, and a Criterion-based benchmarking suite.
