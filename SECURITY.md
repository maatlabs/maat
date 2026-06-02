# Security Policy & Threat Model

This document describes the trust boundaries, attacker model, and mitigations for the Maat compiler toolchain. It covers the current state as of v0.17.0 and will be updated as subsequent versions introduce new attack surfaces.

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

4. **Proof boundary:** `.proof.bin` files arrive over an untrusted channel. The verifier must be sound against adversarially crafted proofs and against valid-looking proofs that disagree with the claimed program-segment cells, public inputs, or output. Soundness rests on the AIR constraint system, static transition-degree declarations compiled at construction time, and Winterfell's FRI low-degree test.

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

| Attack vector                             | Mitigation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | Location                                                               |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Wrong-program substitution                | The bytecode is pinned cell-by-cell into the AIR's public-memory accumulator: `Bytecode::serialize()` bytes are written into `SEG_PROGRAM` at runner startup, mirrored as `program_segment` cells in `MaatPublicInputs`, and the boundary endpoint `z^(L_out+L_prog)/∏(z - addr - α·val)` over the merged public-memory set binds the prover's claim to those exact bytes. A verifier disagreeing with the prover on any program byte derives a different Fiat-Shamir-absorbed `MaatPublicInputs::to_elements()` and a different recomputed endpoint, so the proof is rejected.                                                                                                                                                                          | `maat_air/src/aux_segment.rs`, `maat_trace/src/lib.rs`                 |
| Truncated or short proof                  | 32-byte minimum header parse rejects on insufficient bytes; magic `b"MATP"` and version `u16` (currently 5) validated before payload decode                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | `maat_prover/src/gadgets.rs`                                           |
| Wrong magic / version drift               | Magic check + version match required before payload deserialization. Output and program segments capped at `MAX_PUBLIC_SEGMENT_CELLS = 2^20` to reject oversized envelopes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | `maat_prover/src/gadgets.rs`                                           |
| Tampered execution trace                  | Per-row transition constraints (debug builds also panic at the prover) + boundary assertions; tampered output rows fail FRI                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | `maat_trace/src/main_segment.rs`                                       |
| Tampered memory permutation               | Auxiliary segment grand-product accumulator must telescope to the public-memory accumulator endpoint (`z^l / ∏(z - α·v_i - a_i)` over the public-output segment, or `1` when empty); address-continuity constraint over sorted pairs                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `maat_air/src/aux_segment.rs`                                          |
| Physical-address gap injection            | The aux constraint `addr_delta * (addr_delta - 1) = 0` over the sorted L2 list is the sole enforcement; any injected gap produces a value ≠ {0,1} that fails the degree-2 check                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `maat_air/src/aux_segment.rs`                                          |
| Out-of-range integer witness              | 16-bit limb decomposition + 8-bit-byte-chunked LogUp pool force every range-checked value into `[0, 2^64)`. The LogUp argument needs no padding to keep address continuity                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | `maat_air/src/builtin/range_check.rs`, `maat_air/src/builtin/logup.rs` |
| Division-by-zero witness                  | `sel_div_mod * (s0 * nonzero_inv - 1) = 0` makes the prover commit to the divisor's modular inverse                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | `maat_trace/src/main_segment.rs`                                       |
| Falsified arithmetic output               | Per-opcode sub-selectors gate output-correctness constraints for arithmetic, division, unary, felt, equality/inequality, bitwise (via `BitwiseBuiltin` with v0.16.0 chunked-LogUp diluted-form pool covering AND/OR/XOR and a pow2-lookup pool covering SHL/SHR), and ordering across every integer width up through `u64`/`i64`/`isize` (via the LogUp byte argument)                                                                                                                                                                                                                                                                                                                                                                                   | `maat_trace/src/main_segment.rs`, `maat_air/src/builtin/`              |
| Adversarial constraint-degree declaration | Main-segment transition degrees are static arrays compiled into the binary; aux-segment degrees are reconstructed from the registered `Builtin` set's runtime `Layout` (computed at `BuiltinSet::new()` from each builtin's trait-method shape, then frozen for the process lifetime). Both prover and verifier reconstruct `AirContext` from the same constants and the same registered set -- no per-trace metadata to forge                                                                                                                                                                                                                                                                                                                           | `maat_trace/src/main_segment.rs`, `maat_air/src/builtin.rs`            |
| Calling-convention forgery                | Synthetic `SEL_NOP` parameter rows + saved-FP write/read pair the callee's `GetLocal` reads with provable writes through the memory permutation argument                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `maat_trace/src/recorder.rs`                                           |
| Forged public output                      | `out[last] = public_output` boundary assertion is bound to the proof header; the verifier checks both the proof and the embedded inputs. For multi-cell public outputs, the public-memory accumulator's L2 endpoint binds the `(addr, val)` pairs of every published cell (alongside the program-segment cells); the segment length is bound through Fiat-Shamir transcript absorption (`MaatPublicInputs::to_elements()`), so a verifier disagreeing with the prover on `output_segment.len()` or `program_segment.len()` derives different challenges and rejects. A `Vector<T>` returned from the implicit main is auto-published per-cell to `SEG_PUBLIC_OUTPUT` via `Opcode::Index + HeapWrite` rows, each flowing through this same accumulator    | `maat_air/src/lib.rs`, `maat_air/src/aux_segment.rs`                   |
| Forged builtin-allocated cell             | Cells materialised by builtin returns (`Vector::rev`, `Vector::map`, etc., plus Rescue input/digest cells emitted via `record_rescue_call`) flow through the memory permutation argument via synthetic heap-write rows (`SUB_SEL_SYNTHETIC_HEAP`). A tampered cell value fails the aux endpoint; structural constraint #81 `sub_synthetic_heap * (sub_synthetic_heap - sel_heap_alloc) = 0` enforces the sub-selector's binary shape                                                                                                                                                                                                                                                                                                                     | `maat_trace/src/main_segment.rs`, `maat_trace/src/recorder.rs`         |
| Forged closure capture                    | Closure captures are written into a per-closure segment via the same synthetic heap-write primitive. A tampered capture cell collides with the matching `GetFree` read row in the L2 sort and trips aux constraint 1 (single-value-per-address)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `maat_vm/src/lib.rs::allocate_closure`                                 |
| `MatchTag` jump-row bypass                | `Opcode::MatchTag` is in `SEL_CONSTRUCT` (linear-advance class) but conditionally jumps on tag mismatch. A new sub-selector `SUB_SEL_MATCH_TAG_JUMP` set by the recorder excludes jumping rows from `pc_uniform_gate` (constraint 26); structural constraint #82 `sub_match_tag_jump * (sub_match_tag_jump - sel_construct) = 0` keeps the marker honest. Pre-v0.15.0 the verifier rejected `match` arms that took the jump branch                                                                                                                                                                                                                                                                                                                       | `maat_trace/src/main_segment.rs`, `maat_vm/src/lib.rs`                 |
| Range-check pool soundness                | The v0.16.0 LogUp byte-decomposition argument constrains every `COL_RC_L0..L3` limb to `[0, 2^16)` on every row via the on-row identity `limb - (256*hi + lo) = 0` plus the shared-pool LogUp lookup of `hi`/`lo` against the fixed `{0..255}` byte table. No padding is required: the LogUp argument's multiplicity column absorbs unused table entries directly.                                                                                                                                                                                                                                                                                                                                                                                       | `maat_air/src/builtin/range_check.rs`, `maat_air/src/builtin/logup.rs` |
| Cross-proof replay                        | Public inputs (`program_segment`, public-input cells, output, output_segment) are bound into the proof via boundary assertions and the public-memory accumulator endpoint over the merged set; copying a proof to a new program changes the `program_segment` and fails the endpoint check                                                                                                                                                                                                                                                                                                                                                                                                                                                               | `maat_prover/src/gadgets.rs`, `maat_air/src/aux_segment.rs`            |
| Wrong-public-input substitution           | `fn main`'s `pub` parameters are bound cell-by-cell into the public-memory accumulator at the input segment's flat base (proof header carries `INPUT_BASE u32 BE`). The boundary endpoint merges `(input, output, program)` segments into a single accumulator; a verifier disagreeing with the prover on any public-input cell derives a different endpoint and rejects. Pre-v0.17.0 the public-input cells were transcript-bound through `MaatPublicInputs::to_elements()` only---adequate for cryptographic soundness but with no in-AIR address pin                                                                                                                                                                                                  | `maat_air/src/aux_segment.rs`, `maat_trace/src/recorder.rs`            |
| Forged Rescue digest                      | `Opcode::HashRescue` emits a period-8 block of round witness rows; the per-element transition `(s_next[i] - ARK2[round, i])^7 - sum_j MDS[i][j] * s_curr[j]^7 - ARK1[round, i] = 0` (degree 8 once gated by `SUB_SEL_RESCUE_ROW`) verifies each round, and the input + digest cells flow through the unified memory permutation via `N + DIGEST_SIZE` synthetic heap writes per dispatch. A tampered round state breaks the degree-8 transition; a tampered input or digest cell collides at the matching read row in the sorted L2 list and trips aux constraint 1. The Rescue parameter set is `rp64_256` (Goldilocks, state width 12, capacity 4, rate 8, 7 rounds); intermediate post-S-box state never occupies a witness column (INV_MDS collapse) | `maat_trace/src/main_segment.rs`, `maat_trace/src/recorder.rs`         |
| Verifier accepts mismatched public values | The proof envelope cryptographically binds the embedded public values, but a verifier blindly accepting whatever the proof commits to cannot say "this proof attests to `f(x = 3) = y`". `maat verify --public-io <bundle.json>` / `--input` / `--expect-output` / `--expect-program{,-hash}` add an outer assertion layer: cryptographic verify runs first, then assertions are applied in order (program-hash -> public inputs cell-by-cell -> output) and any mismatch exits non-zero with a structured diagnostic. The `PublicIo` bundle round-trips byte-identically between `prove --write-public-io` and `verify --public-io`                                                                                                                     | `maat/src/cmd.rs`, `maat_prover/src/public_io.rs`                      |

