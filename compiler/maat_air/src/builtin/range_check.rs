//! Range-check builtin segment.
//!
//! Owns the per-row byte witnesses that decompose `COL_RC_L0..COL_RC_L3`
//! and the four degree-1 limb-decomposition constraints `limb_k = 256*hi + lo`.

use maat_field::{BaseElement, ExtensionOf, FieldElement};
use maat_trace::table::{COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3};
use winter_air::Assertion;

use super::Builtin;

/// LogUp lookup table size (8-bit byte table `{0, ..., 255}`).
pub const TABLE_SIZE: usize = 256;

/// Number of byte channels per row (4 main-trace limbs x 2 bytes each).
pub const NUM_CHANNELS: usize = 8;

/// Aux column offset (within this builtin's slice): limb-0 high byte.
pub const RC_B0_HI: usize = 0;
/// Aux column offset: limb-0 low byte.
pub const RC_B0_LO: usize = 1;
/// Aux column offset: limb-1 high byte.
pub const RC_B1_HI: usize = 2;
/// Aux column offset: limb-1 low byte.
pub const RC_B1_LO: usize = 3;
/// Aux column offset: limb-2 high byte.
pub const RC_B2_HI: usize = 4;
/// Aux column offset: limb-2 low byte.
pub const RC_B2_LO: usize = 5;
/// Aux column offset: limb-3 high byte.
pub const RC_B3_HI: usize = 6;
/// Aux column offset: limb-3 low byte.
pub const RC_B3_LO: usize = 7;

const LIMB_COLS: [usize; 4] = [COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3];

const LIMB_BYTE_PAIRS: [(usize, usize); 4] = [
    (RC_B0_HI, RC_B0_LO),
    (RC_B1_HI, RC_B1_LO),
    (RC_B2_HI, RC_B2_LO),
    (RC_B3_HI, RC_B3_LO),
];

#[derive(Clone, Copy, Debug, Default)]
pub struct RangeCheckBuiltin;

impl RangeCheckBuiltin {
    pub const NAME: &'static str = "range_check";

    /// 8 byte witness columns, one per `(hi, lo)` pair across 4 limbs.
    const AUX_WIDTH: usize = 2 * 4;

    const NUM_AUX_RANDS: usize = 0;

    /// 4 limb-decomposition (degree 1)
    const NUM_AUX_CONSTRAINTS: usize = 4;

    const NUM_AUX_ASSERTIONS: usize = 0;

    const AUX_CONSTRAINT_DEGREES: &'static [usize] = &[1, 1, 1, 1];

    pub const RESERVED_ADDRESS_RANGE: (u64, u64) = (1u64 << 33, (1u64 << 34) - 1);

    /// Minimum trace length required to embed the full 8-bit table.
    /// One row reserved for boundary placeholder; the remaining `active`
    /// rows must hold all `TABLE_SIZE` entries.
    pub const MIN_TRACE_LEN: usize = TABLE_SIZE + 1;
}

impl Builtin for RangeCheckBuiltin {
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
        _main_curr: &[F],
        main_next: &[F],
        _aux_curr: &[E],
        aux_next: &[E],
        base_offset: usize,
        _rand_elements: &[E],
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
    {
        let local_next = &aux_next[base_offset..base_offset + Self::AUX_WIDTH];

        debug_assert_eq!(result.len(), Self::NUM_AUX_CONSTRAINTS);

        let byte_base = E::from(BaseElement::new(256));

        for (k, &(hi_off, lo_off)) in LIMB_BYTE_PAIRS.iter().enumerate() {
            let limb = E::from(main_next[LIMB_COLS[k]]);
            let hi = local_next[hi_off];
            let lo = local_next[lo_off];
            result[k] = limb - (byte_base * hi + lo);
        }
    }

    fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_columns: &[&[BaseElement]],
        _rand_elements: &[E],
    ) -> Vec<Vec<E>> {
        let n = main_columns[COL_RC_L0].len();

        let byte_cols: Vec<Vec<E>> = (0..NUM_CHANNELS)
            .map(|channel| {
                let limb_col = LIMB_COLS[channel / 2];
                let is_high = channel % 2 == 0;
                (0..n)
                    .map(|row| {
                        let limb = main_columns[limb_col][row].as_int();
                        let byte = if is_high {
                            (limb >> 8) & 0xff
                        } else {
                            limb & 0xff
                        };
                        E::from(BaseElement::new(byte))
                    })
                    .collect()
            })
            .collect();

        byte_cols
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
    use super::*;

    type F = BaseElement;

    fn mock_main_with_limbs(limbs: &[[u64; 4]]) -> Vec<Vec<F>> {
        use maat_trace::table::TRACE_WIDTH;

        let n = limbs.len();
        let mut cols = vec![vec![F::ZERO; n]; TRACE_WIDTH];
        for (i, ls) in limbs.iter().enumerate() {
            cols[COL_RC_L0][i] = F::new(ls[0]);
            cols[COL_RC_L1][i] = F::new(ls[1]);
            cols[COL_RC_L2][i] = F::new(ls[2]);
            cols[COL_RC_L3][i] = F::new(ls[3]);
        }
        cols
    }

    #[test]
    fn build_emits_byte_witness_columns_only() {
        let n = RangeCheckBuiltin::MIN_TRACE_LEN.next_power_of_two();
        let mut limbs: Vec<[u64; 4]> = vec![[0, 0, 0, 0]; n];
        limbs[1] = [0xABCD, 0, 0, 0];
        let main = mock_main_with_limbs(&limbs);
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();

        let aux = RangeCheckBuiltin.build_aux_columns::<F>(&main_slices, &[]);
        assert_eq!(aux.len(), RangeCheckBuiltin::AUX_WIDTH);
        assert_eq!(aux[RC_B0_HI][1], F::new(0xAB));
        assert_eq!(aux[RC_B0_LO][1], F::new(0xCD));
        assert!(aux[RC_B1_HI..=RC_B3_LO].iter().all(|c| c[1] == F::ZERO));
    }

    #[test]
    fn limb_decomposition_passes_on_correct_bytes() {
        let n = RangeCheckBuiltin::MIN_TRACE_LEN.next_power_of_two();
        let mut limbs: Vec<[u64; 4]> = vec![[0, 0, 0, 0]; n];
        limbs[5] = [0x1234, 0x5678, 0x9ABC, 0xDEF0];
        let main = mock_main_with_limbs(&limbs);
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();

        let aux = RangeCheckBuiltin.build_aux_columns::<F>(&main_slices, &[]);
        let aux_curr: Vec<F> = aux.iter().map(|c| c[4]).collect();
        let aux_next: Vec<F> = aux.iter().map(|c| c[5]).collect();
        let main_curr: Vec<F> = main.iter().map(|c| c[4]).collect();
        let main_next: Vec<F> = main.iter().map(|c| c[5]).collect();

        let mut result = vec![F::ZERO; RangeCheckBuiltin::NUM_AUX_CONSTRAINTS];
        RangeCheckBuiltin.evaluate_aux_transition::<F, F>(
            &main_curr,
            &main_next,
            &aux_curr,
            &aux_next,
            0,
            &[],
            &mut result,
        );
        for (j, &r) in result.iter().enumerate() {
            assert_eq!(r, F::ZERO, "limb-decomposition constraint {j} violated");
        }
    }

    #[test]
    fn tampered_byte_breaks_decomposition() {
        let n = RangeCheckBuiltin::MIN_TRACE_LEN.next_power_of_two();
        let mut limbs: Vec<[u64; 4]> = vec![[0, 0, 0, 0]; n];
        limbs[5] = [0x1234, 0, 0, 0];
        let main = mock_main_with_limbs(&limbs);
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();

        let mut aux = RangeCheckBuiltin.build_aux_columns::<F>(&main_slices, &[]);
        aux[RC_B0_HI][5] += F::ONE;

        let main_curr = main.iter().map(|c| c[4]).collect::<Vec<F>>();
        let main_next = main.iter().map(|c| c[5]).collect::<Vec<F>>();
        let aux_curr = aux.iter().map(|c| c[4]).collect::<Vec<F>>();
        let aux_next = aux.iter().map(|c| c[5]).collect::<Vec<F>>();
        let mut result = vec![F::ZERO; RangeCheckBuiltin::NUM_AUX_CONSTRAINTS];
        RangeCheckBuiltin.evaluate_aux_transition::<F, F>(
            &main_curr,
            &main_next,
            &aux_curr,
            &aux_next,
            0,
            &[],
            &mut result,
        );
        assert_ne!(
            result[0],
            F::ZERO,
            "decomposition must fail at tampered row"
        );
    }

    #[test]
    fn aux_widths_match_constants() {
        assert_eq!(RangeCheckBuiltin::AUX_WIDTH, 8);
        assert_eq!(RangeCheckBuiltin::NUM_AUX_CONSTRAINTS, 4);
        assert_eq!(RangeCheckBuiltin::NUM_AUX_ASSERTIONS, 0);
        assert_eq!(
            RangeCheckBuiltin::AUX_CONSTRAINT_DEGREES.len(),
            RangeCheckBuiltin::NUM_AUX_CONSTRAINTS
        );
    }
}
