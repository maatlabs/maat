//! Range-check builtin segment, LogUp-backed.
//!
//! Proves that every 16-bit limb emitted on a range-check trigger row
//! (across columns `COL_RC_L0..COL_RC_L3`) lies in `[0, 2^16)` via
//! Häböck's logarithmic-derivative lookup argument.

use maat_field::{BaseElement, ExtensionOf, FieldElement};
use maat_trace::table::{COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3};
use winter_air::Assertion;

use super::Builtin;

/// LogUp lookup table size (8-bit byte table `{0, ..., 255}`).
pub const TABLE_SIZE: usize = 256;

/// Number of byte channels per row (4 main-trace limbs x 2 bytes each).
pub const NUM_CHANNELS: usize = 8;

/// Aux column offset: pinned 8-bit table column (t).
pub const RC_T: usize = 0;
/// Aux column offset: multiplicity column (m).
pub const RC_M: usize = 1;
/// Aux column offset: limb-0 high byte.
pub const RC_B0_HI: usize = 2;
/// Aux column offset: limb-0 low byte.
pub const RC_B0_LO: usize = 3;
/// Aux column offset: limb-1 high byte.
pub const RC_B1_HI: usize = 4;
/// Aux column offset: limb-1 low byte.
pub const RC_B1_LO: usize = 5;
/// Aux column offset: limb-2 high byte.
pub const RC_B2_HI: usize = 6;
/// Aux column offset: limb-2 low byte.
pub const RC_B2_LO: usize = 7;
/// Aux column offset: limb-3 high byte.
pub const RC_B3_HI: usize = 8;
/// Aux column offset: limb-3 low byte.
pub const RC_B3_LO: usize = 9;
/// Aux column offset: base of the eight per-channel grand-sum columns
/// `s_0, s_1, ..., s_7` (one per byte channel).
pub const RC_S_BASE: usize = 10;
/// Aux column offset: m-side grand-sum (`s_m`).
pub const RC_SM: usize = RC_S_BASE + NUM_CHANNELS;
/// Aux column offset: balance witness `bal = Σ_k s_k − s_m`.
pub const RC_BAL: usize = RC_SM + 1;

/// Verifier-challenge offset within this builtin's randomness slice.
const RAND_ALPHA: usize = 0;

