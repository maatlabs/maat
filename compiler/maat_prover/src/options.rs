//! Proof-option presets for development and production use.
//!
//! Both presets use [`FieldExtension::Quadratic`] because the auxiliary trace
//! segment (memory permutation + range-check accumulators) evaluates constraints
//! over `QuadExtension<BaseElement>`.

use winter_air::{BatchingMethod, FieldExtension, ProofOptions};

/// Returns proof options tuned for fast iteration during development.
///
/// Security is intentionally minimal (no grinding, few queries) so that
/// proof generation completes in milliseconds on small traces.
pub fn development_options() -> ProofOptions {
    ProofOptions::new(
        4, // num_queries
        8, // blowup_factor
        0, // grinding_factor
        FieldExtension::Quadratic,
        4,   // fri_folding_factor
        255, // fri_remainder_max_degree
        BatchingMethod::Algebraic,
        BatchingMethod::Algebraic,
    )
}

/// Returns proof options for production proofs.
///
/// Targets ~97 bits conjectural security:
/// `27 queries * log2(8) = 81` FRI bits + 16 grinding bits.
/// Proven security is approximately half the conjectured level.
pub fn production_options() -> ProofOptions {
    ProofOptions::new(
        27, // num_queries
        8,  // blowup_factor
        16, // grinding_factor
        FieldExtension::Quadratic,
        8,   // fri_folding_factor
        127, // fri_remainder_max_degree
        BatchingMethod::Algebraic,
        BatchingMethod::Algebraic,
    )
}
