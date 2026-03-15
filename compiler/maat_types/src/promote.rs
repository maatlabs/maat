//! Numeric promotion rules for binary operations.
//!
//! Implements widening promotion rules for integer arithmetic:
//! - Same-sign: narrower widens to wider (`i8 + i16 -> i16`)
//! - Cross-sign: unsigned promotes to next larger signed (`u8 + i8 -> i16`)
//! - Float: implicit promotion is forbidden (requires explicit `as` cast)

use crate::Type;

/// Determines the common numeric type for a binary operation.
///
/// Returns the wider type that both operands should be promoted to,
/// or an error if the promotion is not allowed.
///
/// # Rules
///
/// - Identical types: no promotion needed.
/// - Two signed integers: widen to the larger.
/// - Two unsigned integers: widen to the larger.
/// - Mixed signed/unsigned: unsigned promotes to the next signed type
///   wide enough to hold both ranges. E.g., `u8 + i8 -> i16`.
/// - Any float involved: error (implicit float promotion forbidden).
/// - `isize`/`usize` treated as 64-bit for deterministic ZK execution.
pub fn common_numeric_type(a: &Type, b: &Type) -> Result<Type, PromotionError> {
    if a == b {
        return Ok(a.clone());
    }
    let (a_signed, a_width) = a
        .int_sign_bit_width()
        .ok_or(PromotionError::NonNumeric(a.clone()))?;
    let (b_signed, b_width) = b
        .int_sign_bit_width()
        .ok_or(PromotionError::NonNumeric(b.clone()))?;
    match (a_signed, b_signed) {
        (true, true) => Ok(bit_width_to_signed_int(a_width.max(b_width))),
        (false, false) => Ok(bit_width_to_unsigned_int(a_width.max(b_width))),
        (true, false) => cross_sign(a_width, b_width),
        (false, true) => cross_sign(b_width, a_width),
    }
}

/// Errors from numeric promotion.
#[derive(Debug, Clone)]
pub enum PromotionError {
    /// A non-numeric type appeared in an arithmetic context.
    NonNumeric(Type),
}

/// Returns the signed integer type with the given bit width.
fn bit_width_to_signed_int(bits: u32) -> Type {
    match bits {
        8 => Type::I8,
        16 => Type::I16,
        32 => Type::I32,
        64 => Type::I64,
        128 => Type::I128,
        _ => unreachable!("invalid signed integer width: {bits}"),
    }
}

/// Returns the unsigned integer type with the given bit width.
fn bit_width_to_unsigned_int(bits: u32) -> Type {
    match bits {
        8 => Type::U8,
        16 => Type::U16,
        32 => Type::U32,
        64 => Type::U64,
        128 => Type::U128,
        _ => unreachable!("invalid unsigned integer width: {bits}"),
    }
}

/// Cross-sign promotion: when mixing signed and unsigned integers,
/// promote to the next wider signed type that can hold both ranges.
///
/// `signed_width` is the width of the signed operand;
/// `unsigned_width` is the width of the unsigned operand.
fn cross_sign(signed_width: u32, unsigned_width: u32) -> Result<Type, PromotionError> {
    let min_width = if unsigned_width >= signed_width {
        next_width(unsigned_width).ok_or(PromotionError::NonNumeric(Type::U128))?
    } else {
        signed_width
    };
    Ok(bit_width_to_signed_int(min_width))
}

/// Returns the next larger bit width, or `None` if already at 128.
fn next_width(bits: u32) -> Option<u32> {
    match bits {
        8 => Some(16),
        16 => Some(32),
        32 => Some(64),
        64 => Some(128),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_type() {
        assert_eq!(
            common_numeric_type(&Type::I64, &Type::I64).unwrap(),
            Type::I64
        );
    }

    #[test]
    fn widen_signed() {
        assert_eq!(
            common_numeric_type(&Type::I8, &Type::I16).unwrap(),
            Type::I16
        );
        assert_eq!(
            common_numeric_type(&Type::I32, &Type::I64).unwrap(),
            Type::I64
        );
    }

    #[test]
    fn widen_unsigned() {
        assert_eq!(
            common_numeric_type(&Type::U8, &Type::U32).unwrap(),
            Type::U32
        );
    }

    #[test]
    fn promote_cross_sign() {
        assert_eq!(
            common_numeric_type(&Type::U8, &Type::I8).unwrap(),
            Type::I16
        );
        assert_eq!(
            common_numeric_type(&Type::U16, &Type::I32).unwrap(),
            Type::I32
        );
        assert_eq!(
            common_numeric_type(&Type::U32, &Type::I64).unwrap(),
            Type::I64
        );
        assert_eq!(
            common_numeric_type(&Type::U64, &Type::I128).unwrap(),
            Type::I128
        );
    }
}
