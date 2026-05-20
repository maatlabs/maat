# Security Policy & Threat Model

This document describes the trust boundaries, attacker model, and mitigations for the Maat compiler toolchain. It covers the current state as of v0.15.0 and will be updated as subsequent versions introduce new attack surfaces.

## Trust Boundaries

The Maat toolchain has four distinct trust boundaries:

```text
Source (.maat) --> Compiler Pipeline --> Bytecode (.mtc) --> VM Execution --> STARK Proof (.proof.bin)
     │                   │                     │                  │                       │
  Untrusted          Trusted               Untrusted           Trusted                Untrusted
(user input)       (our code)            (file on disk)      (our code)      (potentially adversarial)
```

1. **Source boundary:** `.maat` source files are untrusted input. The lexer, parser, type checker, and compiler must handle arbitrary, malformed, or adversarial source without panicking or consuming unbounded resources.

2. **Bytecode boundary:** `.mtc` files are untrusted input. A user may hand-craft or corrupt a bytecode file to exploit the deserializer or VM. The deserializer must reject malformed files before allocating resources, and the VM must validate all operands during execution.

3. **VM execution boundary:** Even well-formed bytecode may attempt resource exhaustion (infinite loops, deep recursion, stack overflow). The VM enforces runtime limits.

4. **Proof boundary:** `.proof.bin` files arrive over an untrusted channel. The verifier must be sound against adversarially crafted proofs and against valid-looking proofs that disagree with the claimed program hash, public inputs, or output. Soundness rests on the AIR constraint system, static transition-degree declarations compiled into `pub const` arrays, and Winterfell's FRI low-degree test.

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

