# Maat

<p align="center">
  <img src="./assets/maat-lang-transparent-logo.png" alt="Logo" width="200">
</p>

Turing-complete programming language for writing zero-knowledge proofs (ZKPs).

**WARNING:** This is a research project. It has not been audited and may contain bugs and security flaws. This implementation is NOT ready for production use.

## Overview

Proof-Driven Development (PDD) is software development methodology that emphasizes formal verification and mathematical proofs to ensure the correctness and reliability of code. It is an extension of test-driven development (TDD), but instead of relying solely on tests, it uses formal methods to prove properties of the code.

Source files written in Maat use the `.maat` extension. Compiled bytecode files use the `.mtc` extension.

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

### The `maat` Binary

Maat provides a single binary with four subcommands:

| Subcommand                               | Description                                 |
| ---------------------------------------- | ------------------------------------------- |
| `maat run <file.maat>`                   | Compile and execute a `.maat` source file   |
| `maat build <file.maat> -o <output.mtc>` | Compile a `.maat` file to `.mtc` bytecode   |
| `maat exec <file.mtc>`                   | Execute a pre-compiled `.mtc` bytecode file |
| `maat repl`                              | Start an interactive REPL session           |

To see version information:

```bash
cargo run --release -- --version
```

### Running Source Files

Compile and execute a Maat source file in a single step:

```bash
cargo run --release -- run examples/hello_world.maat
```

Or use the build-then-execute workflow for faster repeated execution:

```bash
cargo run --release -- build examples/hello_world.maat -o hello_world.mtc
cargo run --release -- exec hello_world.mtc
```

### Multi-Module Projects

Maat supports multi-file programs with `mod`, `use`, and `pub`. Imports are resolved relative to the entry file, and the compiler builds a dependency graph, type-checks each module, and produces a single linked bytecode output.

Given a project layout:

```txt
my_project/
  main.maat
  geometry.maat
  math.maat
```

**`main.maat`**:

```rust
mod geometry;
mod math;

use geometry::Point;
use math::add;

let p = Point { x: 3, y: 4 };
print(add(p.x, p.y));
print(p.sum());
```

**`geometry.maat`**:

```rust
pub struct Point {
    pub x: i64,
    pub y: i64,
}

impl Point {
    pub fn sum(self) -> i64 { self.x + self.y }
}
```

**`math.maat`**:

```rust
pub fn add(a: i64, b: i64) -> i64 { a + b }

fn internal_helper() -> i64 { 0 }
```

Run it:

```bash
cargo run --release -- run my_project/main.maat
```

Build it to a single `.mtc`:

```bash
cargo run --release -- build my_project/main.maat -o my_project.mtc
cargo run --release -- exec my_project.mtc
```

Key rules:

- `mod foo;` declares a dependency on `foo.maat` (or `foo/mod.maat`) relative to the declaring file
- `use foo::bar;` or `use foo::{bar, baz};` imports specific public items -- no glob imports (`use foo::*`) for ZK auditability
- Items without `pub` are module-private and inaccessible to importers
- Circular module dependencies are detected and rejected at compile time
- `pub use foo::bar;` re-exports items through intermediate modules

A working multi-module example is included at `examples/modules/`.

### Running the REPL

Start an interactive REPL session. The REPL compiles each line to bytecode and executes it on the VM:

```bash
cargo run --release -- repl
```

Example session:

```rust
>> 5 + 10;
15

>> let add = fn(x, y) { x + y };
>> add(2, 3);
5

>> let new_adder = fn(x) { fn(y) { x + y } };
>> let add_five = new_adder(5);
>> add_five(10);
15

>> let fibonacci = fn(x) {
..     if x == 0 {
..         0
..     } else if x == 1 {
..         return 1;
..     } else {
..         fibonacci(x - 1) + fibonacci(x - 2)
..     }
.. };
>> fibonacci(15);
610

>> let map = fn(arr, f) {
..     let iter = fn(arr, acc) {
..         if arr.len() == 0 {
..             acc
..         } else {
..             iter(arr.split_first(), acc.push(f(arr.first().unwrap())))
..         }
..     };
..     iter(arr, [])
.. };
>> map([1, 2, 3, 4], fn(x) { x * x });
[1, 4, 9, 16]

>> let unless = macro(cond, cons, alt) {
..     quote(
..         if !(unquote(cond)) {
..             unquote(cons);
..         } else {
..             unquote(alt);
..         }
..     )
.. };
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

Maat includes a Criterion-based benchmark suite for the bytecode VM:

```bash
# Run all benchmarks
cargo bench -p maat_tests --bench benchmarks

# Run specific benchmarks
cargo bench -p maat_tests --bench benchmarks -- hello_world

# Save a baseline and compare after changes
cargo bench -p maat_tests --bench benchmarks -- --save-baseline before
# ... make changes ...
cargo bench -p maat_tests --bench benchmarks -- --baseline before
```

HTML reports are generated at `target/criterion/report/index.html`.

### Fuzz Testing

Maat includes [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets for each compiler pipeline stage. Fuzz testing requires the nightly Rust toolchain.

```bash
# Install cargo-fuzz (one-time)
cargo install cargo-fuzz

# List available fuzz targets
cargo +nightly fuzz list

