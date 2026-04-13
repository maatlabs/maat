//! Zero knowledge STARK prover and verifier for Maat.
//!
//! This crate provides [`MaatProver`] which implements Winterfell's [`Prover`] trait.
//! It wires together the Maat AIR constraint system, the trace table, as well as
//! Winterfell's default LDE/evaluator/commitment setup to produce
//! cryptographic proofs of correct program execution.
//!
//! # Architecture
//!
//! ```text
//! Bytecode --> TraceVM --> TraceTable --> MaatProver --> Proof
//!                                            |            |
//!                                            v            v
//!                                     MaatPublicInputs    |
//!                                            |            |
//!                                      verify(proof) <----+
//! ```
//!
//! # Proof generation flow
//!
//! 1. Compile source to [`Bytecode`](maat_bytecode::Bytecode).
//! 2. Run `maat_trace::run_trace(bytecode)` to obtain the execution trace.
//! 3. Construct [`MaatPublicInputs`] from the program hash, inputs, and output.
//! 4. Construct `MaatProver::new(options, public_inputs)`.
//! 5. Call [`MaatProver::generate_proof`] to produce a Winterfell [`Proof`].
//!
//! # Proof options
//!
//! Two presets are provided via [`development_options`] (fast, insecure) and
//! [`production_options`] (~97 bits conjectural security). Both require
//! `FieldExtension::Quadratic` because the auxiliary trace segment evaluates
//! constraints over `QuadExtension<BaseElement>`.
//!
//! # Proof file format
//!
//! Serialized proofs carry a 38-byte header (`MATP` magic, version `u16`,
//! 32-byte Blake3 program hash) followed by Winterfell's native proof encoding.
//! See [`proof_file`] for details.
#![forbid(unsafe_code)]

pub mod options;
pub mod program_hash;
pub mod proof_file;
pub mod verifier;

use maat_air::{AUX_WIDTH, MaatAir, MaatPublicInputs, NUM_AUX_RANDS, build_aux_columns};
use maat_errors::ProverError;
use maat_trace::{TRACE_WIDTH, TraceTable};
pub use options::{development_options, production_options};
pub use program_hash::{compute_program_hash, compute_program_hash_bytes};
pub use proof_file::{deserialize_proof, serialize_proof};
pub use verifier::{verify, verify_proof_file};
use winter_air::{AuxRandElements, EvaluationFrame, PartitionOptions, ProofOptions, TraceInfo};
use winter_crypto::hashers::Blake3_256;
use winter_crypto::{DefaultRandomCoin, MerkleTree};
use winter_math::FieldElement;
use winter_math::fields::f64::BaseElement;
use winter_prover::matrix::ColMatrix;
use winter_prover::{
    CompositionPoly, CompositionPolyTrace, ConstraintCompositionCoefficients,
    DefaultConstraintEvaluator, DefaultTraceLde, Proof, Prover, StarkDomain, Trace, TracePolyTable,
};

/// Type alias for the hash function used throughout proof generation.
type HashFn = Blake3_256<BaseElement>;

/// Type alias for the vector commitment (Merkle tree) scheme.
type VC = MerkleTree<HashFn>;

/// Execution trace carrying multi-segment [`TraceInfo`].
///
/// Winterfell's [`TraceTable`](winter_prover::TraceTable) always creates
/// single-segment metadata, but the Maat AIR declares an auxiliary segment
/// (8 columns, 3 random elements) via Winterfell's
/// [`AirContext::new_multi_segment`](winter_air::AirContext::new_multi_segment()).
/// `MaatTrace` bridges this gap by pairing the main-segment column matrix
/// with a [`TraceInfo`] that declares the auxiliary segment dimensions,
/// allowing the prover to discover and build the auxiliary trace during
/// proof generation.
pub struct MaatTrace {
    info: TraceInfo,
    main: ColMatrix<BaseElement>,
}

impl Trace for MaatTrace {
    type BaseField = BaseElement;

    fn info(&self) -> &TraceInfo {
        &self.info
    }

    fn main_segment(&self) -> &ColMatrix<BaseElement> {
        &self.main
    }

    fn read_main_frame(&self, row_idx: usize, frame: &mut EvaluationFrame<BaseElement>) {
        let next_row_idx = (row_idx + 1) % self.info.length();
        self.main.read_row_into(row_idx, frame.current_mut());
        self.main.read_row_into(next_row_idx, frame.next_mut());
    }
}

