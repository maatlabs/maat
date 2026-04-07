//! [`MaatAir`]: Winterfell [`Air`] implementation for the Maat CPU.
//!
//! This module wires together the constraint evaluation, boundary assertions,
//! and AIR context required by the STARK prover/verifier.

use maat_trace::{COL_OUT, COL_PC, COL_SP};
use winter_air::{
    Air, AirContext, Assertion, EvaluationFrame, ProofOptions, TraceInfo,
    TransitionConstraintDegree,
};
use winter_math::FieldElement;
use winter_math::fields::f64::BaseElement;

use crate::constraints::{self, CONSTRAINT_DEGREES, NUM_CONSTRAINTS};
use crate::public_inputs::MaatPublicInputs;

/// Algebraic Intermediate Representation for the Maat virtual machine.
///
/// Encodes the execution semantics as 37 polynomial transition constraints
/// and 3 boundary assertions over a 29-column Goldilocks trace.
pub struct MaatAir {
    context: AirContext<BaseElement>,
    public_inputs: MaatPublicInputs,
}

impl Air for MaatAir {
    type BaseField = BaseElement;
    type PublicInputs = MaatPublicInputs;

    fn new(trace_info: TraceInfo, pub_inputs: Self::PublicInputs, options: ProofOptions) -> Self {
        let degrees = CONSTRAINT_DEGREES
            .iter()
            .map(|&d| TransitionConstraintDegree::new(d))
            .collect::<Vec<TransitionConstraintDegree>>();

        assert_eq!(degrees.len(), NUM_CONSTRAINTS);

        let num_assertions = 3;
        let context = AirContext::new(trace_info, degrees, num_assertions, options);

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
        constraints::evaluate(frame.current(), frame.next(), result);
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        let last_step = self.trace_length() - 1;

        vec![
            // pc[0] = 0: execution starts at instruction zero
            Assertion::single(COL_PC, 0, BaseElement::ZERO),
            // sp[0] = 0: empty operand stack at start
            Assertion::single(COL_SP, 0, BaseElement::ZERO),
            // out[last] = public_output: program produces the claimed result
            Assertion::single(COL_OUT, last_step, self.public_inputs.output),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maat_trace::TRACE_WIDTH;

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

    #[test]
    fn air_construction_succeeds() {
        let trace_info = TraceInfo::new(TRACE_WIDTH, 8);
        let pub_inputs = MaatPublicInputs::with_output(BaseElement::new(42));
        let air = MaatAir::new(trace_info, pub_inputs, test_options());

        assert_eq!(air.context().trace_info().width(), TRACE_WIDTH);
        assert_eq!(air.context().num_transition_constraints(), NUM_CONSTRAINTS);
    }

    #[test]
    fn assertions_target_correct_columns() {
        let trace_info = TraceInfo::new(TRACE_WIDTH, 8);
        let pub_inputs = MaatPublicInputs::with_output(BaseElement::new(99));
        let air = MaatAir::new(trace_info, pub_inputs, test_options());

        let assertions = air.get_assertions();
        assert_eq!(assertions.len(), 3);
        assert_eq!(assertions[0].column(), COL_PC);
        assert_eq!(assertions[1].column(), COL_SP);
        assert_eq!(assertions[2].column(), COL_OUT);
    }
}
