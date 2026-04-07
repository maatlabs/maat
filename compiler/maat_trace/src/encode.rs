//! Value-to-field-element encoding for the execution trace.
//!
//! Each runtime [`Value`] that appears in a trace-visible position (stack top,
//! memory cell, instruction output) must be encoded as a single [`Felt`]. This
//! module provides the canonical encoding used by the trace-generating VM.
//!
//! Composite types (`Vector`, `Map`, `Struct`, closures, etc.) are not
//! representable as a single field element. They encode as [`Felt::ZERO`] for now;
//! the trace VM will reject attempts to *prove* programs that place
//! composite values in trace-visible positions.

use maat_field::Felt;
use maat_runtime::Value;

/// Encodes a runtime value as a single Goldilocks field element.
///
/// Primitive types encode losslessly (within the 64-bit field):
///
/// | Value type   | Encoding                                                |
/// |--------------|---------------------------------------------------------|
/// | `Integer`    | Direct cast; negatives via two's complement mod `p`     |
/// | `Felt`       | Identity                                                |
/// | `Bool`       | `0` or `1`                                              |
/// | `Char`       | Unicode scalar as `u32`                                 |
/// | `Unit`       | `0`                                                     |
/// | `i128`/`u128`| `0` (lossy; two-column encoding deferred)               |
/// | Composites   | `0` (currently not encodable)                           |
pub fn value_to_felt(v: &Value) -> Felt {
    match v {
        Value::Integer(int) => int.to_felt(),
        Value::Felt(f) => *f,
        Value::Bool(b) => Felt::new(*b as u64),
        Value::Char(c) => Felt::new(*c as u32 as u64),
        Value::Unit => Felt::ZERO,
        _ => Felt::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maat_runtime::Integer;

    #[test]
    fn encode_integers() {
        assert_eq!(
            value_to_felt(&Value::Integer(Integer::I64(42))),
            Felt::new(42)
        );
        assert_eq!(
            value_to_felt(&Value::Integer(Integer::U8(255))),
            Felt::new(255)
        );
        assert_eq!(value_to_felt(&Value::Integer(Integer::I64(0))), Felt::ZERO);
    }

    #[test]
    fn encode_negative_integers() {
        let neg = value_to_felt(&Value::Integer(Integer::I64(-1)));
        assert_eq!(neg + Felt::ONE, Felt::ZERO);
    }

    #[test]
    fn encode_felt() {
        let f = Felt::new(999);
        assert_eq!(value_to_felt(&Value::Felt(f)), f);
    }

    #[test]
    fn encode_bool() {
        assert_eq!(value_to_felt(&Value::Bool(true)), Felt::ONE);
        assert_eq!(value_to_felt(&Value::Bool(false)), Felt::ZERO);
    }

    #[test]
    fn encode_char() {
        assert_eq!(value_to_felt(&Value::Char('A')), Felt::new(65));
    }

    #[test]
    fn encode_unit() {
        assert_eq!(value_to_felt(&Value::Unit), Felt::ZERO);
    }

    #[test]
    fn encode_composite_is_zero() {
        assert_eq!(value_to_felt(&Value::Str("hi".into())), Felt::ZERO);
        assert_eq!(value_to_felt(&Value::Vector(vec![])), Felt::ZERO);
    }
}