const BYTE_OFFSETS: [usize; NUM_CHANNELS] = [
    RC_B0_HI, RC_B0_LO, RC_B1_HI, RC_B1_LO, RC_B2_HI, RC_B2_LO, RC_B3_HI, RC_B3_LO,
];

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

    const AUX_WIDTH: usize = RC_BAL + 1;

    const NUM_AUX_RANDS: usize = 1;

    /// 4 limb-decomposition (degree 1) + 8 channel transitions (degree 2) +
    /// 1 m-side transition (degree 2) + 1 balance-witness binding (degree 1).
    const NUM_AUX_CONSTRAINTS: usize = 4 + NUM_CHANNELS + 1 + 1;

    /// 8 channel zero-starts + 1 m-side zero-start + 1 final-balance check.
    const NUM_AUX_ASSERTIONS: usize = NUM_CHANNELS + 1 + 1;

    const AUX_CONSTRAINT_DEGREES: &'static [usize] = &[1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1];

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

    fn aux_constraint_degrees(&self) -> &'static [usize] {
        Self::AUX_CONSTRAINT_DEGREES
    }

    fn reserved_address_range(&self) -> (u64, u64) {
        Self::RESERVED_ADDRESS_RANGE
    }

    fn evaluate_aux_transition<F, E>(
        &self,
        _main_curr: &[F],
        main_next: &[F],
        aux_curr: &[E],
        aux_next: &[E],
        rand_elements: &[E],
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
    {
        debug_assert_eq!(aux_curr.len(), Self::AUX_WIDTH);
        debug_assert_eq!(aux_next.len(), Self::AUX_WIDTH);
        debug_assert_eq!(rand_elements.len(), Self::NUM_AUX_RANDS);
        debug_assert_eq!(result.len(), Self::NUM_AUX_CONSTRAINTS);

        let alpha = rand_elements[RAND_ALPHA];
        let one = E::ONE;
        let byte_base = E::from(BaseElement::new(256));

        for (k, &(hi_off, lo_off)) in LIMB_BYTE_PAIRS.iter().enumerate() {
            let limb = E::from(main_next[LIMB_COLS[k]]);
            let hi = aux_next[hi_off];
            let lo = aux_next[lo_off];
            result[k] = limb - (byte_base * hi + lo);
        }

        let channel_base = LIMB_BYTE_PAIRS.len();
        for k in 0..NUM_CHANNELS {
            let s_curr = aux_curr[RC_S_BASE + k];
            let s_next = aux_next[RC_S_BASE + k];
            let b_next = aux_next[BYTE_OFFSETS[k]];
            result[channel_base + k] = (s_next - s_curr) * (alpha - b_next) - one;
        }

        let m_side_index = channel_base + NUM_CHANNELS;
        let sm_curr = aux_curr[RC_SM];
        let sm_next = aux_next[RC_SM];
        let t_next = aux_next[RC_T];
        let m_next = aux_next[RC_M];
        result[m_side_index] = (sm_next - sm_curr) * (alpha - t_next) - m_next;

        let balance_index = m_side_index + 1;
        let bal_next = aux_next[RC_BAL];
        let sum_s_next = (0..NUM_CHANNELS)
            .map(|k| aux_next[RC_S_BASE + k])
            .fold(E::ZERO, |acc, v| acc + v);
        result[balance_index] = bal_next - (sum_s_next - sm_next);
    }

    fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_columns: &[&[BaseElement]],
        rand_elements: &[E],
    ) -> Vec<Vec<E>> {
        let n = main_columns[COL_RC_L0].len();
        let alpha = rand_elements[RAND_ALPHA];
        let active = n.saturating_sub(1);
        let embedding_rows = TABLE_SIZE.min(active);

        let mut byte_values = (0..NUM_CHANNELS)
            .map(|_| vec![0u64; n])
            .collect::<Vec<Vec<u64>>>();
        for i in 0..n {
            for k in 0..4 {
                let limb = main_columns[LIMB_COLS[k]][i].as_int();
                byte_values[2 * k][i] = (limb >> 8) & 0xff;
                byte_values[2 * k + 1][i] = limb & 0xff;
            }
        }

        let mut counts = vec![0u64; TABLE_SIZE];
        for channel in &byte_values {
            for &v in channel.iter().skip(1) {
                if (v as usize) < TABLE_SIZE {
                    counts[v as usize] += 1;
                }
            }
        }

        let mut t_col = vec![E::ZERO; n];
        let mut m_col = vec![E::ZERO; n];
        for v in 0..embedding_rows {
            t_col[1 + v] = E::from(BaseElement::new(v as u64));
            m_col[1 + v] = E::from(BaseElement::new(counts[v]));
        }

        let byte_cols = byte_values
            .iter()
            .map(|channel| {
                channel
                    .iter()
                    .map(|&v| E::from(BaseElement::new(v)))
                    .collect()
            })
            .collect::<Vec<Vec<E>>>();

        let mut s_cols = (0..NUM_CHANNELS)
            .map(|_| vec![E::ZERO; n])
            .collect::<Vec<Vec<E>>>();
        for k in 0..NUM_CHANNELS {
            for i in 1..n {
                let b = byte_cols[k][i];
                s_cols[k][i] = s_cols[k][i - 1] + (alpha - b).inv();
            }
        }

        let mut sm_col = vec![E::ZERO; n];
        for i in 1..n {
            sm_col[i] = sm_col[i - 1] + m_col[i] * (alpha - t_col[i]).inv();
        }

        let bal_col = (0..n)
            .map(|i| {
                let sum_s = (0..NUM_CHANNELS)
                    .map(|k| s_cols[k][i])
                    .fold(E::ZERO, |acc, v| acc + v);
                sum_s - sm_col[i]
            })
            .collect::<Vec<E>>();

        let mut out = Vec::with_capacity(Self::AUX_WIDTH);
        out.push(t_col);
        out.push(m_col);
        out.extend(byte_cols);
        out.extend(s_cols);
        out.push(sm_col);
        out.push(bal_col);
        out
    }

    fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
        &self,
        column_base: usize,
        last_step: usize,
    ) -> Vec<Assertion<E>> {
        let mut out = Vec::with_capacity(Self::NUM_AUX_ASSERTIONS);
        for k in 0..NUM_CHANNELS {
            out.push(Assertion::single(column_base + RC_S_BASE + k, 0, E::ZERO));
        }
        out.push(Assertion::single(column_base + RC_SM, 0, E::ZERO));
        out.push(Assertion::single(column_base + RC_BAL, last_step, E::ZERO));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type F = BaseElement;

    fn alpha() -> F {
        F::new(7919)
    }

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

    fn check_all_transitions(main: &[Vec<F>], aux: &[Vec<F>], rands: &[F]) {
        let n = aux[0].len();
        for i in 0..n - 1 {
            let main_curr = main.iter().map(|c| c[i]).collect::<Vec<F>>();
            let main_next = main.iter().map(|c| c[i + 1]).collect::<Vec<F>>();
            let aux_curr = aux.iter().map(|c| c[i]).collect::<Vec<F>>();
            let aux_next = aux.iter().map(|c| c[i + 1]).collect::<Vec<F>>();
            let mut result = vec![F::ZERO; RangeCheckBuiltin::NUM_AUX_CONSTRAINTS];
            RangeCheckBuiltin.evaluate_aux_transition::<F, F>(
                &main_curr,
                &main_next,
                &aux_curr,
                &aux_next,
                rands,
                &mut result,
            );
            for (j, &r) in result.iter().enumerate() {
                assert_eq!(r, F::ZERO, "constraint {j} violated at row {i}");
            }
        }
    }

    #[test]
    fn empty_limbs_satisfy_all_constraints() {
        let n = RangeCheckBuiltin::MIN_TRACE_LEN.next_power_of_two();
        let limbs: Vec<[u64; 4]> = vec![[0, 0, 0, 0]; n];
        let main = mock_main_with_limbs(&limbs);
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();
        let rands = vec![alpha()];

        let aux = RangeCheckBuiltin.build_aux_columns::<F>(&main_slices, &rands);
        assert_eq!(aux.len(), RangeCheckBuiltin::AUX_WIDTH);
        assert_eq!(aux[RC_BAL][n - 1], F::ZERO, "balance must close to zero");
        check_all_transitions(&main, &aux, &rands);
    }

    #[test]
    fn diverse_limbs_satisfy_all_constraints() {
        let n = RangeCheckBuiltin::MIN_TRACE_LEN.next_power_of_two();
        let mut limbs: Vec<[u64; 4]> = vec![[0, 0, 0, 0]; n];
        limbs[1] = [0xFFFF, 1, 0xABCD, 256];
        limbs[2] = [255, 256, 1, 0xFF00];
        limbs[3] = [0x1234, 0xFEDC, 0x00FF, 0xFF00];
        let main = mock_main_with_limbs(&limbs);
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();
        let rands = vec![alpha()];

        let aux = RangeCheckBuiltin.build_aux_columns::<F>(&main_slices, &rands);
        assert_eq!(aux[RC_BAL][n - 1], F::ZERO);
        check_all_transitions(&main, &aux, &rands);
    }

    #[test]
    fn tampered_byte_breaks_decomposition() {
        let n = RangeCheckBuiltin::MIN_TRACE_LEN.next_power_of_two();
        let mut limbs: Vec<[u64; 4]> = vec![[0, 0, 0, 0]; n];
        limbs[5] = [0x1234, 0, 0, 0];
        let main = mock_main_with_limbs(&limbs);
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();
        let rands = vec![alpha()];

        let mut aux = RangeCheckBuiltin.build_aux_columns::<F>(&main_slices, &rands);
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
            &rands,
            &mut result,
        );
        assert_ne!(
            result[0],
            F::ZERO,
            "decomposition must fail at tampered row"
        );
    }

    #[test]
    fn tampered_multiplicity_breaks_balance() {
        let n = RangeCheckBuiltin::MIN_TRACE_LEN.next_power_of_two();
        let mut limbs: Vec<[u64; 4]> = vec![[0, 0, 0, 0]; n];
        limbs[1] = [42, 0, 0, 0];
        let main = mock_main_with_limbs(&limbs);
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();
        let rands = vec![alpha()];

        let mut aux = RangeCheckBuiltin.build_aux_columns::<F>(&main_slices, &rands);
        let m_row = 1 + 42;
        aux[RC_M][m_row] += F::ONE;

        for i in 1..n {
            aux[RC_SM][i] = aux[RC_SM][i - 1] + aux[RC_M][i] * (alpha() - aux[RC_T][i]).inv();
        }
        let bal = (0..n)
            .map(|i| {
                let sum_s = (0..NUM_CHANNELS)
                    .map(|k| aux[RC_S_BASE + k][i])
                    .fold(F::ZERO, |acc, v| acc + v);
                sum_s - aux[RC_SM][i]
            })
            .collect::<Vec<F>>();
        for (i, b) in bal.into_iter().enumerate() {
            aux[RC_BAL][i] = b;
        }
        assert_ne!(
            aux[RC_BAL][n - 1],
            F::ZERO,
            "tampered multiplicity must break the final balance"
        );
    }

    #[test]
    fn aux_widths_match_constants() {
        assert_eq!(RangeCheckBuiltin::AUX_WIDTH, 20);
        assert_eq!(RangeCheckBuiltin::NUM_AUX_CONSTRAINTS, 14);
        assert_eq!(RangeCheckBuiltin::NUM_AUX_ASSERTIONS, 10);
        assert_eq!(
            RangeCheckBuiltin::AUX_CONSTRAINT_DEGREES.len(),
            RangeCheckBuiltin::NUM_AUX_CONSTRAINTS
        );
    }
}
