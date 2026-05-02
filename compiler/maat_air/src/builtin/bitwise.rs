//! Bitwise builtin segment.

use maat_bytecode::selector::{
    SEL_BITWISE, SUB_SEL_AND, SUB_SEL_OR, SUB_SEL_SHL, SUB_SEL_SHR, SUB_SEL_XOR,
};
use maat_field::{BaseElement, ExtensionOf, FieldElement};
use maat_trace::table::{COL_OUT, COL_S0, COL_S1, COL_SEL_BASE, COL_SUB_SEL_BASE};
use winter_air::Assertion;

use super::Builtin;

/// Number of bits decomposed per operand.
const NUM_BITS: usize = 64;

/// Aux column offset (within this builtin): first column of the bit-`a` slice.
const BIT_A_BASE: usize = 0;
/// Aux column offset: first column of the bit-`b` slice.
const BIT_B_BASE: usize = NUM_BITS;

#[derive(Clone, Copy, Debug, Default)]
pub struct BitwiseBuiltin;

impl BitwiseBuiltin {
    pub const NAME: &'static str = "bitwise";

    pub const AUX_WIDTH: usize = 2 * NUM_BITS;

    pub const NUM_AUX_RANDS: usize = 0;

    pub const NUM_AUX_CONSTRAINTS: usize = 11;

    pub const NUM_AUX_ASSERTIONS: usize = 0;

    pub const AUX_CONSTRAINT_DEGREES: &'static [usize] = &[2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3];

    pub const RESERVED_ADDRESS_RANGE: (u64, u64) = (1u64 << 35, (1u64 << 36) - 1);
}

/// Returns the row's selector flag at `offset` from `COL_SEL_BASE`.
#[inline]
fn sel<F: FieldElement>(main: &[F], offset: usize) -> F {
    main[COL_SEL_BASE + offset]
}

/// Returns the row's sub-selector flag at `offset` from `COL_SUB_SEL_BASE`.
#[inline]
fn sub<F: FieldElement>(main: &[F], offset: usize) -> F {
    main[COL_SUB_SEL_BASE + offset]
}

/// Decomposes `x` into [`NUM_BITS`] little-endian boolean bits.
fn bits_of(x: u64) -> [BaseElement; NUM_BITS] {
    std::array::from_fn(|i| BaseElement::new((x >> i) & 1))
}

/// Returns a one-hot encoding of `index` of length [`NUM_BITS`].
fn encode(index: u64) -> [BaseElement; NUM_BITS] {
    std::array::from_fn(|i| {
        if i as u64 == index {
            BaseElement::ONE
        } else {
            BaseElement::ZERO
        }
    })
}

