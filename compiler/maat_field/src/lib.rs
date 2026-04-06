//! Finite field arithmetic and value-to-field encoding for the Maat zero-knowledge backend.
//!
//! This crate defines [`Felt`], a newtype wrapper around Winterfell's `f64::BaseElement`
//! (the 64-bit Goldilocks prime field `p = 2^64 - 2^32 + 1`). `Felt` is exposed as a
//! first-class primitive type in Maat and is used throughout the ZK pipeline
//! as the canonical algebraic element over which traces, constraints, and proofs are built.
//!
//! # Field choice
//!
//! The Goldilocks prime is selected for its native 64-bit arithmetic (no Montgomery
//! overhead at the logical level), first-class Winterfell support, and proven production
//! use in systems such as Plonky2 and Plonky3. The concrete backing field is intentionally
//! localized to this crate so that future work--e.g. swapping to M31 or STARK-252--
//! requires only a configuration change here.
//!
//! # Value-to-field encoding contract
//!
//! The [`Encodable`] trait defines how each runtime value is lifted into the field.
//!
//! | Maat type                                                 | Felt encoding                                               |
//! | --------------------------------------------------------- | ----------------------------------------------------------- |
//! | `i8`..`i64`                                               | Sign-extended `i64`, reduced mod `p` via two's-complement   |
//! | `u8`..`u64`                                               | Direct reduction mod `p`                                    |
//! | `i128`, `u128`                                            | Two elements `(hi, lo)` split at 64 bits                    |
//! | `bool`                                                    | `0` or `1`                                                  |
//! | `char`                                                    | Unicode scalar as `u32`                                     |
//! | `Felt`                                                    | Direct (native element)                                     |
//! | `Unit`                                                    | `0`                                                         |
//! | `str`, `Vector<T>`, `Map`, `Set`, `Struct`, `EnumVariant` | Currently not encodable                                     |
//!
//! # Division semantics
//!
//! Field division is defined as multiplication by the modular inverse. `Felt::div(a, b)`
//! delegates to `a * b.inv()`. The only operation that can fail is `Felt::inv(Felt::ZERO)`,
//! which returns [`FieldError::InverseOfZero`]. All other field arithmetic is total.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

use core::fmt;
use core::ops::{Add, Div, Mul, Neg, Sub};

use winter_math::fields::f64::BaseElement;
use winter_math::{FieldElement, StarkField};

/// The Goldilocks prime `p = 2^64 - 2^32 + 1`.
///
/// This is the canonical modulus of the base field used by the ZK backend.
pub const MODULUS: u64 = BaseElement::MODULUS;

/// A canonical element of the base field.
///
/// Internally wraps Winterfell's `BaseElement` so that the algebraic surface used by the
/// compiler and VM is decoupled from the specific field implementation. All arithmetic is
/// total except [`Felt::inv`] applied to zero, which returns [`FieldError::InverseOfZero`].
#[derive(Copy, Clone, Default)]
pub struct Felt(BaseElement);

impl PartialEq for Felt {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.as_int() == other.0.as_int()
    }
}

impl Eq for Felt {}

impl core::hash::Hash for Felt {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.as_int().hash(state);
    }
}

/// Errors raised by fallible field operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FieldError {
    /// Attempted to invert the zero element of the field.
    #[error("cannot invert the zero element of the field")]
    InverseOfZero,
}

impl Felt {
    /// The additive identity of the field.
    pub const ZERO: Self = Self(BaseElement::ZERO);

    /// The multiplicative identity of the field.
    pub const ONE: Self = Self(BaseElement::ONE);

    /// Constructs a field element from a `u64` value, reducing modulo [`MODULUS`].
    #[inline]
    pub const fn new(value: u64) -> Self {
        Self(BaseElement::new(value))
    }

    /// Returns the canonical integer representation of this field element in `[0, p)`.
    #[inline]
    pub fn as_u64(self) -> u64 {
        self.0.as_int()
    }

    /// Lifts an `i64` into the field, encoding negatives as `p + value`.
    ///
    /// The computation is performed in `u64` wrapping arithmetic; because `|value| < p`
    /// for every `i64`, the canonical result lies in `[0, p)` after a single addition.
    #[inline]
    pub fn from_i64(value: i64) -> Self {
        if value >= 0 {
            Self::new(value as u64)
        } else {
            Self::new(MODULUS.wrapping_add(value as u64))
        }
    }

    /// Multiplicative inverse. Returns [`FieldError::InverseOfZero`] when applied to zero.
    #[inline]
    pub fn inv(self) -> Result<Self, FieldError> {
        if self.0 == BaseElement::ZERO {
            Err(FieldError::InverseOfZero)
        } else {
            Ok(Self(self.0.inv()))
        }
    }