impl MaatTrace {
    /// Converts a Maat trace table into a [`MaatTrace`].
    ///
    /// The Maat trace table stores rows of [`maat_field::Felt`]. This function
    /// transposes the row-major matrix into column-major [`ColMatrix`] and
    /// pairs it with Winterfell's [`TraceInfo`] that declares the
    /// auxiliary segment dimensions required by [`MaatAir`].
    fn from_trace_table(table: TraceTable) -> MaatTrace {
        let columns = table
            .into_columns()
            .into_iter()
            .map(|col| col.into_iter().map(|f| f.into_base_element()).collect())
            .collect::<Vec<Vec<BaseElement>>>();

        let trace_length = columns[0].len();
        let info = TraceInfo::new_multi_segment(
            TRACE_WIDTH,
            AUX_WIDTH,
            NUM_AUX_RANDS,
            trace_length,
            vec![],
        );
        let main = ColMatrix::new(columns);

        MaatTrace { info, main }
    }

    /// Extracts column-major data from a [`MaatTrace`].
    ///
    /// The `build_aux_columns` function in `maat_air` expects `&[Vec<BaseElement>]`.
    /// This function copies each column from the [`ColMatrix`] into an owned `Vec`
    /// for the auxiliary builder.
    fn extract_main_columns(&self) -> Vec<Vec<BaseElement>> {
        (0..TRACE_WIDTH)
            .map(|i| self.main.get_column(i).to_vec())
            .collect()
    }
}

/// STARK prover for the Maat virtual machine.
///
/// Holds the proof options and public inputs needed to construct the
/// AIR during proof generation.
pub struct MaatProver {
    options: ProofOptions,
    inputs: MaatPublicInputs,
}

impl MaatProver {
    /// Creates a new prover with the given proof options and public inputs.
    pub fn new(options: ProofOptions, inputs: MaatPublicInputs) -> Self {
        Self { options, inputs }
    }

    /// Generates a STARK proof from a Maat execution trace.
    ///
    /// Converts the Maat trace table into Winterfell's column-major format
    /// and delegates to Winterfell's prover. Returns the serializable
    /// [`Proof`] on success.
    pub fn generate_proof(self, table: TraceTable) -> Result<Proof, ProverError> {
        let trace = MaatTrace::from_trace_table(table);
        self.prove(trace)
            .map_err(|e| ProverError::ProvingFailed(e.to_string()))
    }
}

impl Prover for MaatProver {
    type BaseField = BaseElement;
    type Air = MaatAir;
    type Trace = MaatTrace;
    type HashFn = HashFn;
    type VC = VC;
    type RandomCoin = DefaultRandomCoin<HashFn>;
    type TraceLde<E: FieldElement<BaseField = BaseElement>> = DefaultTraceLde<E, HashFn, VC>;
    type ConstraintEvaluator<'a, E: FieldElement<BaseField = BaseElement>> =
        DefaultConstraintEvaluator<'a, MaatAir, E>;
    type ConstraintCommitment<E: FieldElement<BaseField = BaseElement>> =
        winter_prover::DefaultConstraintCommitment<E, HashFn, VC>;

    fn get_pub_inputs(&self, _trace: &Self::Trace) -> MaatPublicInputs {
        self.inputs.clone()
    }

    fn options(&self) -> &ProofOptions {
        &self.options
    }

    fn new_trace_lde<E: FieldElement<BaseField = BaseElement>>(
        &self,
        trace_info: &TraceInfo,
        main_trace: &ColMatrix<BaseElement>,
        domain: &StarkDomain<BaseElement>,
        partition_option: PartitionOptions,
    ) -> (Self::TraceLde<E>, TracePolyTable<E>) {
        DefaultTraceLde::new(trace_info, main_trace, domain, partition_option)
    }

    fn new_evaluator<'a, E: FieldElement<BaseField = BaseElement>>(
        &self,
        air: &'a Self::Air,
        aux_rand_elements: Option<AuxRandElements<E>>,
        composition_coefficients: ConstraintCompositionCoefficients<E>,
    ) -> Self::ConstraintEvaluator<'a, E> {
        DefaultConstraintEvaluator::new(air, aux_rand_elements, composition_coefficients)
    }

    fn build_constraint_commitment<E: FieldElement<BaseField = BaseElement>>(
        &self,
        composition_poly_trace: CompositionPolyTrace<E>,
        num_constraint_composition_columns: usize,
        domain: &StarkDomain<BaseElement>,
        partition_options: PartitionOptions,
    ) -> (Self::ConstraintCommitment<E>, CompositionPoly<E>) {
        winter_prover::DefaultConstraintCommitment::new(
            composition_poly_trace,
            num_constraint_composition_columns,
            domain,
            partition_options,
        )
    }

    fn build_aux_trace<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_trace: &Self::Trace,
        aux_rand_elements: &AuxRandElements<E>,
    ) -> ColMatrix<E> {
        let main_columns = main_trace.extract_main_columns();
        let aux_columns = build_aux_columns(&main_columns, aux_rand_elements.rand_elements());
        ColMatrix::new(aux_columns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::development_options;

    #[test]
    fn prover_construction() {
        let pub_inputs = MaatPublicInputs::with_output(BaseElement::new(42));
        let prover = MaatProver::new(development_options(), pub_inputs);
        assert_eq!(prover.options().num_queries(), 4);
    }
}
