# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.11.3] - 2026-04-04

Language surface completeness, extension of `Option<T>`, `Result<T, E>`, and `Vector<T>` method suite.

### Added

#### Closure Syntax and Character Conversions

- **Rust-style closure syntax (`|x| expr`):** `|params| expr`, `|params| { block }`, `|| expr`, optional return type and parameter type annotations. Desugars to the existing `Lambda` AST node -- no new opcodes
- **`char` <-> integer conversions via `as`:** `'a' as u32`, `65u32 as char`, and all integer widths. Out-of-range conversions produce runtime errors via `TryFrom` range validation
- **`format!()` macro:** Returns a `str` by compiling to `__to_string` calls with `OpAdd` string concatenation. Same `{}`/`{name}` format string support as `print!`/`println!`

#### Collection and Iteration Completeness

- **`for (k, v) in map`:** Direct `Map<K, V>` iteration with tuple destructuring in the loop header. Deterministic insertion-order iteration via `IndexMap`.
- **Extended `Vector<T>` higher-order methods:** `find`, `position`, `for_each`, `flat_map` -- compiler-desugared to inline loops with no runtime overhead, supporting chaining
- **Extended `Vector<T>` builtin methods:** `rev`, `count`, `take`, `skip`, `dedup`, `chain`, `contains`, `sum`, `product`, `min`, `max`, `enumerate`, `zip`, `windows`, `chunks` -- dispatched as runtime function calls
- **Polymorphic integer literals:** Unsuffixed integer literals (e.g., `2`) are now inference variables (`IntVar`) that unify with any integer type and default to `i64` after inference

#### Error Handling and Type Completeness

- **Extended `Option<T>` methods:** `unwrap_or_else(fn() -> T)`, `ok() -> Result<T, ()>`, `flatten() -> Option<T>`, `zip(Option<U>) -> Option<(T, U)>`. Higher-order methods are compiler-desugared; simple methods are runtime builtins
- **Extended `Result<T, E>` methods:** `map_err(fn(E) -> F) -> Result<T, F>`, `unwrap_err() -> E`, `unwrap_or_else(fn(E) -> T) -> T`, `ok() -> Option<T>`, `err() -> Option<E>`, `or_else(fn(E) -> Result<T, F>) -> Result<T, F>`. Higher-order methods are compiler-desugared; simple methods are runtime builtins
- **`Range<Integer>` type unification:** `Range` and `RangeInclusive` are now generic over all integer types (previously hardcoded to `i64`). Supports `0u8..255u8`, `0i32..=100i32`, typed `for..in` loops

---

## [0.11.2] - 2026-04-01

`crates.io` publication release. All 13 crates published to `crates.io`, enabling `cargo install maat` as the canonical install path.

### Changed

- **All `Cargo.toml` files**: Added `description`, `keywords`, `categories`, and `homepage` metadata required for `crates.io` publication
- **Internal path dependencies**: Added `version` alongside `path` in all internal dep entries so `crates.io` consumers resolve by version while local workspace builds continue using paths
- **Workspace version**: Bumped to `0.11.2`

---

## [0.11.1] - 2026-03-31

REPL bug fixes and public release. CLI diagnostics also improved for better error reporting. Minor internal module/crate reorganization.

### Changed

- **`maat_module`**: Consolidated API -- free functions converted to methods on `ModuleExports` and `ResolvedImport`; tightened module visibility
- **Parse error messages**: Replaced raw `ContextError { context: [], cause: None }` debug output with human-readable `unexpected token \`X\`` messages
- **VM error messages**: Fixed `vm error: vm error:` stutter in CLI error output for runtime errors

### Fixed

- REPL bug fixes from v0.11.0 regression

### Security

- Update the security policy and threat model document (`SECURITY.md`)
- Added copyright lines to MIT and Apache-2.0 license files
- Added `.gitignore` exclusions for profiling artifacts
- Added license declaration to fuzz crate `Cargo.toml`

## [0.11.0] - 2026-03-26

Major language ergonomics and compiler infrastructure release. Maat's compiler frontend has been rebuilt on industrial-strength foundations: `logos` for lexing and `winnow` for parsing. The language gains tuples, `char`, the `?` operator, `Map<K, V>`, higher-order collection methods, builtin macros (`println!`, `assert!`, `panic!`), and Rust-native control-flow syntax (no mandatory parentheses on `if`/`while`). The runtime value system has been overhauled to cleanly separate AST nodes from runtime values, `Null` has been replaced with typed `Unit`, and `Array` has been renamed to `Vector`. Implicit numeric promotion has been removed in favor of explicit conversions.

