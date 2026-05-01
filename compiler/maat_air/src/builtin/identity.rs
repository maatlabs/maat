//! Identity builtin: a width-1 column pinned to the multiplicative identity.
//!
//! The identity builtin owns no real proving work; its sole purpose is to
//! exercise the [`BuiltinSet`](super::BuiltinSet) dispatcher in production
//! traces so a regression that bypasses the registry path is caught
//! structurally rather than relying on a future builtin to surface it.

use winter_air::Assertion;
use winter_math::fields::f64::BaseElement;
use winter_math::{ExtensionOf, FieldElement};

use super::Builtin;

/// Aux column offset (within this builtin): the constant-one column.
pub const IDENTITY_COL: usize = 0;

#[derive(Clone, Copy, Debug, Default)]
pub struct IdentityBuiltin;

impl IdentityBuiltin {
    pub const NAME: &'static str = "identity";

    pub const AUX_WIDTH: usize = 1;

    pub const NUM_AUX_RANDS: usize = 0;

    pub const NUM_AUX_CONSTRAINTS: usize = 1;

    pub const NUM_AUX_ASSERTIONS: usize = 2;

    pub const AUX_CONSTRAINT_DEGREES: &'static [usize] = &[1];

    pub const RESERVED_ADDRESS_RANGE: (u64, u64) = (1u64 << 34, (1u64 << 35) - 1);
}

impl Builtin for IdentityBuiltin {
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
        _main_next: &[F],
        aux_curr: &[E],
        aux_next: &[E],
        _rand_elements: &[E],
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
    {
        debug_assert_eq!(aux_curr.len(), Self::AUX_WIDTH);
        debug_assert_eq!(aux_next.len(), Self::AUX_WIDTH);
        debug_assert_eq!(result.len(), Self::NUM_AUX_CONSTRAINTS);
        result[0] = aux_next[IDENTITY_COL] - aux_curr[IDENTITY_COL];
    }

    fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_columns: &[&[BaseElement]],
        _rand_elements: &[E],
    ) -> Vec<Vec<E>> {
        let n = main_columns[0].len();
        vec![vec![E::ONE; n]]
    }

    fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
        &self,
        column_base: usize,
        last_step: usize,
    ) -> Vec<Assertion<E>> {
        vec![
            Assertion::single(column_base + IDENTITY_COL, 0, E::ONE),
            Assertion::single(column_base + IDENTITY_COL, last_step, E::ONE),
        ]
    }
}
