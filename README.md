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

- **`maat`** - Main compiler/interpreter entry point
- **`repl`** - Interactive REPL for experimenting with Maat code

To see version information:

```bash
cargo run --release --bin maat
```

### Running the REPL

Start an interactive REPL session:

```bash
cargo run --release --bin repl
```

Example session:

```txt
>> let name = "Maat";
>> let version = 0.3;
>> print("Welcome to", name, version);
Welcome to Maat 0.3

>> let double = macro(x) { quote(unquote(x) * 2); };
>> double(21);
42

>> let unless = macro(cond, cons, alt) {
..     quote(if (!(unquote(cond))) {
..         unquote(cons);
..     } else {
..         unquote(alt);
..     });
.. };
>> unless(10 > 5, print("not greater"), print("greater"));
greater
```

### Running Tests

Run the full test suite:

```bash
cargo test --workspace
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

## Contributing

Thank you for your interest in contributing to this project! All contributions large and small are actively accepted. To get started, please read the [contribution guidelines](./CONTRIBUTING.md). A good place to start would be [Good First Issues](https://github.com/maatlabs/maat/labels/good%20first%20issue).

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT license](./LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this codebase by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Status

Maat is currently at version 0.3 and is still going through several improvements in order to deliver the best-in-class experience as a fully-fledged Turing-complete PDD programming language.

## Disclaimer

Early adopters should be aware that Maat 0.3 is a transient accomplishment towards Maat 1.0, for which a formal audit process is expected.
In the meantime, we invite you to know and experiment with Maat, but we don't recommend using it to build mission-critical systems.

## Acknowledgments

Maat v0.3.0 is based on the following excellent sources:

1. [Writing An Interpreter In Go (WAIIG)](https://interpreterbook.com), which implements the `Monkey` programming language.
2. [The Lost Chapter: A Macro System for Monkey](https://interpreterbook.com/lost/), a follow-up to `WAIIG`.

The Maat interpreter implementation follows `Monkey`'s approach of building a tree-walking interpreter with a Lisp-style macro system, translated from Go to Rust with significant enhancements including comprehensive numeric type support, string escape sequences, AST transformation infrastructure, runtime metaprogramming capabilities, and an improved REPL experience.
