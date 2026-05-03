//! CPU constraint system (AIR) for the Maat STARK prover/verifier.
//!
//! This crate defines [`MaatAir`], an Algebraic Intermediate Representation
//! that encodes the execution semantics of the Maat virtual machine as
//! polynomial constraints over a Goldilocks field trace. Implementing
//! Winterfell's [`Air`] trait, the AIR is the bridge between the
//! trace generator (`maat_trace`) and the STARK prover (`maat_prover`).

#![forbid(unsafe_code)]

mod aux_segment;
mod builtin;
mod public_inputs;

pub use aux_segment::{AUX_WIDTH, NUM_AUX_RANDS, build_aux_columns};
use aux_segment::{
    NUM_AUX_ASSERTIONS, NUM_AUX_CONSTRAINTS, aux_assertions, aux_constraint_degrees,
};
pub use builtin::{BitwiseBuiltin, Builtin, BuiltinSet, IdentityBuiltin, RangeCheckBuiltin};
use maat_field::{BaseElement, ExtensionOf, FieldElement};
use maat_trace::main_segment::{self, CONSTRAINT_DEGREES};
use maat_trace::table::{COL_OUT, COL_PC, COL_SP};
pub use public_inputs::MaatPublicInputs;
pub use winter_air::proof::Proof;
use winter_air::{Air, AirContext, Assertion, TransitionConstraintDegree};
pub use winter_air::{
    AuxRandElements, BatchingMethod, EvaluationFrame, FieldExtension, PartitionOptions,
    ProofOptions, TraceInfo,
};

/// Number of boundary assertions on the main trace segment.
const NUM_MAIN_ASSERTIONS: usize = 3;

pub struct MaatAir {
    context: AirContext<BaseElement>,
    public_inputs: MaatPublicInputs,
}

impl Air for MaatAir {
    type BaseField = BaseElement;
    type PublicInputs = MaatPublicInputs;

    fn new(trace_info: TraceInfo, pub_inputs: Self::PublicInputs, options: ProofOptions) -> Self {
        let main_degrees = CONSTRAINT_DEGREES
            .iter()
            .map(|&d| TransitionConstraintDegree::new(d))
            .collect::<Vec<_>>();

        let aux_degrees = aux_constraint_degrees()
            .into_iter()
            .map(TransitionConstraintDegree::new)
            .collect::<Vec<_>>();

        let context = AirContext::new_multi_segment(
            trace_info,
            main_degrees,
            aux_degrees,
            NUM_MAIN_ASSERTIONS,
            NUM_AUX_ASSERTIONS,
            options,
        );

        Self {
            context,
            public_inputs: pub_inputs,
        }
    }

    fn context(&self) -> &AirContext<Self::BaseField> {
        &self.context
    }

    fn evaluate_transition<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        frame: &EvaluationFrame<E>,
        _periodic_values: &[E],
        result: &mut [E],
    ) {
        main_segment::evaluate(frame.current(), frame.next(), result);
    }

    fn evaluate_aux_transition<F, E>(
        &self,
        main_frame: &EvaluationFrame<F>,
        aux_frame: &EvaluationFrame<E>,
        _periodic_values: &[F],
        aux_rand_elements: &AuxRandElements<E>,
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = Self::BaseField>,
        E: FieldElement<BaseField = Self::BaseField> + ExtensionOf<F>,
    {
        debug_assert_eq!(result.len(), NUM_AUX_CONSTRAINTS);
        aux_segment::evaluate(
            main_frame.current(),
            main_frame.next(),
            aux_frame.current(),
            aux_frame.next(),
            aux_rand_elements.rand_elements(),
            result,
        );
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        let last_step = self.trace_length() - 1;
        vec![
            Assertion::single(COL_PC, 0, BaseElement::ZERO),
            Assertion::single(COL_SP, 0, BaseElement::ZERO),
            Assertion::single(COL_OUT, last_step, self.public_inputs.output),
        ]
    }

    fn get_aux_assertions<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        _aux_rand_elements: &AuxRandElements<E>,
    ) -> Vec<Assertion<E>> {
        aux_assertions::<E>(self.trace_length() - 1)
    }
}

#[cfg(test)]
mod tests {
    use maat_trace::table::TRACE_WIDTH;

    use super::*;
    use crate::aux_segment::AUX_COL_MEM_ACC;
    use crate::main_segment::NUM_CONSTRAINTS;

    fn test_options() -> ProofOptions {
        ProofOptions::new(
            27,
            8,
            0,
            winter_air::FieldExtension::None,
            4,
            255,
            winter_air::BatchingMethod::Algebraic,
            winter_air::BatchingMethod::Algebraic,
        )
    }

    fn multi_segment_trace_info(trace_length: usize) -> TraceInfo {
        TraceInfo::new_multi_segment(TRACE_WIDTH, AUX_WIDTH, NUM_AUX_RANDS, trace_length, vec![])
    }

    #[test]
    fn air_construction_succeeds() {
        let trace_info = multi_segment_trace_info(8);
        let pub_inputs = MaatPublicInputs::with_output(BaseElement::new(42));
        let air = MaatAir::new(trace_info, pub_inputs, test_options());

        assert_eq!(air.context().trace_info().main_trace_width(), TRACE_WIDTH);
        assert_eq!(air.context().trace_info().aux_segment_width(), AUX_WIDTH);
        assert_eq!(
            air.context().num_transition_constraints(),
            NUM_CONSTRAINTS + NUM_AUX_CONSTRAINTS
        );
    }

    #[test]
    fn main_assertions_target_correct_columns() {
        let trace_info = multi_segment_trace_info(8);
        let pub_inputs = MaatPublicInputs::with_output(BaseElement::new(99));
        let air = MaatAir::new(trace_info, pub_inputs, test_options());

        let assertions = air.get_assertions();
        assert_eq!(assertions.len(), NUM_MAIN_ASSERTIONS);
        assert_eq!(assertions[0].column(), COL_PC);
        assert_eq!(assertions[1].column(), COL_SP);
        assert_eq!(assertions[2].column(), COL_OUT);
    }

    #[test]
    fn aux_assertions_cover_memory_and_builtins() {
        let trace_info = multi_segment_trace_info(8);
        let pub_inputs = MaatPublicInputs::with_output(BaseElement::new(99));
        let air = MaatAir::new(trace_info, pub_inputs, test_options());

        let rand_elements = AuxRandElements::new(vec![
            BaseElement::new(7),
            BaseElement::new(3),
            BaseElement::new(11),
        ]);
        let assertions = air.get_aux_assertions(&rand_elements);
        assert_eq!(assertions.len(), aux_segment::NUM_AUX_ASSERTIONS);
        // Memory boundary assertions come first.
        assert_eq!(assertions[0].column(), AUX_COL_MEM_ACC);
        assert_eq!(assertions[1].column(), AUX_COL_MEM_ACC);
    }
}
