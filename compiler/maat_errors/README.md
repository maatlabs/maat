# maat_errors

Compiler error types and diagnostic infrastructure for the Maat programming language.

## Role

`maat_errors` defines the canonical `Error` enum and the `Result<T>` alias used across every crate in the Maat compiler. Each compilation stage--lexing, parsing, type checking, code generation, VM execution, and module resolution—contributes a dedicated error variant so that errors can be accumulated, converted with `?`, and reported with precise source locations via embedded `Span` values.

## Usage

```rust
use maat_errors::{Error, ParseError, Result};
use maat_span::Span;

fn parse_identifier(src: &str, span: Span) -> Result<String> {
    if src.is_empty() {
        return Err(ParseError::new("expected identifier", span).into());
    }
    Ok(src.to_owned())
}

// All compiler error variants are accessible through the top-level Error type
fn handle(err: Error) {
    match err {
        Error::Parse(e)  => eprintln!("parse error at {}..{}: {}", e.span.start, e.span.end, e.message),
        Error::Type(e)   => eprintln!("type error: {e}"),
        Error::Vm(e)     => eprintln!("runtime error: {e}"),
        _                => eprintln!("{err}"),
    }
}
```

## API Docs

[docs.rs/maat_errors](https://docs.rs/maat_errors/latest/maat_errors/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