# Run a specific target (e.g., 60-second bounded run)
cargo +nightly fuzz run fuzz_lexer -- -max_total_time=60
cargo +nightly fuzz run fuzz_parser -- -max_total_time=60
cargo +nightly fuzz run fuzz_typechecker -- -max_total_time=60
cargo +nightly fuzz run fuzz_compiler -- -max_total_time=60
cargo +nightly fuzz run fuzz_deserializer -- -max_total_time=60
```

Crash artifacts (if any) are saved to `fuzz/artifacts/<target>/`. Seed corpora live in `fuzz/corpus/`.

### Property-Based Testing

Maat uses [proptest](https://github.com/proptest-rs/proptest) for property-based testing. These tests verify invariants (round-trip correctness, execution determinism, type soundness) over thousands of randomly generated programs.

```bash
# Run all property tests
cargo test -p maat_tests --test properties

# Run a specific property test
cargo test -p maat_tests --test properties -- bytecode_roundtrip
```

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

Maat uses a multi-module compilation pipeline. Source files are parsed into per-module ASTs, organized into a dependency graph by `maat_module`, type-checked independently with cross-module visibility enforcement, compiled to bytecode by a shared `maat_codegen` compiler instance (which implicitly links all modules into a single instruction stream), and executed on the stack-based `maat_vm`. The `maat_eval` crate (the tree-walking evaluator) is reduced to a macro-expansion-only engine (`define_macros`/`expand_macros`).

The type checker infers types for each module using Hindley-Milner inference (Algorithm W), with imported bindings injected from dependency modules' public exports. Type annotations are optional--the inference engine deduces types from usage--but can be provided on `let` bindings, function parameters, and return types for documentation or to constrain polymorphism. Generic functions with parametric polymorphism are supported (`fn identity<T>(x: T) -> T { x }`). Tuples, `char`, `Map<K, V>`, `Set<T>`, and `Vector<T>` are all first-class types with full inference support.

Custom types follow Rust syntax: `struct`, `enum` (with unit, tuple, and struct variants), `trait`, and `impl` blocks (both inherent and trait impls). Pattern matching via `match` supports literal, identifier, tuple-struct, wildcard, and or-patterns. Methods are statically dispatched via compile-time type-directed dispatch. `Option<T>` and `Result<T, E>` are pre-registered as language-level enums with full method suites (`map`, `and_then`, `unwrap_or`, etc.) and the `?` operator for ergonomic error propagation. Range syntax (`0..10` and `0..=10`) produces first-class `Range`/`RangeInclusive` values and integrates with `for..in` loops.

Errors are reported with precise `file:line:col` locations using source maps and [`ariadne`](https://docs.rs/ariadne) diagnostics. Compiled bytecode can be serialized to `.mtc` files and deserialized for later execution, enabling the `build`/`exec` workflow.

### Crate Organization

| Crate           | Description                                                      |
| --------------- | ---------------------------------------------------------------- |
| `maat_span`     | Source location tracking and span management                     |
| `maat_errors`   | Unified error handling with `Result` type alias                  |
| `maat_lexer`    | `logos` compile-time DFA tokenizer                               |
| `maat_ast`      | Abstract Syntax Tree definitions and transformations             |
| `maat_parser`   | `winnow` combinator-based parser                                 |
| `maat_eval`     | Macro expansion engine (`quote`/`unquote`)                       |
| `maat_runtime`  | Value system, built-in functions, and compiled types             |
| `maat_types`    | Hindley-Milner type inference (Algorithm W)                      |
| `maat_bytecode` | Instruction set encoding/decoding and serialization (44 opcodes) |
| `maat_codegen`  | AST-to-bytecode compiler with scope analysis                     |
| `maat_module`   | Module resolution, dependency graph, and multi-module pipeline   |
| `maat_vm`       | Stack-based virtual machine                                      |

## Contributing

Thank you for your interest in contributing to this project! All contributions large and small are actively accepted. To get started, please read the [contribution guidelines](./CONTRIBUTING.md). A good place to start would be [Good First Issues](https://github.com/maatlabs/maat/labels/good%20first%20issue).

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT license](./LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this codebase by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Security

All 13 crates enforce `#![forbid(unsafe_code)]`. The compiler and VM have been hardened against adversarial input with resource limits, checked arithmetic, and safe type conversions. See [`SECURITY.md`](./SECURITY.md) for the full threat model.

## Roadmap

Maat's development follows a phased milestone plan.

| Milestone | Focus                                                               | Status      |
| --------- | ------------------------------------------------------------------- | ----------- |
| **1**     | Rust-native, ZK-correct-by-design, working compiler                 | **Current** |
| **2**     | STARK-based ZK backend (proof generation and verification)          | Planned     |
| **3**     | Advanced type system (linear types, effect system) and self-hosting | Planned     |

## Status

Maat is currently at version `0.11.1` (Milestone 1). The compiler frontend, type system, module system, bytecode VM, and CLI toolchain are functional and tested. This is the first public release.

## Disclaimer

Early adopters should be aware that Maat `0.11.1` is a step toward Maat 1.0, for which a formal audit process is expected. In the meantime, we invite you to explore and experiment with Maat, but we do not recommend using it to build mission-critical systems.

## Acknowledgments

Maat's early architecture (v0.1--v0.4) was inspired by Thorsten Ball's [Writing An Interpreter In Go](https://interpreterbook.com), [The Lost Chapter: A Macro System for Monkey](https://interpreterbook.com/lost/), and [Writing A Compiler In Go](https://compilerbook.com). The lexer structure, Pratt parser skeleton, tree-walking evaluator, macro system, and initial bytecode VM design trace back to these books, translated from Go to Rust.

Since then, Maat has diverged substantially. The language now features `Hindley-Milner` type inference, Rust-native custom types (structs, enums, traits, impl blocks, pattern matching), a multi-file module system with visibility enforcement, a ZK-first design that rejects floating-point and implicit truthiness, built-in `Option<T>` and `Result<T, E>`, source-location diagnostics, bytecode serialization, and a CLI toolchain--none of which originate from the source material.