### Added

#### New Types

- **Tuples as first-class types**: `(i64, bool, str)` syntax in expressions, patterns, type annotations, and destructuring. Tuple field access via `.0`, `.1`, etc. Full type inference and unification support
- **`char` type and character literals**: `'a'`, `'\n'`, `'\\'` with escape sequence support. Methods: `is_alphabetic`, `is_numeric`, `to_string`. Type checker enforces `char` as a distinct type
- **`Map<K, V>` as a first-class collection type**: Literal syntax `{ "key": value }`, methods (`insert`, `get`, `remove`, `contains_key`, `keys`, `values`, `len`), and full type inference
- **`Set<T>` promoted to first-class type**: Bug fixes in stdlib design, full integration with the type system

#### Error Handling

- **Try (`?`) operator**: `expr?` propagates `Err` / `None` early, matching Rust semantics. Works with `Result<T, E>` and `Option<T>`. Compiled to conditional jump + unwrap sequences
- **Methods on `Result<T, E>` and `Option<T>`**: `is_ok`, `is_err`, `is_some`, `is_none`, `unwrap`, `unwrap_or`, `map`, `and_then`

#### Builtin Macros

- **`print!` and `println!`**: Format-string macros with `{}` interpolation, replacing the `print` free function
- **`assert!` and `assert_eq!`**: Runtime assertion macros with descriptive panic messages
- **`panic!`, `todo!`, `unimplemented!`**: Diagnostic macros for signaling unreachable or incomplete code paths

#### Standard Library Enhancements

- **Higher-order methods on `Vector<T>`**: `map`, `filter`, `fold`, `any`, `all` -- compiler-desugared with no runtime overhead, supports chaining
- **`T::default` and `T::from` support**: Static associated functions for type construction and conversion
- **`cmp` module**: `cmp::min`, `cmp::max`, `cmp::clamp` as polymorphic comparison functions over all integer types
- **Typed `str` parse methods**: `parse_i8`, `parse_i16`, `parse_i32`, `parse_i64`, `parse_i128`, `parse_u8`, `parse_u16`, `parse_u32`, `parse_u64`, `parse_u128`, `parse_usize` -- each returning `Result<T, ParseIntError>`

#### Syntax

- **Labelled loops**: `'label: loop { ... }`, `'label: for ... { ... }`, `'label: while ... { ... }` with `break 'label` and `continue 'label`
- **Struct update syntax**: `Point { x: 1, ..existing }` for creating modified copies
- **Documentation comments (`///`)**: Lexer, parser, and AST support for doc comments

#### Testing

- **Type checker unit tests**: Comprehensive test coverage for the Hindley-Milner inference engine
- **Module system unit tests**: Tests for module resolution, visibility enforcement, circular dependency detection, and cross-module type checking

### Changed

#### Compiler Infrastructure

- **Lexer rewritten with `logos`**: Hand-rolled lexer replaced with a compile-time DFA lexer, improving tokenization throughput and correctness
- **Parser rewritten with `winnow`**: Hand-rolled Pratt parser replaced with a combinator-based parser, reducing parser complexity and improving error recovery
- **`O(1)` enum variant lookup**: Replaced `O(n)` linear scan with direct index-based lookup in the compilation pipeline
- **REPL upgraded to `rustyline`**: Replaced raw `std::io::BufRead` loop with `rustyline::Editor` for line editing, history, and signal handling

#### Runtime & Type System

- **Runtime value system overhaul**: Clean separation of AST representation from runtime values; numeric type handling consolidated
- **AST/runtime disambiguation**: AST nodes and runtime values now occupy distinct type hierarchies
- **`Null` replaced with typed `Unit`**: Untyped `Null` sentinel eliminated in favor of the typed `Unit` value `()`, aligning with Rust semantics
- **Implicit numeric promotion removed**: The type checker no longer silently widens integers; explicit `as` casts required
- **`Array` renamed to `Vector`**: Standard library overhaul aligns collection naming with `Vec<T>` conventions; stdlib modules restructured

#### Performance

