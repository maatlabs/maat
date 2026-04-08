# maat_trace

Trace-generating virtual machine for the Maat ZK backend.

## Role

`maat_trace` instruments every bytecode instruction to record a 36-column execution trace suitable for STARK proving. Each step appends one row to the `TraceTable` capturing the program counter, frame pointer, stack pointer, top three stack values, memory address and value, a one-hot opcode selector, instruction operands, and range-check witness data. The table is padded to the next power of two as required by the Winterfell FRI prover. The resulting trace is consumed by `maat_air` for constraint verification.

## Main Segment Schema (36 columns)

This crate produces the **main** trace segment. `maat_air` appends an 8-column auxiliary segment (address-sorted memory pairs, memory grand-product accumulator, sorted range-check limb pool, and range-check permutation accumulator), bringing the full proving system to 44 columns total.

| Column group     | Count | Description                                          |
| ---------------- | ----- | ---------------------------------------------------- |
| `pc`, `sp`, `fp` | 3     | Control registers                                    |
| `opcode`         | 1     | Raw opcode byte                                      |
| `operand_0/1`    | 2     | Instruction operand bytes                            |
| `s0`, `s1`, `s2` | 3     | Top three stack values before instruction            |
| `out`            | 1     | Result value pushed to stack                         |
| `mem_addr/val`   | 2     | Memory access address and value                      |
| `is_read`        | 1     | Read (`1`) or write (`0`) flag                       |
| Selector columns | 17    | One-hot opcode class encoding (17 classes)           |
| `rc_val`         | 1     | Value being range-checked (zero on non-trigger rows) |
| `rc_l0`..`rc_l3` | 4     | 16-bit limb decomposition of `rc_val`                |
| `nonzero_inv`    | 1     | Multiplicative inverse of divisor on div/mod rows    |

## Usage

```rust
use maat_trace::run_trace;

let (trace, result) = run_trace(bytecode)?;

// Export to CSV for inspection
println!("{}", trace.to_csv());

// Pass the raw trace matrix to maat_air for constraint checking
let matrix: Vec<Vec<_>> = trace.into_columns();
```

## API Docs

[docs.rs/maat_trace](https://docs.rs/maat_trace/latest/maat_trace/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
