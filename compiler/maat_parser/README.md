# maat_parser

Combinator-based parser for the Maat programming language.

## Role

`maat_parser` transforms the token stream produced by `maat_lexer` into a typed `Program` AST. It uses [`winnow`](https://crates.io/crates/winnow) combinators for statement dispatch with two-token lookahead and a manual Pratt loop for operator-precedence expression parsing. Parse errors are collected rather than aborting, enabling multi-error
reporting in a single pass.

## Usage

```rust
use maat_lexer::MaatLexer;
use maat_parser::MaatParser;

let source = "fn add(a: i64, b: i64) -> i64 { a + b }";
let lexer  = MaatLexer::new(source);
let mut parser = MaatParser::new(lexer);
let program = parser.parse();

if parser.errors().is_empty() {
    println!("parsed {} statement(s)", program.statements.len());
} else {
    for err in parser.errors() {
        eprintln!("{err}");
    }
}
```

## API Docs

[docs.rs/maat_parser](https://docs.rs/maat_parser/latest/maat_parser/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
