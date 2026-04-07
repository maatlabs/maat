# maat_trace

Trace-generating virtual machine for the Maat ZK backend.

## Role

`maat_trace` instruments every bytecode instruction to record a 29-column execution trace suitable for STARK proving. Each step appends one row to the `TraceTable` capturing the program counter, frame pointer, stack pointer, top three stack values, memory address and value, a one-hot opcode selector, and the instruction operands. The table is padded to the next power of two as required by the Winterfell FRI prover. The resulting trace is consumed by `maat_air` for constraint verification.

## Main Segment Schema (29 columns)

This crate produces the **main** trace segment. `maat_air` appends a 5-column auxiliary segment (address-sorted memory pairs and a grand-product accumulator), bringing the full proving system to 34 columns total.

| Column group     | Count | Description                     |
| ---------------- | ----- | ------------------------------- |
| `pc`, `fp`, `sp` | 3     | Control registers               |
| `s0`, `s1`, `s2` | 3     | Top three stack values          |
| `mem_addr/val`   | 2     | Memory access address and value |
| `is_read`        | 1     | Read (`1`) or write (`0`) flag  |
| `opcode`         | 1     | Raw opcode byte                 |
| `operand_0/1`    | 2     | Instruction operand bytes       |
| `out`            | 1     | Final output value (last row)   |
| Selector columns | 16    | One-hot opcode class encoding   |

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
