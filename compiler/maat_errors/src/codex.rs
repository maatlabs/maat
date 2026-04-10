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

/// Errors arising during bytecode serialization or deserialization.
///
/// Header-level errors (`InvalidMagic`, `UnsupportedVersion`, `UnexpectedEof`)
/// are checked before the payload is decoded. Payload-level errors are
/// reported by `postcard` via `serde` and surfaced as `PostcardEncode` or
/// `PostcardDecode`.
#[derive(Debug, Error)]
pub enum SerializationError {
    /// The file does not begin with the expected `MAAT` magic bytes.
    #[error("invalid magic bytes: expected MAAT header")]
    InvalidMagic,

    /// The format version in the header is not supported by this build.
    #[error("unsupported bytecode format version: {0}")]
    UnsupportedVersion(u32),

    /// The byte stream was truncated before the header could be fully read.
    #[error("unexpected end of bytecode at offset {offset}: needed {needed} more bytes")]
    UnexpectedEof { offset: usize, needed: usize },

    /// An error occurred while encoding bytecode with postcard.
    #[error("bytecode encode error: {0}")]
    PostcardEncode(String),

    /// An error occurred while decoding bytecode with postcard.
    #[error("bytecode decode error: {0}")]
    PostcardDecode(String),

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
