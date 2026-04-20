//! Main segment transition constraint evaluation for the Maat CPU AIR.
//!
//! All constraints evaluate to zero on valid execution traces. Each constraint
//! is gated by one or more selector flags so that only the relevant rows are
//! checked. The constraint index assignments are documented in [`CONSTRAINT_DEGREES`].
//!
//! # Constraint index map
//!
//! | Index | Description                              | Degree |
//! |-------|------------------------------------------|--------|
//! | 0-16  | Selector binary validity (`sel_i`)       | 2      |
//! | 17    | Selector sum = 1                         | 1      |
//! | 18    | SP: push (net +1)                        | 2      |
//! | 19    | SP: binary ops (net -1)                  | 2      |
//! | 20    | SP: unary (net 0)                        | 2      |
//! | 21    | SP: store (net -1)                       | 2      |
//! | 22    | SP: load (net +1)                        | 2      |
//! | 23    | PC: single-byte opcodes (pc + 1)         | 2      |
//! | 24    | PC: sel_convert (pc + 2)                 | 2      |
//! | 25    | PC: unconditional jump                   | 2      |
//! | 26    | PC: cond jump, not taken                 | 3      |
//! | 27    | PC: cond jump, taken                     | 3      |
//! | 28    | Cond jump: s0 is binary                  | 3      |
//! | 29    | Load: is_read = 1                        | 2      |
//! | 30    | Load: out = mem_val                      | 2      |
//! | 31    | Store: is_read = 0                       | 2      |
//! | 32    | Store: mem_val = s0                      | 2      |
//! | 33    | FP: call (fp_next = out)                 | 2      |
//! | 34    | FP: return (fp_next = mem_val)           | 2      |
//! | 35    | NOP: pc frozen                           | 2      |
//! | 36    | NOP: sp frozen                           | 2      |
//! | 37    | NOP: fp frozen                           | 2      |
//! | 38    | RC: reconstruction                       | 1      |
//! | 39    | RC: convert linking                      | 2      |
//! | 40    | RC: non-zero divisor                     | 3      |
//! | 41    | NOP: output frozen                       | 2      |

use maat_trace::{
    COL_FP, COL_IS_READ, COL_MEM_VAL, COL_NONZERO_INV, COL_OPERAND_0, COL_OUT, COL_PC, COL_RC_L0,
    COL_RC_L1, COL_RC_L2, COL_RC_L3, COL_RC_VAL, COL_S0, COL_SEL_BASE, COL_SP, TRACE_WIDTH,
};
use winter_math::FieldElement;

// Selector column indices relative to `COL_SEL_BASE`.
pub(crate) const SEL_NOP: usize = 0;
pub(crate) const SEL_PUSH: usize = 1;
pub(crate) const SEL_ARITH: usize = 2;
pub(crate) const SEL_BITWISE: usize = 3;
pub(crate) const SEL_CMP: usize = 4;
pub(crate) const SEL_UNARY: usize = 5;
pub(crate) const SEL_LOAD: usize = 6;
pub(crate) const SEL_STORE: usize = 7;
pub(crate) const SEL_JUMP: usize = 8;
pub(crate) const SEL_COND_JUMP: usize = 9;
pub(crate) const SEL_CALL: usize = 10;
pub(crate) const SEL_RETURN: usize = 11;
pub(crate) const SEL_CONVERT: usize = 13;
pub(crate) const SEL_FELT: usize = 15;
pub(crate) const SEL_DIV_MOD: usize = 16;

/// Number of selector columns (must match `maat_trace::selector::NUM_SELECTORS`).
pub(crate) const NUM_SELECTORS: usize = 17;

/// Number of transition constraints enforced by the AIR.
pub const NUM_CONSTRAINTS: usize = 42;

