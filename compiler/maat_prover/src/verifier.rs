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

use crate::gadgets::proof_serializer::deserialize_proof;
use crate::{development_options, production_options};

pub fn verify(proof_bytes: &[u8]) -> Result<(), VerificationError> {
    let (proof, embedded) = deserialize_proof(proof_bytes)?;
    let public_inputs = MaatPublicInputs::with_segments(
        embedded.inputs,
        embedded.output,
        embedded.output_base,
        embedded.output_segment,
        embedded.program_base,
        embedded.program_segment,
    );
    verify_with_inputs(proof, public_inputs)
}

pub fn verify_with_inputs(proof: Proof, inputs: MaatPublicInputs) -> Result<(), VerificationError> {
    let options = AcceptableOptions::OptionSet(vec![development_options(), production_options()]);
    winter_verifier::verify::<
        MaatAir,
        Blake3_256<BaseElement>,
        DefaultRandomCoin<Blake3_256<BaseElement>>,
        MerkleTree<Blake3_256<BaseElement>>,
    >(proof, inputs, &options)
    .map_err(|e| VerificationError::Rejected(e.to_string()))
}
