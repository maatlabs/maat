# maat_runtime

Runtime value types for the Maat virtual machine.

## Role

`maat_runtime` defines the `Value` enum--the common currency between the tree-walking macro evaluator (`maat_eval`) and the bytecode VM (`maat_vm`). It also provides `Env` (a scope-chained variable store), the built-in function registry, and integer wrapper types (`Integer`, `WideInt`) that enforce ZK-safe arithmetic (no floating-point, overflow-checked integer ops). `Felt` from `maat_field` is re-exported here as a first-class `Value` variant.

## Usage

```rust
use maat_runtime::{Value, Env, Felt, Integer};

// Build a simple environment
let env = Env::new();
env.set("x".into(), &Value::Integer(Integer::I64(42)));

// Retrieve a binding
if let Some(Value::Integer(Integer::I64(n))) = env.get("x") {
    println!("x = {n}");
}

// Field element values
let v = Value::Felt(Felt::from(7u64));
```

## API Docs

[docs.rs/maat_runtime](https://docs.rs/maat_runtime/latest/maat_runtime/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
