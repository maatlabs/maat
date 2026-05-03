# maat_codegen

Bytecode code generation for the Maat programming language.

## Role

`maat_codegen` translates a type-checked AST into `Bytecode` in a single deterministic pass. The `Compiler` manages a scope stack, a constant pool, a symbol table (`SymbolsTable`), and a type registry for user-defined structs and enums. It emits stack-based opcodes for arithmetic, control flow, closures, and heap-allocated compound values. The output `Bytecode` is self-contained and ready for `maat_vm` or `maat_trace`.

## Usage

```rust
use maat_codegen::Compiler;
use maat_lexer::MaatLexer;
use maat_parser::MaatParser;

let source = "fn add(a: i64, b: i64) -> i64 { a + b }";
let mut p = MaatParser::new(MaatLexer::new(source));
let program = p.parse();

let mut c = Compiler::new();
c.compile_program(&program)?;

let bytecode = c.bytecode()?;

// The bytecode can be passed directly to the VM or trace-generating entry point
// maat_vm::VM::new(bytecode).run()
// maat_trace::run(bytecode)
```

## API Docs

[docs.rs/maat_codegen](https://docs.rs/maat_codegen/latest/maat_codegen/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