/// Degree of each transition constraint, indexed by constraint number.
///
/// The prover uses these to allocate the correct evaluation domain size.
/// All constraints are degree <= 3 to keep the blowup factor manageable.
pub const CONSTRAINT_DEGREES: [usize; NUM_CONSTRAINTS] = [
    // 0-16: selector binary validity
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // 17: selector sum
    1, // 18-22: SP updates
    2, 2, 2, 2, 2, // 23-24: PC increment (uniform-width)
    2, 2, // 25: unconditional jump
    2, // 26-27: conditional jump
    3, 3, // 28: cond jump s0 binary
    3, // 29-32: load/store
    2, 2, 2, 2, // 33-34: frame pointer
    2, 2, // 35-37: NOP padding
    2, 2, 2, // 38: RC reconstruction
    1, // 39: RC convert linking
    2, // 40: RC non-zero divisor
    3, // 41: NOP output frozen
    2,
];

/// Reads a selector flag from the current row.
#[inline]
fn sel<E: FieldElement>(current: &[E], index: usize) -> E {
    current[COL_SEL_BASE + index]
}

/// Computes `2^16`, `2^32`, and `2^48` as field elements via repeated squaring.
#[inline]
fn power_of_two_constants<E: FieldElement>() -> (E, E, E) {
    let two = E::ONE + E::ONE;
    // 2^16 via repeated squaring: 2 --> 4 --> 16 --> 256 --> 65536
    let p2 = two * two; // 2^2
    let p4 = p2 * p2; // 2^4
    let p8 = p4 * p4; // 2^8
    let p16 = p8 * p8; // 2^16
    let p32 = p16 * p16; // 2^32
    let p48 = p32 * p16; // 2^48
    (p16, p32, p48)
}

/// Evaluates all 42 transition constraints.
///
/// `current` and `next` are consecutive trace rows (each of width [`TRACE_WIDTH`]).
/// The result slice must have length [`NUM_CONSTRAINTS`]; each entry is the
/// constraint evaluation (zero on valid traces).
pub fn evaluate<E: FieldElement>(current: &[E], next: &[E], result: &mut [E]) {
    debug_assert_eq!(current.len(), TRACE_WIDTH);
    debug_assert_eq!(next.len(), TRACE_WIDTH);
    debug_assert_eq!(result.len(), NUM_CONSTRAINTS);

    let one = E::ONE;

    let pc = current[COL_PC];
    let sp = current[COL_SP];
    let fp = current[COL_FP];
    let operand_0 = current[COL_OPERAND_0];
    let s0 = current[COL_S0];
    let out = current[COL_OUT];
    let mem_val = current[COL_MEM_VAL];
    let is_read = current[COL_IS_READ];

    let pc_next = next[COL_PC];
    let sp_next = next[COL_SP];
    let fp_next = next[COL_FP];

    for (i, slot) in result[..NUM_SELECTORS].iter_mut().enumerate() {
        let s = sel(current, i);
        *slot = s * (one - s);
    }

    let mut sel_sum = E::ZERO;
    for i in 0..NUM_SELECTORS {
        sel_sum += sel(current, i);
    }
    result[17] = sel_sum - one;
    result[18] = sel(current, SEL_PUSH) * (sp_next - sp - one);

    let sel_binop = sel(current, SEL_ARITH)
        + sel(current, SEL_BITWISE)
        + sel(current, SEL_CMP)
        + sel(current, SEL_DIV_MOD);
    result[19] = sel_binop * (sp_next - sp + one);
    result[20] = sel(current, SEL_UNARY) * (sp_next - sp);
    result[21] = sel(current, SEL_STORE) * (sp_next - sp + one);
    result[22] = sel(current, SEL_LOAD) * (sp_next - sp - one);

    let sel_single_byte = sel(current, SEL_ARITH)
        + sel(current, SEL_BITWISE)
        + sel(current, SEL_CMP)
        + sel(current, SEL_UNARY)
        + sel(current, SEL_FELT)
        + sel(current, SEL_DIV_MOD);
    result[23] = sel_single_byte * (pc_next - pc - one);

    let two = one + one;
    result[24] = sel(current, SEL_CONVERT) * (pc_next - pc - two);

    result[25] = sel(current, SEL_JUMP) * (pc_next - operand_0);
    let three = two + one;
    result[26] = sel(current, SEL_COND_JUMP) * s0 * (pc_next - pc - three);
    result[27] = sel(current, SEL_COND_JUMP) * (one - s0) * (pc_next - operand_0);
    result[28] = sel(current, SEL_COND_JUMP) * s0 * (one - s0);

    result[29] = sel(current, SEL_LOAD) * (is_read - one);
    result[30] = sel(current, SEL_LOAD) * (out - mem_val);
    result[31] = sel(current, SEL_STORE) * is_read;
    result[32] = sel(current, SEL_STORE) * (mem_val - s0);

    result[33] = sel(current, SEL_CALL) * (fp_next - out);
    result[34] = sel(current, SEL_RETURN) * (fp_next - mem_val);

    result[35] = sel(current, SEL_NOP) * (pc_next - pc);
    result[36] = sel(current, SEL_NOP) * (sp_next - sp);
    result[37] = sel(current, SEL_NOP) * (fp_next - fp);

    let rc_val = current[COL_RC_VAL];
    let l0 = current[COL_RC_L0];
    let l1 = current[COL_RC_L1];
    let l2 = current[COL_RC_L2];
    let l3 = current[COL_RC_L3];

    let (p16, p32, p48) = power_of_two_constants::<E>();

    result[38] = rc_val - (l0 + p16 * l1 + p32 * l2 + p48 * l3);

    result[39] = sel(current, SEL_CONVERT) * (rc_val - out);

    let nonzero_inv = current[COL_NONZERO_INV];
    result[40] = sel(current, SEL_DIV_MOD) * (s0 * nonzero_inv - one);

    let out_next = next[COL_OUT];
    result[41] = sel(current, SEL_NOP) * (out_next - out);
}

