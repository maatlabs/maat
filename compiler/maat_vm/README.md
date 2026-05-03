# maat_vm

Stack-based virtual machine for the Maat programming language.

## Role

`maat_vm` is the single execution backend for Maat programs. The `VM` struct maintains a value stack, a globals store, and a frame stack for function calls. Each opcode dispatch pops operands, applies the operation, and pushes the result back onto the stack. Built-in functions are resolved by index, and closures capture their environment at definition time.

For ZK-enabled execution, `VM::run_with_recorder<R: Tracer>` accepts any `maat_vm::Tracer` implementation and calls it at every instrumentation point (pre-dispatch, output value, memory accesses, call/return, witness data). The production `VM::run` path supplies a `NoOpRecorder`; monomorphization collapses it to the same emitted code as a non-tracing run. `maat_trace::TraceRecorder` is the concrete `Tracer` implementation that writes rows into a `TraceTable` for STARK proving.

## Usage

```rust
use maat_vm::VM;

// Standard (non-tracing) execution
let mut vm = VM::new(bytecode);
vm.run()?;

if let Some(result) = vm.last_popped_stack_elem() {
    println!("{result}");
}

// Trace-enabled execution (e.g. from `maat_trace`)
use maat_vm::Tracer;

let mut recorder = MyTracer::default();
let mut vm = VM::new(bytecode);
vm.run_with_recorder(&mut recorder)?;
```

## API Docs

[docs.rs/maat_vm](https://docs.rs/maat_vm/latest/maat_vm/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