**Soundness scope (v0.17.0):** the proof binds a program execution over integers (`i8`..`i64`, `u8`..`u64`, `usize`, `isize`), `bool`, `Felt`, fixed-size arrays `[T; N]` over primitive `T`, **segment-backed `Vector<T>`** for primitive `T` (with every cell---including builtin-allocated returns---covered by the memory permutation argument via `SUB_SEL_SYNTHETIC_HEAP`), **segment-backed closures** (captures covered by the same synthetic heap-write primitive), **Rescue-Prime hashing** (`hash::rescue_2` / `rescue_4` / `rescue_8`, with `N + DIGEST_SIZE` input and digest cells per dispatch covered by the memory permutation), and user-defined functions over those types---including a top-level `fn main(<params>) -> T` entry point with public (`pub`) parameters bound cell-by-cell into the public-memory accumulator and private (bare) parameters bound to an uncommitted prover-supplied witness segment, bitwise operators (AND/OR/XOR via the chunked-LogUp diluted-form pool, SHL/SHR via the pow2-lookup pool), ordering comparisons across every integer width up through `u64`/`i64`/`usize`/`isize` (`<`/`>`/`<=`/`>=`), `Option<T>` / `Result<T, E>` pattern matching (`SUB_SEL_MATCH_TAG_JUMP` covers the jumping arm), and `#[bounded(N)]` loops. The bytecode is committed cell-by-cell to the AIR's public-memory accumulator, so the proof additionally binds the exact program that produced the trace. The verifier-side public-I/O bundle (`maat verify --public-io / --input / --expect-output / --expect-program{,-hash}`) adds an assertion layer on top of the cryptographic verify so embedded values can be pinned to verifier-supplied expectations rather than blindly accepted. All `examples/*.maat` programs prove and verify end-to-end under `development_options`.

