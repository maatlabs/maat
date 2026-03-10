# Maat

<p align="center">
  <img src="./assets/maat-lang-transparent-logo.png" alt="Logo" width="200">
</p>

## Overview

_Maat_ is a Turing-complete programming language designed to encourage proof-driven development.

Proof-Driven Development (PDD) is software development methodology that emphasizes formal verification and mathematical proofs to ensure the correctness and reliability of code. It is an extension of test-driven development (TDD), but instead of relying solely on tests, it uses formal methods to prove properties of the code.

Source files written in Maat use the `.mt` extension. Compiled bytecode files use the `.mtc` extension.

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

| Subcommand                             | Description                                 |
| -------------------------------------- | ------------------------------------------- |
| `maat run <file.mt>`                   | Compile and execute a `.mt` source file     |
| `maat build <file.mt> -o <output.mtc>` | Compile a `.mt` file to `.mtc` bytecode     |
| `maat exec <file.mtc>`                 | Execute a pre-compiled `.mtc` bytecode file |
| `maat repl`                            | Start an interactive REPL session           |

To see version information:

```bash
cargo run --release -- --version
```

### Running Source Files

Compile and execute a Maat source file in a single step:

```bash
cargo run --release -- run examples/fibonacci.mt
```

Or use the build-then-execute workflow for faster repeated execution:

```bash
cargo run --release -- build examples/fibonacci.mt -o fibonacci.mtc
cargo run --release -- exec fibonacci.mtc
```

### Multi-Module Projects

Maat supports multi-file programs with `mod`, `use`, and `pub`. Imports are resolved relative to the entry file, and the compiler builds a dependency graph, type-checks each module, and produces a single linked bytecode output.

Given a project layout:

```txt
my_project/
  main.mt
  geometry.mt
  math.mt
```

**`main.mt`**:

```rust
mod geometry;
mod math;

use geometry::Point;
use math::add;

let p = Point { x: 3, y: 4 };
print(add(p.x, p.y));
print(p.sum());
```

**`geometry.mt`**:

```rust
pub struct Point {
    pub x: i64,
    pub y: i64,
}

impl Point {
    pub fn sum(self) -> i64 { self.x + self.y }
}
```

**`math.mt`**:

```rust
pub fn add(a: i64, b: i64) -> i64 { a + b }

fn internal_helper() -> i64 { 0 }
```

Run it:

```bash
cargo run --release -- run my_project/main.mt
```

Build it to a single `.mtc`:

```bash
cargo run --release -- build my_project/main.mt -o my_project.mtc
cargo run --release -- exec my_project.mtc
```

Key rules:

- `mod foo;` declares a dependency on `foo.mt` (or `foo/mod.mt`) relative to the declaring file
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

Maat includes a Criterion-based benchmark suite for the bytecode VM:

```bash
# Run all benchmarks
cargo bench -p maat_tests --bench benchmarks

# Run specific benchmarks
cargo bench -p maat_tests --bench benchmarks -- fibonacci

# Save a baseline and compare after changes
cargo bench -p maat_tests --bench benchmarks -- --save-baseline before
# ... make changes ...
cargo bench -p maat_tests --bench benchmarks -- --baseline before
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

Maat uses a multi-module compilation pipeline. Source files are parsed into per-module ASTs, organized into a dependency graph by `maat_module`, type-checked independently with cross-module visibility enforcement, compiled to bytecode by a shared `maat_codegen` compiler instance (which implicitly links all modules into a single instruction stream), and executed on the stack-based `maat_vm`. The `maat_eval` crate (the tree-walking evaluator) is reduced to a macro-expansion-only engine (`define_macros`/`expand_macros`).

The type checker infers types for each module using Hindley-Milner inference (Algorithm W), with imported bindings injected from dependency modules' public exports. Type annotations are optional--the inference engine deduces types from usage--but can be provided on `let` bindings, function parameters, and return types for documentation or to constrain polymorphism. Generic functions with parametric polymorphism are supported (`fn identity<T>(x: T) -> T { x }`).

Custom types follow Rust syntax: `struct`, `enum` (with unit, tuple, and struct variants), `trait`, and `impl` blocks (both inherent and trait impls). Pattern matching via `match` supports literal, identifier, tuple-struct, wildcard, and or-patterns. Methods are statically dispatched. `Option<T>` and `Result<T, E>` are pre-registered as language-level enums.

Errors are reported with precise `file:line:col` locations using source maps and [`ariadne`](https://docs.rs/ariadne) diagnostics. Compiled bytecode can be serialized to `.mtc` files and deserialized for later execution, enabling the `build`/`exec` workflow.

### Crate Organization

| Crate           | Description                                                      |
| --------------- | ---------------------------------------------------------------- |
| `maat_span`     | Source location tracking and span management                     |
| `maat_errors`   | Unified error handling with `Result` type alias                  |
| `maat_lexer`    | Tokenization and lexical analysis                                |
| `maat_ast`      | Abstract Syntax Tree definitions and transformations             |
| `maat_parser`   | Pratt parser with operator precedence                            |
| `maat_eval`     | Macro expansion engine (`quote`/`unquote`)                       |
| `maat_runtime`  | Object system, built-in functions, and compiled types            |
| `maat_types`    | Hindley-Milner type inference (Algorithm W)                      |
| `maat_bytecode` | Instruction set encoding/decoding and serialization (35 opcodes) |
| `maat_codegen`  | AST-to-bytecode compiler with scope analysis                     |
| `maat_module`   | Module resolution, dependency graph, and multi-module pipeline   |
| `maat_vm`       | Stack-based virtual machine                                      |

## Contributing

Thank you for your interest in contributing to this project! All contributions large and small are actively accepted. To get started, please read the [contribution guidelines](./CONTRIBUTING.md). A good place to start would be [Good First Issues](https://github.com/maatlabs/maat/labels/good%20first%20issue).

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT license](./LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this codebase by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Status

Maat is currently at version 0.8 and is still going through several improvements in order to deliver the best-in-class experience as a fully-fledged Turing-complete ZK programming language.

## Disclaimer

Early adopters should be aware that Maat 0.8 is a transient accomplishment towards Maat 1.0, for which a formal audit process is expected. In the meantime, we invite you to know and experiment with Maat, but we don't recommend using it to build mission-critical systems.

## Acknowledgments

Maat's early architecture (v0.1--v0.4) was inspired by Thorsten Ball's [Writing An Interpreter In Go](https://interpreterbook.com), [The Lost Chapter: A Macro System for Monkey](https://interpreterbook.com/lost/), and [Writing A Compiler In Go](https://compilerbook.com). The lexer structure, Pratt parser skeleton, tree-walking evaluator, macro system, and initial bytecode VM design trace back to these books, translated from Go to Rust.

Since then, Maat has diverged substantially. The language now features `Hindley-Milner` type inference, Rust-native custom types (structs, enums, traits, impl blocks, pattern matching), a multi-file module system with visibility enforcement, a ZK-first design that rejects floating-point and implicit truthiness, built-in `Option<T>` and `Result<T, E>`, source-location diagnostics, bytecode serialization, and a CLI toolchain--none of which originate from the source material.
