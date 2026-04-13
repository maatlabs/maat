//! Proof file serialization and deserialization.
//!
//! Wire format:
//!
//! ```text
//! PROOF_MAGIC:        b"MATP"    (4 bytes)
//! PROOF_VERSION:      u16 BE     (2 bytes, currently 1)
//! PROGRAM_HASH:       [u8; 32]   (32 bytes, raw Blake3 digest)
//! PAYLOAD:            Winterfell  (variable, Winterfell's native Proof encoding)
//! ```
//!
//! Total header: 38 bytes.

use maat_errors::SerializationError;
use winter_air::proof::Proof;

/// Magic bytes identifying a Maat proof file.
const PROOF_MAGIC: [u8; 4] = *b"MATP";

/// Current proof file format version.
const PROOF_VERSION: u16 = 1;

/// Fixed header size: 4 (magic) + 2 (version) + 32 (hash).
const HEADER_SIZE: usize = 38;

/// Serializes a STARK proof with its program hash into a self-contained byte vector.
///
/// The result can be written directly to a `.proof.bin` file and later
/// deserialized with [`deserialize_proof`].
pub fn serialize_proof(proof: &Proof, program_hash_bytes: &[u8; 32]) -> Vec<u8> {
    let payload = proof.to_bytes();
    let mut buf = Vec::with_capacity(HEADER_SIZE + payload.len());
    buf.extend_from_slice(&PROOF_MAGIC);
    buf.extend_from_slice(&PROOF_VERSION.to_be_bytes());
    buf.extend_from_slice(program_hash_bytes);
    buf.extend_from_slice(&payload);
    buf
}

/// Deserializes a proof file, returning the STARK proof and the raw program hash.
///
/// The caller can reconstruct [`MaatPublicInputs`](maat_air::MaatPublicInputs)
/// from the returned hash bytes and the expected inputs/output.
pub fn deserialize_proof(bytes: &[u8]) -> Result<(Proof, [u8; 32]), SerializationError> {
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
    if bytes.len() < HEADER_SIZE {
        return Err(SerializationError::UnexpectedEof {
            offset: 6,
            needed: 32,
        });
    }
    let mut program_hash = [0u8; 32];
    program_hash.copy_from_slice(&bytes[6..38]);

    let proof = Proof::from_bytes(&bytes[HEADER_SIZE..])
        .map_err(|e| SerializationError::WinterfellDecode(e.to_string()))?;

    Ok((proof, program_hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_magic_rejected() {
        let bytes = b"NOTZ_rest_of_data_here_padding__extra";
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
        bytes.extend_from_slice(&[0u8; 32]);
        let err = deserialize_proof(&bytes).unwrap_err();
        assert!(matches!(err, SerializationError::UnsupportedVersion(99)));
    }

    #[test]
    fn truncated_header_rejected() {
        let bytes = b"MATP\x00\x01"; // 6 bytes, missing 32-byte hash
        let err = deserialize_proof(bytes).unwrap_err();
        assert!(matches!(err, SerializationError::UnexpectedEof { .. }));
    }
}
