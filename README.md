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

### Running the REPL

Start an interactive REPL session:

```bash
cargo run --release
```

Example session:

```txt
>> let name = "Maat";
>> let version = 0.1;
>> print("Welcome to", name, version);
Welcome to Maat 0.1
```

### Running Tests

Run the full test suite:

```bash
cargo test --all-features
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

Maat is currently at version 0.1 and is still going through several improvements in order to deliver the best-in-class experience as a fully-fledged Turing-complete PDD programming language.

## Disclaimer

Early adopters should be aware that Maat 0.1 is a transient accomplishment towards Maat 1.0, for which a formal audit process is expected.
In the meantime, we invite you to know and experiment with Maat, but we don't recommend using it to build mission-critical systems.

## Acknowledgments

Maat v0.1.0 is based on the excellent book [Writing An Interpreter In Go](https://interpreterbook.com) by [Thorsten Ball](https://thorstenball.com). The interpreter implementation follows the book's approach of building a tree-walking interpreter for the Monkey programming language, translated from Go to Rust with significant enhancements including comprehensive numeric type support, string escape sequences, span tracking for error reporting, and an improved REPL experience.