- **Zero-allocation `Substitution::apply`** on struct/enum types: Eliminated heap allocations in the hot path of type inference
- **Zero-allocation `Display` implementations**: `std::fmt::Display` impls rewritten to avoid intermediate `String` allocations
- **Reduced boilerplate in runtime integer ops**: Macro-based dispatch consolidated

#### Bug Fixes

- **Parser rejects reserved type names and keywords** used as identifiers
- **Compiler hygiene pass**: Dead code removal, unused import cleanup, warning elimination across all crates

---

## [0.10.0] - 2026-03-17

Security hardening and fuzz testing release. Maat's compiler and VM have been hardened against adversarial input. All arithmetic uses checked operations, all resource limits are enforced, and zero `unsafe` code exists. Five fuzz targets and nine property-based tests verify crash-freedom across millions of inputs. A comprehensive threat model documents trust boundaries and mitigations. The CI pipeline now gates fuzz testing, Miri, and code coverage.

### Added

#### Fuzz Testing

- **Five `cargo-fuzz` targets** covering the full compiler pipeline: `fuzz_lexer` (arbitrary bytes), `fuzz_parser` (arbitrary UTF-8), `fuzz_typechecker` (syntactically valid programs), `fuzz_compiler` (well-typed programs), `fuzz_deserializer` (arbitrary bytes as `.mtc`)
- **Seed corpus** derived from example programs, stdlib sources, and adversarial edge cases (deeply nested parentheses, huge integers, unterminated strings, malformed bytecode)
- Zero crashes across ~7.3M total fuzz runs

#### Property-Based Testing

- **Nine `proptest` tests** verifying invariants over thousands of randomly generated programs:
  - Lexer, parser, type checker, compiler, and deserializer never panic on arbitrary input
  - AST `Display` round-trip is idempotent (`parse -> Display -> parse` produces identical AST)
  - Bytecode serialization round-trips perfectly
  - Execution is deterministic (same program produces same result)
  - Well-typed programs never produce runtime type errors (type soundness)

#### Security Hardening

- **Parser nesting depth cap** (`MAX_NESTING_DEPTH = 256`): prevents stack overflow from deeply nested expressions
- **Bytecode deserialization limits**: `MAX_PAYLOAD_SIZE` (16 MiB), `MAX_CONSTANT_POOL_SIZE` (65535), `MAX_INSTRUCTION_COUNT` (1M) -- malicious `.mtc` files rejected before allocation
- **`#![forbid(unsafe_code)]`** enforced in all 13 workspace crates -- zero `unsafe` blocks
- **Safe numeric conversions**: `Object::from_number_literal()` returns `Result` via `TryFrom` instead of panicking. Enum variant field count uses `u8::try_from()` with error propagation
- **Module visibility enforcement**: `pub use` verified to enforce transitivity -- private items cannot leak through re-export chains

#### Threat Model

- **`SECURITY.md`**: Comprehensive threat model covering trust boundaries (source -> bytecode -> VM), three attacker models (malicious `.maat` source, malicious `.mtc` bytecode, resource exhaustion), mitigation tables with code locations, memory safety guarantees, arithmetic safety, and timing side-channel baseline

#### Benchmark Expansion

- **28 benchmarks across 7 groups**: method dispatch, range iteration (for vs. while), bitwise operators, compilation pipeline breakdown (lexer/parser/typechecker/codegen individually), serialization round-trip, and baseline overhead -- in addition to existing fibonacci, closures, arrays, strings, structs, enums, and option matching

#### CI Hardening

- **Fuzz testing CI** (`fuzz.yml`): 60s per target on PRs, 300s per target overnight on `main`
- **Coverage reporting** (`coverage.yml`): `cargo-llvm-cov` generates LCOV reports per PR
- **Windows tests**: CI matrix expanded to run tests (not just builds) on Windows with stable + nightly Rust

#### Prelude

- **`Some`, `None`, `Ok`, `Err` available unqualified** in expression position (construction) -- matching existing pattern-position behavior. Qualified `Option::Some` / `Result::Ok` syntax remains valid

### Changed

