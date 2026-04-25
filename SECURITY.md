# Security Policy & Threat Model

This document describes the trust boundaries, attacker model, and mitigations for the Maat compiler toolchain. It covers the current state as of v0.13.0 and will be updated as subsequent versions introduce new attack surfaces.

## Trust Boundaries

The Maat toolchain has four distinct trust boundaries:

```text
Source (.maat) --> Compiler Pipeline --> Bytecode (.mtc) --> VM Execution --> STARK Proof (.proof.bin)
     │                   │                     │                  │                       │
  Untrusted          Trusted               Untrusted           Trusted                Untrusted
  (user input)       (our code)            (file on disk)      (our code)             (potentially adversarial)
```

1. **Source boundary:** `.maat` source files are untrusted input. The lexer, parser, type checker, and compiler must handle arbitrary, malformed, or adversarial source without panicking or consuming unbounded resources.

2. **Bytecode boundary:** `.mtc` files are untrusted input. A user may hand-craft or corrupt a bytecode file to exploit the deserializer or VM. The deserializer must reject malformed files before allocating resources, and the VM must validate all operands during execution.

3. **VM execution boundary:** Even well-formed bytecode may attempt resource exhaustion (infinite loops, deep recursion, stack overflow). The VM enforces runtime limits.

4. **Proof boundary:** `.proof.bin` files arrive over an untrusted channel. The verifier must be sound against adversarially crafted proofs and against valid-looking proofs that disagree with the claimed program hash, public inputs, or output. Soundness rests on the AIR constraint system, the per-trace tight degree declarations shipped through `TraceInfo::meta`, and Winterfell's FRI low-degree test.

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
| Field element division by zero     | `Felt::div` returns `Err(FieldError)` on zero divisor                     | `maat_field/src/lib.rs`      |
| Out-of-bounds array access         | VM validates index against array length at runtime                        | `maat_vm/src/lib.rs`         |
| Narrowing `as` casts               | VM uses `TryFrom` with range-check errors via `Integer::cast_to()`        | `maat_runtime/src/num.rs`    |
| Literal-to-object truncation       | `from_number_literal()` returns `Result` via `TryFrom` (defense-in-depth) | `maat_runtime/src/lib.rs`    |
| Stack overflow via recursion       | VM frame stack limit (`MAX_FRAMES = 1024`)                                | `maat_bytecode/src/lib.rs`   |
| Stack exhaustion                   | VM stack size limit (`MAX_STACK_SIZE = 2048`)                             | `maat_bytecode/src/lib.rs`   |
| Enum with >256 variants            | Rejected at compile time (`MAX_ENUM_VARIANTS = 256`)                      | `maat_bytecode/src/lib.rs`   |
| Unbounded loops                    | `while`/`loop` without `#[bounded(N)]` annotation rejected at parse time  | `maat_parser/src/lib.rs`     |
| Loop bound exceeded                | Counter-guarded desugaring halts with `BoundExceeded` at runtime          | `maat_codegen/src/lib.rs`    |
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

### Attacker 3: Malicious `.proof.bin` STARK Proof

**Goal:** Convince the verifier to accept a proof for a program execution that did not actually happen, or for a program/output the verifier did not request.

**Mitigations:**

| Attack vector                             | Mitigation                                                                                                                                                                             | Location                                               |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| Wrong-program substitution                | 32-byte Blake3 program hash embedded in proof header; verifier requires `compute_program_hash(bytecode) == header.program_hash`                                                        | `maat_prover/src/program_hash.rs`                      |
| Truncated or short proof                  | 38-byte header parse rejects on insufficient bytes; magic `b"MATP"` and version `u16` validated before payload decode                                                                  | `maat_prover/src/proof_file.rs`                        |
| Wrong magic / version drift               | Magic check + version match required before payload deserialization                                                                                                                    | `maat_prover/src/proof_file.rs`                        |
| Tampered execution trace                  | Per-row transition constraints (debug builds also panic at the prover) + boundary assertions; tampered output rows fail FRI                                                            | `maat_air/src/main_segment.rs`                         |
| Tampered memory permutation               | Auxiliary segment grand-product accumulator must telescope to 1 at the last row; address-continuity constraint over sorted pairs                                                       | `maat_air/src/aux_segment.rs`                          |
| Physical-address gap injection            | `run_trace` post-execution `validate_address_contiguity` returns `Err(VmError)`; debug `build_aux_columns` asserts contiguity                                                          | `maat_trace/src/lib.rs`, `maat_air/src/aux_segment.rs` |
| Out-of-range integer witness              | 16-bit limb decomposition + sorted-limb permutation argument force every range-checked value into `[0, 2^64)`                                                                          | `maat_air/src/aux_segment.rs`                          |
| Division-by-zero witness                  | `sel_div_mod * (s0 * nonzero_inv - 1) = 0` makes the prover commit to the divisor's modular inverse                                                                                    | `maat_air/src/main_segment.rs`                         |
| Falsified arithmetic output               | Per-opcode sub-selectors gate output-correctness constraints (`add`/`sub`/`mul`/`div`/`mod`/`neg`/`not`/`felt_add`/`...`/`eq`/`neq`)                                                   | `maat_air/src/main_segment.rs`                         |
| Adversarial constraint-degree declaration | Verifier reconstructs the per-constraint tight degree array from the same bytes the prover shipped via `TraceInfo::meta`; an under-declared constraint makes FRI reject                | `maat_air/src/degree.rs`                               |
| Calling-convention forgery                | Synthetic `SEL_NOP` parameter rows + saved-FP write/read pair the callee's `GetLocal` reads with provable writes through the memory permutation argument                               | `maat_trace/src/vm.rs`                                 |
| Forged public output                      | `out[last] = public_output` boundary assertion is bound to the proof header; the verifier checks both the proof and the embedded inputs                                                | `maat_air/src/lib.rs`                                  |
| Cross-proof replay                        | Public inputs (program hash, input values, output) are bound into the proof via boundary assertions; copying a proof to a new program changes the program hash and fails the assertion | `maat_prover/src/proof_file.rs`                        |

