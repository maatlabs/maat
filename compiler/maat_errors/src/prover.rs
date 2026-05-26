use thiserror::Error;

use crate::codex::SerializationError;

/// Errors arising during proof generation.
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

/// Errors raised by the LogUp lookup-argument primitive during table
/// registration, lookup recording, or witness-column construction.
#[derive(Debug, Error)]
pub enum LogUpError {
    #[error("duplicate table id: {0}")]
    DuplicateTable(u32),

    #[error("unknown table id: {0}")]
    UnknownTable(u32),

    #[error("table {table}: lookup value {value} is not a registered entry")]
    ValueNotInTable { table: u32, value: u64 },

    #[error(
        "trace length {trace_len} is too small for table size {table_size} and lookup count \
         {lookup_count} (need >= 1 + max(table_size, lookup_count))"
    )]
    TraceTooSmall {
        trace_len: usize,
        table_size: usize,
        lookup_count: usize,
    },

    #[error("table {0} is empty but received {1} lookup(s)")]
    EmptyTableWithLookups(u32, usize),
}