| Attack vector                             | Mitigation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | Location                                                        |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| Wrong-program substitution                | 32-byte Blake3 program hash embedded in proof header; verifier requires `compute_program_hash(bytecode) == header.program_hash`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `maat_prover/src/program_hash.rs`                               |
| Truncated or short proof                  | 48-byte minimum header parse rejects on insufficient bytes; magic `b"MATP"` and version `u16` validated before payload decode                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | `maat_prover/src/gadgets.rs`                                    |
| Wrong magic / version drift               | Magic check + version match required before payload deserialization                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `maat_prover/src/gadgets.rs`                                    |
| Tampered execution trace                  | Per-row transition constraints (debug builds also panic at the prover) + boundary assertions; tampered output rows fail FRI                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `maat_trace/src/main_segment.rs`                                |
| Tampered memory permutation               | Auxiliary segment grand-product accumulator must telescope to the public-memory accumulator endpoint (`z^l / ∏(z - α·v_i - a_i)` over the public-output segment, or `1` when empty); address-continuity constraint over sorted pairs                                                                                                                                                                                                                                                                                                                                                                                                                                                            | `maat_air/src/aux_segment.rs`                                   |
| Physical-address gap injection            | The aux constraint `addr_delta * (addr_delta - 1) = 0` over the sorted L2 list is the sole enforcement; any injected gap produces a value ≠ {0,1} that fails the degree-2 check                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `maat_air/src/aux_segment.rs`                                   |
| Out-of-range integer witness              | 16-bit limb decomposition + sorted-limb permutation argument force every range-checked value into `[0, 2^64)`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | `maat_air/src/aux_segment.rs`                                   |
| Division-by-zero witness                  | `sel_div_mod * (s0 * nonzero_inv - 1) = 0` makes the prover commit to the divisor's modular inverse                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `maat_trace/src/main_segment.rs`                                |
| Falsified arithmetic output               | Per-opcode sub-selectors gate output-correctness constraints for arithmetic, division, unary, felt, equality/inequality, bitwise (via `BitwiseBuiltin`), and ordering (via range-check sign-pattern)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | `maat_trace/src/main_segment.rs`, `maat_air/src/builtin/`       |
| Adversarial constraint-degree declaration | Transition degrees are static `pub const` arrays compiled into the binary; both prover and verifier reconstruct `AirContext` from the same constants -- no per-trace metadata to forge                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `maat_trace/src/main_segment.rs`, `maat_air/src/aux_segment.rs` |
| Calling-convention forgery                | Synthetic `SEL_NOP` parameter rows + saved-FP write/read pair the callee's `GetLocal` reads with provable writes through the memory permutation argument                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | `maat_trace/src/recorder.rs`                                    |
| Forged public output                      | `out[last] = public_output` boundary assertion is bound to the proof header; the verifier checks both the proof and the embedded inputs. For multi-cell public outputs, the public-memory accumulator's L2 endpoint binds the `(addr, val)` pairs of every published cell; the segment length is bound through Fiat-Shamir transcript absorption (`MaatPublicInputs::to_elements()`), so a verifier disagreeing with the prover on `output_segment.len()` derives different challenges and rejects. In v0.15.0 a `Vector<T>` returned from the implicit main is auto-published per-cell to `SEG_PUBLIC_OUTPUT` via `Opcode::Index + HeapWrite` rows, each flowing through this same accumulator | `maat_air/src/lib.rs`, `maat_air/src/aux_segment.rs`            |
| Forged builtin-allocated cell             | Cells materialised by builtin returns (`Vector::rev`, `Vector::map`, etc.) flow through the memory permutation argument via synthetic heap-write rows (`SUB_SEL_SYNTHETIC_HEAP`). A tampered cell value fails the aux endpoint; the new structural constraint #81 `sub_synthetic_heap * (sub_synthetic_heap - sel_heap_alloc) = 0` enforces the sub-selector's binary shape                                                                                                                                                                                                                                                                                                                     | `maat_trace/src/main_segment.rs`, `maat_trace/src/recorder.rs`  |
| Forged closure capture                    | Closure captures are written into a per-closure segment via the same synthetic heap-write primitive. A tampered capture cell collides with the matching `GetFree` read row in the L2 sort and trips aux constraint 1 (single-value-per-address)                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `maat_vm/src/lib.rs::allocate_closure`                          |
| `MatchTag` jump-row bypass                | `Opcode::MatchTag` is in `SEL_CONSTRUCT` (linear-advance class) but conditionally jumps on tag mismatch. A new sub-selector `SUB_SEL_MATCH_TAG_JUMP` set by the recorder excludes jumping rows from `pc_uniform_gate` (constraint 26); structural constraint #82 `sub_match_tag_jump * (sub_match_tag_jump - sel_construct) = 0` keeps the marker honest. Pre-v0.15.0 the verifier rejected `match` arms that took the jump branch                                                                                                                                                                                                                                                              | `maat_trace/src/main_segment.rs`, `maat_vm/src/lib.rs`          |
| Range-check limb gap injection            | `fill_range_check_gaps` (`maat_trace::mem`) pads the trace with NOP rows whose `COL_RC_L0..L3` cover `1, 2, ..., max_limb` so the sorted-pool sortedness constraint `d * (d - 1) = 0` holds across non-contiguous natural limbs. Pre-v0.15.0 the sortedness constraint could be bypassed when a limb gap landed in a non-final row                                                                                                                                                                                                                                                                                                                                                              | `maat_trace/src/mem.rs`                                         |
| Cross-proof replay                        | Public inputs (program hash, input values, output) are bound into the proof via boundary assertions; copying a proof to a new program changes the program hash and fails the assertion                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `maat_prover/src/gadgets.rs`                                    |

**Soundness scope (v0.15.0):** the proof binds a program execution over integers (`i8`..`i64`, `u8`..`u64`, `usize`), `bool`, `Felt`, fixed-size arrays `[T; N]` over primitive `T`, **segment-backed `Vector<T>`** for primitive `T` (with every cell--including builtin-allocated returns--covered by the memory permutation argument via `SUB_SEL_SYNTHETIC_HEAP`), **segment-backed closures** (captures covered by the same synthetic heap-write primitive), and user-defined functions over those types--including bitwise operators, unsigned ordering comparisons (`<`/`>`/`<=`/`>=` on `u8`/`u16`/`u32`/`usize`/`char`), `Option<T>` / `Result<T, E>` pattern matching (`SUB_SEL_MATCH_TAG_JUMP` covers the jumping arm), and `#[bounded(N)]` loops. All twelve `examples/*.maat` programs prove and verify end-to-end under `development_options`.