`Map<K, V>`, `Set<T>`, `str`, `struct`, and `enum` continue to execute via inline `Value` variants. Their inline forms prove and verify cleanly for current programs (the cells never enter the heap), but those cells are not yet covered by the AIR's memory permutation argument---a malicious prover could substitute their contents without trace-level detection. No currently-shipping example or test exercises this gap; segment-backed migration is deferred until a consumer demonstrates that AIR-level cell coverage is load-bearing (recursive proofs, STARK-to-SNARK wrapping, in-AIR composite-cell content).

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
- *Single-byte tamper rejection* (500 cases): mutating any byte of a serialized proof causes verification to fail. Coverage spans the v0.17.0 36-byte Maat header (magic, version, claimed output, input count, input/output/program base+length headers) plus the public-input cells, output-segment cells, program-segment cells, and the entire Winterfell payload (options, context, commitments, queries, FRI). The verifier is pinned to `AcceptableOptions::OptionSet(vec![development_options(), production_options()])`, so a tampered proof advertising weakened options is rejected before any cryptographic check; cryptographic checks then catch every other mutation
- *Relaxed address continuity* (20 cases): multi-variable programs with non-monotonic memory access prove and verify under the unified memory permutation argument
- *Program-segment commitment distinctness*: distinct compiled bytecodes produce distinct `program_segment` cells by the injectivity of postcard serialization

## Reporting Vulnerabilities

**Please do not open a GitHub issue or pull request to report a security vulnerability.** This makes the problem immediately visible to everyone, including malicious actors.

Report privately via [GitHub Security Advisories](https://github.com/maatlabs/maat/security/advisories/new).

### Coordinated disclosure

We commit to:

- **Acknowledging your report within 7 days** of receipt.
- **Releasing a fix within 90 days**, or coordinating an extension with you in writing if the issue is unusually involved.

If 90 days pass without a fix and without a written extension, you are free to disclose publicly. We ask only that you give us a final 7-day notice before doing so, so we can prepare downstream users.
