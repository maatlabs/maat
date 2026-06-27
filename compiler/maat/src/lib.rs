//! The Maat programming language, as a library.
//!
//! `maat` is the umbrella crate for the Maat compiler and zero-knowledge proof
//! toolchain. It re-exports every member crate under a short, namespaced
//! module so downstream Rust tooling---recursive provers, STARK-to-SNARK
//! wrappers, audit harnesses, editor integrations---can depend on a single
//! `maat` crate rather than the individual `maat_*` crates.
//!
//! Each module is the corresponding member crate verbatim, so the full surface
//! of every compiler stage is reachable through it (`maat::air`,
//! `maat::prover`, `maat::types`, ...). The [`prelude`] gathers the handful of
//! entry points most callers need to compile, execute, prove, and verify a
//! program:
//!
//! ```
//! use maat::prelude::*;
//!
//! // The proof options the prover and verifier agree on.
//! let _opts = development_options();
//! ```
#![forbid(unsafe_code)]

pub use maat_air as air;
pub use maat_ast as ast;
pub use maat_bytecode as bytecode;
pub use maat_codegen as codegen;
pub use maat_errors as errors;
pub use maat_eval as eval;
pub use maat_field as field;
pub use maat_lexer as lexer;
pub use maat_module as module;
pub use maat_parser as parser;
pub use maat_prover as prover;
pub use maat_runtime as runtime;
pub use maat_span as span;
pub use maat_stdlib as stdlib;
pub use maat_trace as trace;
pub use maat_types as types;
pub use maat_vm as vm;

/// The common entry points for compiling, executing, proving, and verifying a
/// Maat program. Glob-import with `use maat::prelude::*;`.
pub mod prelude {
    pub use crate::air::{MaatPublicInputs, Proof, PublicMemory, PublicSegment};
    pub use crate::bytecode::Bytecode;
    pub use crate::field::Felt;
    pub use crate::module::{
        ModuleGraph, check_and_compile, main_entry_arity, resolve_module_graph,
    };
    pub use crate::prover::{
        MaatProver, PublicIo, deserialize_proof, development_options, extract_public_io,
        production_options, serialize_proof, verify, verify_with_inputs,
    };
    pub use crate::runtime::Value;
    pub use crate::trace::{run_with_inputs, run_with_io, run_with_output};
    pub use crate::vm::VM;
}
