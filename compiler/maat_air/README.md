# maat_air

CPU constraint system (AIR) for the Maat programming language.

## Role

`maat_air` encodes the execution semantics of the Maat VM as polynomial constraints over the Goldilocks field, implementing Winterfell's `Air` trait. It bridges the trace-generating VM (`maat_trace`) and the STARK prover (`maat_prover`). The constraint system is split into two segments: a main segment that enforces instruction-level invariants (selectors, SP/PC/FP transitions, output correctness for arithmetic/bitwise/ordering/equality, multi-row chunked-bitwise structural shape, range-check reconstruction, non-zero divisor proof) and an auxiliary segment that enforces memory consistency, range-check soundness via the shared-pool LogUp argument, chunked-bitwise diluted-form correctness, SHL/SHR pow2-lookup correctness, and the identity builtin.

## Constraint Summary (v0.18.0)

| Segment             | Columns | Constraints | Notes                                                                                                                                                                                                                                                                                                                                                                |
| ------------------- | ------- | ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Main (`maat_trace`) | 69      | 105         | 21 selectors, 20 sub-selectors (incl. `SUB_SEL_CHUNK_ROW` and `SUB_SEL_RESCUE_ROW`), SP/PC/FP, output correctness, memory, NOP, range-check, div, chunked-bitwise structural, ordering, inline Rescue round transitions (12 × degree 8) + 2 structural markers, `SEL_ARENA_FINALIZE` binary validity, and the heap-write value pin `sel_heap_write · (mem_val − s0)` |
| Auxiliary           | 45      | 39          | Memory permutation (including the merged public-memory endpoint over `(input, output, program)` segments), `RangeCheckBuiltin` byte-decomposition, `BitwiseBuiltin` chunked-LogUp + Horner accumulator + SHL/SHR, `IdentityBuiltin`, and `LogUpBuiltin` shared pools (byte / pow2 / diluted)                                                                         |
| **Total**           | **114** | **144**     | Max declared degree 8 (Rescue rounds); pre-Rescue max was 5                                                                                                                                                                                                                                                                                                          |

**Boundary assertions:** the AIR's `get_assertions` returns 3 main-segment boundaries (`pc[0]=0`, `sp[0]=0`, `out[last]=output`); `get_aux_assertions` returns 14 aux-segment boundaries---2 for the unified memory permutation (`mem_acc[0]=1`, `mem_acc[last]` = the public-memory endpoint over `(input_base, input_segment)`, `(output_base, output_segment)`, and `(program_base, program_segment)`) plus 12 LogUp-pool boundaries (8 byte-table channel zero-starts and the `m-side` / final-balance pair per pinned pool). When all three public-memory segments are empty the endpoint collapses to `1`.

**Inline Rescue AIR:** `Opcode::HashRescue` lays down period-8 blocks (one prefix row + seven round witness rows) in the main segment. Each round emits 12 degree-8 transitions of the form `(s_next[i] − ARK2[round, i])^7 − Σ_j MDS[i][j] · s_curr[j]^7 − ARK1[round, i] = 0`---the INV_MDS collapse keeps the post-S-box intermediate state out of the witness columns. State elements `0..8` reuse the wide working columns that idle on NOP rows; `COL_RESCUE_S8..S11` host elements `8..12`. ARK1 / ARK2 constants flow through 24 periodic columns of period 7 (`MaatAir::get_periodic_column_values`). Input and digest cells are synthesised as `N + DIGEST_SIZE` heap-write rows per dispatch into a lazily-allocated rescue-I/O segment that flows through the unified memory permutation argument.

**Program-hash anchor:** `maat_air::program_hash(&PublicSegment) -> [u8; 32]` is Blake3-256 over the little-endian `u64` bytes of the program-segment cells in cell-emission order, reusing `winter_crypto::hashers::Blake3_256` so the verifier surface stays on a single cryptographic primitive. Base-address independent. Plays the verification-key role for `maat verify --expect-program{,-hash}`.

**Output correctness:** dedicated constraints for `Add`/`Sub`/`Mul`, `Div`/`Mod` (with `COL_DIV_AUX` remainder witness), `Neg`/`Not`, `FeltAdd`/`FeltSub`/`FeltMul`, `Equal`/`NotEqual` (with `COL_CMP_INV` inverse witness), `BitAnd`/`BitOr`/`BitXor` (via the chunked-LogUp diluted-form pool fanned across 8 chunk rows per op), `Shl`/`Shr` (via the pow2 paired-key LogUp pool), and `LessThan`/`GreaterThan` for every integer width through `u64`/`i64`/`isize` (via the range-check sign pattern over the byte-table LogUp argument). Sub-selector witness columns gate each per-opcode constraint within its parent class.

**Universal PC advance:** a single constraint enforces `pc_next = pc + COL_OP_WIDTH` for all non-jump/call/return rows. Continuation chunk rows carry `sel_nop = 1` and freeze PC alongside the primary bitwise row that advances normally.

**Constraint-degree declarations:** main-segment `CONSTRAINT_DEGREES` is a static array compiled into the binary; aux-segment degrees are reconstructed from the registered `BuiltinSet`'s runtime `Layout` (a memoized `LazyLock` registry computed once at process startup from each builtin's trait-method shape). The prover performs no per-trace degree detection and `winter_air::TraceInfo::meta` is empty; the verifier reconstructs the same `TransitionConstraintDegree` array from the same constants and the same registered builtin set. Winterfell's `quotient_degree <= declared` contract makes upper-bound declarations sound on every trace.

**Builtin-segment ABI:** expensive-to-arithmetize operations attach as `Builtin` impls behind the `maat_air::builtin::Builtin` trait; `BuiltinSet` composes them via a runtime `Layout` and exposes them to the AIR through the unified `evaluate_aux_transition` interface. `LogUpBuiltin` owns the shared-pool LogUp bookkeeping (`{t, m, s_m, balance}` per pinned table plus one channel-grand-sum cell per consumer-registered channel) for the byte-table (`{0..255}`), pow2-paired (`{δ·k + 2^k : k=0..63}`), and diluted-paired (`{γ·v + dilute(v) : v=0..255}`) pools that `RangeCheckBuiltin` and `BitwiseBuiltin` lookup against. The CPU AIR's main segment is sealed against new operation classes.

## Usage

```rust
use maat_air::{MaatAir, MaatPublicInputs, PublicMemory, PublicSegment, aux_width, build_aux_columns, num_aux_rands};
use winter_air::TraceInfo;

// `aux_width()` and `num_aux_rands()` are functions, not constants, because
// the aux layout depends on the registered `BuiltinSet`.
let public_inputs = MaatPublicInputs::new(
    output_felt,
    PublicMemory {
        input: PublicSegment::new(input_base, input_segment),
        output: PublicSegment::new(output_base, output_segment),
        program: PublicSegment::new(program_base, program_segment),
    },
);
let trace_info = TraceInfo::new_multi_segment(
    maat_trace::TRACE_WIDTH,
    aux_width(),
    num_aux_rands(),
    trace_length,
    Vec::new(),
);
// Pass `MaatAir` to the Winterfell prover alongside the trace and auxiliary columns
```

## API Docs

[docs.rs/maat_air](https://docs.rs/maat_air/latest/maat_air/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
