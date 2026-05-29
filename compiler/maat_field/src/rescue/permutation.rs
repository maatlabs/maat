//! Rescue-Prime permutation over the Goldilocks field.
//!
//! Per-round structure (one of `NUM_ROUNDS`):
//!
//! ```text
//! state -> apply_sbox(state)                (element-wise x -> x^ALPHA)
//!       -> apply_mds(state)                 (12x12 matrix multiply)
//!       -> add_constants(state, ARK1[round])
//!       -> apply_inv_sbox(state)            (element-wise x -> x^INV_ALPHA)
//!       -> apply_mds(state)                 (12x12 matrix multiply)
//!       -> add_constants(state, ARK2[round])
//! ```
//!
//! MDS multiplication runs at `STATE_WIDTH^2 = 144` field operations per
//! call. A frequency-domain alternative is ~3x faster; the naive form is
//! retained until callers surface a measurable cost.

use super::constants::{ARK1, ARK2, MDS};
use super::{NUM_ROUNDS, STATE_WIDTH};
use crate::{BaseElement, FieldElement};

/// Per-round witness captured during a Rescue-Prime permutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RescueRoundWitness {
    pub state_in: [BaseElement; STATE_WIDTH],
    pub state_after_sbox: [BaseElement; STATE_WIDTH],
    pub state_after_inv_sbox: [BaseElement; STATE_WIDTH],
}

/// Applies the Rescue-Prime permutation in place.
pub fn rescue_permutation(state: &mut [BaseElement; STATE_WIDTH]) {
    for round in 0..NUM_ROUNDS {
        apply_round(state, round);
    }
}

/// Applies the Rescue-Prime permutation in place and returns the per-round
/// witness vectors the AIR commits to.
///
/// Callers that do not need the witness should prefer
/// [`rescue_permutation`], which avoids three `STATE_WIDTH`-wide copies per
/// round.
pub fn rescue_permutation_with_witness(
    state: &mut [BaseElement; STATE_WIDTH],
) -> [RescueRoundWitness; NUM_ROUNDS] {
    std::array::from_fn(|round| apply_round_with_witness(state, round))
}

#[inline]
fn apply_round(state: &mut [BaseElement; STATE_WIDTH], round: usize) {
    apply_sbox(state);
    apply_mds(state);
    add_constants(state, &ARK1[round]);

    apply_inv_sbox(state);
    apply_mds(state);
    add_constants(state, &ARK2[round]);
}

#[inline]
fn apply_round_with_witness(
    state: &mut [BaseElement; STATE_WIDTH],
    round: usize,
) -> RescueRoundWitness {
    let state_in = *state;

    apply_sbox(state);
    let state_after_sbox = *state;

    apply_mds(state);
    add_constants(state, &ARK1[round]);

    apply_inv_sbox(state);
    let state_after_inv_sbox = *state;

    apply_mds(state);
    add_constants(state, &ARK2[round]);

    RescueRoundWitness {
        state_in,
        state_after_sbox,
        state_after_inv_sbox,
    }
}

#[inline]
fn apply_sbox(state: &mut [BaseElement; STATE_WIDTH]) {
    for cell in state.iter_mut() {
        *cell = cell.exp7();
    }
}

#[inline]
fn apply_inv_sbox(state: &mut [BaseElement; STATE_WIDTH]) {
    let mut t1 = *state;
    t1.iter_mut().for_each(|t| *t = t.square());

    let mut t2 = t1;
    t2.iter_mut().for_each(|t| *t = t.square());

    let t3 = exp_acc::<3>(t2, t2);
    let t4 = exp_acc::<6>(t3, t3);
    let t5 = exp_acc::<12>(t4, t4);
    let t6 = exp_acc::<6>(t5, t3);
    let t7 = exp_acc::<31>(t6, t6);

    for (i, s) in state.iter_mut().enumerate() {
        let a = (t7[i].square() * t6[i]).square().square();
        let b = t1[i] * t2[i] * *s;
        *s = a * b;
    }
}

#[inline]
fn apply_mds(state: &mut [BaseElement; STATE_WIDTH]) {
    let input = *state;
    for (i, row) in MDS.iter().enumerate() {
        let mut acc = BaseElement::ZERO;
        for j in 0..STATE_WIDTH {
            acc += row[j] * input[j];
        }
        state[i] = acc;
    }
}

#[inline]
fn add_constants(state: &mut [BaseElement; STATE_WIDTH], ark: &[BaseElement; STATE_WIDTH]) {
    state.iter_mut().zip(ark).for_each(|(s, &k)| *s += k);
}

/// `base^(2^N) * tail` with `N` squarings and one multiplication per
/// element. Walks one segment of the INV_ALPHA addition chain.
#[inline(always)]
fn exp_acc<const N: usize>(
    base: [BaseElement; STATE_WIDTH],
    tail: [BaseElement; STATE_WIDTH],
) -> [BaseElement; STATE_WIDTH] {
    let mut result = base;
    for _ in 0..N {
        result.iter_mut().for_each(|r| *r = r.square());
    }
    result.iter_mut().zip(tail).for_each(|(r, t)| *r *= t);
    result
}

#[cfg(test)]
mod tests {
    use super::super::constants::INV_MDS;
    use super::super::{ALPHA, INV_ALPHA};
    use super::*;
    use crate::StarkField;

