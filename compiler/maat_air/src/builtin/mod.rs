//! Builtin-segment ABI for the Maat AIR.
//!
//! A *builtin* is a self-contained mini-AIR that owns a slice of the
//! auxiliary trace segment together with the verifier challenges and
//! transition constraints needed to prove its operation semantics.
//! Operations expensive to arithmetize, e.g., range check, bitwise,
//! ordering, hash builtins, etc. are implemented as builtins.
//!
//! # Composition
//!
//! [`BuiltinSet`] aggregates all builtins. Aux columns are laid out in
//! registration order, immediately after the unified memory permutation columns
//! owned by the CPU AIR. Verifier challenges are partitioned the same way:
//! the memory permutation consumes the first `MEMORY_NUM_AUX_RANDS` entries, then each
//! builtin consumes its share in registration order.
//!
//! # Adding a new builtin
//!
//! 1. Define a unit struct (e.g. `BitwiseBuiltin`).
//! 2. Implement the [`Builtin`] trait.
//! 3. Register the struct as a field of [`BuiltinSet`] and add layout
//!    constants + dispatch lines in `BuiltinSet::evaluate_aux_transition`,
//!    `build_aux_columns`, and `aux_assertions`.

pub mod bitwise;
pub mod identity;
pub mod range_check;

pub use bitwise::BitwiseBuiltin;
pub use identity::IdentityBuiltin;
use maat_field::{BaseElement, ExtensionOf, FieldElement};
pub use range_check::RangeCheckBuiltin;
use winter_air::Assertion;

use crate::aux_segment::{MEMORY_AUX_WIDTH, MEMORY_NUM_AUX_RANDS};

pub trait Builtin {
    fn name(&self) -> &'static str;

    fn aux_width(&self) -> usize;

    fn num_aux_rands(&self) -> usize;

    fn aux_constraint_degrees(&self) -> &'static [usize];

    fn reserved_address_range(&self) -> (u64, u64);

    fn num_aux_assertions(&self) -> usize;

    fn evaluate_aux_transition<F, E>(
        &self,
        main_curr: &[F],
        main_next: &[F],
        aux_curr: &[E],
        aux_next: &[E],
        rand_elements: &[E],
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>;

    fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_columns: &[&[BaseElement]],
        rand_elements: &[E],
    ) -> Vec<Vec<E>>;

    fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
        &self,
        column_base: usize,
        last_step: usize,
    ) -> Vec<Assertion<E>>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BuiltinSet {
    pub range_check: RangeCheckBuiltin,
    pub bitwise: BitwiseBuiltin,
    pub identity: IdentityBuiltin,
}

impl BuiltinSet {
    pub const RANGE_CHECK_AUX_BASE: usize = MEMORY_AUX_WIDTH;

    pub const BITWISE_AUX_BASE: usize = Self::RANGE_CHECK_AUX_BASE + RangeCheckBuiltin::AUX_WIDTH;

    pub const IDENTITY_AUX_BASE: usize = Self::BITWISE_AUX_BASE + BitwiseBuiltin::AUX_WIDTH;

    pub const RANGE_CHECK_RAND_BASE: usize = MEMORY_NUM_AUX_RANDS;

    pub const BITWISE_RAND_BASE: usize =
        Self::RANGE_CHECK_RAND_BASE + RangeCheckBuiltin::NUM_AUX_RANDS;

    pub const IDENTITY_RAND_BASE: usize = Self::BITWISE_RAND_BASE + BitwiseBuiltin::NUM_AUX_RANDS;

    pub const TOTAL_AUX_WIDTH: usize =
        RangeCheckBuiltin::AUX_WIDTH + BitwiseBuiltin::AUX_WIDTH + IdentityBuiltin::AUX_WIDTH;

    pub const TOTAL_NUM_AUX_RANDS: usize = RangeCheckBuiltin::NUM_AUX_RANDS
        + BitwiseBuiltin::NUM_AUX_RANDS
        + IdentityBuiltin::NUM_AUX_RANDS;

    pub const TOTAL_NUM_AUX_CONSTRAINTS: usize = RangeCheckBuiltin::NUM_AUX_CONSTRAINTS
        + BitwiseBuiltin::NUM_AUX_CONSTRAINTS
        + IdentityBuiltin::NUM_AUX_CONSTRAINTS;

    pub const TOTAL_NUM_AUX_ASSERTIONS: usize = RangeCheckBuiltin::NUM_AUX_ASSERTIONS
        + BitwiseBuiltin::NUM_AUX_ASSERTIONS
        + IdentityBuiltin::NUM_AUX_ASSERTIONS;

    /// Constructs a fresh registry holding default-initialized builtins.
    pub const fn new() -> Self {
        Self {
            range_check: RangeCheckBuiltin,
            bitwise: BitwiseBuiltin,
            identity: IdentityBuiltin,
        }
    }

