# maat_field

Finite field arithmetic and value-to-field encoding for the Maat programming language.

## Role

`maat_field` exposes `Felt`, a newtype over Winterfell's 64-bit Goldilocks prime field (`p = 2^64 - 2^32 + 1`). It is the canonical algebraic element used by `maat_trace` and `maat_air` for all trace values, constraint evaluations, and boundary assertions. The `Encodable` trait defines how every Maat runtime value is lifted into the field, making this crate the bridge between the dynamic value system and the cryptographic proof machinery.

## Usage

```rust
use maat_field::{Felt, MODULUS};

// Arithmetic is closed and total (except inv(Felt::ZERO))
let a = Felt::from(7u64);
let b = Felt::from(3u64);
let sum  = a + b;
let prod = a * b;
let quot = (a / b).unwrap(); // returns Err for b == Felt::ZERO

// Encoding runtime values into the field
use maat_field::Encodable;
let elems = true.encode(); // [Felt(1)]
let elems = 42i64.encode(); // [Felt(42)]
```

## Field Properties

| Property       | Value                         |
| -------------- | ----------------------------- |
| Prime modulus  | `2^64 - 2^32 + 1`             |
| Backing type   | Winterfell `f64::BaseElement` |
| Security level | 64-bit native, no Montgomery  |

## API Docs

[docs.rs/maat_field](https://docs.rs/maat_field/latest/maat_field/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.