/// Computes `2^i` for `i in 0..NUM_BITS` as field elements.
#[inline]
fn pow2_table<E: FieldElement>() -> [E; NUM_BITS] {
    let mut table = [E::ZERO; NUM_BITS];
    let mut acc = E::ONE;
    for slot in &mut table {
        *slot = acc;
        acc = acc + acc;
    }
    table
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

    fn aux_constraint_degrees(&self) -> &'static [usize] {
        Self::AUX_CONSTRAINT_DEGREES
    }

    fn reserved_address_range(&self) -> (u64, u64) {
        Self::RESERVED_ADDRESS_RANGE
    }

    fn num_aux_assertions(&self) -> usize {
        Self::NUM_AUX_ASSERTIONS
    }

    fn evaluate_aux_transition<F, E>(
        &self,
        main_curr: &[F],
        _main_next: &[F],
        aux_curr: &[E],
        _aux_next: &[E],
        _rand_elements: &[E],
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
    {
        debug_assert_eq!(aux_curr.len(), Self::AUX_WIDTH);
        debug_assert_eq!(result.len(), Self::NUM_AUX_CONSTRAINTS);

        let one = E::ONE;
        let pow2: [E; NUM_BITS] = pow2_table();

        let bit_a: [E; NUM_BITS] = std::array::from_fn(|i| aux_curr[BIT_A_BASE + i]);
        let bit_b: [E; NUM_BITS] = std::array::from_fn(|i| aux_curr[BIT_B_BASE + i]);

        let s0 = E::from(main_curr[COL_S0]);
        let s1 = E::from(main_curr[COL_S1]);
        let out = E::from(main_curr[COL_OUT]);
        let sel_bw = E::from(sel(main_curr, SEL_BITWISE));
        let sub_and = E::from(sub(main_curr, SUB_SEL_AND));
        let sub_or = E::from(sub(main_curr, SUB_SEL_OR));
        let sub_xor = E::from(sub(main_curr, SUB_SEL_XOR));
        let sub_shl = E::from(sub(main_curr, SUB_SEL_SHL));
        let sub_shr = E::from(sub(main_curr, SUB_SEL_SHR));
        let sub_logical = sub_and + sub_or + sub_xor;
        let sub_shift = sub_shl + sub_shr;

        let mut bool_a = E::ZERO;
        for i in 0..NUM_BITS {
            bool_a += pow2[i] * bit_a[i] * (bit_a[i] - one);
        }
        result[0] = bool_a;

        let mut bool_b = E::ZERO;
        for i in 0..NUM_BITS {
            bool_b += pow2[i] * bit_b[i] * (bit_b[i] - one);
        }
        result[1] = bool_b;

        let mut recon_a = E::ZERO;
        for i in 0..NUM_BITS {
            recon_a += pow2[i] * bit_a[i];
        }
        result[2] = sel_bw * (s1 - recon_a);

        let mut recon_b = E::ZERO;
        for i in 0..NUM_BITS {
            recon_b += pow2[i] * bit_b[i];
        }
        result[3] = sub_logical * (s0 - recon_b);

        let one_hot_sum = bit_b.iter().copied().fold(E::ZERO, |acc, b| acc + b);
        result[4] = sub_shift * (one_hot_sum - one);

        let mut one_hot_index = E::ZERO;
        for (i, &b) in bit_b.iter().enumerate() {
            one_hot_index += E::from(BaseElement::new(i as u64)) * b;
        }
        result[5] = sub_shift * (s0 - one_hot_index);

        let mut and_out = E::ZERO;
        for i in 0..NUM_BITS {
            and_out += pow2[i] * bit_a[i] * bit_b[i];
        }
        result[6] = sub_and * (out - and_out);

        let mut or_out = E::ZERO;
        for i in 0..NUM_BITS {
            or_out += pow2[i] * (bit_a[i] + bit_b[i] - bit_a[i] * bit_b[i]);
        }
        result[7] = sub_or * (out - or_out);

        let two = one + one;
        let mut xor_out = E::ZERO;
        for i in 0..NUM_BITS {
            xor_out += pow2[i] * (bit_a[i] + bit_b[i] - two * bit_a[i] * bit_b[i]);
        }
        result[8] = sub_xor * (out - xor_out);

        let mut shl_out = E::ZERO;
        for k in 0..NUM_BITS {
            let mut partial = E::ZERO;
            for i in 0..(NUM_BITS - k) {
                partial += pow2[i + k] * bit_a[i];
            }
            shl_out += bit_b[k] * partial;
        }
        result[9] = sub_shl * (out - shl_out);

        let mut shr_out = E::ZERO;
        for k in 0..NUM_BITS {
            let mut partial = E::ZERO;
            for i in k..NUM_BITS {
                partial += pow2[i - k] * bit_a[i];
            }
            shr_out += bit_b[k] * partial;
        }
        result[10] = sub_shr * (out - shr_out);
    }

    fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_columns: &[&[BaseElement]],
        _rand_elements: &[E],
    ) -> Vec<Vec<E>> {
        let n = main_columns[COL_S0].len();
        let mut cols: Vec<Vec<E>> = (0..Self::AUX_WIDTH)
            .map(|_| Vec::with_capacity(n))
            .collect();

        let sel_bitwise_col = main_columns[COL_SEL_BASE + SEL_BITWISE];
        let sub_shl_col = main_columns[COL_SUB_SEL_BASE + SUB_SEL_SHL];
        let sub_shr_col = main_columns[COL_SUB_SEL_BASE + SUB_SEL_SHR];
        let s0_col = main_columns[COL_S0];
        let s1_col = main_columns[COL_S1];

        for row in 0..n {
            let is_bitwise = sel_bitwise_col[row] == BaseElement::ONE;
            let is_shift =
                sub_shl_col[row] == BaseElement::ONE || sub_shr_col[row] == BaseElement::ONE;

            let bit_a = if is_bitwise {
                bits_of(s1_col[row].as_int())
            } else {
                [BaseElement::ZERO; NUM_BITS]
            };

            let bit_b = if !is_bitwise {
                [BaseElement::ZERO; NUM_BITS]
            } else if is_shift {
                encode(s0_col[row].as_int())
            } else {
                bits_of(s0_col[row].as_int())
            };

            for (i, b) in bit_a.iter().enumerate() {
                cols[BIT_A_BASE + i].push(E::from(*b));
            }
            for (i, b) in bit_b.iter().enumerate() {
                cols[BIT_B_BASE + i].push(E::from(*b));
            }
        }

        cols
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
    use maat_trace::table::TRACE_WIDTH;

    use super::*;

    type F = BaseElement;

    /// Builds a minimal zeroed main row.
    fn empty_row() -> Vec<F> {
        vec![F::ZERO; TRACE_WIDTH]
    }

    fn evaluate_row(main: &[F], aux: &[F]) -> [F; BitwiseBuiltin::NUM_AUX_CONSTRAINTS] {
        let next = empty_row();
        let aux_next = vec![F::ZERO; BitwiseBuiltin::AUX_WIDTH];
        let mut result = [F::ZERO; BitwiseBuiltin::NUM_AUX_CONSTRAINTS];
        BitwiseBuiltin.evaluate_aux_transition::<F, F>(
            main,
            &next,
            aux,
            &aux_next,
            &[],
            &mut result,
        );
        result
    }

    fn populate_logical_row(a: u64, b: u64, out: u64, sub_offset: usize) -> (Vec<F>, Vec<F>) {
        let mut main = empty_row();
        main[COL_SEL_BASE + SEL_BITWISE] = F::ONE;
        main[COL_SUB_SEL_BASE + sub_offset] = F::ONE;
        main[COL_S1] = F::new(a);
        main[COL_S0] = F::new(b);
        main[COL_OUT] = F::new(out);

        let mut aux = vec![F::ZERO; BitwiseBuiltin::AUX_WIDTH];
        let bit_a = bits_of(a);
        let bit_b = bits_of(b);
        for (i, v) in bit_a.iter().enumerate() {
            aux[BIT_A_BASE + i] = *v;
        }
        for (i, v) in bit_b.iter().enumerate() {
            aux[BIT_B_BASE + i] = *v;
        }
        (main, aux)
    }

    fn populate_shift_row(a: u64, b: u64, out: u64, sub_offset: usize) -> (Vec<F>, Vec<F>) {
        let mut main = empty_row();
        main[COL_SEL_BASE + SEL_BITWISE] = F::ONE;
        main[COL_SUB_SEL_BASE + sub_offset] = F::ONE;
        main[COL_S1] = F::new(a);
        main[COL_S0] = F::new(b);
        main[COL_OUT] = F::new(out);

        let mut aux = vec![F::ZERO; BitwiseBuiltin::AUX_WIDTH];
        let bit_a = bits_of(a);
        let encoded_b = encode(b);
        for (i, v) in bit_a.iter().enumerate() {
            aux[BIT_A_BASE + i] = *v;
        }
        for (i, v) in encoded_b.iter().enumerate() {
            aux[BIT_B_BASE + i] = *v;
        }
        (main, aux)
    }

    #[test]
    fn and_passes_on_correct_witness() {
        let a = 0xCAFE_BABE_DEAD_BEEFu64;
        let b = 0x0F0F_0F0F_0F0F_0F0Fu64;
        let (main, aux) = populate_logical_row(a, b, a & b, SUB_SEL_AND);
        let result = evaluate_row(&main, &aux);
        for (i, r) in result.iter().enumerate() {
            assert_eq!(*r, F::ZERO, "constraint {i} non-zero on valid AND row");
        }
    }

    #[test]
    fn and_rejects_tampered_output() {
        let a = 0xFFFFu64;
        let b = 0x00FFu64;
        let (mut main, aux) = populate_logical_row(a, b, a & b, SUB_SEL_AND);
        main[COL_OUT] = F::new((a & b).wrapping_add(1));
        let result = evaluate_row(&main, &aux);
        assert_ne!(result[6], F::ZERO);
    }

    #[test]
    fn or_passes_on_correct_witness() {
        let a = 0xAAAA_AAAA_AAAA_AAAAu64;
        let b = 0x5555_5555_5555_5555u64;
        let (main, aux) = populate_logical_row(a, b, a | b, SUB_SEL_OR);
        let result = evaluate_row(&main, &aux);
        for (i, r) in result.iter().enumerate() {
            assert_eq!(*r, F::ZERO, "constraint {i} non-zero on valid OR row");
        }
    }

    #[test]
    fn xor_passes_on_correct_witness() {
        let a = 0x1234_5678_9ABC_DEF0u64;
        let b = 0xFEDC_BA98_7654_3210u64;
        let (main, aux) = populate_logical_row(a, b, a ^ b, SUB_SEL_XOR);
        let result = evaluate_row(&main, &aux);
        for (i, r) in result.iter().enumerate() {
            assert_eq!(*r, F::ZERO, "constraint {i} non-zero on valid XOR row");
        }
    }

    #[test]
    fn xor_rejects_tampered_output() {
        let a = 0xAAu64;
        let b = 0x55u64;
        let (mut main, aux) = populate_logical_row(a, b, a ^ b, SUB_SEL_XOR);
        main[COL_OUT] = F::new(0xFE);
        let result = evaluate_row(&main, &aux);
        assert_ne!(result[8], F::ZERO);
    }

    #[test]
    fn shl_passes_on_correct_witness() {
        let a = 0xCAFEu64;
        let b = 16u64;
        let out = a.wrapping_shl(b as u32);
        let (main, aux) = populate_shift_row(a, b, out, SUB_SEL_SHL);
        let result = evaluate_row(&main, &aux);
        for (i, r) in result.iter().enumerate() {
            assert_eq!(*r, F::ZERO, "constraint {i} non-zero on valid SHL row");
        }
    }

    #[test]
    fn shl_modular_wrap_passes() {
        let a = 0xFFFF_FFFF_FFFF_FFFFu64;
        let b = 4u64;
        let out = a.wrapping_shl(b as u32);
        let (main, aux) = populate_shift_row(a, b, out, SUB_SEL_SHL);
        let result = evaluate_row(&main, &aux);
        for (i, r) in result.iter().enumerate() {
            assert_eq!(*r, F::ZERO, "constraint {i} non-zero on wrapping SHL row");
        }
    }

    #[test]
    fn shl_rejects_tampered_output() {
        let a = 0x1u64;
        let b = 8u64;
        let out = a.wrapping_shl(b as u32);
        let (mut main, aux) = populate_shift_row(a, b, out, SUB_SEL_SHL);
        main[COL_OUT] = F::new(out.wrapping_add(1));
        let result = evaluate_row(&main, &aux);
        assert_ne!(result[9], F::ZERO);
    }

    #[test]
    fn shr_passes_on_correct_witness() {
        let a = 0xDEAD_BEEF_0000_0000u64;
        let b = 32u64;
        let out = a.wrapping_shr(b as u32);
        let (main, aux) = populate_shift_row(a, b, out, SUB_SEL_SHR);
        let result = evaluate_row(&main, &aux);
        for (i, r) in result.iter().enumerate() {
            assert_eq!(*r, F::ZERO, "constraint {i} non-zero on valid SHR row");
        }
    }

    #[test]
    fn shr_rejects_tampered_output() {
        let a = 0x100u64;
        let b = 4u64;
        let out = a.wrapping_shr(b as u32);
        let (mut main, aux) = populate_shift_row(a, b, out, SUB_SEL_SHR);
        main[COL_OUT] = F::new(out.wrapping_add(1));
        let result = evaluate_row(&main, &aux);
        assert_ne!(result[10], F::ZERO);
    }

    #[test]
    fn nop_row_passes_with_zero_aux() {
        let main = empty_row();
        let aux = vec![F::ZERO; BitwiseBuiltin::AUX_WIDTH];
        let result = evaluate_row(&main, &aux);
        for (i, r) in result.iter().enumerate() {
            assert_eq!(*r, F::ZERO, "constraint {i} non-zero on NOP row");
        }
    }
}
