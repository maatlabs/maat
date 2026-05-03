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

#[derive(Debug, Error)]
pub enum SerializationError {
    #[error("invalid magic bytes: expected {expected}")]
    InvalidMagic { expected: &'static str },

    #[error("unsupported format version: {0}")]
    UnsupportedVersion(u64),

    #[error("unexpected end of file at offset {offset}: needed {needed} more bytes")]
    UnexpectedEof { offset: usize, needed: usize },

    #[error("bytecode encode error: {0}")]
    PostcardEncode(String),

    #[error("bytecode decode error: {0}")]
    PostcardDecode(String),

    #[error("proof decode error: {0}")]
    WinterfellDecode(String),

    #[error("bytecode payload too large: {size} bytes exceeds {limit} byte limit")]
    PayloadTooLarge { size: usize, limit: usize },

    #[error("{field} too large: {size} exceeds limit of {limit}")]
    ResourceLimitExceeded {
        field: &'static str,
        size: usize,
        limit: usize,
    },
}

#[derive(Debug, Error)]
pub enum ProverError {
    #[error("trace generation failed: {0}")]
    Trace(String),

    #[error("proof generation failed: {0}")]
    ProvingFailed(String),

    #[error("bytecode serialization failed: {0}")]
    Serialization(#[from] SerializationError),
}

/// Errors arising during proof verification.
#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("proof verification failed: {0}")]
    Rejected(String),

    #[error("proof deserialization failed: {0}")]
    Deserialization(#[from] SerializationError),
}
