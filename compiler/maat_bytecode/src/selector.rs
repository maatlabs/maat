//! Per-opcode metadata used by the trace recorder and the AIR.
//!
//! Each [`Opcode`] maps to exactly one *selector class*: a coarse grouping that
//! partitions opcodes by the algebraic shape of the constraints they satisfy.
//! A subset of opcodes additionally maps to a *sub-selector*: a fine-grained
//! flag distinguishing same-class operations whose output formulas differ
//! (e.g. `Add` vs. `Sub`).
//!
//! The [`OpcodeInfo`] struct exposes both lookups together with the opcode's
//! operand widths so that the trace recorder, the VM, and the AIR
//! constraint system all read from a single source.

use crate::Opcode;

/// Number of selector columns reserved by the trace.
pub const NUM_SELECTORS: usize = 20;

/// Padding / no-operation rows.
pub const SEL_NOP: usize = 0;
/// Stack push operations: `Constant`, `True`, `False`, `Unit`, `GetBuiltin`,
/// `GetFree`, `CurrentClosure`. These push values without memory access.
pub const SEL_PUSH: usize = 1;
/// Integer arithmetic (non-dividing): `Add`, `Sub`, `Mul`.
pub const SEL_ARITH: usize = 2;
/// Bitwise operations: `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`.
pub const SEL_BITWISE: usize = 3;
/// Comparison operations: `Equal`, `NotEqual`, `LessThan`, `GreaterThan`.
pub const SEL_CMP: usize = 4;
/// Unary operations: `Minus`, `Bang`.
pub const SEL_UNARY: usize = 5;
/// Memory load operations: `GetLocal`, `GetGlobal`.
pub const SEL_LOAD: usize = 6;
/// Store operations: `SetLocal`, `SetGlobal`.
pub const SEL_STORE: usize = 7;
/// Unconditional jump: `Jump`.
pub const SEL_JUMP: usize = 8;
/// Conditional jump: `CondJump`.
pub const SEL_COND_JUMP: usize = 9;
/// Function call: `Call`.
pub const SEL_CALL: usize = 10;
/// Function return: `ReturnValue`, `Return`.
pub const SEL_RETURN: usize = 11;
/// Struct/enum construction: `Construct`, `GetField`, `MatchTag`, `Closure`.
pub const SEL_CONSTRUCT: usize = 12;
/// Type conversion: `Convert`.
pub const SEL_CONVERT: usize = 13;
/// Collection construction: `Vector`, `Map`, `Tuple`, `Array`, `MakeRange`,
/// `MakeRangeInclusive`, `Index`, `Pop`.
pub const SEL_COLLECTION: usize = 14;
/// Field-element arithmetic: `FeltAdd`, `FeltSub`, `FeltMul`, `FeltInv`, `FeltPow`.
pub const SEL_FELT: usize = 15;
/// Division and modulo: `Div`, `Mod`. Separated from [`SEL_ARITH`] so the
/// AIR can enforce a non-zero divisor constraint on exactly these rows.
pub const SEL_DIV_MOD: usize = 16;
/// Heap allocation: `HeapAlloc`.
pub const SEL_HEAP_ALLOC: usize = 17;
/// Heap read: `HeapRead`.
pub const SEL_HEAP_READ: usize = 18;
/// Heap write: `HeapWrite`.
pub const SEL_HEAP_WRITE: usize = 19;

/// Number of per-opcode sub-selector flags.
pub const NUM_SUB_SELECTORS: usize = 9;

/// Sub-selector index: `Add` (parent [`SEL_ARITH`]).
pub const SUB_SEL_ADD: usize = 0;
/// Sub-selector index: `Sub` (parent [`SEL_ARITH`]).
pub const SUB_SEL_SUB: usize = 1;
/// Sub-selector index: `Div` (parent [`SEL_DIV_MOD`]).
pub const SUB_SEL_DIV: usize = 2;
/// Sub-selector index: `Minus` (parent [`SEL_UNARY`]).
pub const SUB_SEL_NEG: usize = 3;
/// Sub-selector index: `FeltAdd` (parent [`SEL_FELT`]).
pub const SUB_SEL_FELT_ADD: usize = 4;
/// Sub-selector index: `FeltSub` (parent [`SEL_FELT`]).
pub const SUB_SEL_FELT_SUB: usize = 5;
/// Sub-selector index: `FeltMul` (parent [`SEL_FELT`]).
pub const SUB_SEL_FELT_MUL: usize = 6;
/// Sub-selector index: `Equal` (parent [`SEL_CMP`]).
pub const SUB_SEL_EQ: usize = 7;
/// Sub-selector index: `NotEqual` (parent [`SEL_CMP`]).
pub const SUB_SEL_NEQ: usize = 8;

/// Compile-time metadata for a single opcode.
///
/// Combines the opcode's selector class, optional sub-selector index, and
/// operand widths into one record. The trace recorder, the VM, and the
/// AIR constraint system all consume this record so that no layer
/// can drift from the others without a corresponding match-arm update here.
#[derive(Debug, Clone, Copy)]
pub struct OpcodeInfo {
    /// Selector class index in `[0, NUM_SELECTORS)`. One-hot at `COL_SEL_BASE + class`.
    pub selector: usize,
    /// Sub-selector column index in `[0, NUM_SUB_SELECTORS)` for opcodes whose
    /// per-opcode behaviour the AIR needs to distinguish; `None` otherwise.
    pub sub_selector: Option<usize>,
    /// Operand widths in bytes (matches [`Opcode::operand_widths`]).
    pub operand_widths: &'static [usize],
}