- **`ExprStmt::Display`** now emits trailing semicolons, fixing AST round-trip correctness (discovered by proptest)
- **Constant pool overflow check** fires before push (was off-by-one: 65536th constant was allocated before error)
- **Shift operations** use `checked_shl`/`checked_shr` with bit-width validation (was casting to `u32` without bounds check)
- **Enum variant tag limit** enforced at registration time (`MAX_ENUM_VARIANTS = 256`) with proper error (was silently truncating via `& 0xFF`)
- **`resolve_symbol().unwrap()` calls** in compiler replaced with proper `Result` propagation (was panicking on malformed input)
- **Redundant `find_variant_in_registry()` double lookups** consolidated to single lookup with stored result
- **Interpreter block scoping** fixed: `eval_block_statement`, `eval_for_statement`, `eval_while_statement` now wrap bodies in `Env::new_enclosed()` (variables were leaking out of blocks)
- **Conditional stack corruption** fixed: `Expr::Cond` compilation emits `Opcode::Null` when branches contain only statements
- **Numeric literal AST** consolidated from 24 variants into unified `Number { kind: NumberKind, value: i128 }`
- **`compile_expression()`** decomposed from ~260 lines into focused sub-functions
- **VM integer macros** unified and simplified
- **Example programs** replaced with 10 progressive, stress-testing programs
- **Test suite** overhauled: deceptive/trivial tests removed, critical edge cases added (overflow, underflow, division by zero, deep recursion, empty ranges, etc.)

### Security

- All integer arithmetic in the VM uses `checked_*` methods -- overflow, underflow, division by zero, and out-of-range shifts produce runtime errors, never silent wrapping
- `as` casts go through `TryFrom` with range validation -- out-of-range conversions produce errors, not silent truncation
- Constant folding uses identical checked arithmetic
- `#![forbid(unsafe_code)]` in all 13 crates -- memory safety guaranteed by the Rust type system
- `pub use` re-exports enforce visibility transitivity

---

## [0.9.0] - 2026-03-15

Standard library, methods, and iterators release. Maat now features Rust-native method syntax on built-in types, a standard library importable via the module system, full language surface completeness (comments, bitwise operators, compound assignment, mutable bindings, forward references), compile-time type-directed method dispatch, and first-class range syntax with `for..in` integration.

### Added

#### Method Syntax for Built-in Types

- **`impl [T]`**: `arr.len()`, `arr.first()`, `arr.last()`, `arr.rest()`, `arr.push(x)`, `arr.join(sep)`
- **`impl str`**: `s.len()`, `s.trim()`, `s.contains(sub)`, `s.starts_with(pre)`, `s.ends_with(suf)`, `s.split(delim)`, `s.parse_int()`
- `print` remains a free function
- Old free-function forms (`len(arr)`, `first(arr)`, etc.) removed

#### Standard Library Modules

- **`std::math`**: `abs`, `min`, `max`, `pow`, `gcd`, `lcm` -- implemented as Maat source files (`.maat`), importable via `use std::math::abs;`
- **`std::string`**: `split`, `join`, `trim`, `contains`, `starts_with`, `ends_with`, `parse_int`
- **`std::collections`**: `Set` backed by `IndexMap` with `insert`, `contains`, `remove`, `len`, `to_array` methods
- Compiler resolves `std::` imports by searching a built-in stdlib path in addition to the project directory

#### Language Surface Completeness

