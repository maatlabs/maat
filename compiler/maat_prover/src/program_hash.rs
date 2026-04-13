//! Program identity hash for binding proofs to specific bytecode.
//!
//! The program hash is a 32-byte Blake3 digest of the serialized bytecode,
//! split into four Goldilocks field elements. This binds every proof to the
//! exact program that produced the execution trace.

use maat_bytecode::Bytecode;
use maat_errors::ProverError;
use winter_math::fields::f64::BaseElement;
use winter_math::{FieldElement, StarkField};

/// Computes the program hash as four Goldilocks field elements.
///
/// The 32-byte Blake3 digest of the serialized bytecode is partitioned into
/// four 8-byte little-endian limbs, each reduced modulo the Goldilocks prime
/// (`p = 2^64 - 2^32 + 1`).
pub fn compute_program_hash(bytecode: &Bytecode) -> Result<[BaseElement; 4], ProverError> {
    let bytes = bytecode.serialize()?;
    let hash = blake3::hash(&bytes);
    Ok(hash_bytes_to_elements(hash.as_bytes()))
}

/// Computes the raw 32-byte Blake3 digest of the serialized bytecode.
///
/// This is stored in the proof file header so the verifier can reconstruct
/// [`MaatPublicInputs`](maat_air::MaatPublicInputs) without the original bytecode.
pub fn compute_program_hash_bytes(bytecode: &Bytecode) -> Result<[u8; 32], ProverError> {
    let bytes = bytecode.serialize()?;
    Ok(*blake3::hash(&bytes).as_bytes())
}

/// Converts a 32-byte hash into four Goldilocks field elements.
///
/// Each 8-byte chunk is interpreted as a little-endian `u64` and reduced
/// modulo the field prime.
pub fn hash_bytes_to_elements(hash: &[u8; 32]) -> [BaseElement; 4] {
    let mut elements = [BaseElement::ZERO; 4];
    for (i, chunk) in hash.chunks_exact(8).enumerate() {
        let limb = u64::from_le_bytes(chunk.try_into().expect("chunk is exactly 8 bytes"));
        elements[i] = BaseElement::new(limb % BaseElement::MODULUS);
    }
    elements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_hash_compute_deterministic() {
        let bytecode = Bytecode::default();
        let h1 = compute_program_hash(&bytecode).unwrap();
        let h2 = compute_program_hash(&bytecode).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_bytes_to_elements_splits_correctly() {
        let mut hash = [0u8; 32];
        hash[0] = 1; // limb 0 = 1
        hash[8] = 2; // limb 1 = 2
        let elements = hash_bytes_to_elements(&hash);
        assert_eq!(elements[0], BaseElement::new(1));
        assert_eq!(elements[1], BaseElement::new(2));
        assert_eq!(elements[2], BaseElement::ZERO);
        assert_eq!(elements[3], BaseElement::ZERO);
    }
}
