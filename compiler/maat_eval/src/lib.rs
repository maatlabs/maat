//! Macro expansion engine for Maat.
//!
//! This crate provides the macro system that powers Maat's metaprogramming
//! capabilities. It exposes two primary operations:
//!
//! - [`define_macros`] — Extracts macro definitions from a program and stores
//!   them in the environment.
//! - [`expand_macros`] — Replaces macro calls in the AST with their expanded forms.
//!
//! Internally, macro bodies are evaluated using a tree-walking interpreter
//! ([`eval`]) that is not exposed as a general-purpose execution API. Program
//! execution is handled exclusively by the bytecode VM (`maat_vm`).
#![forbid(unsafe_code)]

mod interpreter;
mod macros;

pub use interpreter::eval;
pub use macros::{define_macros, expand_macros};