impl OpcodeInfo {
    /// Total instruction width (opcode byte plus operand bytes).
    #[inline]
    pub const fn instruction_width(&self) -> usize {
        let mut total = 1;
        let mut i = 0;
        while i < self.operand_widths.len() {
            total += self.operand_widths[i];
            i += 1;
        }
        total
    }
}

impl Opcode {
    /// Returns the per-opcode static metadata used by the trace recorder and AIR.
    #[inline]
    pub const fn info(self) -> OpcodeInfo {
        OpcodeInfo {
            selector: selector_index(self),
            sub_selector: sub_selector_index(self),
            operand_widths: self.operand_widths(),
        }
    }
}

/// Returns the selector class index for the given opcode.
///
/// Every executed opcode maps to exactly one class. [`SEL_NOP`] is reserved
/// for trace padding rows and never returned by this function.
pub const fn selector_index(op: Opcode) -> usize {
    match op {
        Opcode::Constant
        | Opcode::True
        | Opcode::False
        | Opcode::Unit
        | Opcode::GetBuiltin
        | Opcode::GetFree
        | Opcode::CurrentClosure => SEL_PUSH,

        Opcode::Add | Opcode::Sub | Opcode::Mul => SEL_ARITH,

        Opcode::Div | Opcode::Mod => SEL_DIV_MOD,

        Opcode::BitAnd | Opcode::BitOr | Opcode::BitXor | Opcode::Shl | Opcode::Shr => SEL_BITWISE,

        Opcode::Equal | Opcode::NotEqual | Opcode::LessThan | Opcode::GreaterThan => SEL_CMP,

        Opcode::Minus | Opcode::Bang => SEL_UNARY,

        Opcode::GetLocal | Opcode::GetGlobal => SEL_LOAD,

        Opcode::SetLocal | Opcode::SetGlobal => SEL_STORE,

        Opcode::Jump => SEL_JUMP,

        Opcode::CondJump => SEL_COND_JUMP,

        Opcode::Call => SEL_CALL,

        Opcode::ReturnValue | Opcode::Return => SEL_RETURN,

        Opcode::Construct | Opcode::GetField | Opcode::MatchTag | Opcode::Closure => SEL_CONSTRUCT,

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

        Opcode::HeapAlloc => SEL_HEAP_ALLOC,
        Opcode::HeapRead => SEL_HEAP_READ,
        Opcode::HeapWrite => SEL_HEAP_WRITE,
    }
}

/// Returns the sub-selector column index for the given opcode, or `None` if
/// the opcode does not require sub-selector witness data.
pub const fn sub_selector_index(op: Opcode) -> Option<usize> {
    match op {
        Opcode::Add => Some(SUB_SEL_ADD),
        Opcode::Sub => Some(SUB_SEL_SUB),
        Opcode::Div => Some(SUB_SEL_DIV),
        Opcode::Minus => Some(SUB_SEL_NEG),
        Opcode::FeltAdd => Some(SUB_SEL_FELT_ADD),
        Opcode::FeltSub => Some(SUB_SEL_FELT_SUB),
        Opcode::FeltMul => Some(SUB_SEL_FELT_MUL),
        Opcode::Equal => Some(SUB_SEL_EQ),
        Opcode::NotEqual => Some(SUB_SEL_NEQ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_opcode_maps_to_valid_selector() {
        for byte in 0..=52u8 {
            let op = Opcode::from_byte(byte).unwrap();
            let class = selector_index(op);
            assert!(
                class < NUM_SELECTORS,
                "opcode {op:?} mapped to out-of-range selector {class}"
            );
            assert_ne!(
                class, SEL_NOP,
                "opcode {op:?} must not map to SEL_NOP (reserved for padding)"
            );
        }
    }

    #[test]
    fn heap_opcodes_map_to_heap_selectors() {
        assert_eq!(selector_index(Opcode::HeapAlloc), SEL_HEAP_ALLOC);
        assert_eq!(selector_index(Opcode::HeapRead), SEL_HEAP_READ);
        assert_eq!(selector_index(Opcode::HeapWrite), SEL_HEAP_WRITE);
    }

    #[test]
    fn div_mod_use_dedicated_selector() {
        assert_eq!(selector_index(Opcode::Div), SEL_DIV_MOD);
        assert_eq!(selector_index(Opcode::Mod), SEL_DIV_MOD);
    }

    #[test]
    fn arith_excludes_div_mod() {
        assert_eq!(selector_index(Opcode::Add), SEL_ARITH);
        assert_eq!(selector_index(Opcode::Sub), SEL_ARITH);
        assert_eq!(selector_index(Opcode::Mul), SEL_ARITH);
        assert_ne!(selector_index(Opcode::Div), SEL_ARITH);
        assert_ne!(selector_index(Opcode::Mod), SEL_ARITH);
    }

    #[test]
    fn sub_selectors_are_in_range() {
        for byte in 0..=52u8 {
            let op = Opcode::from_byte(byte).unwrap();
            if let Some(sub) = sub_selector_index(op) {
                assert!(
                    sub < NUM_SUB_SELECTORS,
                    "opcode {op:?} sub-selector {sub} out of range"
                );
            }
        }
    }

    #[test]
    fn opcode_info_matches_helpers() {
        let info = Opcode::Add.info();
        assert_eq!(info.selector, SEL_ARITH);
        assert_eq!(info.sub_selector, Some(SUB_SEL_ADD));
        assert_eq!(info.operand_widths, Opcode::Add.operand_widths());
        assert_eq!(info.instruction_width(), 1);

        let info = Opcode::Closure.info();
        assert_eq!(info.selector, SEL_CONSTRUCT);
        assert_eq!(info.sub_selector, None);
        assert_eq!(info.instruction_width(), 1 + 2 + 1);
    }
}
