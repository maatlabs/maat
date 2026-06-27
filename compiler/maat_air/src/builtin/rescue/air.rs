//! `RescueHashBuiltin`: STARK-friendly Rescue-Prime hash registered on the
//! Maat builtin segment.

use maat_field::{BaseElement, ExtensionOf, FieldElement};
use winter_air::Assertion;

use super::super::Builtin;

#[derive(Clone, Copy, Debug, Default)]
pub struct RescueHashBuiltin;

impl RescueHashBuiltin {
    pub const NAME: &'static str = "rescue";

    const AUX_WIDTH: usize = 0;

    const NUM_AUX_RANDS: usize = 0;

    const NUM_AUX_CONSTRAINTS: usize = 0;

    const NUM_AUX_ASSERTIONS: usize = 0;

    pub const RESERVED_ADDRESS_RANGE: (u64, u64) = (1u64 << 37, (1u64 << 38) - 1);
}

impl Builtin for RescueHashBuiltin {
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
        Vec::new()
    }

    fn reserved_address_range(&self) -> (u64, u64) {
        Self::RESERVED_ADDRESS_RANGE
    }

    fn evaluate_aux_transition<F, E>(
        &self,
        _main_curr: &[F],
        _main_next: &[F],
        _aux_curr: &[E],
        _aux_next: &[E],
        _base_offset: usize,
        _rand_elements: &[E],
        _result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
    {
    }

    fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        _main_columns: &[&[BaseElement]],
        _rand_elements: &[E],
    ) -> Vec<Vec<E>> {
        Vec::new()
    }

    fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
        &self,
        _column_base: usize,
        _last_step: usize,
    ) -> Vec<Assertion<E>> {
        Vec::new()
    }

    fn periodic_columns(&self) -> Vec<Vec<BaseElement>> {
        maat_trace::main_segment::rescue_periodic_columns()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::{BitwiseBuiltin, LogUpBuiltin, RangeCheckBuiltin};

    #[test]
    fn rescue_builtin_has_zero_footprint() {
        let builtin = RescueHashBuiltin;
        assert_eq!(builtin.aux_width(), 0);
        assert_eq!(builtin.num_aux_rands(), 0);
        assert_eq!(builtin.num_aux_constraints(), 0);
        assert_eq!(builtin.num_aux_assertions(), 0);
        assert!(builtin.aux_constraint_degrees().is_empty());

        let periodic = builtin.periodic_columns();
        assert_eq!(periodic.len(), 24);
        assert!(periodic.iter().all(|c| c.len() == 8));
    }

    #[test]
    fn rescue_reserved_address_range_is_disjoint_from_other_builtins() {
        let other_ranges = [
            RangeCheckBuiltin::RESERVED_ADDRESS_RANGE,
            BitwiseBuiltin::RESERVED_ADDRESS_RANGE,
            LogUpBuiltin::RESERVED_ADDRESS_RANGE,
        ];
        let (lo, hi) = RescueHashBuiltin::RESERVED_ADDRESS_RANGE;
        assert!(lo <= hi);
        for (other_lo, other_hi) in other_ranges {
            assert!(
                hi < other_lo || other_hi < lo,
                "rescue range overlaps another builtin"
            );
        }
    }

    #[test]
    fn rescue_address_range_continues_doubling_pattern() {
        let (rescue_lo, _) = RescueHashBuiltin::RESERVED_ADDRESS_RANGE;
        let (_, logup_hi) = LogUpBuiltin::RESERVED_ADDRESS_RANGE;
        assert_eq!(
            rescue_lo,
            logup_hi + 1,
            "rescue range must abut the LogUp range with no gap"
        );
    }
}