**Soundness scope (v0.13.0):** the proof binds a primitive-typed program execution -- integers, `bool`, `Felt`, and user-defined functions over those types. Programs that exercise composite types (`Vector<T>`, `Map<K,V>`, `Set<T>`, `str`, `struct`, `enum`, fixed-size arrays `[T; N]`, closures) execute correctly under `maat run` but are not yet trace-VM-complete. `maat prove` will emit a proof, but the verifier rejects it because the trace omits constraint-satisfying rows for those operations. This is a completeness gap, not a soundness gap -- a verifier never accepts a tampered proof.

**Bitwise and ordering output gaps:** `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `<`, `>` retain selector-level structural constraints in v0.13.0 but do not yet have per-row output constraints. A malicious prover can in principle commit a bitwise/ordering result that disagrees with the operands while still satisfying the structural constraints. Closing this gap is scoped for a future release (bit-decomposition witness columns and a comparison range argument).

### Attacker 4: Resource Exhaustion

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
| Loop iterations           | `N` per loop | `#[bounded(N)]` annotation    |
| Bytecode payload          | 16 MiB       | `deserialize()` pre-check     |
| Instruction stream        | 1M bytes     | `deserialize()` post-check    |

**Not yet mitigated:**

- **Algorithmic complexity attacks:** Hash map key collisions are not defended against (Rust's `IndexMap` uses default hashing). This is acceptable for the current single-user execution model.

## Memory Safety

All 17 crates in the workspace enforce `#![forbid(unsafe_code)]`. The compiler toolchain contains zero `unsafe` blocks. Memory safety is guaranteed by the Rust type system and borrow checker.

## Arithmetic Safety

All integer arithmetic in the VM uses Rust's `checked_*` methods. Overflow, underflow, division by zero, and out-of-range shifts produce runtime errors with diagnostic messages —- never silent wrapping or undefined behavior.

The constant folding pass (`maat_ast/src/fold.rs`) uses identical checked arithmetic and validates that folded results fit within the target type's range.

Type conversions (`as` casts in Maat source) go through `TryFrom` with range validation. Out-of-range conversions produce runtime errors, not silent truncation.

## Timing Side-Channel Baseline

The current VM executes all arithmetic operations using Rust's native integer instructions, which are constant-time for fixed-width types on modern hardware. However:

- **Comparison operations** use short-circuit evaluation for `&&`/`||`, which is timing-variable.
- **String operations** have length-dependent timing.
- **Hash lookups** have timing that depends on key distribution.

These are acceptable for the current architecture. `Felt` (field element) arithmetic delegates to Winterfell's `BaseElement` implementation, which uses constant-time field operations over the Goldilocks prime (`p = 2^64 - 2^32 + 1`). This prevents timing side-channels in proof generation for all field-element computations. The ZK constraint evaluation in `maat_air` operates over the same constant-time field.

## Fuzz Testing

All five compiler pipeline stages have been fuzz-tested with `cargo-fuzz` (libFuzzer):

- `fuzz_lexer` —- arbitrary bytes --> tokenization
- `fuzz_parser` —- arbitrary UTF-8 --> parsing
- `fuzz_typechecker` —- syntactically valid programs --> type checking
- `fuzz_compiler` —- well-typed programs --> compilation
- `fuzz_deserializer` —- arbitrary bytes --> bytecode deserialization

Results: zero crashes across ~10.7M total fuzz runs (60s per target). See `fuzz/` for targets, corpus, and instructions.

The proof-system surface introduced in v0.12.x and extended in v0.13.0 is **not yet covered by fuzzing.** Planned targets include `fuzz_proof_deserializer` (arbitrary bytes through `deserialize_proof`), `fuzz_verifier` (well-formed bytecode + adversarial proof bytes through `verify`), and `fuzz_trace_vm` (well-typed primitive-only programs through `run_trace` then `prove` + `verify`).

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