    pub fn aux_constraint_degrees() -> Vec<usize> {
        let mut out = Vec::with_capacity(Self::TOTAL_NUM_AUX_CONSTRAINTS);
        out.extend_from_slice(RangeCheckBuiltin::AUX_CONSTRAINT_DEGREES);
        out.extend_from_slice(BitwiseBuiltin::AUX_CONSTRAINT_DEGREES);
        out.extend_from_slice(IdentityBuiltin::AUX_CONSTRAINT_DEGREES);
        out
    }

    pub fn evaluate_aux_transition<F, E>(
        main_curr: &[F],
        main_next: &[F],
        aux_curr: &[E],
        aux_next: &[E],
        rand_elements: &[E],
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
    {
        debug_assert_eq!(result.len(), Self::TOTAL_NUM_AUX_CONSTRAINTS);

        let (rc_result, rest) = result.split_at_mut(RangeCheckBuiltin::NUM_AUX_CONSTRAINTS);
        let (bw_result, id_result) = rest.split_at_mut(BitwiseBuiltin::NUM_AUX_CONSTRAINTS);

        let rc_aux_curr = &aux_curr[Self::RANGE_CHECK_AUX_BASE..Self::BITWISE_AUX_BASE];
        let rc_aux_next = &aux_next[Self::RANGE_CHECK_AUX_BASE..Self::BITWISE_AUX_BASE];
        let rc_rands = &rand_elements[Self::RANGE_CHECK_RAND_BASE..Self::BITWISE_RAND_BASE];

        RangeCheckBuiltin.evaluate_aux_transition::<F, E>(
            main_curr,
            main_next,
            rc_aux_curr,
            rc_aux_next,
            rc_rands,
            rc_result,
        );

        let bw_aux_curr = &aux_curr[Self::BITWISE_AUX_BASE..Self::IDENTITY_AUX_BASE];
        let bw_aux_next = &aux_next[Self::BITWISE_AUX_BASE..Self::IDENTITY_AUX_BASE];
        let bw_rands = &rand_elements[Self::BITWISE_RAND_BASE..Self::IDENTITY_RAND_BASE];

        BitwiseBuiltin.evaluate_aux_transition::<F, E>(
            main_curr,
            main_next,
            bw_aux_curr,
            bw_aux_next,
            bw_rands,
            bw_result,
        );

        let id_end = Self::IDENTITY_AUX_BASE + IdentityBuiltin::AUX_WIDTH;
        let id_aux_curr = &aux_curr[Self::IDENTITY_AUX_BASE..id_end];
        let id_aux_next = &aux_next[Self::IDENTITY_AUX_BASE..id_end];
        let id_rand_end = Self::IDENTITY_RAND_BASE + IdentityBuiltin::NUM_AUX_RANDS;
        let id_rands = &rand_elements[Self::IDENTITY_RAND_BASE..id_rand_end];

        IdentityBuiltin.evaluate_aux_transition::<F, E>(
            main_curr,
            main_next,
            id_aux_curr,
            id_aux_next,
            id_rands,
            id_result,
        );
    }

    pub fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        main_columns: &[&[BaseElement]],
        rand_elements: &[E],
    ) -> Vec<Vec<E>> {
        let rc_rands = &rand_elements[Self::RANGE_CHECK_RAND_BASE..Self::BITWISE_RAND_BASE];
        let bw_rands = &rand_elements[Self::BITWISE_RAND_BASE..Self::IDENTITY_RAND_BASE];
        let id_rand_end = Self::IDENTITY_RAND_BASE + IdentityBuiltin::NUM_AUX_RANDS;
        let id_rands = &rand_elements[Self::IDENTITY_RAND_BASE..id_rand_end];

        let mut cols = Vec::with_capacity(Self::TOTAL_AUX_WIDTH);
        cols.extend(RangeCheckBuiltin.build_aux_columns(main_columns, rc_rands));
        cols.extend(BitwiseBuiltin.build_aux_columns(main_columns, bw_rands));
        cols.extend(IdentityBuiltin.build_aux_columns(main_columns, id_rands));
        cols
    }

    pub fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
        last_step: usize,
    ) -> Vec<Assertion<E>> {
        let mut out = Vec::with_capacity(Self::TOTAL_NUM_AUX_ASSERTIONS);
        out.extend(RangeCheckBuiltin.aux_assertions::<E>(Self::RANGE_CHECK_AUX_BASE, last_step));
        out.extend(BitwiseBuiltin.aux_assertions::<E>(Self::BITWISE_AUX_BASE, last_step));
        out.extend(IdentityBuiltin.aux_assertions::<E>(Self::IDENTITY_AUX_BASE, last_step));
        out
    }
}

#[cfg(test)]
mod tests {
    use maat_trace::table::TRACE_WIDTH;

