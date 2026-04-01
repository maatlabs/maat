# Security Policy & Threat Model

This document describes the trust boundaries, attacker model, and mitigations for the Maat compiler toolchain. It covers the current state as of v0.11.1 and will be updated as subsequent versions introduce new attack surfaces.

## Trust Boundaries

The Maat toolchain has three distinct trust boundaries:

```text
Source (.maat) --> Compiler Pipeline --> Bytecode (.mtc) --> VM Execution
     │                   │                     │                  │
  Untrusted          Trusted               Untrusted           Trusted
  (user input)       (our code)            (file on disk)      (our code)
```

1. **Source boundary:** `.maat` source files are untrusted input. The lexer, parser, type checker, and compiler must handle arbitrary, malformed, or adversarial source without panicking or consuming unbounded resources.

2. **Bytecode boundary:** `.mtc` files are untrusted input. A user may hand-craft or corrupt a bytecode file to exploit the deserializer or VM. The deserializer must reject malformed files before allocating resources, and the VM must validate all operands during execution.

3. **VM execution boundary:** Even well-formed bytecode may attempt resource exhaustion (infinite loops, deep recursion, stack overflow). The VM enforces runtime limits.

## Attacker Model

### Attacker 1: Malicious `.maat` Source

**Goal:** Crash the compiler, exhaust memory/stack, or cause undefined behavior (UB) by submitting crafted source code.

**Mitigations:**

| Attack vector                      | Mitigation                                                                | Location                     |
| ---------------------------------- | ------------------------------------------------------------------------- | ---------------------------- |
| Deeply nested expressions          | Parser nesting depth cap (`MAX_NESTING_DEPTH = 256`)                      | `maat_parser/src/lib.rs`     |
| Extremely long programs            | Constant pool size limit (`MAX_CONSTANT_POOL_SIZE = 65535`)               | `maat_bytecode/src/lib.rs`   |
| Integer overflow in literals       | Type checker validates literal range via `check_literal_range()`          | `maat_types/src/lib.rs`      |
| Integer overflow in arithmetic     | VM uses `checked_add/sub/mul/div/rem/neg/shl/shr` for all operations      | `maat_vm/src/lib.rs`         |
| Narrowing `as` casts               | VM uses `TryFrom` with range-check errors via `Integer::cast_to()`        | `maat_runtime/src/num.rs`    |
| Literal-to-object truncation       | `from_number_literal()` returns `Result` via `TryFrom` (defense-in-depth) | `maat_runtime/src/lib.rs`    |
| Stack overflow via recursion       | VM frame stack limit (`MAX_FRAMES = 1024`)                                | `maat_bytecode/src/lib.rs`   |
| Stack exhaustion                   | VM stack size limit (`MAX_STACK_SIZE = 2048`)                             | `maat_bytecode/src/lib.rs`   |
| Enum with >256 variants            | Rejected at compile time (`MAX_ENUM_VARIANTS = 256`)                      | `maat_bytecode/src/lib.rs`   |
| Circular module imports            | Detected and rejected by module resolver                                  | `maat_module/src/resolve.rs` |
| Private item leakage via `pub use` | `pub use` only re-exports items already accessible to the module          | `maat_module/src/lib.rs`     |

### Attacker 2: Malicious `.mtc` Bytecode

**Goal:** Exploit the deserializer to allocate excessive memory, crash the VM, or execute unintended operations via hand-crafted bytecode.

**Mitigations:**

| Attack vector          | Mitigation                                                          | Location                         |
| ---------------------- | ------------------------------------------------------------------- | -------------------------------- |
| Oversized payload      | Payload size cap (`MAX_PAYLOAD_SIZE = 16 MiB`)                      | `maat_bytecode/src/serialize.rs` |
| Excessive constants    | Constant pool count validated post-deserialization                  | `maat_bytecode/src/serialize.rs` |
| Excessive instructions | Instruction stream count validated post-deserialization (1M limit)  | `maat_bytecode/src/serialize.rs` |
| Invalid magic/version  | Header validation before any payload processing                     | `maat_bytecode/src/serialize.rs` |
| Truncated payload      | `postcard` returns decode errors on truncated data                  | `maat_bytecode/src/serialize.rs` |
| Invalid opcodes        | VM rejects unknown opcode bytes at execution time                   | `maat_vm/src/lib.rs`             |
| Out-of-bounds operands | VM validates all constant/global/local indices before access        | `maat_vm/src/lib.rs`             |
| Type confusion         | VM validates operand types for all arithmetic/comparison operations | `maat_vm/src/lib.rs`             |

