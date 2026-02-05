# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-02-05

Major feature release adding a Lisp-style runtime macro system for metaprogramming, based on "The Lost Chapter: A Macro System for Monkey" by Thorsten Ball.

### Added

#### Macro System

- **`macro` Keyword**: New language construct for defining compile-time code transformations
  - Syntax: `let name = macro(params...) { body };`
  - Macros are first-class objects stored in the environment
  - Support for zero or more parameters
  - Lexically-scoped macro definitions

- **`quote` Special Form**: Captures AST nodes without evaluation
  - Syntax: `quote(expression)`
  - Returns a `Quote` object wrapping the unevaluated AST
  - Enables code-as-data manipulation
  - Handles `unquote` calls within quoted expressions

- **`unquote` Special Form**: Splices evaluated expressions into quoted code
  - Syntax: `unquote(expression)` (used within `quote`)
  - Evaluates the expression and inserts its AST into the surrounding quote
  - Enables dynamic code generation
  - Supports arbitrary nesting depth

- **AST Transformation Infrastructure**: `transform()` function for traversing and modifying AST
  - Post-order traversal of entire AST
  - Functional approach using closures
  - Type-safe node transformation
  - Used by macro expansion and available for future compiler optimizations

- **Macro Expansion Pipeline**: Two-phase macro processing
  - `define_macros()`: Extracts macro definitions from programs and stores in environment
  - `expand_macros()`: Recursively expands all macro calls using AST transformation
  - Expansion happens before evaluation (compile-time metaprogramming)
  - Properly handles nested macro calls

#### New Object Types

- **`Macro` Object**: Runtime representation of macro definitions
  - Stores parameter names, body (block statement), and closure environment
  - Evaluated during macro expansion, not normal evaluation
  - Display format: `macro(params...) { body }`

- **`Quote` Object**: Wrapper for unevaluated AST nodes
  - Enables passing code as data to macros
  - Display format: `quote(expression)`
  - Converts back to AST during macro expansion

#### New AST Nodes

- **`MacroLiteral` Expression**: Parser representation of macro definitions
  - Similar structure to `FunctionLiteral`
  - Parameters and block statement body
  - Distinct from functions in evaluation semantics

#### Constants for Special Forms

- **`QUOTE` Constant**: Centralized name for `quote` special form
  - Ensures consistent string comparisons
  - Single source of truth for special form names
  - Fully documented as special form (not regular builtin)

- **`UNQUOTE` Constant**: Centralized name for `unquote` special form
  - Used in macro expansion to identify unquote calls
  - Prevents magic strings in codebase
  - Type-safe string handling

#### Testing

- **Comprehensive Macro Test Suite**: 7 new tests in `tests/macros.rs`
  - `test_define_macros`: Verifies macro extraction from programs
  - `test_expand_macros`: Tests basic macro expansion
  - `test_expand_macros_with_unquote`: Tests unquote splicing
  - `test_quote_builtin`: Verifies quote special form
  - `test_macro_expansion_unless`: Complex conditional macro (`unless`)
  - `test_macro_double`: Simple arithmetic macro
  - `test_macro_with_multiple_args`: Multi-parameter macros
- **Transform Test Suite**: 13 new tests in `tests/transform.rs`
  - Tests for all AST node types (programs, statements, expressions)
  - Nested structure transformation tests
  - Post-order traversal verification

### Changed

- **Evaluator**: Integrated macro processing into evaluation pipeline
  - Macros are now extracted and expanded before program evaluation
  - `eval()` function handles `quote` special form directly (not as regular builtin)
  - Special handling for `Macro` and `Quote` object types
  - Expanded code is evaluated normally after expansion

- **Parser**: Added support for `macro` keyword
  - `parse_macro()` function mirrors `parse_function()` structure
  - Macro literals parsed as expressions
  - Proper precedence and associativity handling

- **Lexer**: Added `TokenKind::Macro`
  - Recognized as keyword via `keyword_or_ident()` function
  - Distinct from identifiers in token stream

### Documentation

- **README**: Updated example session to showcase macro system
  - `double` macro example demonstrating basic metaprogramming
  - `unless` macro showing conditional code generation
  - Version bumped to 0.2.0 throughout documentation
  - Updated acknowledgments to credit `The Lost Chapter: A Macro System for Monkey`

### Performance

- **Efficient AST Transformation**: Post-order traversal minimizes allocations
  - Transform children before parents
  - Reuses existing AST structure where possible
  - Functional approach with closures avoids virtual dispatch

- **Lazy Macro Expansion**: Only expands macros that are actually called
  - Unused macro definitions don't impact performance
  - Expansion happens once per program evaluation

---

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

[0.2.0]: https://github.com/maatlabs/maat/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/maatlabs/maat/releases/tag/v0.1.0
