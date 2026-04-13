use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error(
        "bytecode truncated: needed {needed} bytes at offset {offset}, but only {available} bytes available"
    )]
    UnexpectedEndOfBytecode {
        offset: usize,
        needed: usize,
        available: usize,
    },

    #[error("unsupported operand width: {0} (valid widths: 1, 2, 4, 8)")]
    UnsupportedOperandWidth(usize),

    #[error("invalid opcode: 0x{0:02x}")]
    InvalidOpcode(u8),
}

/// Errors arising during binary serialization or deserialization.
///
/// Shared by both the bytecode (`.mtc`) and proof (`.proof.bin`) file
/// formats. Header-level errors (`InvalidMagic`, `UnsupportedVersion`,
/// `UnexpectedEof`) are checked before the payload is decoded.
#[derive(Debug, Error)]
pub enum SerializationError {
    /// The file does not begin with the expected magic bytes.
    #[error("invalid magic bytes: expected {expected}")]
    InvalidMagic { expected: &'static str },

    /// The format version in the header is not supported by this build.
    #[error("unsupported format version: {0}")]
    UnsupportedVersion(u64),

    /// The byte stream was truncated before the header could be fully read.
    #[error("unexpected end of file at offset {offset}: needed {needed} more bytes")]
    UnexpectedEof { offset: usize, needed: usize },

    /// An error occurred while encoding bytecode with postcard.
    #[error("bytecode encode error: {0}")]
    PostcardEncode(String),

    /// An error occurred while decoding bytecode with postcard.
    #[error("bytecode decode error: {0}")]
    PostcardDecode(String),

    /// An error occurred while decoding a Winterfell proof.
    #[error("proof decode error: {0}")]
    WinterfellDecode(String),

    /// The bytecode payload exceeds the maximum allowed size.
    #[error("bytecode payload too large: {size} bytes exceeds {limit} byte limit")]
    PayloadTooLarge { size: usize, limit: usize },

    /// A deserialized field exceeds its allowed resource limit.
    #[error("{field} too large: {size} exceeds limit of {limit}")]
    ResourceLimitExceeded {
        field: &'static str,
        size: usize,
        limit: usize,
    },
}

/// Errors arising during proof generation.
#[derive(Debug, Error)]
pub enum ProverError {
    /// The trace-generating VM produced an error.
    #[error("trace generation failed: {0}")]
    Trace(String),

    /// The Winterfell prover rejected the trace or encountered an internal error.
    #[error("proof generation failed: {0}")]
    ProvingFailed(String),

    /// Bytecode serialization failed during program hash computation.
    #[error("bytecode serialization failed: {0}")]
    Serialization(#[from] SerializationError),
}

/// Errors arising during proof verification.
#[derive(Debug, Error)]
pub enum VerificationError {
    /// The Winterfell verifier rejected the proof.
    #[error("proof verification failed: {0}")]
    Rejected(String),

    /// The proof file could not be deserialized.
    #[error("proof deserialization failed: {0}")]
    Deserialization(#[from] SerializationError),
}
