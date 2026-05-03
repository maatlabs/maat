# maat_field

Goldilocks field element type alias and value-to-field encoding for the Maat programming language.

## Role

`maat_field` defines `Felt` as a transparent type alias for `winter_math::fields::f64::BaseElement` (Goldilocks prime `p = 2^64 - 2^32 + 1`). Arithmetic operators, `FieldElement`, `ExtensionOf`, and `StarkField` trait impls are provided directly by Winterfell. This crate adds only the Maat-specific encoding surface: `from_i64` (two's-complement sign-extension into the field), `try_inv` and `try_div` (fallible inverses that surface `FieldError::InverseOfZero` instead of returning silent zeros), and the `Encodable` trait that maps every Maat runtime primitive to its canonical `Vec<Felt>` representation. Downstream crates depend only on `maat_field` for the encoding contract and do not need a direct dependency on `winter_math`.

## Usage

```rust
use maat_field::{Felt, FieldElement, from_i64, try_inv, try_div};

// Field element construction and arithmetic
let a = Felt::new(7u64);
let b = Felt::new(3u64);
let sum  = a + b;
let prod = a * b;

// Fallible inverse and division
let inv_b = try_inv(b).expect("b is non-zero");
let quot  = try_div(a, b).expect("b is non-zero");

// Two's-complement integer encoding
let neg_one = from_i64(-1i64); // encodes as p - 1

// Encoding runtime values into the field
use maat_field::Encodable;
let elems = true.encode();    // [Felt(1)]
let elems = 42i64.encode();   // [Felt(42)]
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