`Map<K, V>`, `Set<T>`, `str`, `struct`, and `enum` continue to execute via inline `Value` variants. Their inline forms prove and verify cleanly for current programs (the cells never enter the heap), but those cells are not yet covered by the AIR's memory permutation argument--a malicious prover could substitute their contents without trace-level detection. No currently-shipping example or test exercises this gap; segment-backed migration is deferred until a consumer demonstrates that AIR-level cell coverage is load-bearing (recursive proofs, STARK-to-SNARK wrapping, in-AIR composite-cell content).

Ordering for `u64`/`i64` and signed types requires a tighter range primitive and is deferred to a future release.

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

Nine fuzz targets cover the full compiler and proof-system pipeline via `cargo-fuzz` (libFuzzer). All are exercised in CI on every pull request (60 s) and nightly (300 s).

**Compiler pipeline** (no seed corpus required; libFuzzer builds coverage from a single null byte):

- `fuzz_lexer` -- arbitrary bytes -> tokenization
- `fuzz_parser` -- arbitrary UTF-8 -> parsing
- `fuzz_typechecker` -- syntactically valid programs -> type checking
- `fuzz_compiler` -- well-typed programs -> compilation
- `fuzz_deserializer` -- arbitrary bytes -> bytecode deserialization

**Proof system** (requires `cargo run --release -p maat_tests --bin corpus_gen` after a fresh clone to seed `fuzz_proof_deserializer` and `fuzz_verifier`; the other two use committed text/binary seeds):

- `fuzz_proof_deserializer` -- arbitrary bytes -> `deserialize_proof` + the underlying STARK-proof decoder. Hook-swap + `catch_unwind` intercepts panics from the proof-decoding path.
- `fuzz_verifier` -- arbitrary bytes -> the full `verify` path.
- `fuzz_trace_recorder` -- arbitrary UTF-8 -> compile -> trace -> prove -> verify. Hook-swap intercepts the Winterfell `evaluation_table` assertion on degenerate traces.
- `fuzz_air_constraints` -- structured `(row_idx: u32, col_idx: u8, delta: u64)` inputs that tamper with individual trace cells before proving; confirms the AIR rejects tampered traces.

## Property-Based Testing

Property tests (`proptest`) verify invariants across hundreds to thousands of randomly generated programs and proof objects. See `tests/tests/properties.rs` for the full suite.

**Compiler pipeline:**

- Lexer, parser, type checker, compiler, and deserializer never panic on arbitrary input
- AST Display round-trip is idempotent
- Bytecode serialization round-trips perfectly
- Execution is deterministic (same program --> same result)
- Well-typed programs never produce runtime type errors

**Proof system:**

- *Round-trip* (20 cases): every provable `fn main() -> i64` program verifies after proving
- *Single-byte tamper rejection* (500 cases): mutating any byte of a serialized proof causes verification to fail. Coverage spans the 48-byte Maat header (magic, version, program hash, claimed output, input count) and the entire Winterfell payload (options, context, commitments, queries, FRI). The verifier is pinned to `AcceptableOptions::OptionSet(vec![development_options(), production_options()])`, so a tampered proof advertising weakened options is rejected before any cryptographic check; cryptographic checks then catch every other mutation
- *Relaxed address continuity* (20 cases): multi-variable programs with non-monotonic memory access prove and verify under the unified memory permutation argument
- *Program-hash collision resistance* (500 cases): distinct compiled bytecodes never share a Blake3 program hash

## Reporting Vulnerabilities

**Please do not open a GitHub issue or pull request to report a security vulnerability.** This makes the problem immediately visible to everyone, including malicious actors.

Report privately via [GitHub Security Advisories](https://github.com/maatlabs/maat/security/advisories/new).

### Coordinated disclosure

We commit to:

- **Acknowledging your report within 7 days** of receipt.
- **Releasing a fix within 90 days**, or coordinating an extension with you in writing if the issue is unusually involved.

If 90 days pass without a fix and without a written extension, you are free to disclose publicly. We ask only that you give us a final 7-day notice before doing so, so we can prepare downstream users.
