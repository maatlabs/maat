//! STARK proof verification for the Maat virtual machine.
//!
//! Provides a thin wrapper around Winterfell's verifier,
//! binding the Maat AIR, hash function, and commitment scheme.

use maat_air::{MaatAir, MaatPublicInputs};
use maat_errors::VerificationError;
use winter_air::proof::Proof;
use winter_crypto::hashers::Blake3_256;
use winter_crypto::{DefaultRandomCoin, MerkleTree};
use winter_math::fields::f64::BaseElement;
use winter_verifier::AcceptableOptions;

use crate::program_hash::hash_bytes_to_elements;

/// Minimum conjectural security level accepted during verification.
///
/// Proof options that do not achieve at least this many bits of
/// conjectured security are rejected before constraint checking begins.
const MIN_SECURITY_BITS: u32 = 0;

/// Verifies a serialized proof file using its embedded public inputs.
///
/// This is the primary verification entry point. The proof file contains
/// all information needed for verification (program hash, inputs, output),
/// so no external arguments are required.
///
/// Returns `Ok(())` if the proof is valid or a [`VerificationError`]
/// describing the failure.
pub fn verify(proof_bytes: &[u8]) -> Result<(), VerificationError> {
    let (proof, embedded) = crate::proof_file::deserialize_proof(proof_bytes)?;
    let program_hash = hash_bytes_to_elements(&embedded.program_hash);
    let public_inputs = MaatPublicInputs::new(program_hash, embedded.inputs, embedded.output);
    verify_with_inputs(proof, public_inputs)
}

/// Verifies a STARK proof against explicitly provided public inputs.
///
/// This is the lower-level verification API for cases where you have
/// already parsed the proof and constructed the public inputs (e.g.,
/// in tests or when building custom verification pipelines).
///
/// For most use cases, prefer [`verify`] which extracts public inputs
/// from the proof file automatically.
pub fn verify_with_inputs(proof: Proof, inputs: MaatPublicInputs) -> Result<(), VerificationError> {
    let acceptable = AcceptableOptions::MinConjecturedSecurity(MIN_SECURITY_BITS);
    winter_verifier::verify::<
        MaatAir,
        Blake3_256<BaseElement>,
        DefaultRandomCoin<Blake3_256<BaseElement>>,
        MerkleTree<Blake3_256<BaseElement>>,
    >(proof, inputs, &acceptable)
    .map_err(|e| VerificationError::Rejected(e.to_string()))
}
