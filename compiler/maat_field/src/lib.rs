//! Goldilocks-field type alias and runtime-value-to-field encoding for the Maat STARK prover/verifier.
//!
//! This crate exposes [`Felt`] as a transparent alias for Winterfell's
//! `f64::BaseElement` (the Goldilocks prime field `p = 2^64 - 2^32 + 1`), so the
//! algebraic operators (`Add`, `Sub`, `Mul`, `Neg`, `Div`, `inv`, `exp`,
//! `as_int`, ...) carry through directly from the upstream implementation
//! without a forwarding wrapper. The crate's own contribution is the
//! [`Encodable`] trait that lifts every primitive Maat runtime type into the
//! field, the [`from_i64`] helper that bakes in two's-complement sign
//! extension, and the [`try_inv`]/[`try_div`] wrappers that surface the
//! division-by-zero distinction Winterfell silently absorbs.
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
//! The [`Encodable`] trait defines how each primitive runtime value is lifted
//! into the field.
//!
//! | Maat type        | Felt encoding                                              |
//! | ---------------- | ---------------------------------------------------------- |
//! | `i8`..`i64`      | Sign-extended `i64`, reduced mod `p` via two's-complement  |
//! | `u8`..`u64`      | Direct reduction mod `p`                                   |
//! | `i128`, `u128`   | Two elements `(hi, lo)` split at 64 bits                   |
//! | `bool`           | `0` or `1`                                                 |
//! | `char`           | Unicode scalar as `u32`                                    |
//! | `Felt`           | Direct (native element)                                    |
//! | `Unit`           | `0`                                                        |
//!
//! Composite runtime values (`str`, `Vector<T>`, `Map`, `Set`, `Struct`,
//! `EnumVariant`) are not encodable here and are lowered onto the unified
//! memory segment by `maat_codegen`.
//!
//! # Division semantics
//!
//! The `Div` operator on [`Felt`] (inherited from Winterfell) returns the field
//! product `lhs * rhs.inv()` and silently yields zero when `rhs` is zero.
//! Maat's compiler emits divisions that must surface a runtime error in that
//! case; [`try_div`] and [`try_inv`] are the wrappers used by the VM and the
//! trace recorder to preserve the [`FieldError::InverseOfZero`] distinction.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub use winter_math::fields::f64::BaseElement;
pub use winter_math::{ExtensionOf, FieldElement, StarkField, ToElements};

/// A canonical element of the Maat base field.
///
/// Transparent alias for Winterfell's Goldilocks `BaseElement`. All arithmetic
/// is total except [`try_inv`] applied to zero, which returns
/// [`FieldError::InverseOfZero`].
pub type Felt = BaseElement;

/// The Goldilocks prime `p = 2^64 - 2^32 + 1`.
pub const MODULUS: u64 = <Felt as StarkField>::MODULUS;

/// Errors raised by fallible field operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FieldError {
    /// Attempted to invert the zero element of the field.
    #[error("cannot invert the zero element of the field")]
    InverseOfZero,
}

/// Lifts an `i64` into the field, encoding negatives as `p + value`.
///
/// The computation is performed in `u64` wrapping arithmetic; because
/// `|value| < p` for every `i64`, the canonical result lies in `[0, p)` after
/// a single addition.
#[inline]
pub fn from_i64(value: i64) -> Felt {
    if value >= 0 {
        Felt::new(value as u64)
    } else {
        Felt::new(MODULUS.wrapping_add(value as u64))
    }
}

/// Multiplicative inverse, surfacing the zero-element case as a typed error.
///
/// Winterfell's `BaseElement::inv` returns zero for `Felt::ZERO`; this wrapper
/// lifts that case into [`FieldError::InverseOfZero`] so callers cannot
/// silently miscompute.
#[inline]
pub fn try_inv(value: Felt) -> Result<Felt, FieldError> {
    if value == Felt::ZERO {
        Err(FieldError::InverseOfZero)
    } else {
        Ok(value.inv())
    }
}

/// Field division `lhs * try_inv(rhs)`. Fails only when `rhs` is zero.
#[inline]
pub fn try_div(lhs: Felt, rhs: Felt) -> Result<Felt, FieldError> {
    try_inv(rhs).map(|inv| lhs * inv)
}

/// Encoding of a runtime value as one or more base-field elements.
///
/// Most scalar types fit in a single field element. The 128-bit integer types
/// are the only values that require a two-element `(hi, lo)` split because the
/// Goldilocks modulus is itself 64 bits wide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeltEncode {
    /// A single-element encoding.
    Single(Felt),
    /// A two-element encoding, high limb first.
    Pair(Felt, Felt),
}

