# maat_span

Source span and location types for the Maat compiler.

## Role

`maat_span` is the foundation of all error reporting in the compiler pipeline. Every token, AST node, and bytecode instruction carries a `Span` so that parse errors, type errors, and runtime panics can point back to the exact byte range in the original source file. `SourceMap` extends this to bytecode by mapping instruction offsets to their originating spans.

## Usage

```rust
use maat_span::{Span, SourceMap};

// Create a span covering bytes 4..7
let span = Span::new(4, 7);

// Merge two spans into one covering both ranges
let a = Span::new(0, 3);
let b = Span::new(5, 8);
let merged = a.merge(b); // Span { start: 0, end: 8 }

// Record instruction-to-source mappings for VM error messages
let mut map = SourceMap::new();
map.insert(0, span);
assert_eq!(map.lookup(0), Some(span));
```

## API Docs

[docs.rs/maat_span](https://docs.rs/maat_span/latest/maat_span/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
