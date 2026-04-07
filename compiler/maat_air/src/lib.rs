//! CPU constraint system (AIR) for the Maat ZK backend.
//!
//! This crate defines [`MaatAir`], an Algebraic Intermediate Representation that
//! encodes the execution semantics of the Maat virtual machine as polynomial
//! constraints over a Goldilocks field trace. Implementing Winterfell's `Air`
//! trait, the AIR is the bridge between the trace-generating VM (`maat_trace`)
//! and the STARK prover (`maat_prover`).
//!
//! # Constraint summary
//!
//! The constraint system enforces:
//!
//! - **Selector validity** (17): one-hot encoding of 16 opcode classes.
//! - **Stack pointer transitions** (5): net SP change per selector class.
//! - **Program counter transitions** (5): PC increment for uniform-width
//!   opcode classes, unconditional and conditional jumps.
//! - **Memory access consistency** (4): load/store read/write flags and values.
//! - **Frame pointer management** (2): FP updates on call and return.
//! - **NOP padding invariance** (3): frozen state during trace padding rows.
//!
//! Total: 36 main transition constraints, all degree <= 3.
//!
//! # Boundary assertions
//!
//! Three assertions anchor the trace to the public inputs:
//! - `pc[0] = 0` (execution begins at instruction zero)
//! - `sp[0] = 0` (empty stack at start)
//! - `out[last] = public_output` (program result matches claimed output)
#![forbid(unsafe_code)]

mod air;
mod constraints;
mod public_inputs;

pub use air::MaatAir;
pub use public_inputs::MaatPublicInputs;

use winter_math::fields::f64::BaseElement;

/// The base field type used throughout the AIR.
///
/// This is the Goldilocks prime field `p = 2^64 - 2^32 + 1`, matching
/// the field used by `maat_field::Felt`.
pub type Felt = BaseElement;
