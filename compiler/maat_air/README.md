# maat_air

CPU constraint system (AIR) for the Maat ZK backend.

## Role

`maat_air` encodes the execution semantics of the Maat VM as polynomial constraints over the Goldilocks field, implementing Winterfell's `Air` trait. It bridges the trace-generating VM (`maat_trace`) and the STARK prover (`maat_prover`). The constraint system is split into two segments: a main segment that enforces instruction-level invariants (including range-check reconstruction and non-zero divisor proofs) and an auxiliary segment that enforces memory consistency and range-check soundness via grand-product permutation arguments.

## Constraint Summary

| Segment             | Columns | Constraints | Notes                                                     |
| ------------------- | ------- | ----------- | --------------------------------------------------------- |
| Main (`maat_trace`) | 36      | 41          | Selectors, SP/PC/FP transitions, memory, range-check, div |
| Auxiliary           | 8       | 8           | Memory permutation, RC sorted continuity, RC permutation  |
| **Total**           | **44**  | **49**      | Max degree 5                                              |

**Boundary assertions:** 7 — `pc[0]=0`, `sp[0]=0`, `out[last]=output`, `mem_acc[0]=1`, `mem_acc[last]=1`, `rc_acc[0]=1`, `rc_acc[last]=1`

## Usage

```rust
use maat_air::{MaatAir, MaatPublicInputs, build_aux_columns, AUX_WIDTH, NUM_AUX_RANDS};
use winter_air::TraceInfo;

let public_inputs = MaatPublicInputs::new(trace_length, output_felt);
let trace_info = TraceInfo::new_multi_segment(
    maat_trace::TRACE_WIDTH, AUX_WIDTH, NUM_AUX_RANDS, trace_length, vec![],
);
// Pass `MaatAir` to the Winterfell prover alongside the trace and auxiliary columns
```

## API Docs

[docs.rs/maat_air](https://docs.rs/maat_air/latest/maat_air/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
