# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2026-02-26

Type system foundation release. Maat now performs Hindley-Milner type inference (Algorithm W) over the entire program, catching type errors at compile time. This release also introduces loop constructs (`for`, `while`, `loop`), typed function parameters, return type annotations, generic type parameters, and trait bounds--laying the groundwork for custom types in a future release.

### Added

#### Type System (Hindley-Milner / Algorithm W)

- **`maat_types` crate**: Full Hindley-Milner type inference engine
  - Algorithm W with unification and occurs-check for sound polymorphism
  - Type inference for `let` bindings, function literals, calls, conditionals, loops, arrays, hashes, index expressions, and cast expressions
  - Type annotations on `let` bindings (`let x: i64 = 5;`) enforced via unification
  - Typed function parameters (`fn(x: i64, y: i64) -> i64 { x + y }`)
  - Return type annotations validated against inferred body type
  - Generic functions (`fn identity<T>(x: T) -> T { x }`) with parametric polymorphism
  - Compile-time type errors for mismatches (e.g., `let x: i8 = 256;` is rejected)
  - Constant folding: `1 + 2` folds to `3` at compile time

#### Loops and Control Flow

- **`for` loops**: `for x in collection { body }` iteration over arrays
- **`while` loops**: `while condition { body }` conditional loops
- **`loop` loops**: `loop { body }` infinite loops (exit via `break`)
- **`break` statement**: Exit from any loop construct
- **`continue` statement**: Skip to the next loop iteration
- Full compilation to bytecode for all loop constructs

#### Typed Syntax Extensions

- **Arrow token (`->`)**: Return type annotation syntax for functions
- **`where` keyword**: Reserved for future trait bound clauses
- **Type expressions**: Parser support for type annotations (`i8`, `i16`, ..., `f64`, `usize`, `bool`, `str`, named types, generic types `T<U>`, array types `[T]`, function types `fn(T) -> U`)
- **Typed parameters**: Function parameters with optional type annotations (`x: i64`)
- **Generic parameters**: Type parameter lists on functions (`fn foo<T, U>(...)`)
- **Trait bounds**: Bound syntax on generic parameters (`T: Display + Clone`)

#### Testing & Examples

- New example programs exercising the type checker: typed functions, generics, type errors
- Existing example programs updated with type annotations
- `Display` implementation tests for all AST node types

### Changed

- **Lexer**: Added `Arrow` (`->`) and `Where` keyword tokens
- **AST**: Extended with `TypeExpr`, `TypedParam`, `GenericParam`, and `TraitBound` nodes; function literals carry optional generic parameters and return type annotations
- **Parser**: Refactored to parse type annotations, typed parameters, generic parameter lists, and trait bounds
- **Runtime `Object`**: Extended to support `for`, `while`, and `loop` evaluation
- **Evaluator**: Added evaluation logic for `for`, `while`, `loop`, `break`, and `continue`
- **Compiler**: Added bytecode compilation for `for`, `while`, and `loop` statements
- **Builtins**: Consolidated built-in function registry into a single location
- **Parser internals**: Removed numeric suffix strip macros (superseded by type system)
- **VM**: Eliminated non-determinism; `F32`/`F64` retained in standard execution mode for now

### Security

- Type checking catches type mismatches at compile time, preventing classes of runtime errors
- Occurs-check in unification prevents infinite types (soundness guarantee)
- No unsafe code

---

## [0.5.0] - 2026-02-23

CLI toolchain and language foundation release. Maat now supports file-based compilation and execution via `maat run`, `maat build`, and `maat exec`, with source-location error reporting, cast expressions, bytecode serialization, and shared instruction memory.

### Added

#### CLI Toolchain

- **`maat run <file.mt>`**: Compile and execute a Maat source file in a single step
- **`maat build <file.mt> -o <output.mtc>`**: Compile a source file to serialized bytecode
- **`maat exec <file.mtc>`**: Execute a pre-compiled bytecode file
- **`maat repl`**: Interactive REPL (formerly the standalone `repl` binary)
- File extension validation: `.mt` for source files, `.mtc` for compiled bytecode
- Shared compilation pipeline (`pipeline.rs`) used by all subcommands

#### Bytecode Serialization

