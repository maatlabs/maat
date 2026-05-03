# maat_trace

Trace-generating virtual machine for the Maat programming language.

## Role

`maat_trace` instruments every bytecode instruction to record a 56-column execution trace suitable for STARK proving. Each step appends one row to the `TraceTable` capturing variables such as the program counter, frame pointer, stack pointer, memory address and value, a one-hot opcode selector, etc. The table is padded to the next power of two (minimum 8 rows) as required by the Winterfell FRI prover. The resulting trace is consumed by `maat_air` for constraint verification.

## Main Segment Schema (56 columns)

This crate produces the **main** trace segment. `maat_air` appends a 137-column auxiliary segment (memory permutation, range-check builtin, bitwise builtin, and identity builtin), bringing the full proving system to 193 columns total.

| Column group        | Count | Description                                                                                                                                         |
| ------------------- | ----- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pc`, `sp`, `fp`    | 3     | Control registers                                                                                                                                   |
| `operand_0`         | 1     | First instruction operand byte                                                                                                                      |
| `s0`, `s1`, `s2`    | 3     | Top three stack values before instruction                                                                                                           |
| `out`               | 1     | Result value pushed to stack                                                                                                                        |
| `mem_addr/val`      | 2     | Memory access address and value (unified segment; heap at `[2^32, 2^33)`)                                                                           |
| `is_read`           | 1     | Read (`1`) or write (`0`) flag                                                                                                                      |
| Selector columns    | 20    | One-hot opcode class encoding (20 classes)                                                                                                          |
| `rc_val`            | 1     | Value being range-checked; also carries `cmp_diff = s1 - s0` on ordering rows                                                                       |
| `rc_l0`..`rc_l3`    | 4     | 16-bit limb decomposition of `rc_val`                                                                                                               |
| `nonzero_inv`       | 1     | Multiplicative inverse of divisor on div/mod rows                                                                                                   |
| `op_width`          | 1     | Encoded operand width (`operand_widths().sum() + 1`) for the universal PC-advance constraint                                                        |
| `cmp_inv`           | 1     | `(s0 - s1)^{-1}` on equality/inequality rows; arbitrary otherwise                                                                                   |
| `div_aux`           | 1     | Remainder witness on `Div`, quotient witness on `Mod`                                                                                               |
| Sub-selector witns. | 16    | Per-opcode sub-selectors: `add`, `sub`, `div`, `neg`, `felt_add`, `felt_sub`, `felt_mul`, `eq`, `neq`, `and`, `or`, `xor`, `shl`, `shr`, `lt`, `gt` |

## Usage

```rust
use maat_trace::run;

let (trace, result) = run(bytecode)?;

// Export to CSV for inspection
println!("{}", trace.to_csv());

// Pass the raw trace matrix to `maat_air` for constraint checking
let matrix: Vec<Vec<_>> = trace.into_columns();
```

## API Docs

[docs.rs/maat_trace](https://docs.rs/maat_trace/latest/maat_trace/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
