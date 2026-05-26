//! Bitwise builtin segment.
//!
//! AND/OR/XOR are proven via the chunked diluted-form argument: each 64-bit
//! operation is split into 8 bytewise chunks across the primary AND/OR/XOR
//! row plus the 7 continuation rows emitted by the trace recorder
//! ([`SUB_SEL_CHUNK_ROW`]).
//!
//! SHL/SHR continue to use the [`POW_K_OFFSET`] aux cell, validated by the
//! pow2-paired LogUp pool. They are not chunked.

use maat_field::{BaseElement, ExtensionOf, FieldElement};
use maat_trace::selector::{
    SUB_SEL_AND, SUB_SEL_CHUNK_ROW, SUB_SEL_OR, SUB_SEL_SHL, SUB_SEL_SHR, SUB_SEL_XOR,
};
use maat_trace::table::{
    COL_CHUNK_A, COL_CHUNK_AND, COL_CHUNK_B, COL_CHUNK_OUT, COL_OUT, COL_RC_VAL, COL_S0, COL_S1,
    COL_SUB_SEL_BASE,
};
use winter_air::Assertion;

use super::Builtin;
use super::diluted::dilute;

/// Native byte width used by the diluted-form chunked argument.
const CHUNK_BITS: u64 = 8;

/// Field-element-friendly weight `2^CHUNK_BITS`.
const CHUNK_BASE: u64 = 1u64 << CHUNK_BITS;

/// Aux column offset: per-row spread-form of `chunk_a`.
pub const D_A_OFFSET: usize = 0;
/// Aux column offset: per-row spread-form of `chunk_b`.
pub const D_B_OFFSET: usize = 1;
/// Aux column offset: per-row spread-form of `chunk_a & chunk_b`.
pub const D_AND_OFFSET: usize = 2;
/// Aux column offset: per-row spread-form of `chunk_out`.
pub const D_OUT_OFFSET: usize = 3;
/// Aux column offset: accumulator reconstructing `s1` from
/// `chunk_a` bytes across the 8-row chunk sequence.
pub const ACC_S1_OFFSET: usize = 4;
/// Aux column offset: accumulator reconstructing `s0` from
/// `chunk_b` bytes across the 8-row chunk sequence.
pub const ACC_S0_OFFSET: usize = 5;
/// Aux column offset: accumulator reconstructing `out` from
/// `chunk_out` bytes across the 8-row chunk sequence.
pub const ACC_OUT_OFFSET: usize = 6;
/// Aux column offset: the `pow_k` witness consumed by the pow2-paired LogUp
/// pool on SHL/SHR rows.
pub const POW_K_OFFSET: usize = 7;

#[derive(Clone, Copy, Debug, Default)]
pub struct BitwiseBuiltin;

impl BitwiseBuiltin {
    pub const NAME: &'static str = "bitwise";

    const AUX_WIDTH: usize = 8;

    const NUM_AUX_RANDS: usize = 0;

    const NUM_AUX_CONSTRAINTS: usize = 12;

    const NUM_AUX_ASSERTIONS: usize = 0;

    const AUX_CONSTRAINT_DEGREES: &'static [usize] = &[2, 2, 3, 2, 3, 2, 3, 3, 3, 3, 2, 2];

    pub const RESERVED_ADDRESS_RANGE: (u64, u64) = (1u64 << 35, (1u64 << 36) - 1);
}

#[inline]
fn sub<F: FieldElement>(main: &[F], offset: usize) -> F {
    main[COL_SUB_SEL_BASE + offset]
}

impl Builtin for BitwiseBuiltin {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn aux_width(&self) -> usize {
        Self::AUX_WIDTH
    }

    fn num_aux_rands(&self) -> usize {
        Self::NUM_AUX_RANDS
    }

    fn num_aux_constraints(&self) -> usize {
        Self::NUM_AUX_CONSTRAINTS
    }

