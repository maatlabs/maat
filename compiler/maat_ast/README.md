# maat_ast

Abstract syntax tree (AST) node definitions for the Maat programming language.

## Role

`maat_ast` is the shared data model for every post-parse stage of the Maat compiler. The `Program` struct, as well as the `Stmt` and `Expr` enums represent the full surface syntax, including `let` bindings, function definitions, struct/enum/trait declarations, loops, closures, and pattern matching. It also provides `fold_constants` for compile-time constant folding and `transform` for structure-preserving AST rewrites used by the macro expander and type checker.

## Usage

```rust
use maat_ast::{Program, Stmt, Expr, Node, fold_constants};

// Traverse all statements in a program
fn count_fns(program: &Program) -> usize {
    program.statements.iter()
        .filter(|s| matches!(s, Stmt::FuncDef(_)))
        .count()
}

// Apply constant folding before type checking
let folded = fold_constants(program);
```

## API Docs

[docs.rs/maat_ast](https://docs.rs/maat_ast/latest/maat_ast/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
