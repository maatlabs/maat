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

/// Verifies a STARK proof against the given public inputs.
///
/// Delegates to Winterfell's verifier with the Maat-specific AIR,
/// Blake3 hashing, and Merkle tree commitment scheme. Returns `Ok(())` if
/// the proof is valid or a [`VerificationError`] describing the failure.
pub fn verify(proof: Proof, inputs: MaatPublicInputs) -> Result<(), VerificationError> {
    let acceptable = AcceptableOptions::MinConjecturedSecurity(MIN_SECURITY_BITS);
    winter_verifier::verify::<
        MaatAir,
        Blake3_256<BaseElement>,
        DefaultRandomCoin<Blake3_256<BaseElement>>,
        MerkleTree<Blake3_256<BaseElement>>,
    >(proof, inputs, &acceptable)
    .map_err(|e| VerificationError::Rejected(e.to_string()))
}

/// Verifies a serialized proof file against known inputs and expected output.
///
/// This is a convenience function that:
/// 1. Deserializes the proof file header and payload.
/// 2. Reconstructs [`MaatPublicInputs`] from the stored program hash and
///    the caller-provided inputs and output.
/// 3. Delegates to [`verify`].
pub fn verify_proof_file(
    proof_bytes: &[u8],
    inputs: Vec<BaseElement>,
    expected_output: BaseElement,
) -> Result<(), VerificationError> {
    let (proof, program_hash_bytes) = crate::proof_file::deserialize_proof(proof_bytes)?;
    let program_hash = hash_bytes_to_elements(&program_hash_bytes);
    let public_inputs = MaatPublicInputs::new(program_hash, inputs, expected_output);
    verify(proof, public_inputs)
}