- Binary serialization format using `serde` + `postcard`
- `MAAT` magic header (`4D 41 41 54`) and version byte for format identification
- Round-trip serialization of all constant pool object types (numeric literals, strings, compiled functions with `Rc<[u8]>` instructions)
- Source map included in serialized format for error reporting from pre-compiled bytecode
- Custom `Serialize`/`Deserialize` implementations for `Object` that reject non-serializable variants

#### Source-Location Error Reporting

- `Span` field on every AST node (all `Statement` and `Expression` variants)
- Source map in bytecode: `Vec<Span>` mapping instruction offsets to source positions
- Rich terminal diagnostics via [`ariadne`](https://docs.rs/ariadne) with colored output, source snippets, and underlined spans
- File:line:col context in all error messages when running from source files

#### Cast Expressions

- `as` operator for explicit numeric type conversion (`expression as type`)
- `CastExpr` AST node with support for all numeric types (`i8`, `i16`, ..., `f64`, `usize`)
- `OpConvert` opcode with source/target type tag operands
- Runtime rejection of lossy casts (e.g., `256i64 as u8` produces an error)

#### Testing & Examples

- 12 serialization round-trip integration tests
- 6 example programs: `fibonacci.mt`, `factorial.mt`, `closures.mt`, `macros.mt`, `map_reduce.mt`, `binary_search.mt`

### Changed

- **`CompiledFunction::instructions`**: `Vec<u8>` → `Rc<[u8]>` (closures from the same function literal share instruction memory)
- **`len()` built-in**: Returns `Object::Usize` instead of `Object::I64` (its natural type; use `len(x) as i64` for explicit conversion)
- **REPL**: Moved from standalone `repl` binary (`src/tools/maat-repl/`) into the main `maat` binary as the `maat repl` subcommand
- **`maat_eval`**: Reduced to macro-expansion engine; `eval()` and `eval_program()` removed as public execution APIs; only `define_macros()` and `expand_macros()` remain
- **Benchmarks**: Evaluator benchmarks removed; benchmark suite now covers VM-only execution paths
- **Opcode count**: 31 -> 32 (`OpConvert` added)

### Security

- `serde` + `postcard` for serialization (no arbitrary code execution during deserialization)
- Custom `Serialize`/`Deserialize` for `Object` rejects non-serializable variants at the type level

### Dependencies

- Added `serde` 1.x with `derive` and `rc` features
- Added `postcard` 1.x with `alloc` feature
- Added `clap` for CLI argument parsing
- Added `ariadne` 0.6 for rich error diagnostics

---

## [0.4.0] - 2026-02-19

Major feature release introducing a bytecode compiler and stack-based virtual machine, based on "Writing a Compiler in Go" by Thorsten Ball. Maat now compiles source code to bytecode and executes it on a VM, in addition to the existing tree-walking interpreter.

### Added

#### Bytecode Compiler

- **`maat_bytecode` crate**: Bytecode instruction encoding and decoding
  - 31 opcodes covering arithmetic, comparisons, control flow, data structures, functions, closures, and built-in dispatch
  - Big-endian encoding with variable-width operands (8-bit and 16-bit)
  - Human-readable instruction disassembly for debugging
  - Compile-time constants for stack, globals, and frame limits

- **`maat_codegen` crate**: AST-to-bytecode compilation
  - Recursive AST traversal with scope-aware code generation
  - Symbol table with lexical scoping (global, local, free, function, builtin)
  - Constant pool management with overflow protection (65,535 constant limit)
  - Free variable tracking and closure compilation
  - Named function support for recursive closures via `OpCurrentClosure`
  - Incremental compilation via `Compiler::with_state` for REPL sessions

- **`maat_vm` crate**: Stack-based virtual machine
  - Call frames with base pointer-relative local variable addressing
  - Closure execution with captured free variables
  - Built-in function dispatch via `OpGetBuiltin` + `OpCall`
  - Checked arithmetic to prevent integer overflow in all build profiles
  - Safe stack management with underflow/overflow guards using `checked_sub`
  - Global variable persistence for REPL sessions via `VM::with_globals`

#### New Opcodes

| Opcode                                         | Description                                            |
| ---------------------------------------------- | ------------------------------------------------------ |
| `Constant`                                     | Push constant from pool onto stack                     |
| `Pop`                                          | Discard top of stack                                   |
| `Add`, `Sub`, `Mul`, `Div`                     | Integer arithmetic and string concatenation            |
| `True`, `False`, `Null`                        | Push boolean/null literals                             |
| `Equal`, `NotEqual`, `GreaterThan`, `LessThan` | Comparison operators                                   |
| `Bang`, `Minus`                                | Unary operators                                        |
| `Jump`, `CondJump`                             | Unconditional and conditional branching                |
| `SetGlobal`, `GetGlobal`                       | Global variable storage and retrieval                  |
| `SetLocal`, `GetLocal`                         | Frame-relative local variable access                   |
| `Array`, `Hash`, `Index`                       | Data structure construction and indexing               |
| `Call`, `ReturnValue`, `Return`                | Function invocation and return                         |
| `GetBuiltin`                                   | Built-in function lookup by index                      |
| `Closure`                                      | Create closure from compiled function + free variables |
| `GetFree`                                      | Load captured free variable by index                   |
| `CurrentClosure`                               | Push current closure for recursive self-reference      |

#### Runtime Types

- **`CompiledFunction`**: Bytecode representation of user-defined functions
  - Instruction bytes, local variable count, and parameter count
- **`Closure`**: Runtime wrapper pairing a compiled function with captured free variables

#### Benchmarking Suite

- Criterion-based benchmark suite in `tests/benches/benchmarks.rs`
  - VM vs. tree-walking evaluator comparison at multiple Fibonacci depths
  - Compile-only pipeline overhead measurement
  - Pre-compiled VM execution (isolating dispatch overhead)
  - Feature-specific benchmarks: closures, array iteration, string operations
  - HTML report generation for visual performance analysis

### Changed

- **REPL**: Migrated from tree-walking evaluator to compiler+VM execution
  - Persistent session state across iterations (symbol table, constants pool, globals store)
  - Snapshot/rollback on compilation errors to preserve session integrity
  - Suppresses output for let-only statements and null values
  - Reports VM errors with descriptive messages

- **AST**: Added optional `name` field to `Function` for recursive closure support
  - Parser sets function name from `let` binding context

- **`maat_runtime`**: Extended object system with compiler-specific types
  - Added `CompiledFunction` and `Closure` variants to `Object`
  - Added `Builtin` variant for built-in function pointers
  - Centralized built-in function registry (`BUILTINS`) with name-to-index mapping

- **`maat_eval`**: Decoupled the object system from the tree-walking interpreter
  - Original `maat_eval` remains the evaluation engine
  - New `maat_runtime` implements the object system

- **`maat_driver`** orchestration layer completely removed
- **`maat_parser`**: Renamed from `maat_parse`

### Security

- Checked arithmetic throughout the VM (prevents integer overflow panics in all build profiles)
- Stack bounds checking with `checked_sub` to prevent underflow in release mode
- Frame depth limit (`MAX_FRAMES = 1024`) to prevent stack overflow from unbounded recursion
- No unsafe code

### Performance

- Bytecode VM provides significant speedup over tree-walking evaluation
- Pre-allocated stack and globals stores minimize allocation overhead
- Inline hints on hot-path VM methods (`current_frame`, `read_u16_operand`, `read_u8_operand`)
- String concatenation pre-allocates capacity to avoid reallocation

---

## [0.3.0] - 2026-02-07

### Changed - Major Architecture Restructuring

**Breaking Changes:**

- **New Project Structure**:
  - `compiler/` - Language implementation (9 crates: `maat_span`, `maat_errors`, `maat_lexer`, `maat_ast`, `maat_parse`, `maat_eval`, `maat_driver`, `maat`)
  - `src/tools/` - Development tools (`maat-repl`)

**Crate Organization:**

- **maat_span** - Source location tracking and span management
- **maat_errors** - Unified error handling with Result type alias
- **maat_lexer** - Tokenization and lexical analysis
- **maat_ast** - Abstract Syntax Tree definitions and transformations
- **maat_parse** - Pratt parser with operator precedence
- **maat_eval** - Tree-walking interpreter with integrated macro system
- **maat_driver** - Orchestration layer providing unified API
- **maat** - Compiler binary entry point
- **maat-repl** - Interactive REPL tool

---

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

[0.6.0]: https://github.com/maatlabs/maat/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/maatlabs/maat/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/maatlabs/maat/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/maatlabs/maat/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/maatlabs/maat/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/maatlabs/maat/releases/tag/v0.1.0