    /// Field division, defined as `self * rhs.inv()`. Fails only when `rhs` is zero.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn div(self, rhs: Self) -> Result<Self, FieldError> {
        rhs.inv().map(|inv| self * inv)
    }

    /// Exponentiation by a non-negative integer exponent using Winterfell's constant-time
    /// square-and-multiply. `Felt::ZERO.pow(0)` returns [`Felt::ONE`], matching the
    /// conventional empty-product semantics.
    #[inline]
    pub fn pow(self, exponent: u64) -> Self {
        Self(self.0.exp(exponent))
    }

    /// Returns the underlying Winterfell field element.
    ///
    /// Exposed for downstream crates that must
    /// interface directly with Winterfell APIs.
    #[inline]
    pub fn into_base_element(self) -> BaseElement {
        self.0
    }

    /// Wraps a Winterfell base element into a [`Felt`].
    #[inline]
    pub fn from_base_element(element: BaseElement) -> Self {
        Self(element)
    }
}

impl fmt::Debug for Felt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Felt({})", self.as_u64())
    }
}

impl fmt::Display for Felt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_u64())
    }
}

impl Add for Felt {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Felt {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl Mul for Felt {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}

impl Neg for Felt {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl Div for Felt {
    type Output = Result<Self, FieldError>;
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        self.div(rhs)
    }
}

/// Encoding of a runtime value as one or more base-field elements.
///
/// Most scalar types fit in a single field element. The 128-bit integer types are the
/// only values that require a two-element `(hi, lo)` split because the Goldilocks
/// modulus is itself 64 bits wide.
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
/// This trait is the single source of truth for the value-to-field encoding contract.
/// Implementations must be total for their supported input space;
/// values that fall outside that space must return [`EncodingError::UnsupportedValueKind`].
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
                    Ok(FeltEncode::Single(Felt::from_i64(i64::from(*self))))
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
    fn zero_and_one_are_identities() {
        let x = Felt::new(1234567);
        assert_eq!(x + Felt::ZERO, x);
        assert_eq!(x * Felt::ONE, x);
    }

    #[test]
    fn addition_wraps_modulo_prime() {
        let a = Felt::new(MODULUS - 1);
        let b = Felt::new(2);
        assert_eq!((a + b).as_u64(), 1);
    }

    #[test]
    fn subtraction_is_inverse_of_addition() {
        let a = Felt::new(42);
        let b = Felt::new(1729);
        assert_eq!((a + b) - b, a);
    }

    #[test]
    fn negation_produces_additive_inverse() {
        let a = Felt::new(99);
        assert_eq!(a + a.neg(), Felt::ZERO);
    }

    #[test]
    fn from_i64_handles_negatives_via_twos_complement() {
        let neg = Felt::from_i64(-1);
        let one = Felt::new(1);
        assert_eq!(neg + one, Felt::ZERO);
    }

    #[test]
    fn multiplication_is_commutative() {
        let a = Felt::new(7);
        let b = Felt::new(11);
        assert_eq!(a * b, b * a);
    }

    #[test]
    fn inverse_is_multiplicative_identity_witness() {
        let a = Felt::new(987654321);
        let inv = a.inv().expect("non-zero invertible");
        assert_eq!(a * inv, Felt::ONE);
    }

    #[test]
    fn inverse_of_zero_is_an_error() {
        assert_eq!(Felt::ZERO.inv(), Err(FieldError::InverseOfZero));
    }

    #[test]
    fn division_by_non_zero_is_total() {
        let a = Felt::new(100);
        let b = Felt::new(25);
        assert_eq!(a.div(b).unwrap() * b, a);
    }

    #[test]
    fn division_by_zero_is_an_error() {
        let a = Felt::new(1);
        assert_eq!(a.div(Felt::ZERO), Err(FieldError::InverseOfZero));
    }

    #[test]
    fn pow_zero_exponent_is_one() {
        assert_eq!(Felt::new(123).pow(0), Felt::ONE);
        assert_eq!(Felt::ZERO.pow(0), Felt::ONE);
    }

    #[test]
    fn pow_matches_repeated_multiplication() {
        let a = Felt::new(3);
        let expected = a * a * a * a * a;
        assert_eq!(a.pow(5), expected);
    }

    #[test]
    fn fermat_little_theorem_holds() {
        let a = Felt::new(12345);
        assert_eq!(a.pow(MODULUS - 1), Felt::ONE);
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