- **Line comments (`//`) and block comments (`/* */`)** in the lexer
- **Modulo operator (`%`)**: Lexer token, parser infix rule, `OpMod` opcode, VM execution with Euclidean semantics (`checked_rem_euclid`) for cryptographic correctness
- **`else if` chains**: `else if (cond) { ... }` without requiring `else { if ... }` nesting
- **Bitwise operators (`&`, `|`, `^`, `<<`, `>>`)**: Full pipeline from lexer to VM with `OpBitAnd`, `OpBitOr`, `OpBitXor`, `OpShl`, `OpShr` opcodes
- **Compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`)**: Desugared to `x = x op rhs` in the parser
- **Mutable bindings (`let mut`)**: `let mut x = 0; x = x + 1;` with compile-time `ImmutableAssignment` error for non-`mut` bindings. Plain `let` rebinding inside loops is now block-scoped
- **Forward function references**: Two-pass compilation collects all function signatures in pass 1, compiles bodies in pass 2 -- enables top-down code organization and mutual recursion
- **Block scoping**: `let` bindings inside `if`/`while`/`for`/`loop` blocks are scoped to the block

#### Range Syntax & `for..in` Integration

- **Range expressions**: `start..end` (half-open) and `start..=end` (inclusive) as first-class `Range`/`RangeInclusive` runtime values
- **`MakeRange` / `MakeRangeInclusive` opcodes**: Pop two i64 values from the stack, push a range object
- **`Type::Range(Box<Type>)`**: Full type inference and unification support; type checker validates that range bounds are integer types
- **`for i in 0..10 { ... }`**: Desugars to an efficient counter-based loop (no heap allocation), with `break`/`continue` support
- **`for..in` on arrays**: Retained as index-based desugaring via `Array::len` + `Index`

#### New Runtime Objects

- **`Object::Range(i64, i64)`** and **`Object::RangeInclusive(i64, i64)`**: First-class range values with `Display`, `PartialEq`, `Serialize`/`Deserialize`

#### New Error Types

- `CompileErrorKind::ImmutableAssignment { name }` for assignment to non-`mut` bindings

### Changed

- **Source file extension**: Changed from `.mt` to `maat` to avoid "conflict" with Wolfram Mathematica (`.mt`) and MT Library for C++ (`.mt`) data files
- **Opcode count**: 35 -> 43 (`Mod`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `MakeRange`, `MakeRangeInclusive`)
- **Builtin functions**: Multi-dispatch `builtin_len` and `str_contains` split into single-type functions (`array_len`, `str_len`, `str_contains` str-only)
- **For-loop compilation**: `Stmt::For` now branches on `Expr::Range` (counter-based) vs array (index-based) desugaring
- **Block scoping**: All blocks (`if`, `while`, `for`, `loop`) use `push_block_scope()`/`pop_block_scope()` for proper lexical scoping
- **AST**: Added `Expr::Range(RangeExpr)`, `LetStmt::mutable`, `Stmt::ReAssign(ReAssignStmt)`. `MethodCallExpr` gained `receiver: Option<String>` field
- **Lexer**: Added `DotDot` (`..`), `DotDotEqual` (`..=`), `Mut` keyword; compound assignment operators (`+=`, `-=`, `*=`, `/=`, `%=`)
- **Parser**: Added range expression parsing at `RANGE` precedence level; compound assignment desugaring; `else if` chain parsing; `let mut` binding parsing
- **Type system**: Added `Type::Range(Box<Type>)` with unification, occurs-check, and free-var collection
- **Evaluator**: Added `Expr::Range` evaluation producing `Object::Range`/`Object::RangeInclusive`

### Removed

- **`examples/macros.mt`**: Removed (macro expansion belongs in the eval pipeline, not the compile pipeline)

### Security

- Euclidean modulo (`checked_rem_euclid`) ensures correct results for negative operands -- critical for cryptographic algorithms
- Mutable binding enforcement prevents accidental mutation of immutable variables
- Block scoping prevents variable leakage across scope boundaries
- No unsafe code

---

## [0.8.0] - 2026-03-10

Module system release. Maat now supports multi-file programs with `mod`, `use`, and `pub`. Modules are resolved from the filesystem, organized into a dependency graph, type-checked independently with cross-module visibility enforcement, and compiled into a single linked bytecode via a shared compiler. No separate linking pass is required.

### Added

#### Module System

- **`mod` declarations**: `mod foo;` resolves to `foo.mt` or `foo/mod.mt` relative to the declaring file. `mod foo { ... }` for inline modules
- **`use` imports**: `use foo::bar;` for single items, `use foo::{bar, baz};` for grouped imports. No glob imports (`use foo::*`) -- explicit imports only for ZK auditability
- **`pub` visibility**: `pub fn`, `pub struct`, `pub enum`, `pub trait`, and `pub` on struct fields and `impl` methods. Items without `pub` are module-private
- **`pub use` re-exports**: `pub use foo::bar;` forwards items through intermediate modules
- **Nested modules**: `mod outer;` where `outer.mt` contains `mod inner;` resolving to `outer/inner.mt`
- **Cycle detection**: DFS with gray/black coloring detects circular module dependencies at resolution time
- **Diamond dependencies**: Multiple modules depending on the same leaf module compile correctly

#### New Crate: `maat_module`

- `ModuleId` unique identifier per module (`ROOT = 0`)
- `ModuleGraph` directed acyclic graph with topological ordering (leaves-first compilation)
- `resolve_module_graph(entry: &Path)` recursively parses all reachable files and builds the DAG
- `check_and_compile(graph: &mut ModuleGraph)` orchestrates per-module type checking and shared-compiler compilation
- `ModuleExports` tracks public bindings, structs, enums, traits, and impl blocks per module

#### Symbol Masking

- `SymbolsTable::mask_symbol` / `global_symbol_names` for cross-module visibility enforcement in the shared compiler
- After compiling each non-root module, all newly-defined globals are masked. Downstream modules see only explicitly imported symbols via `define_symbol` unmasking

#### New Error Types

- `ModuleError` with `ModuleErrorKind` variants: `FileNotFound`, `CyclicDependency`, `DuplicateModule`, `ParseErrors`, `TypeErrors`, `CompileErrors`, `Io`
- `TypeErrorKind::PrivateAccess { item, module }` for cross-module visibility violations

#### Testing & Examples

- Integration tests in `tests/tests/modules.rs` covering resolution, cycle detection, exports, visibility, diamond dependencies, re-exports, nested modules, struct/enum imports, and serialization round-trips
- New `examples/modules/` multi-module example demonstrating `mod`/`use`/`pub` with `main.mt`, `geometry.mt`, and `math.mt`

### Changed

- **CLI pipeline**: `maat run` and `maat build` now use the multi-module pipeline (`resolve_module_graph` + `check_and_compile`). Single-file programs work unchanged as a single-node module graph
- **`maat_codegen`**: `compile_program()` made public. Added `symbols_table_mut()` and `type_registry_mut()` accessors for cross-module import injection. `Stmt::Use` and `Stmt::Mod` compile as no-ops
- **`maat_types`**: Added `env()`, `env_mut()`, `check_program_mut()`, `errors()`, `into_errors()` to `TypeChecker`. Added `all_structs()`, `all_traits()`, `all_impls()`, `global_bindings()` to `TypeEnv`. `Stmt::Use` and `Stmt::Mod` type-check as no-ops
- **`maat_ast`**: Added `Stmt::Use(UseStmt)` and `Stmt::Mod(ModStmt)`. Added `is_public: bool` to `FuncDef`, `StructDecl`, `EnumDecl`, `TraitDecl`. Added `is_public` to `UseStmt` for re-exports. Display, transform, and fold updated for new variants
- **Lexer**: Added `Mod`, `Use`, `Pub` keywords
- **Parser**: `pub` prefix modifier delegates to item parsers with `is_public: true`. `parse_use_stmt()` handles simple and grouped imports. `parse_mod_stmt()` handles external and inline modules
- **Evaluator**: `Stmt::Use` and `Stmt::Mod` are no-ops (evaluator is macro-expansion only)
- **Diagnostics**: Added `report_module_error()` for rendering module-level errors
- **AST rename**: `FnItem` renamed to `FuncDef` across the codebase
- **Crate table**: `maat_module` added to workspace members

### Security

- Explicit imports only (no glob `*`) ensures all cross-module dependencies are auditable
- Private-by-default visibility prevents accidental exposure of internal implementation details
- Cycle detection prevents infinite loops in module resolution
- Symbol masking enforces module boundaries at the compiler level, not just the type checker
- No unsafe code

---

## [0.7.0] - 2026-03-06

Custom types release. Maat now supports user-defined structs, enums, traits, impl blocks, and pattern matching with Rust-native syntax. Floating-point types have been removed as they are incompatible with finite-field arithmetic. `Option<T>` and `Result<T, E>` are pre-registered as language-level enums.

### Added

#### Custom Types

- **Structs**: `struct Point { x: i64, y: i64 }` with named fields, generics (`struct Wrapper<T> { inner: T }`), and nested structs
- **Enums**: `enum Shape { Circle(i64), Rect(i64, i64) }` with unit, tuple, and struct variants
- **Traits**: `trait Describable { fn describe(self) -> i64; }` with required and default method signatures
- **Impl blocks**: Inherent (`impl Point { ... }`) and trait (`impl Describable for Point { ... }`) implementations
- **Pattern matching**: `match expr { Pattern => body, ... }` with literal, identifier, tuple-struct, wildcard (`_`), and or-patterns
- **Field access**: `point.x` dot syntax for struct fields
- **Method calls**: `point.sum()` with automatic `self` passing; static methods via `Type::method()` path syntax
- **`Option<T>`** (`Some(T)` / `None`) and **`Result<T, E>`** (`Ok(T)` / `Err(E)`) pre-registered as built-in enums--no user declaration required
- Duplicate declarations of `Option` or `Result` rejected at compile time

#### New Opcodes

| Opcode      | Description                                                                 |
| ----------- | --------------------------------------------------------------------------- |
| `Construct` | Build a struct or enum variant from stack values (type index + field count) |
| `GetField`  | Extract a field from a struct or enum variant by index                      |
| `MatchTag`  | Compare enum variant tag and conditionally jump (peek, no pop)              |

#### New Runtime Objects

- **`Object::Struct`**: Type index + ordered field vector
- **`Object::EnumVariant`**: Type index + variant tag + field vector
- Full serialization support for both via `postcard`

#### Type System

- Two-pass type checking: pass 1 registers all type declarations (enabling forward references), pass 2 checks all expressions and statements
- Struct literal field validation (unknown fields, missing fields, type mismatches)
- Enum variant constructor resolution (`Shape::Circle(5)` as tuple constructor, `Color::Red` as unit value)
- Method resolution: inherent impls first, then trait impls (static dispatch)
- Trait satisfaction checking: all required methods must be implemented with correct signatures
- `TypeScheme`-based let-polymorphism: `let id = fn(x) { x }; id(5); id(true);` now works correctly
- Polymorphic function instantiation per call site

#### Testing & Examples

- Integration tests in `tests/tests/custom_types.rs` covering structs, enums, methods, traits, generics, `Option`, `Result`, chained matching, and serialization round-trips
- New `examples/custom_types.mt` showcasing structs, enums, methods, `Option`, and `Result`
- Benchmarks for struct construction + methods, enum matching, and `Option` matching

### Changed

- **Opcode count**: 32 -> 35 (`Construct`, `GetField`, `MatchTag`)
- **Bytecode**: Added `type_registry: Vec<TypeDef>` field for struct/enum metadata, included in serialization
- **AST**: Added `Stmt::StructDecl`, `Stmt::EnumDecl`, `Stmt::TraitDecl`, `Stmt::ImplBlock`, `Expr::Match`, `Expr::FieldAccess`, `Expr::MethodCall`, `Expr::StructLit`, `Expr::PathExpr`, and `Pattern` enum
- **Lexer**: Added keywords (`struct`, `enum`, `match`, `impl`, `trait`, `self`, `Self`) and tokens (`=>`, `::`, `.`)
- **Parser**: Struct literal disambiguation (uppercase-first + `{` = struct, lowercase = hash map); path expression parsing for `Enum::Variant` and `Type::method`
- **Compiler**: Type registry shared between compiler and VM; methods compiled as `Type::method` qualified bindings; `match` compiled as `MatchTag`/`CondJump` chains with back-patching
- **Evaluator**: `StructLit`, `PathExpr`, `Match`, `FieldAccess`, `MethodCall` added to unsupported-in-tree-walker error group
- **Constant folding**: Moved from `maat_types` to `maat_ast` crate; updated to treat type declarations as leaves
- **CLI**: Consolidated error handling into `diagnostic.rs`; merged `pipeline.rs` into `cmd.rs`

### Removed

- **`F32`/`F64` floating-point types**: Removed from lexer, parser, AST, type system, codegen, VM, and runtime. Floats are fundamentally incompatible with finite-field arithmetic--any source using floats is now a compile-time error.
- **`Expr::FnItem` conflation**: Split into `Stmt::FnItem` (named function declaration) and `Expr::Lambda` (anonymous closure expression).
- **Truthy condition semantics**: `if`/`while` conditions must be `Type::Bool`--no implicit truthiness from integers, strings, or null.

### Security

- Float removal eliminates a class of non-determinism incompatible with ZK proofs
- Explicit boolean conditions prevent implicit type coercion in control flow
- Struct field access validated at compile time (no runtime field lookup by name)
- No unsafe code

---

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

[0.11.1]: https://github.com/maatlabs/maat/compare/v0.11.0...v0.11.1
[0.11.0]: https://github.com/maatlabs/maat/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/maatlabs/maat/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/maatlabs/maat/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/maatlabs/maat/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/maatlabs/maat/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/maatlabs/maat/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/maatlabs/maat/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/maatlabs/maat/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/maatlabs/maat/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/maatlabs/maat/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/maatlabs/maat/releases/tag/v0.1.0
