//! Utilities for STARK proof generation and verification.

pub mod hasher {
    //! Program identity hash for binding proofs to specific bytecode.
    //!
    //! The program hash is a 32-byte Blake3 digest of the serialized bytecode,
    //! split into four Goldilocks field elements. This binds every proof to the
    //! exact program that produced the execution trace.

    use maat_bytecode::Bytecode;
    use maat_errors::ProverError;
    use maat_field::{BaseElement, FieldElement, StarkField};

    pub fn compute_program_hash(bytecode: &Bytecode) -> Result<[BaseElement; 4], ProverError> {
        let bytes = bytecode.serialize()?;
        let hash = blake3::hash(&bytes);
        Ok(hash_bytes_to_field_elements(hash.as_bytes()))
    }

    pub fn compute_program_hash_bytes(bytecode: &Bytecode) -> Result<[u8; 32], ProverError> {
        let bytes = bytecode.serialize()?;
        Ok(*blake3::hash(&bytes).as_bytes())
    }

    pub fn hash_bytes_to_field_elements(hash: &[u8; 32]) -> [BaseElement; 4] {
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
            let elements = hash_bytes_to_field_elements(&hash);
            assert_eq!(elements[0], BaseElement::new(1));
            assert_eq!(elements[1], BaseElement::new(2));
            assert_eq!(elements[2], BaseElement::ZERO);
            assert_eq!(elements[3], BaseElement::ZERO);
        }
    }
}

pub mod proof_serializer {
    //! Proof serialization and deserialization.
    //!
    //! Wire format (version 2):
    //!
    //! ```text
    //! PROOF_MAGIC:        b"MATP"       (4 bytes)
    //! PROOF_VERSION:      u16 BE        (2 bytes, currently 2)
    //! PROGRAM_HASH:       [u8; 32]      (32 bytes, raw Blake3 digest)
    //! OUTPUT:             u64 LE        (8 bytes, claimed program output)
    //! INPUT_COUNT:        u16 BE        (2 bytes, number of public inputs)
    //! INPUTS:             [u64; N] LE   (8 * N bytes, public input values)
    //! PAYLOAD:            Winterfell    (variable, Winterfell's native Proof encoding)
    //! ```
    //!
    //! Minimum header: 48 bytes (with zero inputs).

    use maat_air::Proof;
    use maat_errors::SerializationError;
    use maat_field::BaseElement;

    const PROOF_MAGIC: [u8; 4] = *b"MATP";
    const PROOF_VERSION: u16 = 2;
    // Minimum header size: 4 (magic) + 2 (version) + 32 (hash) + 8 (output) + 2 (input count).
    const MIN_HEADER_SIZE: usize = 48;
    const MAX_INPUT_COUNT: usize = 1024;

    #[derive(Debug, Clone)]
    pub struct ProofPublicInputs {
        pub program_hash: [u8; 32],
        pub output: BaseElement,
        pub inputs: Vec<BaseElement>,
    }

    pub fn serialize_proof(
        proof: &Proof,
        program_hash_bytes: &[u8; 32],
        output: BaseElement,
        inputs: &[BaseElement],
    ) -> Vec<u8> {
        let payload = proof.to_bytes();
        let input_count = inputs.len() as u16;
        let total_size = MIN_HEADER_SIZE + (inputs.len() * 8) + payload.len();

        let mut buf = Vec::with_capacity(total_size);
        buf.extend_from_slice(&PROOF_MAGIC);
        buf.extend_from_slice(&PROOF_VERSION.to_be_bytes());
        buf.extend_from_slice(program_hash_bytes);
        buf.extend_from_slice(&output.as_int().to_le_bytes());
        buf.extend_from_slice(&input_count.to_be_bytes());
        for input in inputs {
            buf.extend_from_slice(&input.as_int().to_le_bytes());
        }
        buf.extend_from_slice(&payload);
        buf
    }

