# maat_air

CPU constraint system (AIR) for the Maat ZK backend.

## Role

`maat_air` encodes the execution semantics of the Maat VM as polynomial constraints over the Goldilocks field, implementing Winterfell's `Air` trait. It bridges the trace-generating VM (`maat_trace`) and the STARK prover (`maat_prover`). The constraint system is split into two segments: a main segment that enforces instruction-level invariants and an auxiliary segment that enforces memory consistency via a grand-product permutation argument.

## Constraint Summary

| Segment             | Columns | Constraints | Notes                                     |
| ------------------- | ------- | ----------- | ----------------------------------------- |
| Main (`maat_trace`) | 29      | 37          | Selectors, SP/PC/FP transitions, memory   |
| Auxiliary           | 5       | 3           | Address sort, single-value, grand-product |
| **Total**           | **34**  | **40**      | All degree <= 3                           |

**Boundary assertions:** 5 — `pc[0]=0`, `sp[0]=0`, `out[last]=output`, `perm_acc[0]=1`, `perm_acc[last]=1`

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