    fn num_aux_assertions(&self) -> usize {
        Self::NUM_AUX_ASSERTIONS
    }

    fn aux_constraint_degrees(&self) -> Vec<usize> {
        Self::AUX_CONSTRAINT_DEGREES.to_vec()
    }

    fn reserved_address_range(&self) -> (u64, u64) {
        Self::RESERVED_ADDRESS_RANGE
    }

    fn evaluate_aux_transition<F, E>(
        &self,
        main_curr: &[F],
        main_next: &[F],
        aux_curr: &[E],
        aux_next: &[E],
        base_offset: usize,
        _rand_elements: &[E],
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
    {
        debug_assert_eq!(result.len(), Self::NUM_AUX_CONSTRAINTS);

        let one = E::ONE;
        let two = one + one;
        let chunk_base = E::from(BaseElement::new(CHUNK_BASE));

        let local_curr = &aux_curr[base_offset..base_offset + Self::AUX_WIDTH];
        let local_next = &aux_next[base_offset..base_offset + Self::AUX_WIDTH];

        let d_a = local_curr[D_A_OFFSET];
        let d_b = local_curr[D_B_OFFSET];
        let d_and = local_curr[D_AND_OFFSET];
        let d_out = local_curr[D_OUT_OFFSET];

        let s0 = E::from(main_curr[COL_S0]);
        let s1 = E::from(main_curr[COL_S1]);
        let out = E::from(main_curr[COL_OUT]);
        let rc_val = E::from(main_curr[COL_RC_VAL]);

        let sub_and = E::from(sub(main_curr, SUB_SEL_AND));
        let sub_or = E::from(sub(main_curr, SUB_SEL_OR));
        let sub_xor = E::from(sub(main_curr, SUB_SEL_XOR));
        let sub_shl = E::from(sub(main_curr, SUB_SEL_SHL));
        let sub_shr = E::from(sub(main_curr, SUB_SEL_SHR));
        let sub_chunk_row = E::from(sub(main_curr, SUB_SEL_CHUNK_ROW));

        let chunk_active_curr = sub_and + sub_or + sub_xor;

        let sub_and_next = E::from(sub(main_next, SUB_SEL_AND));
        let sub_or_next = E::from(sub(main_next, SUB_SEL_OR));
        let sub_xor_next = E::from(sub(main_next, SUB_SEL_XOR));
        let chunk_active_next = sub_and_next + sub_or_next + sub_xor_next;

        result[0] = sub_and * (d_out - d_and)
            + sub_or * (d_out - d_a - d_b + d_and)
            + sub_xor * (d_out - d_a - d_b + two * d_and);

        let chunk_a_next = E::from(main_next[COL_CHUNK_A]);
        let chunk_b_next = E::from(main_next[COL_CHUNK_B]);
        let chunk_out_next = E::from(main_next[COL_CHUNK_OUT]);

        let acc_s1_curr = local_curr[ACC_S1_OFFSET];
        let acc_s1_next = local_next[ACC_S1_OFFSET];
        let acc_s0_curr = local_curr[ACC_S0_OFFSET];
        let acc_s0_next = local_next[ACC_S0_OFFSET];
        let acc_out_curr = local_curr[ACC_OUT_OFFSET];
        let acc_out_next = local_next[ACC_OUT_OFFSET];

        let one_minus_active_next = one - chunk_active_next;

        result[1] = one_minus_active_next * acc_s1_next;
        result[2] = chunk_active_next
            * (acc_s1_next - chunk_base * chunk_active_curr * acc_s1_curr - chunk_a_next);

        result[3] = one_minus_active_next * acc_s0_next;
        result[4] = chunk_active_next
            * (acc_s0_next - chunk_base * chunk_active_curr * acc_s0_curr - chunk_b_next);

        result[5] = one_minus_active_next * acc_out_next;
        result[6] = chunk_active_next
            * (acc_out_next - chunk_base * chunk_active_curr * acc_out_curr - chunk_out_next);

        let is_primary = chunk_active_curr * (one - sub_chunk_row);
        result[7] = is_primary * (acc_s1_curr - s1);
        result[8] = is_primary * (acc_s0_curr - s0);
        result[9] = is_primary * (acc_out_curr - out);

        let pow_k = local_curr[POW_K_OFFSET];
        let two_32_minus_1 = E::from(BaseElement::new((1u64 << 32) - 1));
        result[10] = sub_shl * (s1 * pow_k - out - rc_val * two_32_minus_1);
        result[11] = sub_shr * (s1 - out * pow_k - rc_val);
    }

    fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_columns: &[&[BaseElement]],
        _rand_elements: &[E],
    ) -> Vec<Vec<E>> {
        let n = main_columns[COL_S0].len();
        let zero = E::ZERO;
        let chunk_base = E::from(BaseElement::new(CHUNK_BASE));

        let chunk_a_col = main_columns[COL_CHUNK_A];
        let chunk_b_col = main_columns[COL_CHUNK_B];
        let chunk_and_col = main_columns[COL_CHUNK_AND];
        let chunk_out_col = main_columns[COL_CHUNK_OUT];
        let sub_and_col = main_columns[COL_SUB_SEL_BASE + SUB_SEL_AND];
        let sub_or_col = main_columns[COL_SUB_SEL_BASE + SUB_SEL_OR];
        let sub_xor_col = main_columns[COL_SUB_SEL_BASE + SUB_SEL_XOR];
        let sub_shl_col = main_columns[COL_SUB_SEL_BASE + SUB_SEL_SHL];
        let sub_shr_col = main_columns[COL_SUB_SEL_BASE + SUB_SEL_SHR];
        let s0_col = main_columns[COL_S0];

        let chunk_active = |row: usize| -> bool {
            sub_and_col[row] == BaseElement::ONE
                || sub_or_col[row] == BaseElement::ONE
                || sub_xor_col[row] == BaseElement::ONE
        };

        let mut d_a = vec![zero; n];
        let mut d_b = vec![zero; n];
        let mut d_and = vec![zero; n];
        let mut d_out = vec![zero; n];
        let mut acc_s1 = vec![zero; n];
        let mut acc_s0 = vec![zero; n];
        let mut acc_out = vec![zero; n];
        let mut pow_k = vec![zero; n];

        for row in 0..n {
            if chunk_active(row) {
                d_a[row] = E::from(BaseElement::new(dilute(chunk_a_col[row].as_int())));
                d_b[row] = E::from(BaseElement::new(dilute(chunk_b_col[row].as_int())));
                d_and[row] = E::from(BaseElement::new(dilute(chunk_and_col[row].as_int())));
                d_out[row] = E::from(BaseElement::new(dilute(chunk_out_col[row].as_int())));

                let prev_acc_s1 = if row > 0 && chunk_active(row - 1) {
                    acc_s1[row - 1]
                } else {
                    zero
                };
                let prev_acc_s0 = if row > 0 && chunk_active(row - 1) {
                    acc_s0[row - 1]
                } else {
                    zero
                };
                let prev_acc_out = if row > 0 && chunk_active(row - 1) {
                    acc_out[row - 1]
                } else {
                    zero
                };
                acc_s1[row] = chunk_base * prev_acc_s1 + E::from(chunk_a_col[row]);
                acc_s0[row] = chunk_base * prev_acc_s0 + E::from(chunk_b_col[row]);
                acc_out[row] = chunk_base * prev_acc_out + E::from(chunk_out_col[row]);
            }

            let is_shift =
                sub_shl_col[row] == BaseElement::ONE || sub_shr_col[row] == BaseElement::ONE;
            if is_shift {
                let s0 = s0_col[row].as_int();
                if s0 < 64 {
                    pow_k[row] = E::from(BaseElement::new(1u64 << s0));
                }
            }
        }

        vec![d_a, d_b, d_and, d_out, acc_s1, acc_s0, acc_out, pow_k]
    }

    fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
        &self,
        _column_base: usize,
        _last_step: usize,
    ) -> Vec<Assertion<E>> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use maat_trace::selector::{SEL_BITWISE, SEL_NOP};
    use maat_trace::table::{COL_SEL_BASE, TRACE_WIDTH};

    use super::*;
    use crate::builtin::diluted::CHUNKS_PER_OPERAND;

    type F = BaseElement;

    fn empty_row() -> Vec<F> {
        vec![F::ZERO; TRACE_WIDTH]
    }

    fn evaluate_pair(
        main_curr: &[F],
        main_next: &[F],
        aux_curr: &[F],
        aux_next: &[F],
    ) -> [F; BitwiseBuiltin::NUM_AUX_CONSTRAINTS] {
        let mut result = [F::ZERO; BitwiseBuiltin::NUM_AUX_CONSTRAINTS];
        BitwiseBuiltin.evaluate_aux_transition::<F, F>(
            main_curr,
            main_next,
            aux_curr,
            aux_next,
            0,
            &[],
            &mut result,
        );
        result
    }

    fn build_chunk_sequence(
        sub_op: usize,
        s1_val: u64,
        s0_val: u64,
        out_val: u64,
    ) -> (Vec<Vec<F>>, Vec<Vec<F>>) {
        let n = CHUNKS_PER_OPERAND + 2;
        let mut main = (0..TRACE_WIDTH)
            .map(|_| vec![F::ZERO; n])
            .collect::<Vec<Vec<F>>>();

        for k in (1..=7).rev() {
            let row = 1 + (7 - k);
            let shift = (k as u64) * CHUNK_BITS;
            main[COL_SEL_BASE + SEL_NOP][row] = F::ONE;
            main[COL_SUB_SEL_BASE + SUB_SEL_CHUNK_ROW][row] = F::ONE;
            main[COL_SUB_SEL_BASE + sub_op][row] = F::ONE;
            main[COL_CHUNK_A][row] = F::new((s1_val >> shift) & 0xff);
            main[COL_CHUNK_B][row] = F::new((s0_val >> shift) & 0xff);
            main[COL_CHUNK_AND][row] = F::new(((s1_val & s0_val) >> shift) & 0xff);
            main[COL_CHUNK_OUT][row] = F::new((out_val >> shift) & 0xff);
        }

        let primary = n - 2;
        main[COL_SEL_BASE + SEL_BITWISE][primary] = F::ONE;
        main[COL_SUB_SEL_BASE + sub_op][primary] = F::ONE;
        main[COL_S1][primary] = F::new(s1_val);
        main[COL_S0][primary] = F::new(s0_val);
        main[COL_OUT][primary] = F::new(out_val);
        main[COL_CHUNK_A][primary] = F::new(s1_val & 0xff);
        main[COL_CHUNK_B][primary] = F::new(s0_val & 0xff);
        main[COL_CHUNK_AND][primary] = F::new((s1_val & s0_val) & 0xff);
        main[COL_CHUNK_OUT][primary] = F::new(out_val & 0xff);

        let trailing = n - 1;
        main[COL_SEL_BASE + SEL_NOP][trailing] = F::ONE;

        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();
        let aux_cols = BitwiseBuiltin.build_aux_columns::<F>(&main_slices, &[]);
        let aux = aux_cols.into_iter().collect::<Vec<Vec<F>>>();

        (main, aux)
    }

    fn row_at(columns: &[Vec<F>], row: usize) -> Vec<F> {
        columns.iter().map(|c| c[row]).collect()
    }

    fn chunk_sequence_passes(sub_op: usize, s1_val: u64, s0_val: u64, out_val: u64) {
        let (main, aux) = build_chunk_sequence(sub_op, s1_val, s0_val, out_val);
        let n = main[0].len();
        for i in 0..n - 1 {
            let main_curr = row_at(&main, i);
            let main_next = row_at(&main, i + 1);
            let aux_curr = row_at(&aux, i);
            let aux_next = row_at(&aux, i + 1);
            let result = evaluate_pair(&main_curr, &main_next, &aux_curr, &aux_next);
            for (j, r) in result.iter().enumerate() {
                assert_eq!(
                    *r,
                    F::ZERO,
                    "constraint {j} non-zero at chunk-sequence transition {i} -> {}",
                    i + 1,
                );
            }
        }
    }

    #[test]
    fn and_chunk_sequence_passes_on_correct_witness() {
        let a = 0xCAFE_BABE_DEAD_BEEFu64;
        let b = 0x0F0F_0F0F_0F0F_0F0Fu64;
        chunk_sequence_passes(SUB_SEL_AND, a, b, a & b);
    }

    #[test]
    fn or_chunk_sequence_passes_on_correct_witness() {
        let a = 0xAAAA_AAAA_AAAA_AAAAu64;
        let b = 0x5555_5555_5555_5555u64;
        chunk_sequence_passes(SUB_SEL_OR, a, b, a | b);
    }

    #[test]
    fn xor_chunk_sequence_passes_on_correct_witness() {
        let a = 0x1234_5678_9ABC_DEF0u64;
        let b = 0xFEDC_BA98_7654_3210u64;
        chunk_sequence_passes(SUB_SEL_XOR, a, b, a ^ b);
    }

    #[test]
    fn and_tampered_chunk_out_breaks_reconstruction() {
        let a = 0xFFFFu64;
        let b = 0x00FFu64;
        let (mut main, _) = build_chunk_sequence(SUB_SEL_AND, a, b, a & b);
        let n = main[0].len();
        let primary = n - 2;
        main[COL_CHUNK_OUT][primary] = F::new(((a & b) & 0xff).wrapping_add(1));
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();
        let aux_rebuilt = BitwiseBuiltin.build_aux_columns::<F>(&main_slices, &[]);
        let main_curr = row_at(&main, primary);
        let main_next = row_at(&main, primary + 1);
        let aux_curr = row_at(&aux_rebuilt, primary);
        let aux_next = row_at(&aux_rebuilt, primary + 1);
        let result = evaluate_pair(&main_curr, &main_next, &aux_curr, &aux_next);
        assert_ne!(
            result[9],
            F::ZERO,
            "tampered chunk_out must break the ACC_OUT = out reconstruction at primary",
        );
    }

    #[test]
    fn xor_tampered_chunk_and_breaks_dilute_identity() {
        let a = 0x1234_5678_9ABC_DEF0u64;
        let b = 0xFEDC_BA98_7654_3210u64;
        let (mut main, _) = build_chunk_sequence(SUB_SEL_XOR, a, b, a ^ b);
        let n = main[0].len();
        let primary = n - 2;
        main[COL_CHUNK_AND][primary] = F::new(((a & b) & 0xff).wrapping_add(1));
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();
        let aux = BitwiseBuiltin.build_aux_columns::<F>(&main_slices, &[]);
        let main_curr = row_at(&main, primary);
        let main_next = row_at(&main, primary + 1);
        let aux_curr = row_at(&aux, primary);
        let aux_next = row_at(&aux, primary + 1);
        let result = evaluate_pair(&main_curr, &main_next, &aux_curr, &aux_next);
        assert_ne!(
            result[0],
            F::ZERO,
            "tampered chunk_and must break the diluted-form identity on the primary XOR row",
        );
    }

    #[test]
    fn shl_passes_on_correct_witness() {
        let a = 0xCAFEu64;
        let b = 16u64;
        let out = a.wrapping_shl(b as u32);
        let mut main = empty_row();
        main[COL_SUB_SEL_BASE + SUB_SEL_SHL] = F::ONE;
        main[COL_S1] = F::new(a);
        main[COL_S0] = F::new(b);
        main[COL_OUT] = F::new(out);
        main[COL_RC_VAL] = F::new(if b == 0 {
            0
        } else if b < 64 {
            a >> (64 - b)
        } else {
            a
        });
        let mut aux = vec![F::ZERO; BitwiseBuiltin::AUX_WIDTH];
        aux[POW_K_OFFSET] = F::new(if b < 64 { 1u64 << b } else { 0 });
        let next = empty_row();
        let aux_next = vec![F::ZERO; BitwiseBuiltin::AUX_WIDTH];
        let result = evaluate_pair(&main, &next, &aux, &aux_next);
        for (i, r) in result.iter().enumerate() {
            assert_eq!(*r, F::ZERO, "constraint {i} non-zero on valid SHL row");
        }
    }

    #[test]
    fn shl_rejects_tampered_output() {
        let a = 0x1u64;
        let b = 8u64;
        let out = a.wrapping_shl(b as u32);
        let mut main = empty_row();
        main[COL_SUB_SEL_BASE + SUB_SEL_SHL] = F::ONE;
        main[COL_S1] = F::new(a);
        main[COL_S0] = F::new(b);
        main[COL_OUT] = F::new(out.wrapping_add(1));
        let mut aux = vec![F::ZERO; BitwiseBuiltin::AUX_WIDTH];
        aux[POW_K_OFFSET] = F::new(1u64 << b);
        let result = evaluate_pair(
            &main,
            &empty_row(),
            &aux,
            &[F::ZERO; BitwiseBuiltin::AUX_WIDTH],
        );
        assert_ne!(result[10], F::ZERO);
    }

    #[test]
    fn shr_passes_on_correct_witness() {
        let a = 0xDEAD_BEEF_0000_0000u64;
        let b = 32u64;
        let out = a.wrapping_shr(b as u32);
        let mut main = empty_row();
        main[COL_SUB_SEL_BASE + SUB_SEL_SHR] = F::ONE;
        main[COL_S1] = F::new(a);
        main[COL_S0] = F::new(b);
        main[COL_OUT] = F::new(out);
        main[COL_RC_VAL] = F::new(a & ((1u64 << b) - 1));
        let mut aux = vec![F::ZERO; BitwiseBuiltin::AUX_WIDTH];
        aux[POW_K_OFFSET] = F::new(1u64 << b);
        let result = evaluate_pair(
            &main,
            &empty_row(),
            &aux,
            &[F::ZERO; BitwiseBuiltin::AUX_WIDTH],
        );
        for (i, r) in result.iter().enumerate() {
            assert_eq!(*r, F::ZERO, "constraint {i} non-zero on valid SHR row");
        }
    }

    #[test]
    fn shr_rejects_tampered_output() {
        let a = 0x100u64;
        let b = 4u64;
        let out = a.wrapping_shr(b as u32);
        let mut main = empty_row();
        main[COL_SUB_SEL_BASE + SUB_SEL_SHR] = F::ONE;
        main[COL_S1] = F::new(a);
        main[COL_S0] = F::new(b);
        main[COL_OUT] = F::new(out.wrapping_add(1));
        let mut aux = vec![F::ZERO; BitwiseBuiltin::AUX_WIDTH];
        aux[POW_K_OFFSET] = F::new(1u64 << b);
        let result = evaluate_pair(
            &main,
            &empty_row(),
            &aux,
            &[F::ZERO; BitwiseBuiltin::AUX_WIDTH],
        );
        assert_ne!(result[11], F::ZERO);
    }

    #[test]
    fn nop_row_passes_with_zero_aux() {
        let main = empty_row();
        let aux = vec![F::ZERO; BitwiseBuiltin::AUX_WIDTH];
        let result = evaluate_pair(&main, &main, &aux, &aux);
        for (i, r) in result.iter().enumerate() {
            assert_eq!(*r, F::ZERO, "constraint {i} non-zero on NOP row");
        }
    }
}
