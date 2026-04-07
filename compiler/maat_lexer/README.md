# maat_lexer

DFA-based lexer for the Maat programming language.

## Role

`maat_lexer` converts raw Maat source text into a flat stream of `Token` values. It is the first stage of the compiler pipeline; its output feeds directly into `maat_parser`. The DFA is generated at compile time by [`logos`](https://crates.io/crates/logos), achieving zero-copy tokenization via string slices tied to the source lifetime.

## Usage

```rust
use maat_lexer::{MaatLexer, TokenKind};

let source = "let x: i64 = 42_i64;";
let mut lexer = MaatLexer::new(source);

assert_eq!(lexer.next_token().kind, TokenKind::Let);
assert_eq!(lexer.next_token().kind, TokenKind::Ident);
assert_eq!(lexer.next_token().kind, TokenKind::Colon);
assert_eq!(lexer.next_token().kind, TokenKind::Ident);
assert_eq!(lexer.next_token().kind, TokenKind::Assign);
assert_eq!(lexer.next_token().kind, TokenKind::I64);
assert_eq!(lexer.next_token().kind, TokenKind::Semicolon);
assert_eq!(lexer.next_token().kind, TokenKind::Eof);
```

## API Docs

[docs.rs/maat_lexer](https://docs.rs/maat_lexer/latest/maat_lexer/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
