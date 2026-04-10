# maat_vm

Stack-based virtual machine for the Maat programming language.

## Role

`maat_vm` is the standard execution backend for Maat programs. The `VM` struct maintains a value stack, a globals store, and a frame stack for function calls. Each opcode dispatch pops operands, applies the operation, and pushes the result. Built-in functions are resolved by index, and closures capture their environment at definition time. For ZK-enabled execution with trace recording, use `maat_trace::TraceVM` instead.

## Usage

```rust
use maat_vm::VM;
use maat_bytecode::Bytecode;

let mut vm = VM::new(bytecode);
vm.run()?;

if let Some(result) = vm.last_popped_stack_elem() {
    println!("{result}");
}
```

## API Docs

[docs.rs/maat_vm](https://docs.rs/maat_vm/latest/maat_vm/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
