# maat_air

CPU constraint system (AIR) for the Maat programming language.

## Role

`maat_air` encodes the execution semantics of the Maat VM as polynomial constraints over the Goldilocks field, implementing Winterfell's `Air` trait. It bridges the trace-generating VM (`maat_trace`) and the STARK prover (`maat_prover`). The constraint system is split into two segments: a main segment that enforces instruction-level invariants (including range-check reconstruction and non-zero divisor proofs) and an auxiliary segment that enforces memory consistency and range-check soundness via grand-product permutation arguments.

## Constraint Summary

| Segment             | Columns | Constraints | Notes                                                                                             |
| ------------------- | ------- | ----------- | ------------------------------------------------------------------------------------------------- |
| Main (`maat_trace`) | 48      | 64          | Selectors + per-opcode sub-selectors, SP/PC/FP, output correctness, memory, NOP, range-check, div |
| Auxiliary           | 8       | 8           | Memory permutation, RC sorted continuity, RC permutation                                          |
| **Total**           | **56**  | **72**      | Max declared degree 5                                                                             |

**Boundary assertions:** 7 — `pc[0]=0`, `sp[0]=0`, `out[last]=output`, `mem_acc[0]=1`, `mem_acc[last]=1`, `rc_acc[0]=1`, `rc_acc[last]=1`

**Output correctness:** dedicated constraints for `Add`/`Sub`/`Mul`, `Div`/`Mod` (with `COL_DIV_AUX` remainder witness), `Neg`/`Not`, `FeltAdd`/`FeltSub`/`FeltMul`, `Equal`/`NotEqual` (with `COL_CMP_INV` inverse witness). Sub-selector witness columns gate each per-opcode constraint within its parent class.

**Universal PC advance:** a single constraint enforces `pc_next = pc + COL_OP_WIDTH` for all non-jump/call/return rows, replacing the family of width-specific PC constraints.

**Per-trace tight degrees:** `encode_degrees(main_columns)` runs an FFT-based detector that recovers the exact polynomial degree of every transition constraint on the concrete trace (main and auxiliary). The byte vector is shipped via `winter_air::TraceInfo::meta`; the verifier reconstructs the same `TransitionConstraintDegree` array. Sparse selector activations no longer trigger Winterfell's debug-mode degree-mismatch panic.

## Usage

```rust
use maat_air::{
    AUX_WIDTH, DEGREE_BYTES, MaatAir, MaatPublicInputs, NUM_AUX_RANDS, build_aux_columns,
    encode_degrees,
};
use winter_air::TraceInfo;

let public_inputs = MaatPublicInputs::new(program_hash, vec![], output_felt);
// Compute tight per-constraint degrees from the concrete trace and ship them
// in the trace metadata so the verifier reconstructs the same AIR context.
let meta = encode_degrees(&main_columns);
debug_assert_eq!(meta.len(), DEGREE_BYTES);
let trace_info = TraceInfo::new_multi_segment(
    maat_trace::TRACE_WIDTH,
    AUX_WIDTH,
    NUM_AUX_RANDS,
    trace_length,
    meta,
);
// Pass `MaatAir` to the Winterfell prover alongside the trace and auxiliary columns
```

## API Docs

[docs.rs/maat_air](https://docs.rs/maat_air/latest/maat_air/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
