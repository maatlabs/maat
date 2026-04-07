# maat_bytecode

Bytecode format and serialization for the Maat virtual machine.

## Role

`maat_bytecode` defines the complete instruction set for the Maat VM: opcodes, their operand widths, and the `encode`/`decode` functions that serialize instructions to a compact byte stream. The `Bytecode` struct bundles the encoded instructions, a constant pool, a type registry, and a `SourceMap` into a single artifact that is passed to `maat_vm` for execution or `maat_trace` for trace-generating execution.

## Usage

```rust
use maat_bytecode::{Instructions, Opcode, encode};

// Encode individual instructions
let load_const = encode(Opcode::Constant, &[0, 1]); // load constant at index 1
let add        = encode(Opcode::Add, &[]);

// Combine into an instruction sequence
let mut instructions = Instructions::from(load_const);
instructions.extend(&Instructions::from(add));

// Human-readable disassembly
println!("{instructions}");
```

## Limits

| Limit              | Value  | Constraint source              |
| ------------------ | ------ | ------------------------------ |
| Constant pool size | 65,535 | 2-byte `Constant` operand      |
| Enum variants      | 256    | 8-bit tag in `Construct`       |
| Stack depth        | 2,048  | `MAX_STACK_SIZE`               |
| Global bindings    | 65,535 | 2-byte `SetGlobal`/`GetGlobal` |

## API Docs

[docs.rs/maat_bytecode](https://docs.rs/maat_bytecode/latest/maat_bytecode/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