    /// Cross-check against the canonical Rescue-Prime test vector from
    /// the Winterfell `rp64_256` suite (computed via the sage reference).
    #[test]
    fn permutation_matches_upstream_test_vector() {
        let mut state: [BaseElement; STATE_WIDTH] =
            std::array::from_fn(|i| BaseElement::new(i as u64));
        rescue_permutation(&mut state);

        let expected = [
            BaseElement::new(11084501481526603421),
            BaseElement::new(6291559951628160880),
            BaseElement::new(13626645864671311919),
            BaseElement::new(18397438323058963117),
            BaseElement::new(7443014167353970324),
            BaseElement::new(17930833023906771425),
            BaseElement::new(4275355080008025761),
            BaseElement::new(7676681476902901785),
            BaseElement::new(3460534574143792217),
            BaseElement::new(11912731278641497187),
            BaseElement::new(8104899243369883110),
            BaseElement::new(674509706691634438),
        ];
        assert_eq!(state, expected);
    }

    /// MDS times INV_MDS must equal the identity matrix.
    #[test]
    #[allow(clippy::needless_range_loop)]
    fn mds_inv_test() {
        for i in 0..STATE_WIDTH {
            for j in 0..STATE_WIDTH {
                let mut entry = BaseElement::ZERO;
                for k in 0..STATE_WIDTH {
                    entry += MDS[i][k] * INV_MDS[k][j];
                }
                if i == j {
                    assert_eq!(entry, BaseElement::ONE, "diag [{i}][{j}]");
                } else {
                    assert_eq!(entry, BaseElement::ZERO, "off-diag [{i}][{j}]");
                }
            }
        }
    }

    #[test]
    fn sbox_matches_exp_alpha() {
        let state: [BaseElement; STATE_WIDTH] =
            std::array::from_fn(|i| BaseElement::new((i as u64) * 1_000 + 7));
        let mut by_sbox = state;
        apply_sbox(&mut by_sbox);
        let by_exp: [BaseElement; STATE_WIDTH] = std::array::from_fn(|i| state[i].exp(ALPHA));
        assert_eq!(by_sbox, by_exp);
    }

    #[test]
    fn inv_sbox_matches_exp_inv_alpha() {
        let state: [BaseElement; STATE_WIDTH] =
            std::array::from_fn(|i| BaseElement::new((i as u64) * 1_000 + 7));
        let mut by_inv_sbox = state;
        apply_inv_sbox(&mut by_inv_sbox);
        let by_exp: [BaseElement; STATE_WIDTH] = std::array::from_fn(|i| state[i].exp(INV_ALPHA));
        assert_eq!(by_inv_sbox, by_exp);
    }

    #[test]
    fn sbox_round_trips_through_inv_sbox() {
        let state: [BaseElement; STATE_WIDTH] =
            std::array::from_fn(|i| BaseElement::new((i as u64) * 7919 + 1));
        let mut t = state;
        apply_sbox(&mut t);
        apply_inv_sbox(&mut t);
        assert_eq!(t, state);
    }

    /// `ALPHA * INV_ALPHA = 1 (mod p - 1)` over the Goldilocks field.
    #[test]
    fn alpha_inv_alpha_are_multiplicative_inverses_mod_phi() {
        let phi = BaseElement::MODULUS - 1;
        let product = (ALPHA as u128) * (INV_ALPHA as u128);
        assert_eq!(product % (phi as u128), 1u128);
    }

    #[test]
    fn permutation_with_witness_matches_plain_permutation() {
        let mut a: [BaseElement; STATE_WIDTH] = std::array::from_fn(|i| BaseElement::new(i as u64));
        let mut b = a;

        rescue_permutation(&mut a);
        let witness = rescue_permutation_with_witness(&mut b);

        assert_eq!(a, b, "final states differ between witness/no-witness paths");
        assert_eq!(witness.len(), NUM_ROUNDS);
    }

    #[test]
    fn witness_sbox_equals_state_in_to_the_alpha() {
        let mut state: [BaseElement; STATE_WIDTH] =
            std::array::from_fn(|i| BaseElement::new(i as u64 + 100));
        let witness = rescue_permutation_with_witness(&mut state);
        for w in witness.iter() {
            for i in 0..STATE_WIDTH {
                assert_eq!(w.state_after_sbox[i], w.state_in[i].exp(ALPHA));
            }
        }
    }

    #[test]
    fn witness_inv_sbox_cross_multiplied_equals_mds_after_sbox_plus_ark1() {
        use super::super::constants::{ARK1, MDS};
        let mut state: [BaseElement; STATE_WIDTH] =
            std::array::from_fn(|i| BaseElement::new(i as u64 + 100));
        let witness = rescue_permutation_with_witness(&mut state);
        for (round, w) in witness.iter().enumerate() {
            // expected[i] = sum_j MDS[i][j] * state_after_sbox[j] + ARK1[round][i]
            let expected: [BaseElement; STATE_WIDTH] = std::array::from_fn(|i| {
                let acc = MDS[i]
                    .iter()
                    .zip(w.state_after_sbox.iter())
                    .fold(BaseElement::ZERO, |acc, (&m, &s)| acc + m * s);
                acc + ARK1[round][i]
            });
            for (i, &cell) in w.state_after_inv_sbox.iter().enumerate() {
                assert_eq!(
                    cell.exp(ALPHA),
                    expected[i],
                    "round {round} cell {i}: inv-sbox cross-mult failed",
                );
            }
        }
    }
}
