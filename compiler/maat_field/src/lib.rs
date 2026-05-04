//! Goldilocks-field type alias and runtime-value-to-field encoding for Maat STARK prover/verifier.
//!
//! This crate exposes [`Felt`] as a transparent alias for Winterfell's
//! `f64::BaseElement` (the Goldilocks prime field `p = 2^64 - 2^32 + 1`), so the
//! algebraic operators (`Add`, `Sub`, `Mul`, `Neg`, `Div`, `inv`, `exp`,
//! `as_int`, ...) carry through directly from the upstream implementation
//! without a forwarding wrapper. The crate's own contribution is the
//! [`ToElements`] implementations that lift every primitive runtime type into a
//! flat sequence of base‑field elements, the [`from_i64`] helper that bakes in
//! two's‑complement sign extension, and the [`try_inv`]/[`try_div`] wrappers
//! that surface the division-by-zero distinction Winterfell silently absorbs.
//!
//! # Field choice
//!
//! Goldilocks is selected for native 64-bit arithmetic, first-class Winterfell
//! support, and proven production use in Plonky2/Plonky3. The concrete backing
//! field is intentionally localised to this single type alias so a future move
//! to M31 or STARK-252 only touches this file.
//!
//! # Value-to-field encoding contract
//!
//! Types implement Winterfell's [`ToElements`] trait to define how each runtime
//! value is represented as a sequence of [`Felt`] elements. All encodings are
//! deterministic and infallible.
//!
//! | Maat type          | Encoding (`Vec<Felt>`)                                     |
//! |--------------------|------------------------------------------------------------|
//! | `i8`..`i64`        | `[from_i64(value)]`                                        |
//! | `u8`..`u64`        | `[Felt::new(value)]`                                       |
//! | `i128`, `u128`     | `[Felt::new(hi), Felt::new(lo)]` (split at 64 bits)        |
//! | `bool`             | `[0]` or `[1]`                                             |
//! | `char`             | `[Felt::new(scalar)]`                                      |
//! | `Felt`             | `[value]` (identity)                                       |
//! | `()` (unit)        | `[Felt::ZERO]`                                             |
//!
//! Composite runtime values (`str`, `Vector<T>`, `Map`, `Set`, `Struct`,
//! `EnumVariant`) are not encodable here and are lowered onto the unified
//! memory segment by `maat_codegen`.
//!
//! # Division semantics
//!
//! The `Div` operator on [`Felt`] (inherited from Winterfell) returns the field
//! product `lhs * rhs.inv()` and silently yields zero when `rhs` is zero.
//! The compiler emits divisions that must surface a runtime error in that
//! case; [`try_div`] and [`try_inv`] are the wrappers used by the VM and the
//! trace recorder to preserve the [`FieldError::InverseOfZero`] distinction.

#![forbid(unsafe_code)]

pub use winter_math::fields::f64::BaseElement;
pub use winter_math::{ExtensionOf, FieldElement, StarkField, ToElements};

/// A canonical element of the Maat base field.
pub type Felt = BaseElement;

/// The Goldilocks prime `p = 2^64 - 2^32 + 1`.
pub const MODULUS: u64 = <Felt as StarkField>::MODULUS;

pub type Result<T> = std::result::Result<T, FieldError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FieldError {
    #[error("cannot invert the zero element of the field")]
    InverseOfZero,
}

/// Lifts an `i64` into the field.
///
/// Negative values are turned into their canonical positive representation
/// (`p - |value|`) without a full Montgomery/modular reduction.
#[inline]
pub fn from_i64(value: i64) -> Felt {
    if value >= 0 {
        Felt::new(value as u64)
    } else {
        Felt::new(MODULUS.wrapping_add(value as u64))
    }
}

/// Multiplicative inverse, surfacing the zero-element case as a typed error.
#[inline]
pub fn try_inv(value: Felt) -> Result<Felt> {
    if value == Felt::ZERO {
        Err(FieldError::InverseOfZero)
    } else {
        Ok(value.inv())
    }
}

/// Field division `lhs * try_inv(rhs)`. Fails only when `rhs` is zero.
#[inline]
pub fn try_div(lhs: Felt, rhs: Felt) -> Result<Felt> {
    try_inv(rhs).map(|inv| lhs * inv)
}

