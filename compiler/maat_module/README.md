# maat_module

Module system and multi-file compilation for the Maat programming language.

## Role

`maat_module` resolves `mod` declarations into a directed acyclic dependency graph before compilation begins. Each source file is parsed independently, cycle detection is performed via DFS with gray/black coloring, and the resulting topological ordering drives a two-phase pipeline: per-module type checking (with cross-module export injection) followed by sequential code generation and linking into a single `Bytecode` artifact.

## File Resolution

Resolution follows Rust's module conventions:

- `mod foo;` in `dir/mod.maat` --> `dir/foo.maat` or `dir/foo/mod.maat`
- `mod bar;` in `dir/foo.maat` --> `dir/foo/bar.maat` or `dir/foo/bar/mod.maat`

Ambiguous paths (i.e., when both alternatives exist) and missing paths both produce descriptive `ModuleError` values.

## Usage

```rust
use maat_module::resolve_module_graph;
use std::path::Path;

let graph = resolve_module_graph(Path::new("src/main.maat"))?;
let bytecode = maat_module::compile_graph(graph)?;
```

## API Docs

[docs.rs/maat_module](https://docs.rs/maat_module/latest/maat_module/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
