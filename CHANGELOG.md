# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-04

Initial release of Maat: a Turing-complete programming language designed for proof-driven development. This release includes a complete interpreter implementation.

### Added

#### Core Language Features

- **Lexer**: Fast, zero-copy lexical analyzer with comprehensive token support
  - Single and multi-character operators (`+`, `-`, `*`, `/`, `==`, `!=`, `<=`, `>=`, `<`, `>`)
  - Keywords: `let`, `fn`, `if`, `else`, `return`, `true`, `false`
  - Unicode identifier support via `unicode-xid` crate
  - String literals with escape sequence support (`\"`, `\\`, `\n`, `\r`, `\t`, `\0`)
  - Comments and whitespace handling

- **Numeric Type System**: Full support for all Rust numeric types
  - Signed integers: `i8`, `i16`, `i32`, `i64`, `i128`, `isize`
  - Unsigned integers: `u8`, `u16`, `u32`, `u64`, `u128`, `usize`
  - Floating point: `f32`, `f64`
  - Multiple radix support: binary (`0b`), octal (`0o`), hexadecimal (`0x`)
  - Scientific notation for floats (e.g., `1.5e10`)
  - Type suffixes (e.g., `42i64`, `3.14f32`)
  - Default types: `i64` for integers, `f64` for floats

- **Parser**: Pratt parser with full expression and statement support
  - Let statements: `let x = 5;`
  - Return statements: `return x + y;`
  - Expression statements
  - Prefix operators: `!`, `-`
  - Infix operators with proper precedence
  - Grouped expressions: `(x + y) * z`
  - Conditional expressions: `if (x < 10) { true } else { false }`
  - Function literals: `fn(x, y) { x + y }`
  - Function calls: `add(2, 3)`
  - Array literals: `[1, 2, 3]`
  - Hash literals: `{"key": "value"}`
  - Index expressions: `arr[0]`, `hash["key"]`

- **Evaluator**: Tree-walking interpreter with environment-based evaluation
  - Lexically-scoped environments
  - First-class functions with closures
  - Checked arithmetic operations (prevents overflow panics)
  - Automatic integer type coercion for array indexing (any integer type works)
  - Hash tables with hashable keys (integers, booleans, strings)
  - Proper null handling and early returns

- **Built-in Functions**
  - `len(x)`: Returns length of strings, arrays, or hash tables
  - `first(arr)`: Returns first element of an array
  - `last(arr)`: Returns last element of an array
  - `rest(arr)`: Returns array without first element
  - `push(arr, x)`: Returns new array with element appended
  - `puts(...)`: Prints arguments to stdout

#### Developer Experience

- **REPL**: Interactive Read-Eval-Print Loop
  - Suppresses output for `let` statements (Python-style behavior)
  - Suppresses `null` return values for cleaner output
  - Persistent environment across evaluations
  - Proper error reporting

- **Error Handling**
  - Span tracking for precise error location reporting
  - Line and column number information
  - Descriptive error messages for parse and evaluation errors
  - Type mismatch detection

- **Testing**: Comprehensive test suite with 112+ tests
  - Lexer tests for all token types and edge cases
  - Parser tests for all expression and statement types
  - Evaluator tests for all language features
  - REPL integration tests

#### Infrastructure

- **CI/CD**: GitHub Actions workflows
  - Build verification on Ubuntu, macOS, and Windows
  - Test suite runs on stable and nightly Rust
  - Formatting checks with `rustfmt`
  - Linting with `clippy` (zero warnings policy)
  - Documentation generation and validation

- **Documentation**
  - Comprehensive inline documentation for public APIs
  - Usage examples in doc comments
  - README with project overview and contribution guidelines
  - CONTRIBUTING.md with development workflow

### Security

- Checked arithmetic operations throughout to prevent integer overflow panics
- Proper bounds checking for array and hash access
- Safe handling of negative indices (returns error instead of panicking)
- No unsafe code

### Performance

- Zero-copy lexer using string slices
- Efficient hash table implementation
- Minimal allocations in hot paths
- Optimized release builds

---

## Guidelines for Contributors

When adding entries to this changelog for future releases:

1. **Format**: Follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
2. **Categories**: Use Added, Changed, Deprecated, Removed, Fixed, Security
3. **Audience**: Write for users, not developers (focus on impact, not implementation)
4. **Links**: Add comparison links at the bottom: `[0.2.0]: https://github.com/maatlabs/maat/compare/v0.1.0...v0.2.0`

[0.1.0]: https://github.com/maatlabs/maat/releases/tag/v0.1.0
