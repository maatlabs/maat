# maat_eval

Macro expansion engine for the Maat programming language.

## Role

`maat_eval` powers Maat's metaprogramming layer. It extracts macro definitions from a `Program` via `extract_macros`, then rewrites macro call sites with their expanded AST via `expand_macros`. Expansion is driven by an internal tree-walking interpreter (`eval`) that is not exposed as a general-purpose execution path--program execution is handled exclusively by `maat_vm`. The `quote`/`unquote` special forms allow macro bodies to construct and splice AST fragments.

## Usage

```rust
use maat_eval::{extract_macros, expand_macros};
use maat_runtime::Env;

// Strip macro definitions from the AST and register them in the environment
let env = Env::new();
let program = extract_macros(program, &env);

// Replace all macro call sites with their expanded forms
let expanded = expand_macros(program, &env)?;
```

## API Docs

[docs.rs/maat_eval](https://docs.rs/maat_eval/latest/maat_eval/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
