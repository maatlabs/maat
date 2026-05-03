# maat_air

CPU constraint system (AIR) for the Maat programming language.

## Role

`maat_air` encodes the execution semantics of the Maat VM as polynomial constraints over the Goldilocks field, implementing Winterfell's `Air` trait. It bridges the trace-generating VM (`maat_trace`) and the STARK prover (`maat_prover`). The constraint system is split into two segments: a main segment that enforces instruction-level invariants (selectors, SP/PC/FP transitions, output correctness for arithmetic/bitwise/ordering/equality, range-check reconstruction, non-zero divisor proof) and an auxiliary segment that enforces memory consistency, range-check soundness, per-bit bitwise output formulas, and ordering sign-pattern correctness via the builtin-segment ABI.

## Constraint Summary

| Segment             | Columns | Constraints | Notes                                                                                                         |
| ------------------- | ------- | ----------- | ------------------------------------------------------------------------------------------------------------- |
| Main (`maat_trace`) | 56      | 81          | Selectors, sub-selectors (16), SP/PC/FP, output correctness, memory, NOP, range-check, div, bitwise, ordering |
| Auxiliary           | 137     | 20          | Memory permutation (3), `RangeCheckBuiltin` (5), `BitwiseBuiltin` (11), `IdentityBuiltin` (1)                 |
| **Total**           | **193** | **101**     | Max declared degree 5                                                                                         |

**Boundary assertions:** 9 — `pc[0]=0`, `sp[0]=0`, `out[last]=output` (main); `mem_acc[0]=1`, `mem_acc[last]=1`, `rc_acc[0]=1`, `rc_acc[last]=1`, `id_col[0]=1`, `id_col[last]=1` (auxiliary).

**Output correctness:** dedicated constraints for `Add`/`Sub`/`Mul`, `Div`/`Mod` (with `COL_DIV_AUX` remainder witness), `Neg`/`Not`, `FeltAdd`/`FeltSub`/`FeltMul`, `Equal`/`NotEqual` (with `COL_CMP_INV` inverse witness), `BitAnd`/`BitOr`/`BitXor`/`Shl`/`Shr` (via `BitwiseBuiltin` aux columns), `LessThan`/`GreaterThan` (via range-check sign-pattern constraints). Sub-selector witness columns gate each per-opcode constraint within its parent class.

**Universal PC advance:** a single constraint enforces `pc_next = pc + COL_OP_WIDTH` for all non-jump/call/return rows.

**Static transition degrees:** `CONSTRAINT_DEGREES` and `AUX_CONSTRAINT_DEGREES` are declared as `pub const` arrays. The prover performs no per-trace degree detection and `winter_air::TraceInfo::meta` is empty; the verifier reconstructs the same `TransitionConstraintDegree` array from the same constants. Winterfell's `quotient_degree <= declared` contract makes upper-bound declarations sound on every trace.

**Builtin-segment ABI:** expensive-to-arithmetize operations attach as `Builtin` impls behind the `maat_air::builtin::Builtin` trait; `BuiltinSet` composes them with compile-time layout constants. The CPU AIR's main segment is sealed against new operation classes.

## Usage

```rust
use maat_air::{AUX_WIDTH, MaatAir, MaatPublicInputs, NUM_AUX_RANDS, build_aux_columns};
use winter_air::TraceInfo;

let public_inputs = MaatPublicInputs::new(program_hash, vec![], output_felt);
let trace_info = TraceInfo::new_multi_segment(
    maat_trace::TRACE_WIDTH,
    AUX_WIDTH,
    NUM_AUX_RANDS,
    trace_length,
    Vec::new(),
);
// Pass `MaatAir` to the Winterfell prover alongside the trace and auxiliary columns
```

## API Docs

[docs.rs/maat_air](https://docs.rs/maat_air/latest/maat_air/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