/// Errors raised when a runtime value cannot be encoded into the field.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EncodingError {
    /// The value kind is not encodable. The payload identifies the kind for
    /// diagnostic purposes.
    #[error("value of kind `{0}` is not encodable as a field element")]
    UnsupportedValueKind(&'static str),
}

/// A value that can be lifted into the base field.
///
/// This trait is the single source of truth for the value-to-field encoding
/// contract. Implementations must be total for their supported input space;
/// values that fall outside that space must return
/// [`EncodingError::UnsupportedValueKind`].
pub trait Encodable {
    /// Lifts `self` into its canonical field encoding, or fails with a diagnostic error.
    fn encode(&self) -> Result<FeltEncode, EncodingError>;
}

impl Encodable for Felt {
    #[inline]
    fn encode(&self) -> Result<FeltEncode, EncodingError> {
        Ok(FeltEncode::Single(*self))
    }
}

impl Encodable for bool {
    #[inline]
    fn encode(&self) -> Result<FeltEncode, EncodingError> {
        Ok(FeltEncode::Single(Felt::new(u64::from(*self))))
    }
}

impl Encodable for char {
    #[inline]
    fn encode(&self) -> Result<FeltEncode, EncodingError> {
        Ok(FeltEncode::Single(Felt::new(u64::from(*self as u32))))
    }
}

impl Encodable for () {
    #[inline]
    fn encode(&self) -> Result<FeltEncode, EncodingError> {
        Ok(FeltEncode::Single(Felt::ZERO))
    }
}

macro_rules! impl_encodable_unsigned {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Encodable for $ty {
                #[inline]
                fn encode(&self) -> Result<FeltEncode, EncodingError> {
                    Ok(FeltEncode::Single(Felt::new(u64::from(*self))))
                }
            }
        )*
    };
}

macro_rules! impl_encodable_signed {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Encodable for $ty {
                #[inline]
                fn encode(&self) -> Result<FeltEncode, EncodingError> {
                    Ok(FeltEncode::Single(from_i64(i64::from(*self))))
                }
            }
        )*
    };
}

impl_encodable_unsigned!(u8, u16, u32, u64);
impl_encodable_signed!(i8, i16, i32, i64);

impl Encodable for u128 {
    #[inline]
    fn encode(&self) -> Result<FeltEncode, EncodingError> {
        let hi = (*self >> 64) as u64;
        let lo = *self as u64;
        Ok(FeltEncode::Pair(Felt::new(hi), Felt::new(lo)))
    }
}

impl Encodable for i128 {
    #[inline]
    fn encode(&self) -> Result<FeltEncode, EncodingError> {
        let bits = *self as u128;
        let hi = (bits >> 64) as u64;
        let lo = bits as u64;
        Ok(FeltEncode::Pair(Felt::new(hi), Felt::new(lo)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_i64_handles_negatives_via_twos_complement() {
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
    fn encode_unsigned_integers_direct() {
        assert_eq!(5u8.encode().unwrap(), FeltEncode::Single(Felt::new(5)));
        assert_eq!(
            u64::MAX.encode().unwrap(),
            FeltEncode::Single(Felt::new(u64::MAX))
        );
    }

    #[test]
    fn encode_signed_negatives_via_twos_complement() {
        let encoded = (-1i32).encode().unwrap();
        match encoded {
            FeltEncode::Single(felt) => assert_eq!(felt + Felt::ONE, Felt::ZERO),
            FeltEncode::Pair(_, _) => panic!("expected single-element encoding"),
        }
    }

    #[test]
    fn encode_bool_zero_and_one() {
        assert_eq!(false.encode().unwrap(), FeltEncode::Single(Felt::ZERO));
        assert_eq!(true.encode().unwrap(), FeltEncode::Single(Felt::ONE));
    }

    #[test]
    fn encode_char_as_scalar() {
        let c = 'A';
        assert_eq!(c.encode().unwrap(), FeltEncode::Single(Felt::new(65)));
    }

    #[test]
    fn encode_unit_is_zero() {
        assert_eq!(().encode().unwrap(), FeltEncode::Single(Felt::ZERO));
    }

    #[test]
    fn encode_u128_splits_into_hi_lo() {
        let value: u128 = (0xdead_beefu128 << 64) | 0xcafe_babe;
        let encoded = value.encode().unwrap();
        assert_eq!(
            encoded,
            FeltEncode::Pair(Felt::new(0xdead_beef), Felt::new(0xcafe_babe))
        );
    }

    #[test]
    fn encode_i128_negative_preserves_round_trip() {
        let value: i128 = -1;
        match value.encode().unwrap() {
            FeltEncode::Pair(hi, lo) => {
                assert_eq!(hi, Felt::new(u64::MAX));
                assert_eq!(lo, Felt::new(u64::MAX));
            }
            FeltEncode::Single(_) => panic!("expected pair encoding"),
        }
    }
}