### Attacker 3: Resource Exhaustion

**Goal:** Cause the compiler or VM to consume unbounded CPU time or memory.

**Mitigations:**

| Resource                  | Limit        | Enforcement                   |
| ------------------------- | ------------ | ----------------------------- |
| Parser recursion depth    | 256 levels   | `maat_parser` nesting counter |
| VM stack                  | 2048 entries | `push_stack()` check          |
| VM call frames            | 1024 frames  | `push_frame()` check          |
| Global variables          | 65535        | `MAX_GLOBALS` constant        |
| Local variables per scope | 255          | `MAX_LOCALS` constant         |
| Constant pool entries     | 65535        | `add_constant()` check        |
| Bytecode payload          | 16 MiB       | `deserialize()` pre-check     |
| Instruction stream        | 1M bytes     | `deserialize()` post-check    |

**Not yet mitigated:**

- **Infinite loops:** The VM does not impose an instruction execution limit. Programs with `loop {}` will run indefinitely. This is intentional for v0.11.1 —- execution limits will be enforced by the ZK trace-generating VM in subsequent versions, where every instruction has a provable cost.
- **Algorithmic complexity attacks:** Hash map key collisions are not defended against (Rust's `IndexMap` uses default hashing). This is acceptable for the current single-user execution model.

## Memory Safety

All 14 crates in the workspace enforce `#![forbid(unsafe_code)]`. The compiler toolchain contains zero `unsafe` blocks. Memory safety is guaranteed by the Rust type system and borrow checker.

## Arithmetic Safety

All integer arithmetic in the VM uses Rust's `checked_*` methods. Overflow, underflow, division by zero, and out-of-range shifts produce runtime errors with diagnostic messages —- never silent wrapping or undefined behavior.

The constant folding pass (`maat_ast/src/fold.rs`) uses identical checked arithmetic and validates that folded results fit within the target type's range.

Type conversions (`as` casts in Maat source) go through `TryFrom` with range validation. Out-of-range conversions produce runtime errors, not silent truncation.

## Timing Side-Channel Baseline

The current VM executes all arithmetic operations using Rust's native integer instructions, which are constant-time for fixed-width types on modern hardware. However:

- **Comparison operations** use short-circuit evaluation for `&&`/`||`, which is timing-variable.
- **String operations** have length-dependent timing.
- **Hash lookups** have timing that depends on key distribution.

These are acceptable for the current architecture. When `Felt` (field element) types are introduced in future, all field arithmetic will use constant-time implementations to prevent timing side-channels in proof generation.

## Fuzz Testing

All five compiler pipeline stages have been fuzz-tested with `cargo-fuzz` (libFuzzer):

- `fuzz_lexer` —- arbitrary bytes --> tokenization
- `fuzz_parser` —- arbitrary UTF-8 --> parsing
- `fuzz_typechecker` —- syntactically valid programs --> type checking
- `fuzz_compiler` —- well-typed programs --> compilation
- `fuzz_deserializer` —- arbitrary bytes --> bytecode deserialization

Results: zero crashes across ~10.7M total fuzz runs (60s per target). See `fuzz/` for targets, corpus, and instructions.

## Property-Based Testing

Property tests (`proptest`) verify invariants across thousands of randomly generated programs:

- Lexer, parser, type checker, compiler, and deserializer never panic on arbitrary input
- AST Display round-trip is idempotent
- Bytecode serialization round-trips perfectly
- Execution is deterministic (same program --> same result)
- Well-typed programs never produce runtime type errors

See `tests/tests/properties.rs` for the full test suite.

## Reporting Vulnerabilities

If you discover a security vulnerability, please report it via [GitHub Security Advisories](https://github.com/maatlabs/maat/security/advisories/new). For non-critical issues, you may also open a [GitHub Issue](https://github.com/maatlabs/maat/issues) with the `security` label.
