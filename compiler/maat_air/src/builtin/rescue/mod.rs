//! Rescue hash builtin: `RescueHashBuiltin` (the [`super::Builtin`] trait
//! impl) plus a convenience re-export of the underlying primitive from
//! [`maat_field::rescue`] for AIR-side callers that want to share the
//! constants without an extra import.

mod air;

pub use air::RescueHashBuiltin;
pub use maat_field::rescue::{
    ALPHA, ARK1, ARK2, CAPACITY, CAPACITY_RANGE, DIGEST_RANGE, DIGEST_SIZE, INV_ALPHA, INV_MDS,
    MDS, NUM_ROUNDS, RATE, RATE_RANGE, STATE_WIDTH, hash, rescue_permutation,
};
