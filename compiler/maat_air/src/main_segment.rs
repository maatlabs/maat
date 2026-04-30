//! Main segment transition constraint evaluation for the Maat CPU AIR.
//!
//! All constraints evaluate to zero on valid execution traces. Each constraint
//! is gated by one or more selector flags so that only the relevant rows are
//! checked. The constraint index assignments are documented in
//! [`CONSTRAINT_DEGREES`].
//!
//! # Constraint index map
//!
//! | Index | Description                                   | Degree |
//! |-------|-----------------------------------------------|--------|
//! | 0-19  | Selector binary validity (`sel_i`)            | 2      |
//! | 20    | Selector sum = 1                              | 1      |
//! | 21    | SP: push (net +1)                             | 2      |
//! | 22    | SP: binary ops (net -1)                       | 2      |
//! | 23    | SP: unary (net 0)                             | 2      |
//! | 24    | SP: store (net -1)                            | 2      |
//! | 25    | SP: load (net +1)                             | 2      |
//! | 26    | PC: universal (`pc_next = pc + op_width`)     | 2      |
//! | 27    | Width binding: width-1 classes                | 2      |
//! | 28    | Width binding: convert (width 2)              | 2      |
//! | 29    | PC: unconditional jump                        | 2      |
//! | 30    | PC: cond jump, not taken                      | 3      |
//! | 31    | PC: cond jump, taken                          | 3      |
//! | 32    | Cond jump: s0 is binary                       | 3      |
//! | 33    | Load: is_read = 1                             | 2      |
//! | 34    | Load: out = mem_val                           | 2      |
//! | 35    | Store: is_read = 0                            | 2      |
//! | 36    | Store: mem_val = s0                           | 2      |
//! | 37    | FP: call (fp_next = out)                      | 2      |
//! | 38    | FP: return (fp_next = mem_val)                | 2      |
//! | 39    | NOP: pc frozen                                | 2      |
//! | 40    | NOP: sp frozen                                | 2      |
//! | 41    | NOP: fp frozen                                | 2      |
//! | 42    | RC: reconstruction                            | 1      |
//! | 43    | RC: convert linking                           | 2      |
//! | 44    | RC: non-zero divisor                          | 3      |
//! | 45    | NOP: output frozen                            | 2      |
//! | 46    | sub_sel_add binary + ⊆ sel_arith              | 2      |
//! | 47    | sub_sel_sub binary + ⊆ sel_arith              | 2      |
//! | 48    | sub_sel_div binary + ⊆ sel_div_mod            | 2      |
//! | 49    | sub_sel_neg binary + ⊆ sel_unary              | 2      |
//! | 50    | sub_sel_felt_add binary + ⊆ sel_felt          | 2      |
//! | 51    | sub_sel_felt_sub binary + ⊆ sel_felt          | 2      |
//! | 52    | sub_sel_felt_mul binary + ⊆ sel_felt          | 2      |
//! | 53    | sub_sel_eq binary + ⊆ sel_cmp                 | 2      |
//! | 54    | sub_sel_neq binary + ⊆ sel_cmp                | 2      |
//! | 55    | arith mutual exclusion                        | 2      |
//! | 56    | felt mutual exclusion (add ⊥ sub)             | 2      |
//! | 57    | felt mutual exclusion (add ⊥ mul)             | 2      |
//! | 58    | felt mutual exclusion (sub ⊥ mul)             | 2      |
//! | 59    | cmp mutual exclusion (eq ⊥ neq)               | 2      |
//! | 60    | Output: arithmetic (add/sub/mul)              | 3      |
//! | 61    | Output: division/modulo identity              | 3      |
//! | 62    | Output: unary (neg/not)                       | 3      |
//! | 63    | Output: felt arithmetic                       | 3      |
//! | 64    | Comparison: out is binary                     | 3      |
//! | 65    | Comparison: not-equal branch                  | 3      |
//! | 66    | Comparison: equal branch                      | 4      |
//! | 67    | SP: heap write (net -2)                       | 2      |

