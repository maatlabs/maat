//! Range-check builtin segment.
//!
//! Owns five auxiliary columns (a four-column sorted limb pool and a
//! permutation accumulator) and proves that every 16-bit limb emitted on a
//! range-check trigger row lies in `[0, 2^16)`.

use maat_trace::table::{COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3};
use winter_air::Assertion;
use winter_math::fields::f64::BaseElement;
use winter_math::{ExtensionOf, FieldElement};

use super::Builtin;

/// Aux column offset (within this builtin): sorted limb pool, column 0.
pub const RC_SORTED_0: usize = 0;
/// Aux column offset: sorted limb pool, column 1.
pub const RC_SORTED_1: usize = 1;
/// Aux column offset: sorted limb pool, column 2.
pub const RC_SORTED_2: usize = 2;
/// Aux column offset: sorted limb pool, column 3.
pub const RC_SORTED_3: usize = 3;
/// Aux column offset: permutation accumulator.
pub const RC_ACC: usize = 4;

/// Verifier-challenge offset (within this builtin's slice).
const RAND_Z_RC: usize = 0;

#[derive(Clone, Copy, Debug, Default)]
pub struct RangeCheckBuiltin;

impl RangeCheckBuiltin {
    pub const NAME: &'static str = "range_check";

    pub const AUX_WIDTH: usize = 5;

    pub const NUM_AUX_RANDS: usize = 1;

    pub const NUM_AUX_CONSTRAINTS: usize = 5;

    pub const NUM_AUX_ASSERTIONS: usize = 2;

    pub const AUX_CONSTRAINT_DEGREES: &'static [usize] = &[2, 2, 2, 2, 5];

    pub const RESERVED_ADDRESS_RANGE: (u64, u64) = (1u64 << 33, (1u64 << 34) - 1);
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

        let one = E::ONE;

        let s0 = aux_curr[RC_SORTED_0];
        let s1 = aux_curr[RC_SORTED_1];
        let s2 = aux_curr[RC_SORTED_2];
        let s3 = aux_curr[RC_SORTED_3];
        let s0_next = aux_next[RC_SORTED_0];
        let s1_next = aux_next[RC_SORTED_1];
        let s2_next = aux_next[RC_SORTED_2];
        let s3_next = aux_next[RC_SORTED_3];

        let rc_acc = aux_curr[RC_ACC];
        let rc_acc_next = aux_next[RC_ACC];
        let z_rc = rand_elements[RAND_Z_RC];

        let d01 = s1 - s0;
        result[0] = d01 * (d01 - one);
        let d12 = s2 - s1;
        result[1] = d12 * (d12 - one);
        let d23 = s3 - s2;
        result[2] = d23 * (d23 - one);
        let d30 = s0_next - s3;
        result[3] = d30 * (d30 - one);

        let l0_next = E::from(main_next[COL_RC_L0]);
        let l1_next = E::from(main_next[COL_RC_L1]);
        let l2_next = E::from(main_next[COL_RC_L2]);
        let l3_next = E::from(main_next[COL_RC_L3]);

        let sorted_prod = (z_rc - s0_next) * (z_rc - s1_next) * (z_rc - s2_next) * (z_rc - s3_next);
        let limb_prod = (z_rc - l0_next) * (z_rc - l1_next) * (z_rc - l2_next) * (z_rc - l3_next);
        result[4] = sorted_prod * rc_acc_next - limb_prod * rc_acc;
    }

    fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_columns: &[&[BaseElement]],
        rand_elements: &[E],
    ) -> Vec<Vec<E>> {
        let n = main_columns[COL_RC_L0].len();
        let z_rc = rand_elements[RAND_Z_RC];

        let mut limb_pool: Vec<u64> = Vec::with_capacity(4 * n);
        for (((l0, l1), l2), l3) in main_columns[COL_RC_L0]
            .iter()
            .zip(main_columns[COL_RC_L1])
            .zip(main_columns[COL_RC_L2])
            .zip(main_columns[COL_RC_L3])
        {
            limb_pool.push(l0.as_int());
            limb_pool.push(l1.as_int());
            limb_pool.push(l2.as_int());
            limb_pool.push(l3.as_int());
        }
        limb_pool.sort_unstable();

        let mut sorted: [Vec<E>; 4] = std::array::from_fn(|_| Vec::with_capacity(n));
        for i in 0..n {
            sorted[0].push(E::from(BaseElement::new(limb_pool[4 * i])));
            sorted[1].push(E::from(BaseElement::new(limb_pool[4 * i + 1])));
            sorted[2].push(E::from(BaseElement::new(limb_pool[4 * i + 2])));
            sorted[3].push(E::from(BaseElement::new(limb_pool[4 * i + 3])));
        }

        let mut acc = Vec::with_capacity(n);
        acc.push(E::ONE);
        for i in 1..n {
            let l0 = E::from(main_columns[COL_RC_L0][i]);
            let l1 = E::from(main_columns[COL_RC_L1][i]);
            let l2 = E::from(main_columns[COL_RC_L2][i]);
            let l3 = E::from(main_columns[COL_RC_L3][i]);
            let limb_prod = (z_rc - l0) * (z_rc - l1) * (z_rc - l2) * (z_rc - l3);
            let sorted_prod = (z_rc - sorted[0][i])
                * (z_rc - sorted[1][i])
                * (z_rc - sorted[2][i])
                * (z_rc - sorted[3][i]);
            acc.push(acc[i - 1] * limb_prod * sorted_prod.inv());
        }

        let [s0, s1, s2, s3] = sorted;
        vec![s0, s1, s2, s3, acc]
    }

    fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
        &self,
        column_base: usize,
        last_step: usize,
    ) -> Vec<Assertion<E>> {
        vec![
            Assertion::single(column_base + RC_ACC, 0, E::ONE),
            Assertion::single(column_base + RC_ACC, last_step, E::ONE),
        ]
    }
}
