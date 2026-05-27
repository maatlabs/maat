//! Rescue-Prime hash primitive over the Goldilocks field.
//!
//! Parameters match Winterfell's `winter_crypto::hash::rescue::rp64_256`:
//! Goldilocks (`p = 2^64 - 2^32 + 1`), state width 12, capacity 4, rate 8,
//! digest size 4, 7 rounds, S-box `x^7`, inverse S-box `x^INV_ALPHA`,
//! Polygon-Zero MDS matrix. The shared parameter set lets Winterfell's
//! published test vectors cross-check this implementation directly.

mod constants;
mod permutation;

pub use constants::{ARK1, ARK2, INV_MDS, MDS};
use maat_field::{BaseElement, FieldElement};
pub use permutation::rescue_permutation;

/// Sponge state width, in field elements (96 bytes total).
pub const STATE_WIDTH: usize = 12;

/// Capacity portion of the state (elements `0..4`).
pub const CAPACITY: usize = 4;

/// Capacity-side range.
pub const CAPACITY_RANGE: std::ops::Range<usize> = 0..CAPACITY;

/// Rate portion of the state (elements `4..12`).
pub const RATE: usize = STATE_WIDTH - CAPACITY;

/// Rate-side range.
pub const RATE_RANGE: std::ops::Range<usize> = CAPACITY..STATE_WIDTH;

/// Digest output width (first four rate elements).
pub const DIGEST_SIZE: usize = 4;

/// Digest-side range.
pub const DIGEST_RANGE: std::ops::Range<usize> = CAPACITY..(CAPACITY + DIGEST_SIZE);

/// Number of permutation rounds.
pub const NUM_ROUNDS: usize = 7;

/// Forward S-box exponent.
pub const ALPHA: u64 = 7;

/// Inverse S-box exponent. The multiplicative inverse of `ALPHA` modulo
/// `p - 1 = 2^64 - 2^32` over the Goldilocks field.
pub const INV_ALPHA: u64 = 10540996611094048183;

/// Hashes a sequence of base-field elements via the Rescue-Prime sponge.
pub fn hash(input: &[BaseElement]) -> [BaseElement; DIGEST_SIZE] {
    let mut state = [BaseElement::ZERO; STATE_WIDTH];
    state[CAPACITY_RANGE.start] = BaseElement::new(input.len() as u64);

    let mut rate_cursor = 0usize;
    for &element in input {
        state[RATE_RANGE.start + rate_cursor] += element;
        rate_cursor += 1;
        if rate_cursor == RATE {
            rescue_permutation(&mut state);
            rate_cursor = 0;
        }
    }
    if rate_cursor > 0 {
        rescue_permutation(&mut state);
    }

    let mut digest = [BaseElement::ZERO; DIGEST_SIZE];
    digest.copy_from_slice(&state[DIGEST_RANGE]);
    digest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_invariant_distinguishes_trailing_zeros() {
        let zero = BaseElement::ZERO;
        let a = hash(&[]);
        let b = hash(&[zero]);
        let c = hash(&[zero, zero]);
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }

    #[test]
    fn rate_block_boundary_distinguishes_lengths() {
        let inputs: Vec<BaseElement> = (1..=10).map(BaseElement::new).collect();
        let h7 = hash(&inputs[..7]);
        let h8 = hash(&inputs[..8]);
        let h9 = hash(&inputs[..9]);
        assert_ne!(h7, h8);
        assert_ne!(h8, h9);
    }

    #[test]
    fn hash_is_deterministic() {
        let input: Vec<BaseElement> = (0..17).map(BaseElement::new).collect();
        let h1 = hash(&input);
        let h2 = hash(&input);
        assert_eq!(h1, h2);
    }

    #[test]
    fn distinct_single_elements_hash_differently() {
        let h_zero = hash(&[BaseElement::ZERO]);
        let h_one = hash(&[BaseElement::ONE]);
        assert_ne!(h_zero, h_one);
    }

    #[test]
    fn digest_range_matches_first_four_rate_elements() {
        let input = [BaseElement::new(42)];
        let digest = hash(&input);

        let mut state = [BaseElement::ZERO; STATE_WIDTH];
        state[CAPACITY_RANGE.start] = BaseElement::new(1);
        state[RATE_RANGE.start] += input[0];
        rescue_permutation(&mut state);

        let expected: [BaseElement; DIGEST_SIZE] = state[DIGEST_RANGE].try_into().unwrap();
        assert_eq!(digest, expected);
    }

    #[test]
    fn parameter_constants_are_consistent() {
        assert_eq!(CAPACITY + RATE, STATE_WIDTH);
        assert_eq!(CAPACITY_RANGE.end, RATE_RANGE.start);
        assert_eq!(RATE_RANGE.end, STATE_WIDTH);
        assert_eq!(DIGEST_RANGE.start, CAPACITY);
        assert_eq!(DIGEST_RANGE.end - DIGEST_RANGE.start, DIGEST_SIZE);
        const { assert!(DIGEST_SIZE <= RATE) };
    }
}