/// A value that can be encoded into a sequence of base‑field elements.
pub trait Encodable {
    fn encode(&self) -> Vec<Felt>;
}

impl Encodable for Felt {
    fn encode(&self) -> Vec<Felt> {
        vec![*self]
    }
}

impl Encodable for bool {
    fn encode(&self) -> Vec<Felt> {
        vec![Felt::new(u64::from(*self))]
    }
}

impl Encodable for char {
    fn encode(&self) -> Vec<Felt> {
        vec![Felt::new(u64::from(*self as u32))]
    }
}

impl Encodable for () {
    fn encode(&self) -> Vec<Felt> {
        vec![Felt::ZERO]
    }
}

macro_rules! impl_encodable_unsigned {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Encodable for $ty {
                fn encode(&self) -> Vec<Felt> {
                    vec![Felt::new(u64::from(*self))]
                }
            }
        )*
    };
}

macro_rules! impl_encodable_signed {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Encodable for $ty {
                fn encode(&self) -> Vec<Felt> {
                    vec![from_i64(i64::from(*self))]
                }
            }
        )*
    };
}

impl_encodable_unsigned!(u8, u16, u32, u64);
impl_encodable_signed!(i8, i16, i32, i64);

impl Encodable for u128 {
    fn encode(&self) -> Vec<Felt> {
        let hi = (*self >> 64) as u64;
        let lo = *self as u64;
        vec![Felt::new(hi), Felt::new(lo)]
    }
}

impl Encodable for i128 {
    fn encode(&self) -> Vec<Felt> {
        let bits = *self as u128;
        let hi = (bits >> 64) as u64;
        let lo = bits as u64;
        vec![Felt::new(hi), Felt::new(lo)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_i64_handles_negatives() {
        let neg = from_i64(-1);
        let one = Felt::new(1);
        assert_eq!(neg + one, Felt::ZERO);
    }

    #[test]
    fn try_inv_of_zero_is_an_error() {
        assert_eq!(try_inv(Felt::ZERO), Err(FieldError::InverseOfZero));
    }

    #[test]
    fn try_inv_of_non_zero_round_trips() {
        let a = Felt::new(987654321);
        assert_eq!(a * try_inv(a).unwrap(), Felt::ONE);
    }

    #[test]
    fn try_div_by_non_zero_is_total() {
        let a = Felt::new(100);
        let b = Felt::new(25);
        assert_eq!(try_div(a, b).unwrap() * b, a);
    }

    #[test]
    fn try_div_by_zero_is_an_error() {
        assert_eq!(
            try_div(Felt::new(1), Felt::ZERO),
            Err(FieldError::InverseOfZero)
        );
    }

    #[test]
    fn encode_unsigned_integers() {
        assert_eq!(5u8.encode(), vec![Felt::new(5)]);
        assert_eq!(u64::MAX.encode(), vec![Felt::new(u64::MAX)]);
    }

    #[test]
    fn encode_signed_negatives() {
        let encoded = (-1i32).encode();
        assert_eq!(encoded.len(), 1);
        assert_eq!(encoded[0] + Felt::ONE, Felt::ZERO);
    }

    #[test]
    fn encode_bool() {
        assert_eq!(false.encode(), vec![Felt::ZERO]);
        assert_eq!(true.encode(), vec![Felt::ONE]);
    }

    #[test]
    fn encode_char() {
        assert_eq!('A'.encode(), vec![Felt::new(65)]);
    }

    #[test]
    fn encode_unit() {
        assert_eq!(().encode(), vec![Felt::ZERO]);
    }

    #[test]
    fn encode_u128_splits_hi_lo() {
        let value: u128 = (0xdead_beefu128 << 64) | 0xcafe_babe;
        let expected = vec![Felt::new(0xdead_beef), Felt::new(0xcafe_babe)];
        assert_eq!(value.encode(), expected);
    }

    #[test]
    fn encode_i128_negative() {
        let value: i128 = -1;
        let encoded = value.encode();
        assert_eq!(encoded, vec![Felt::new(u64::MAX), Felt::new(u64::MAX)]);
    }
}
