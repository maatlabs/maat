//! Opcode-to-selector-class mapping for the execution trace.
//!
//! Each row in the trace has exactly one selector flag set (one-hot encoding).
//! The 17 selector classes partition all opcodes into groups that share
//! algebraic constraint structure in the AIR. The division/modulo operations
//! are separated from general arithmetic to enable per-row non-zero divisor
//! verification in the range-check sub-AIR.

use maat_bytecode::Opcode;

/// Number of selector columns in the trace.
pub const NUM_SELECTORS: usize = 17;

/// Padding / no-operation rows.
pub const SEL_NOP: u8 = 0;
/// Stack push operations: `Constant`, `True`, `False`, `Unit`.
pub const SEL_PUSH: u8 = 1;
/// Integer arithmetic (non-dividing): `Add`, `Sub`, `Mul`.
pub const SEL_ARITH: u8 = 2;
/// Bitwise operations: `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`.
pub const SEL_BITWISE: u8 = 3;
/// Comparison operations: `Equal`, `NotEqual`, `LessThan`, `GreaterThan`.
pub const SEL_CMP: u8 = 4;
/// Unary operations: `Minus`, `Bang`.
pub const SEL_UNARY: u8 = 5;
/// Load operations: `GetLocal`, `GetGlobal`, `GetFree`, `GetBuiltin`, `CurrentClosure`.
pub const SEL_LOAD: u8 = 6;
/// Store operations: `SetLocal`, `SetGlobal`.
pub const SEL_STORE: u8 = 7;
/// Unconditional jump: `Jump`.
pub const SEL_JUMP: u8 = 8;
/// Conditional jump: `CondJump`.
pub const SEL_COND_JUMP: u8 = 9;
/// Function call: `Call`, `Closure`.
pub const SEL_CALL: u8 = 10;
/// Function return: `ReturnValue`, `Return`.
pub const SEL_RETURN: u8 = 11;
/// Struct/enum construction: `Construct`, `GetField`, `MatchTag`.
pub const SEL_CONSTRUCT: u8 = 12;
/// Type conversion: `Convert`.
pub const SEL_CONVERT: u8 = 13;
/// Collection construction: `Vector`, `Map`, `Tuple`, `Array`, `MakeRange`, `MakeRangeInclusive`, `Index`, `Pop`.
pub const SEL_COLLECTION: u8 = 14;
/// Field-element arithmetic: `FeltAdd`, `FeltSub`, `FeltMul`, `FeltInv`, `FeltPow`.
pub const SEL_FELT: u8 = 15;
/// Division and modulo: `Div`, `Mod`. Separated from [`SEL_ARITH`] so the
/// AIR can enforce a non-zero divisor constraint on exactly these rows.
pub const SEL_DIV_MOD: u8 = 16;

/// Returns the selector class index (0..16) for the given opcode.
///
/// Every executed opcode maps to exactly one selector. Padding rows
/// use [`SEL_NOP`], which is never returned by this function since
/// NOP is not a real opcode.
pub const fn class_index(op: Opcode) -> u8 {
    match op {
        Opcode::Constant | Opcode::True | Opcode::False | Opcode::Unit => SEL_PUSH,

        Opcode::Add | Opcode::Sub | Opcode::Mul => SEL_ARITH,

        Opcode::Div | Opcode::Mod => SEL_DIV_MOD,

        Opcode::BitAnd | Opcode::BitOr | Opcode::BitXor | Opcode::Shl | Opcode::Shr => SEL_BITWISE,

        Opcode::Equal | Opcode::NotEqual | Opcode::LessThan | Opcode::GreaterThan => SEL_CMP,

        Opcode::Minus | Opcode::Bang => SEL_UNARY,

        Opcode::GetLocal
        | Opcode::GetGlobal
        | Opcode::GetFree
        | Opcode::GetBuiltin
        | Opcode::CurrentClosure => SEL_LOAD,

        Opcode::SetLocal | Opcode::SetGlobal => SEL_STORE,

        Opcode::Jump => SEL_JUMP,

        Opcode::CondJump => SEL_COND_JUMP,

        Opcode::Call | Opcode::Closure => SEL_CALL,

        Opcode::ReturnValue | Opcode::Return => SEL_RETURN,

        Opcode::Construct | Opcode::GetField | Opcode::MatchTag => SEL_CONSTRUCT,

        Opcode::Convert => SEL_CONVERT,

        Opcode::Vector
        | Opcode::Map
        | Opcode::Tuple
        | Opcode::Array
        | Opcode::MakeRange
        | Opcode::MakeRangeInclusive
        | Opcode::Index
        | Opcode::Pop => SEL_COLLECTION,

        Opcode::FeltAdd | Opcode::FeltSub | Opcode::FeltMul | Opcode::FeltInv | Opcode::FeltPow => {
            SEL_FELT
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_opcode_maps_to_valid_selector() {
        for byte in 0..=49u8 {
            let op = Opcode::from_byte(byte).unwrap();
            let sel = class_index(op);
            assert!(
                sel < NUM_SELECTORS as u8,
                "opcode {:?} mapped to out-of-range selector {sel}",
                op,
            );
            assert_ne!(
                sel, SEL_NOP,
                "opcode {:?} must not map to SEL_NOP (reserved for padding)",
                op,
            );
        }
    }

    #[test]
    fn div_mod_use_dedicated_selector() {
        assert_eq!(class_index(Opcode::Div), SEL_DIV_MOD);
        assert_eq!(class_index(Opcode::Mod), SEL_DIV_MOD);
    }

    #[test]
    fn arith_excludes_div_mod() {
        assert_eq!(class_index(Opcode::Add), SEL_ARITH);
        assert_eq!(class_index(Opcode::Sub), SEL_ARITH);
        assert_eq!(class_index(Opcode::Mul), SEL_ARITH);
        assert_ne!(class_index(Opcode::Div), SEL_ARITH);
        assert_ne!(class_index(Opcode::Mod), SEL_ARITH);
    }
}