#[cfg(test)]
mod tests {
    use winter_math::fields::f64::BaseElement;

    use super::*;

    type F = BaseElement;

    /// Creates a zero-filled row pair with sel_nop set.
    fn nop_rows() -> ([F; TRACE_WIDTH], [F; TRACE_WIDTH]) {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_NOP] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        (current, next)
    }

    fn eval(current: &[F], next: &[F]) -> Vec<F> {
        let mut result = vec![F::ZERO; NUM_CONSTRAINTS];
        evaluate(current, next, &mut result);
        result
    }

    #[test]
    fn nop_rows_satisfy_all_constraints() {
        let (current, next) = nop_rows();
        let result = eval(&current, &next);
        for (i, &r) in result.iter().enumerate() {
            assert_eq!(r, F::ZERO, "constraint {i} violated on NOP rows");
        }
    }

    #[test]
    fn selector_binary_rejects_non_binary() {
        let (mut current, next) = nop_rows();
        current[COL_SEL_BASE + SEL_NOP] = F::new(2);
        let result = eval(&current, &next);
        assert_ne!(
            result[SEL_NOP],
            F::ZERO,
            "sel_nop=2 should fail binary check"
        );
    }

    #[test]
    fn selector_sum_rejects_no_selector() {
        let current = [F::ZERO; TRACE_WIDTH];
        let next = [F::ZERO; TRACE_WIDTH];
        // No selector set: sum = 0
        let result = eval(&current, &next);
        assert_ne!(result[17], F::ZERO, "zero selector sum should fail");

        // Two selectors set: sum = 2
        let mut current = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_SEL_BASE + SEL_PUSH] = F::ONE;
        let result = eval(&current, &next);
        assert_ne!(result[17], F::ZERO, "double selector should fail sum check");
    }

    #[test]
    fn sp_push_increments_by_one() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_PUSH] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_SP] = F::new(3);

        // Correct: sp_next = 4
        next[COL_SP] = F::new(4);
        let result = eval(&current, &next);
        assert_eq!(result[18], F::ZERO);

        // Wrong: sp_next = 3 (no change)
        next[COL_SP] = F::new(3);
        let result = eval(&current, &next);
        assert_ne!(result[18], F::ZERO);
    }

    #[test]
    fn sp_binary_op_decrements_by_one() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_ARITH] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_SP] = F::new(5);

        // Correct: sp_next = 4
        next[COL_SP] = F::new(4);
        let result = eval(&current, &next);
        assert_eq!(result[19], F::ZERO);

        // Wrong: sp_next = 5
        next[COL_SP] = F::new(5);
        let result = eval(&current, &next);
        assert_ne!(result[19], F::ZERO);
    }

    #[test]
    fn sp_div_mod_decrements_by_one() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_DIV_MOD] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_SP] = F::new(5);
        // Satisfy non-zero divisor constraint: S0 * inv = 1
        current[COL_S0] = F::ONE;
        current[COL_NONZERO_INV] = F::ONE;

        next[COL_SP] = F::new(4);
        let result = eval(&current, &next);
        assert_eq!(result[19], F::ZERO, "div_mod should have net SP = -1");
    }

    #[test]
    fn pc_single_byte_increments_by_one() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_ARITH] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_PC] = F::new(10);

        // Correct: pc_next = 11
        next[COL_PC] = F::new(11);
        let result = eval(&current, &next);
        assert_eq!(result[23], F::ZERO);

        // Wrong: pc_next = 13
        next[COL_PC] = F::new(13);
        let result = eval(&current, &next);
        assert_ne!(result[23], F::ZERO);
    }

    #[test]
    fn pc_div_mod_increments_by_one() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_DIV_MOD] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_PC] = F::new(10);
        current[COL_S0] = F::ONE;
        current[COL_NONZERO_INV] = F::ONE;

        next[COL_PC] = F::new(11);
        let result = eval(&current, &next);
        assert_eq!(result[23], F::ZERO);
    }

    #[test]
    fn unconditional_jump_sets_pc_to_operand() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_JUMP] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_OPERAND_0] = F::new(42);

        next[COL_PC] = F::new(42);
        let result = eval(&current, &next);
        assert_eq!(result[25], F::ZERO);

        next[COL_PC] = F::new(10);
        let result = eval(&current, &next);
        assert_ne!(result[25], F::ZERO);
    }

    #[test]
    fn conditional_jump_not_taken() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_COND_JUMP] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_S0] = F::ONE;
        current[COL_PC] = F::new(10);
        current[COL_OPERAND_0] = F::new(50);

        // Correct: fall through to pc + 3
        next[COL_PC] = F::new(13);
        let result = eval(&current, &next);
        assert_eq!(result[26], F::ZERO);
        assert_eq!(result[27], F::ZERO);

        // Wrong: jumped instead
        next[COL_PC] = F::new(50);
        let result = eval(&current, &next);
        assert_ne!(result[26], F::ZERO);
    }

    #[test]
    fn conditional_jump_taken() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_COND_JUMP] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_S0] = F::ZERO;
        current[COL_PC] = F::new(10);
        current[COL_OPERAND_0] = F::new(50);

        // Correct: jump to operand_0
        next[COL_PC] = F::new(50);
        let result = eval(&current, &next);
        assert_eq!(result[26], F::ZERO);
        assert_eq!(result[27], F::ZERO);

        // Wrong: fell through instead
        next[COL_PC] = F::new(13);
        let result = eval(&current, &next);
        assert_ne!(result[27], F::ZERO);
    }

    #[test]
    fn load_is_read_and_value_match() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_LOAD] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_IS_READ] = F::ONE;
        current[COL_MEM_VAL] = F::new(99);
        current[COL_OUT] = F::new(99);
        next[COL_SP] = F::ONE; // sp + 1

        let result = eval(&current, &next);
        assert_eq!(result[29], F::ZERO); // is_read = 1
        assert_eq!(result[30], F::ZERO); // out = mem_val

        // Wrong is_read
        current[COL_IS_READ] = F::ZERO;
        let result = eval(&current, &next);
        assert_ne!(result[29], F::ZERO);
    }

    #[test]
    fn store_is_write_and_value_match() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_STORE] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_IS_READ] = F::ZERO; // write
        current[COL_S0] = F::new(77);
        current[COL_MEM_VAL] = F::new(77);

        let result = eval(&current, &next);
        assert_eq!(result[31], F::ZERO); // is_read = 0
        assert_eq!(result[32], F::ZERO); // mem_val = s0

        // Wrong: is_read = 1
        current[COL_IS_READ] = F::ONE;
        let result = eval(&current, &next);
        assert_ne!(result[31], F::ZERO);
    }

    #[test]
    fn nop_padding_freezes_state() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_NOP] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_PC] = F::new(50);
        current[COL_SP] = F::new(3);
        current[COL_FP] = F::new(100);
        next[COL_PC] = F::new(50);
        next[COL_SP] = F::new(3);
        next[COL_FP] = F::new(100);

        let result = eval(&current, &next);
        assert_eq!(result[35], F::ZERO);
        assert_eq!(result[36], F::ZERO);
        assert_eq!(result[37], F::ZERO);

        // Changed pc
        next[COL_PC] = F::new(51);
        let result = eval(&current, &next);
        assert_ne!(result[35], F::ZERO);
    }

    #[test]
    fn frame_pointer_call_sets_fp_to_out() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_CALL] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_OUT] = F::new(100);
        next[COL_FP] = F::new(100);

        let result = eval(&current, &next);
        assert_eq!(result[33], F::ZERO);

        next[COL_FP] = F::new(5);
        let result = eval(&current, &next);
        assert_ne!(result[33], F::ZERO);
    }

    #[test]
    fn frame_pointer_return_restores_from_mem_val() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_RETURN] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_MEM_VAL] = F::new(200);
        next[COL_FP] = F::new(200);

        let result = eval(&current, &next);
        assert_eq!(result[34], F::ZERO);

        next[COL_FP] = F::new(100);
        let result = eval(&current, &next);
        assert_ne!(result[34], F::ZERO);
    }

    #[test]
    fn reconstruction_constraint_valid_decomposition() {
        let (mut current, next) = nop_rows();
        // val = 0x0003_0002_0001_000A = 3*(2^48) + 2*(2^32) + 1*(2^16) + 10
        let val = 10u64 + (1u64 << 16) + (2u64 << 32) + (3u64 << 48);
        current[COL_RC_VAL] = F::new(val);
        current[COL_RC_L0] = F::new(10);
        current[COL_RC_L1] = F::new(1);
        current[COL_RC_L2] = F::new(2);
        current[COL_RC_L3] = F::new(3);

        let result = eval(&current, &next);
        assert_eq!(result[38], F::ZERO, "valid decomposition should pass");
    }

    #[test]
    fn reconstruction_constraint_invalid_decomposition() {
        let (mut current, next) = nop_rows();
        current[COL_RC_VAL] = F::new(42);
        current[COL_RC_L0] = F::new(99);

        let result = eval(&current, &next);
        assert_ne!(result[38], F::ZERO, "wrong limbs should fail");
    }

    #[test]
    fn convert_linking_constraint() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_CONVERT] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_PC] = F::new(10);
        next[COL_PC] = F::new(12); // pc + 2 for convert
        current[COL_SP] = F::new(3);
        next[COL_SP] = F::new(3); // unary-like sp

        let val = F::new(255);
        current[COL_OUT] = val;
        current[COL_RC_VAL] = val;
        current[COL_RC_L0] = val;
        // l1,l2,l3 = 0

        let result = eval(&current, &next);
        assert_eq!(result[39], F::ZERO, "rc_val == OUT should pass");

        // Wrong: rc_val != OUT
        current[COL_RC_VAL] = F::new(100);
        current[COL_RC_L0] = F::new(100);
        let result = eval(&current, &next);
        assert_ne!(result[39], F::ZERO, "rc_val != OUT should fail");
    }

    #[test]
    fn nonzero_divisor_constraint() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_DIV_MOD] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_SP] = F::new(5);
        next[COL_SP] = F::new(4);
        current[COL_PC] = F::new(10);
        next[COL_PC] = F::new(11);

        // S0 = 7, inv = 7^{-1}
        let divisor = F::new(7);
        current[COL_S0] = divisor;
        current[COL_NONZERO_INV] = divisor.inv();

        let result = eval(&current, &next);
        assert_eq!(result[40], F::ZERO, "valid inverse should pass");

        // Wrong: S0 = 0 (no valid inverse exists)
        current[COL_S0] = F::ZERO;
        current[COL_NONZERO_INV] = F::ZERO;
        let result = eval(&current, &next);
        assert_ne!(result[40], F::ZERO, "zero divisor should fail");
    }
}