use maat_bytecode::{
    SUB_SEL_ADD, SUB_SEL_DIV, SUB_SEL_EQ, SUB_SEL_FELT_ADD, SUB_SEL_FELT_MUL, SUB_SEL_FELT_SUB,
    SUB_SEL_NEG, SUB_SEL_NEQ, SUB_SEL_SUB,
};
use maat_trace::{
    COL_CMP_INV, COL_DIV_AUX, COL_FP, COL_IS_READ, COL_MEM_VAL, COL_NONZERO_INV, COL_OP_WIDTH,
    COL_OPERAND_0, COL_OUT, COL_PC, COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3, COL_RC_VAL, COL_S0,
    COL_S1, COL_SEL_BASE, COL_SP, COL_SUB_SEL_BASE, TRACE_WIDTH,
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
pub(crate) const SEL_HEAP_ALLOC: usize = 17;
pub(crate) const SEL_HEAP_READ: usize = 18;
pub(crate) const SEL_HEAP_WRITE: usize = 19;

/// Number of selector columns (must match `maat_trace::selector::NUM_SELECTORS`).
pub(crate) const NUM_SELECTORS: usize = 20;

/// Number of transition constraints enforced by the AIR.
pub const NUM_CONSTRAINTS: usize = 68;

/// Degree of each transition constraint, indexed by constraint number.
///
/// The prover uses these to allocate the correct evaluation domain size.
pub const CONSTRAINT_DEGREES: [usize; NUM_CONSTRAINTS] = [
    // 0-19: selector binary validity (20 selectors)
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // 20: selector sum
    1, // 21-25: SP updates
    2, 2, 2, 2, 2, // 26: universal PC
    2, // 27-28: width binding
    2, 2, // 29: unconditional jump
    2, // 30-31: conditional jump
    3, 3, // 32: cond jump s0 binary
    3, // 33-36: load/store
    2, 2, 2, 2, // 37-38: frame pointer
    2, 2, // 39-41: NOP padding (pc, sp, fp)
    2, 2, 2, // 42: RC reconstruction
    1, // 43: RC convert linking
    2, // 44: RC non-zero divisor
    3, // 45: NOP output frozen
    2, // 46-54: sub-selector structural (binary + ⊆ parent)
    2, 2, 2, 2, 2, 2, 2, 2, 2, // 55-59: mutual exclusion within sub-selector classes
    2, 2, 2, 2, 2, // 60-63: output correctness (arith, div_mod, unary, felt)
    3, 3, 3, 3, // 64-66: comparison correctness
    3, 3, 4, // 67: SP heap write
    2,
];

/// Reads a selector flag from the current row.
#[inline]
fn sel<E: FieldElement>(current: &[E], index: usize) -> E {
    current[COL_SEL_BASE + index]
}

/// Reads a sub-selector witness from the current row.
#[inline]
fn sub<E: FieldElement>(current: &[E], offset: usize) -> E {
    current[COL_SUB_SEL_BASE + offset]
}

/// Computes `2^16`, `2^32`, and `2^48` as field elements via repeated squaring.
#[inline]
fn power_of_two_constants<E: FieldElement>() -> (E, E, E) {
    let two = E::ONE + E::ONE;
    let p2 = two * two; // 2^2
    let p4 = p2 * p2; // 2^4
    let p8 = p4 * p4; // 2^8
    let p16 = p8 * p8; // 2^16
    let p32 = p16 * p16; // 2^32
    let p48 = p32 * p16; // 2^48
    (p16, p32, p48)
}

/// Evaluates all main-segment transition constraints.
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
    let s1 = current[COL_S1];
    let out = current[COL_OUT];
    let mem_val = current[COL_MEM_VAL];
    let is_read = current[COL_IS_READ];
    let op_width = current[COL_OP_WIDTH];
    let cmp_inv = current[COL_CMP_INV];
    let div_aux = current[COL_DIV_AUX];

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
    result[20] = sel_sum - one;
    result[21] = sel(current, SEL_PUSH) * (sp_next - sp - one);

    let sel_arith = sel(current, SEL_ARITH);
    let sel_bitwise = sel(current, SEL_BITWISE);
    let sel_cmp = sel(current, SEL_CMP);
    let sel_unary = sel(current, SEL_UNARY);
    let sel_load = sel(current, SEL_LOAD);
    let sel_store = sel(current, SEL_STORE);
    let sel_jump = sel(current, SEL_JUMP);
    let sel_cond_jump = sel(current, SEL_COND_JUMP);
    let sel_call = sel(current, SEL_CALL);
    let sel_return = sel(current, SEL_RETURN);
    let sel_convert = sel(current, SEL_CONVERT);
    let sel_felt = sel(current, SEL_FELT);
    let sel_div_mod = sel(current, SEL_DIV_MOD);
    let sel_nop = sel(current, SEL_NOP);
    let sel_heap_alloc = sel(current, SEL_HEAP_ALLOC);
    let sel_heap_read = sel(current, SEL_HEAP_READ);
    let sel_heap_write = sel(current, SEL_HEAP_WRITE);

    let sel_binop = sel_arith + sel_bitwise + sel_cmp + sel_div_mod;
    let two = one + one;
    result[22] = sel_binop * (sp_next - sp + one);
    // HeapAlloc and HeapRead each pop one operand and push one result, matching
    // the unary "net 0" SP pattern.
    result[23] = (sel_unary + sel_heap_alloc + sel_heap_read) * (sp_next - sp);
    result[24] = sel_store * (sp_next - sp + one);
    result[25] = sel_load * (sp_next - sp - one);

    let pc_uniform_gate = one - sel_jump - sel_cond_jump - sel_call - sel_return - sel_nop;
    result[26] = pc_uniform_gate * (pc_next - pc - op_width);

    let width_one_gate = sel_arith
        + sel_bitwise
        + sel_cmp
        + sel_unary
        + sel_felt
        + sel_div_mod
        + sel_heap_alloc
        + sel_heap_read
        + sel_heap_write;
    result[27] = width_one_gate * (op_width - one);

    result[28] = sel_convert * (op_width - two);

    result[29] = sel_jump * (pc_next - operand_0);
    let three = two + one;
    result[30] = sel_cond_jump * s0 * (pc_next - pc - three);
    result[31] = sel_cond_jump * (one - s0) * (pc_next - operand_0);
    result[32] = sel_cond_jump * s0 * (one - s0);

    result[33] = sel_load * (is_read - one);
    result[34] = sel_load * (out - mem_val);
    result[35] = sel_store * is_read;
    result[36] = sel_store * (mem_val - s0);

    result[37] = sel_call * (fp_next - out);
    result[38] = sel_return * (fp_next - mem_val);

    result[39] = sel_nop * (pc_next - pc);
    result[40] = sel_nop * (sp_next - sp);
    result[41] = sel_nop * (fp_next - fp);

    let rc_val = current[COL_RC_VAL];
    let l0 = current[COL_RC_L0];
    let l1 = current[COL_RC_L1];
    let l2 = current[COL_RC_L2];
    let l3 = current[COL_RC_L3];

    let (p16, p32, p48) = power_of_two_constants::<E>();
    result[42] = rc_val - (l0 + p16 * l1 + p32 * l2 + p48 * l3);
    result[43] = sel_convert * (rc_val - out);

    let nonzero_inv = current[COL_NONZERO_INV];
    result[44] = sel_div_mod * (s0 * nonzero_inv - one);

    let out_next = next[COL_OUT];
    result[45] = sel_nop * (out_next - out);

    let sub_add = sub(current, SUB_SEL_ADD);
    let sub_sub = sub(current, SUB_SEL_SUB);
    let sub_div = sub(current, SUB_SEL_DIV);
    let sub_neg = sub(current, SUB_SEL_NEG);
    let sub_felt_add = sub(current, SUB_SEL_FELT_ADD);
    let sub_felt_sub = sub(current, SUB_SEL_FELT_SUB);
    let sub_felt_mul = sub(current, SUB_SEL_FELT_MUL);
    let sub_eq = sub(current, SUB_SEL_EQ);
    let sub_neq = sub(current, SUB_SEL_NEQ);

    result[46] = sub_add * (sub_add - sel_arith);
    result[47] = sub_sub * (sub_sub - sel_arith);
    result[48] = sub_div * (sub_div - sel_div_mod);
    result[49] = sub_neg * (sub_neg - sel_unary);
    result[50] = sub_felt_add * (sub_felt_add - sel_felt);
    result[51] = sub_felt_sub * (sub_felt_sub - sel_felt);
    result[52] = sub_felt_mul * (sub_felt_mul - sel_felt);
    result[53] = sub_eq * (sub_eq - sel_cmp);
    result[54] = sub_neq * (sub_neq - sel_cmp);

    result[55] = sub_add * sub_sub;
    result[56] = sub_felt_add * sub_felt_sub;
    result[57] = sub_felt_add * sub_felt_mul;
    result[58] = sub_felt_sub * sub_felt_mul;
    result[59] = sub_eq * sub_neq;

    let sub_mul = sel_arith - sub_add - sub_sub;
    let sub_mod = sel_div_mod - sub_div;
    let sub_not = sel_unary - sub_neg;

    result[60] = sub_add * (out - s0 - s1) + sub_sub * (out - s1 + s0) + sub_mul * (out - s0 * s1);
    result[61] = sub_div * (s1 - s0 * out - div_aux) + sub_mod * (s1 - s0 * div_aux - out);
    result[62] = sub_neg * (out + s0) + sub_not * (out + s0 - one);
    result[63] = sub_felt_add * (out - s0 - s1)
        + sub_felt_sub * (out - s1 + s0)
        + sub_felt_mul * (out - s0 * s1);

    let cmp_active = sub_eq + sub_neq;
    let diff = s0 - s1;
    let one_minus_diff_inv = one - diff * cmp_inv;

    result[64] = cmp_active * out * (one - out);
    result[65] = diff * (sub_eq * out + sub_neq * (one - out));
    result[66] = one_minus_diff_inv * (sub_eq * (one - out) + sub_neq * out);

    // HeapWrite pops two operands (heap address, value) and pushes nothing.
    result[67] = sel_heap_write * (sp_next - sp + two);
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
        let result = eval(&current, &next);
        assert_ne!(result[20], F::ZERO, "zero selector sum should fail");

        let mut current = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_SEL_BASE + SEL_PUSH] = F::ONE;
        let result = eval(&current, &next);
        assert_ne!(result[20], F::ZERO, "double selector should fail sum check");
    }

    #[test]
    fn pc_universal_uses_op_width() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_ARITH] = F::ONE;
        current[COL_OP_WIDTH] = F::ONE;
        current[COL_SUB_SEL_BASE + SUB_SEL_ADD] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_PC] = F::new(10);

        next[COL_PC] = F::new(11);
        let result = eval(&current, &next);
        assert_eq!(result[26], F::ZERO);
        assert_eq!(result[27], F::ZERO);

        current[COL_OP_WIDTH] = F::new(2);
        let result = eval(&current, &next);
        assert_ne!(result[27], F::ZERO);
    }

    #[test]
    fn pc_convert_width_binding() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_CONVERT] = F::ONE;
        current[COL_OP_WIDTH] = F::new(2);
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;
        current[COL_PC] = F::new(10);
        next[COL_PC] = F::new(12);

        let val = F::new(255);
        current[COL_OUT] = val;
        current[COL_RC_VAL] = val;
        current[COL_RC_L0] = val;

        let result = eval(&current, &next);
        assert_eq!(result[26], F::ZERO);
        assert_eq!(result[28], F::ZERO);

        current[COL_OP_WIDTH] = F::new(3);
        let result = eval(&current, &next);
        assert_ne!(result[28], F::ZERO);
    }

    #[test]
    fn arith_output_constraint_add() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_ARITH] = F::ONE;
        current[COL_SUB_SEL_BASE + SUB_SEL_ADD] = F::ONE;
        current[COL_OP_WIDTH] = F::ONE;
        current[COL_S0] = F::new(7);
        current[COL_S1] = F::new(35);
        current[COL_OUT] = F::new(42);
        current[COL_PC] = F::new(0);
        next[COL_PC] = F::new(1);
        current[COL_SP] = F::new(2);
        next[COL_SP] = F::new(1);
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;

        let result = eval(&current, &next);
        assert_eq!(result[60], F::ZERO, "add output should pass for 7+35=42");

        // Tamper output.
        current[COL_OUT] = F::new(43);
        let result = eval(&current, &next);
        assert_ne!(result[60], F::ZERO, "wrong out must be rejected");
    }

    #[test]
    fn arith_output_constraint_mul_via_derivation() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_ARITH] = F::ONE;
        current[COL_OP_WIDTH] = F::ONE;
        current[COL_S0] = F::new(6);
        current[COL_S1] = F::new(7);
        current[COL_OUT] = F::new(42);
        current[COL_PC] = F::new(0);
        next[COL_PC] = F::new(1);
        current[COL_SP] = F::new(2);
        next[COL_SP] = F::new(1);
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;

        let result = eval(&current, &next);
        assert_eq!(result[60], F::ZERO, "mul output should pass for 6*7=42");

        current[COL_OUT] = F::new(41);
        let result = eval(&current, &next);
        assert_ne!(result[60], F::ZERO, "wrong mul out must be rejected");
    }

    #[test]
    fn unary_output_constraint_neg() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_UNARY] = F::ONE;
        current[COL_SUB_SEL_BASE + SUB_SEL_NEG] = F::ONE;
        current[COL_OP_WIDTH] = F::ONE;
        current[COL_S0] = F::new(5);
        current[COL_OUT] = -F::new(5); // out + s0 = 0
        current[COL_PC] = F::new(0);
        next[COL_PC] = F::new(1);
        current[COL_SP] = F::new(1);
        next[COL_SP] = F::new(1);
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;

        let result = eval(&current, &next);
        assert_eq!(result[62], F::ZERO);

        current[COL_OUT] = F::new(5);
        let result = eval(&current, &next);
        assert_ne!(result[62], F::ZERO);
    }

    #[test]
    fn equality_output_when_equal() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_CMP] = F::ONE;
        current[COL_SUB_SEL_BASE + SUB_SEL_EQ] = F::ONE;
        current[COL_OP_WIDTH] = F::ONE;
        current[COL_S0] = F::new(7);
        current[COL_S1] = F::new(7);
        current[COL_OUT] = F::ONE;
        current[COL_CMP_INV] = F::ZERO; // arbitrary on equal
        current[COL_PC] = F::new(0);
        next[COL_PC] = F::new(1);
        current[COL_SP] = F::new(2);
        next[COL_SP] = F::new(1);
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;

        let result = eval(&current, &next);
        assert_eq!(result[64], F::ZERO);
        assert_eq!(result[65], F::ZERO);
        assert_eq!(result[66], F::ZERO);

        // Tamper: claim out=0 when actually equal.
        current[COL_OUT] = F::ZERO;
        let result = eval(&current, &next);
        assert_ne!(result[66], F::ZERO);
    }

    #[test]
    fn equality_output_when_not_equal() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_CMP] = F::ONE;
        current[COL_SUB_SEL_BASE + SUB_SEL_EQ] = F::ONE;
        current[COL_OP_WIDTH] = F::ONE;
        current[COL_S0] = F::new(7);
        current[COL_S1] = F::new(3);
        current[COL_OUT] = F::ZERO;
        let diff = F::new(7) - F::new(3);
        current[COL_CMP_INV] = diff.inv();
        current[COL_PC] = F::new(0);
        next[COL_PC] = F::new(1);
        current[COL_SP] = F::new(2);
        next[COL_SP] = F::new(1);
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;

        let result = eval(&current, &next);
        assert_eq!(result[64], F::ZERO);
        assert_eq!(result[65], F::ZERO);
        assert_eq!(result[66], F::ZERO);

        // Tamper: claim out=1 when actually not equal.
        current[COL_OUT] = F::ONE;
        let result = eval(&current, &next);
        assert_ne!(result[65], F::ZERO);
    }

    #[test]
    fn inequality_output_when_not_equal() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_CMP] = F::ONE;
        current[COL_SUB_SEL_BASE + SUB_SEL_NEQ] = F::ONE;
        current[COL_OP_WIDTH] = F::ONE;
        current[COL_S0] = F::new(7);
        current[COL_S1] = F::new(3);
        current[COL_OUT] = F::ONE;
        let diff = F::new(7) - F::new(3);
        current[COL_CMP_INV] = diff.inv();
        current[COL_PC] = F::new(0);
        next[COL_PC] = F::new(1);
        current[COL_SP] = F::new(2);
        next[COL_SP] = F::new(1);
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;

        let result = eval(&current, &next);
        assert_eq!(result[64], F::ZERO);
        assert_eq!(result[65], F::ZERO);
        assert_eq!(result[66], F::ZERO);

        current[COL_OUT] = F::ZERO;
        let result = eval(&current, &next);
        assert_ne!(result[65], F::ZERO);
    }

    #[test]
    fn div_mod_identity_for_div() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_DIV_MOD] = F::ONE;
        current[COL_SUB_SEL_BASE + SUB_SEL_DIV] = F::ONE;
        current[COL_OP_WIDTH] = F::ONE;
        current[COL_S0] = F::new(7); // divisor
        current[COL_S1] = F::new(100); // dividend
        current[COL_OUT] = F::new(14); // quotient
        current[COL_DIV_AUX] = F::new(2); // remainder
        current[COL_NONZERO_INV] = F::new(7).inv();
        current[COL_PC] = F::new(0);
        next[COL_PC] = F::new(1);
        current[COL_SP] = F::new(2);
        next[COL_SP] = F::new(1);
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;

        let result = eval(&current, &next);
        assert_eq!(result[61], F::ZERO, "100 = 7*14 + 2");

        current[COL_OUT] = F::new(15);
        let result = eval(&current, &next);
        assert_ne!(result[61], F::ZERO);
    }

    #[test]
    fn sub_selector_outside_class_must_be_zero() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        // PUSH row but with sub_add set.
        current[COL_SEL_BASE + SEL_PUSH] = F::ONE;
        current[COL_SUB_SEL_BASE + SUB_SEL_ADD] = F::ONE;
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;

        let result = eval(&current, &next);
        assert_ne!(
            result[46],
            F::ZERO,
            "sub_add must be zero when sel_arith = 0"
        );
    }

    #[test]
    fn heap_write_sp_net_minus_two() {
        let mut current = [F::ZERO; TRACE_WIDTH];
        let mut next = [F::ZERO; TRACE_WIDTH];
        current[COL_SEL_BASE + SEL_HEAP_WRITE] = F::ONE;
        current[COL_OP_WIDTH] = F::ONE;
        current[COL_SP] = F::new(5);
        next[COL_SP] = F::new(3);
        next[COL_SEL_BASE + SEL_NOP] = F::ONE;

        let result = eval(&current, &next);
        assert_eq!(result[67], F::ZERO, "heap write must drop SP by exactly 2");

        next[COL_SP] = F::new(4);
        let result = eval(&current, &next);
        assert_ne!(
            result[67],
            F::ZERO,
            "heap write SP delta -1 must be rejected"
        );
    }
}