    pub fn deserialize_proof(
        bytes: &[u8],
    ) -> Result<(Proof, ProofPublicInputs), SerializationError> {
        if bytes.len() < 4 {
            return Err(SerializationError::UnexpectedEof {
                offset: 0,
                needed: 4,
            });
        }
        if bytes[..4] != PROOF_MAGIC {
            return Err(SerializationError::InvalidMagic { expected: "MATP" });
        }
        if bytes.len() < 6 {
            return Err(SerializationError::UnexpectedEof {
                offset: 4,
                needed: 2,
            });
        }
        let version = u16::from_be_bytes([bytes[4], bytes[5]]);
        if version != PROOF_VERSION {
            return Err(SerializationError::UnsupportedVersion(version as u64));
        }
        if bytes.len() < MIN_HEADER_SIZE {
            return Err(SerializationError::UnexpectedEof {
                offset: 6,
                needed: MIN_HEADER_SIZE - 6,
            });
        }

        let mut program_hash = [0u8; 32];
        program_hash.copy_from_slice(&bytes[6..38]);

        let output_bytes: [u8; 8] = bytes[38..46].try_into().expect("slice is exactly 8 bytes");
        let output = BaseElement::new(u64::from_le_bytes(output_bytes));

        let input_count = u16::from_be_bytes([bytes[46], bytes[47]]) as usize;
        if input_count > MAX_INPUT_COUNT {
            return Err(SerializationError::ResourceLimitExceeded {
                field: "input_count",
                size: input_count,
                limit: MAX_INPUT_COUNT,
            });
        }

        let inputs_size = input_count * 8;
        let payload_offset = MIN_HEADER_SIZE + inputs_size;
        if bytes.len() < payload_offset {
            return Err(SerializationError::UnexpectedEof {
                offset: MIN_HEADER_SIZE,
                needed: inputs_size,
            });
        }

        let mut inputs = Vec::with_capacity(input_count);
        for i in 0..input_count {
            let start = MIN_HEADER_SIZE + i * 8;
            let input_bytes: [u8; 8] = bytes[start..start + 8]
                .try_into()
                .expect("slice is exactly 8 bytes");
            inputs.push(BaseElement::new(u64::from_le_bytes(input_bytes)));
        }

        let proof = Proof::from_bytes(&bytes[payload_offset..])
            .map_err(|e| SerializationError::WinterfellDecode(e.to_string()))?;

        let public_inputs = ProofPublicInputs {
            program_hash,
            output,
            inputs,
        };

        Ok((proof, public_inputs))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn invalid_magic_rejected() {
            let bytes = b"NOTZ_rest_of_data_here_padding__extra_more_padding_needed";
            let err = deserialize_proof(bytes).unwrap_err();
            assert!(matches!(
                err,
                SerializationError::InvalidMagic { expected: "MATP" }
            ));
        }

        #[test]
        fn unsupported_version_rejected() {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&PROOF_MAGIC);
            bytes.extend_from_slice(&99u16.to_be_bytes());
            bytes.extend_from_slice(&[0u8; 42]); // Fill rest of min header
            let err = deserialize_proof(&bytes).unwrap_err();
            assert!(matches!(err, SerializationError::UnsupportedVersion(99)));
        }

        #[test]
        fn truncated_header_rejected() {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&PROOF_MAGIC);
            bytes.extend_from_slice(&PROOF_VERSION.to_be_bytes());
            bytes.extend_from_slice(&[0u8; 20]); // Incomplete header
            let err = deserialize_proof(&bytes).unwrap_err();
            assert!(matches!(err, SerializationError::UnexpectedEof { .. }));
        }

        #[test]
        fn excessive_input_count_rejected() {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&PROOF_MAGIC);
            bytes.extend_from_slice(&PROOF_VERSION.to_be_bytes());
            bytes.extend_from_slice(&[0u8; 32]); // Program hash
            bytes.extend_from_slice(&0u64.to_le_bytes()); // Output
            bytes.extend_from_slice(&10000u16.to_be_bytes()); // Excessive input count
            let err = deserialize_proof(&bytes).unwrap_err();
            assert!(matches!(
                err,
                SerializationError::ResourceLimitExceeded {
                    field: "input_count",
                    ..
                }
            ));
        }
    }
}
