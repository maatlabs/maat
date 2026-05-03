//! STARK proof verification for the Maat virtual machine.
//!
//! Provides a thin wrapper around Winterfell's verifier,
//! binding the Maat AIR, hash function, and commitment scheme.

use maat_air::{MaatAir, MaatPublicInputs, Proof};
use maat_errors::VerificationError;
use maat_field::BaseElement;
use winter_crypto::hashers::Blake3_256;
use winter_crypto::{DefaultRandomCoin, MerkleTree};
use winter_verifier::AcceptableOptions;

use crate::gadgets::hasher::hash_bytes_to_field_elements;
use crate::gadgets::proof_serializer::deserialize_proof;

const MIN_SECURITY_BITS: u32 = 0;

pub fn verify(proof_bytes: &[u8]) -> Result<(), VerificationError> {
    let (proof, embedded) = deserialize_proof(proof_bytes)?;
    let program_hash = hash_bytes_to_field_elements(&embedded.program_hash);
    let public_inputs = MaatPublicInputs::new(program_hash, embedded.inputs, embedded.output);
    verify_with_inputs(proof, public_inputs)
}

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
