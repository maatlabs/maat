//! CPU constraint system (AIR) for the Maat ZK backend.
//!
//! This crate defines [`MaatAir`], an Algebraic Intermediate Representation that
//! encodes the execution semantics of the Maat virtual machine as polynomial
//! constraints over a Goldilocks field trace. Implementing Winterfell's `Air`
//! trait, the AIR is the bridge between the trace-generating VM (`maat_trace`)
//! and the STARK prover (`maat_prover`).
//!
//! # Constraint summary
//!
//! The constraint system enforces:
//!
//! ## Main segment (29 columns, 37 constraints)
//!
//! - **Selector validity** (17): one-hot encoding of 16 opcode classes.
//! - **Stack pointer transitions** (5): net SP change per selector class.
//! - **Program counter transitions** (5): PC increment for uniform-width
//!   opcode classes, unconditional and conditional jumps.
//! - **Memory access consistency** (4): load/store read/write flags and values.
//! - **Frame pointer management** (2): FP updates on call and return.
//! - **NOP padding invariance** (3): frozen state during trace padding rows.
//!
//! ## Auxiliary segment (5 columns, 3 constraints)
//!
//! - **Address continuity** (1): sorted memory addresses step by at most 1.
//! - **Single-value consistency** (1): same address implies same value.
//! - **Grand-product accumulator** (1): permutation argument proving
//!   execution-order and address-sorted memory lists are the same multiset.
//!
//! Total: 40 transition constraints (37 main + 3 auxiliary), all degree <= 3.
//!
//! # Boundary assertions
//!
//! Five assertions anchor the trace to the public inputs:
//!
//! **Main segment:**
//! - `pc[0] = 0` (execution begins at instruction zero)
//! - `sp[0] = 0` (empty stack at start)
//! - `out[last] = public_output` (program result matches claimed output)
//!
//! **Auxiliary segment:**
//! - `perm_acc[0] = 1` (accumulator multiplicative identity)
//! - `perm_acc[last] = 1` (grand product telescoped to one)
//!
//! # Limitations
//!
//! - Arithmetic/comparison/felt output verification is currently deferred
//!   (requires opcode sub-selectors or auxiliary columns for discrimination).
//! - PC increment for mixed-width selector classes (`sel_push`, `sel_load`,
//!   `sel_construct`, `sel_collection`) is not yet constrained.
//! - The address continuity constraint requires contiguous address allocation;
//!   programs with sparse address spaces may require padding in a future release.
#![forbid(unsafe_code)]

mod aux;
mod constraints;
mod public_inputs;

use aux::{AUX_COL_PERM_ACC, AUX_CONSTRAINT_DEGREES, NUM_AUX_CONSTRAINTS};
pub use aux::{AUX_WIDTH, NUM_AUX_RANDS, build_aux_columns};
use constraints::{CONSTRAINT_DEGREES, NUM_CONSTRAINTS};
use maat_trace::{COL_OUT, COL_PC, COL_SP};
pub use public_inputs::MaatPublicInputs;
use winter_air::{
    Air, AirContext, Assertion, AuxRandElements, EvaluationFrame, ProofOptions, TraceInfo,
    TransitionConstraintDegree,
};
use winter_math::fields::f64::BaseElement;
use winter_math::{ExtensionOf, FieldElement};

/// The base field type used throughout the AIR.
///
/// This is the Goldilocks prime field `p = 2^64 - 2^32 + 1`, matching
/// the field used by `maat_field::Felt`.
pub type Felt = BaseElement;

/// Number of boundary assertions on the main trace segment.
const NUM_MAIN_ASSERTIONS: usize = 3;

/// Number of boundary assertions on the auxiliary trace segment.
const NUM_AUX_ASSERTIONS: usize = 2;

/// Algebraic Intermediate Representation for the Maat virtual machine.
///
/// Encodes the execution semantics as a two-segment STARK constraint system:
///
/// - **Main segment** (29 columns): 37 transition constraints and 3 boundary
///   assertions covering opcode selectors, stack/PC/FP transitions, memory
///   access flags, and NOP padding invariance.
/// - **Auxiliary segment** (5 columns): 3 transition constraints and 2 boundary
///   assertions implementing the memory permutation argument that proves
///   execution-order and address-sorted memory access lists are identical
///   multisets.
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
        assert_eq!(main_degrees.len(), NUM_CONSTRAINTS);

        let aux_degrees = AUX_CONSTRAINT_DEGREES
            .iter()
            .map(|&d| TransitionConstraintDegree::new(d))
            .collect::<Vec<_>>();
        assert_eq!(aux_degrees.len(), NUM_AUX_CONSTRAINTS);

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
        constraints::evaluate(frame.current(), frame.next(), result);
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
        aux::evaluate(
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
            // pc[0] = 0: execution starts at instruction zero.
            Assertion::single(COL_PC, 0, BaseElement::ZERO),
            // sp[0] = 0: empty operand stack at start.
            Assertion::single(COL_SP, 0, BaseElement::ZERO),
            // out[last] = public_output: program produces the claimed result.
            Assertion::single(COL_OUT, last_step, self.public_inputs.output),
        ]
    }

    fn get_aux_assertions<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        _aux_rand_elements: &AuxRandElements<E>,
    ) -> Vec<Assertion<E>> {
        let last_step = self.trace_length() - 1;

        vec![
            // perm_acc[0] = 1: accumulator starts at the multiplicative identity.
            Assertion::single(AUX_COL_PERM_ACC, 0, E::ONE),
            // perm_acc[last] = 1: the grand product telescoped to one, proving
            // that L1 and L2 are permutations of each other.
            Assertion::single(AUX_COL_PERM_ACC, last_step, E::ONE),
        ]
    }
}

#[cfg(test)]
mod tests {
    use maat_trace::TRACE_WIDTH;

    use super::*;
    use crate::aux::{AUX_WIDTH, NUM_AUX_RANDS};

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
    fn aux_assertions_target_perm_acc() {
        let trace_info = multi_segment_trace_info(8);
        let pub_inputs = MaatPublicInputs::with_output(BaseElement::new(99));
        let air = MaatAir::new(trace_info, pub_inputs, test_options());

        let rand_elements = AuxRandElements::new(vec![BaseElement::new(7), BaseElement::new(3)]);
        let assertions = air.get_aux_assertions(&rand_elements);
        assert_eq!(assertions.len(), NUM_AUX_ASSERTIONS);
        assert_eq!(assertions[0].column(), AUX_COL_PERM_ACC);
        assert_eq!(assertions[1].column(), AUX_COL_PERM_ACC);
    }
}