    use super::*;
    use crate::aux_segment::{MEMORY_AUX_WIDTH, MEMORY_NUM_AUX_RANDS};

    type F = BaseElement;

    #[test]
    fn three_builtins_active_compose() {
        assert_eq!(
            BuiltinSet::TOTAL_AUX_WIDTH,
            RangeCheckBuiltin::AUX_WIDTH + BitwiseBuiltin::AUX_WIDTH + IdentityBuiltin::AUX_WIDTH
        );
        assert_eq!(BuiltinSet::TOTAL_NUM_AUX_RANDS, 1);
        assert_eq!(
            BuiltinSet::TOTAL_NUM_AUX_CONSTRAINTS,
            RangeCheckBuiltin::NUM_AUX_CONSTRAINTS
                + BitwiseBuiltin::NUM_AUX_CONSTRAINTS
                + IdentityBuiltin::NUM_AUX_CONSTRAINTS
        );

        let aux_full_width = MEMORY_AUX_WIDTH + BuiltinSet::TOTAL_AUX_WIDTH;
        let n = 8usize;
        let main = vec![vec![F::ZERO; n]; TRACE_WIDTH];
        let main_slices: Vec<&[F]> = main.iter().map(|c| c.as_slice()).collect();

        let total_rands = MEMORY_NUM_AUX_RANDS + BuiltinSet::TOTAL_NUM_AUX_RANDS;
        let rands: Vec<F> = (0..total_rands).map(|i| F::new(7 + i as u64)).collect();

        let builtin_cols = BuiltinSet::build_aux_columns(&main_slices, &rands);
        assert_eq!(builtin_cols.len(), BuiltinSet::TOTAL_AUX_WIDTH);

        let mut aux_full = vec![vec![F::ZERO; n]; aux_full_width];
        for (i, col) in builtin_cols.into_iter().enumerate() {
            aux_full[MEMORY_AUX_WIDTH + i] = col;
        }

        for i in 0..n - 1 {
            let main_curr: Vec<F> = (0..TRACE_WIDTH).map(|c| main[c][i]).collect();
            let main_next: Vec<F> = (0..TRACE_WIDTH).map(|c| main[c][i + 1]).collect();
            let aux_curr: Vec<F> = (0..aux_full_width).map(|c| aux_full[c][i]).collect();
            let aux_next: Vec<F> = (0..aux_full_width).map(|c| aux_full[c][i + 1]).collect();

            let mut result = vec![F::ZERO; BuiltinSet::TOTAL_NUM_AUX_CONSTRAINTS];
            BuiltinSet::evaluate_aux_transition(
                &main_curr,
                &main_next,
                &aux_curr,
                &aux_next,
                &rands,
                &mut result,
            );

            for (j, r) in result.iter().enumerate() {
                assert_eq!(*r, F::ZERO, "builtin constraint {j} non-zero at row {i}");
            }
        }
    }

    #[test]
    fn registry_constants_match_concrete_builtins() {
        assert_eq!(BuiltinSet::RANGE_CHECK_AUX_BASE, MEMORY_AUX_WIDTH);
        assert_eq!(
            BuiltinSet::BITWISE_AUX_BASE,
            MEMORY_AUX_WIDTH + RangeCheckBuiltin::AUX_WIDTH
        );
        assert_eq!(
            BuiltinSet::IDENTITY_AUX_BASE,
            MEMORY_AUX_WIDTH + RangeCheckBuiltin::AUX_WIDTH + BitwiseBuiltin::AUX_WIDTH
        );
        assert_eq!(BuiltinSet::RANGE_CHECK_RAND_BASE, MEMORY_NUM_AUX_RANDS);
        assert_eq!(
            BuiltinSet::BITWISE_RAND_BASE,
            MEMORY_NUM_AUX_RANDS + RangeCheckBuiltin::NUM_AUX_RANDS
        );
        assert_eq!(
            BuiltinSet::IDENTITY_RAND_BASE,
            MEMORY_NUM_AUX_RANDS + RangeCheckBuiltin::NUM_AUX_RANDS + BitwiseBuiltin::NUM_AUX_RANDS
        );
    }

    #[test]
    fn reserved_address_ranges_are_disjoint() {
        let ranges = [
            RangeCheckBuiltin::RESERVED_ADDRESS_RANGE,
            BitwiseBuiltin::RESERVED_ADDRESS_RANGE,
            IdentityBuiltin::RESERVED_ADDRESS_RANGE,
        ];
        for (lo, hi) in ranges {
            assert!(lo <= hi);
        }
        for i in 0..ranges.len() {
            for j in (i + 1)..ranges.len() {
                let (a_lo, a_hi) = ranges[i];
                let (b_lo, b_hi) = ranges[j];
                assert!(a_hi < b_lo || b_hi < a_lo, "ranges {i} and {j} overlap");
            }
        }
    }
}
