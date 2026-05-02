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

#![forbid(unsafe_code)]

mod gadgets;
mod verifier;

pub use gadgets::hasher::{compute_program_hash, compute_program_hash_bytes};
pub use gadgets::proof_serializer::{ProofPublicInputs, deserialize_proof, serialize_proof};
use maat_air::{AUX_WIDTH, MaatAir, MaatPublicInputs, NUM_AUX_RANDS};
use maat_errors::ProverError;
use maat_trace::table::{TRACE_WIDTH, TraceTable};
pub use verifier::{verify, verify_with_inputs};
use winter_air::{
    AuxRandElements, BatchingMethod, EvaluationFrame, FieldExtension, PartitionOptions,
    ProofOptions, TraceInfo,
};
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

/// Returns proof options tuned for fast iteration during development.
///
/// Security is intentionally minimal (no grinding, few queries) so that
/// proof generation completes in milliseconds on small traces.
pub fn development_options() -> ProofOptions {
    ProofOptions::new(
        4, // num_queries
        8, // blowup_factor
        0, // grinding_factor
        FieldExtension::Quadratic,
        4,   // fri_folding_factor
        255, // fri_remainder_max_degree
        BatchingMethod::Algebraic,
        BatchingMethod::Algebraic,
    )
}

/// Returns proof options for production proofs.
///
/// Targets ~97 bits conjectural security:
/// `27 queries * log2(8) = 81` FRI bits + 16 grinding bits.
/// Proven security is approximately half the conjectured level.
pub fn production_options() -> ProofOptions {
    ProofOptions::new(
        27, // num_queries
        8,  // blowup_factor
        16, // grinding_factor
        FieldExtension::Quadratic,
        8,   // fri_folding_factor
        127, // fri_remainder_max_degree
        BatchingMethod::Algebraic,
        BatchingMethod::Algebraic,
    )
}

/// Execution trace carrying multi-segment [`TraceInfo`].
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
    fn from_trace_table(table: TraceTable) -> MaatTrace {
        let columns: Vec<Vec<BaseElement>> = table.into_columns();

        let trace_length = columns[0].len();
        let info = TraceInfo::new_multi_segment(
            TRACE_WIDTH,
            AUX_WIDTH,
            NUM_AUX_RANDS,
            trace_length,
            Vec::new(),
        );
        let main = ColMatrix::new(columns);

        MaatTrace { info, main }
    }

    fn main_column_slices(&self) -> Vec<&[BaseElement]> {
        (0..TRACE_WIDTH).map(|i| self.main.get_column(i)).collect()
    }
}

/// STARK prover for the Maat virtual machine.
pub struct MaatProver {
    options: ProofOptions,
    inputs: MaatPublicInputs,
}

impl MaatProver {
    pub fn new(options: ProofOptions, inputs: MaatPublicInputs) -> Self {
        Self { options, inputs }
    }

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
        let main_columns = main_trace.main_column_slices();
        let aux_columns =
            maat_air::build_aux_columns(&main_columns, aux_rand_elements.rand_elements());
        ColMatrix::new(aux_columns)
    }
}
