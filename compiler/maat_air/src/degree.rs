//! Per-constraint transition degree computation.
//!
//! Winterfell requires that declared constraint degrees match the actual
//! polynomial degrees observed during proof generation exactly. A statically
//! declared degree is the algebraic upper bound for the constraint
//! expression, but the *effective* degree on a concrete trace can be lower
//! when the trace causes leading-coefficient cancellation in the Lagrange
//! interpolation. Declaring an over-estimate triggers Winterfell's
//! `debug_assert` and refuses to ship a proof in debug builds.
//!
//! This module computes the tight effective degree for every transition
//! constraint *before* the AIR is constructed, so that the degree declared
//! to Winterfell matches what the prover will compute.
//!
//! # Algorithm
//!
//! For each main-segment transition constraint `C(x)`:
//!
//! 1. Interpolate every main trace column to obtain a trace polynomial of
//!    degree `n - 1` (where `n` is the trace length).
//! 2. Evaluate every trace polynomial on a constraint evaluation (CE)
//!    coset: a multiplicative subgroup of size `n * ce_blowup` shifted by
//!    the field generator. `ce_blowup` is sized to the maximum static
//!    declared degree so the recovered quotient polynomial cannot alias.
//! 3. Evaluate the constraint expression at every CE row using the AIR's
//!    own `evaluate_transition` implementation.
//! 4. Divide the constraint evaluations by the transition divisor
//!    `(x^n - 1) / (x - g^{n-1})` evaluated at each CE point.
//! 5. Interpolate the resulting quotient polynomial via inverse FFT and
//!    read off its degree.
//! 6. Map the quotient degree back to the declared transition degree:
//!    `D = ceil(quotient_deg / (n - 1)) + 1` for `quotient_deg > 0`,
//!    otherwise `D = 1` (degenerate / zero polynomial).
//!
//! Auxiliary-segment constraints depend on verifier-supplied random
//! challenges that are not available at AIR construction time. The same
//! FFT-based path runs against the auxiliary trace built from a fixed
//! deterministic placeholder triple `[z, alpha, z_rc]`. Using a single
//! fixed triple is sound for degree detection: the polynomial degree of
//! a multivariate constraint in `x` cannot drop unless every coefficient
//! polynomial in the random elements vanishes simultaneously--an event
//! of measure zero over a generic placeholder. Because the verifier
//! reconstructs the same degrees from the same metadata bytes, a prover
//! that picks adversarial randoms cannot affect the declared degrees.
//!
//! # Soundness
//!
//! The computed degrees travel with the proof inside [`winter_air::TraceInfo::meta`].
//! The verifier reconstructs the same degree array from the same metadata
//! and constructs an identical `AirContext`. A malicious prover that
//! claims a constraint is degenerate when it is not produces a quotient
//! polynomial of higher degree than the FRI commitment can accommodate;
//! FRI rejects such proofs. Over-declaring is harmless: FRI checks
//! `degree <=` the declared bound, and a smaller actual degree always
//! satisfies that bound.

use maat_trace::TRACE_WIDTH;
use winter_math::fields::f64::BaseElement;
use winter_math::{FieldElement, StarkField, fft, polynom};

use crate::aux_segment::{
    self, AUX_CONSTRAINT_DEGREES, AUX_WIDTH, NUM_AUX_CONSTRAINTS, NUM_AUX_RANDS,
};
use crate::main_segment::{self, CONSTRAINT_DEGREES, NUM_CONSTRAINTS};

/// Degree assigned to degenerate (zero-polynomial) constraints.
///
/// A transition constraint with degree 1 has a quotient of degree
/// `(1 - 1) * (n - 1) = 0`, matching the zero polynomial exactly.
const DEGENERATE_DEGREE: usize = 1;

/// Number of bytes used to encode the per-constraint degree array in
/// [`winter_air::TraceInfo::meta`].
///
/// One byte per main constraint followed by one byte per auxiliary
/// constraint. Each value is a small integer (1..=5).
pub const DEGREE_BYTES: usize = NUM_CONSTRAINTS + NUM_AUX_CONSTRAINTS;

/// Encodes the effective per-constraint degrees for the given main trace.
///
/// The first `NUM_CONSTRAINTS` bytes hold the main-segment degrees; the
/// remaining `NUM_AUX_CONSTRAINTS` bytes hold the auxiliary-segment
/// degrees. Each byte is in the range `1..=max_static_declared_degree`.
pub fn encode_degrees(main_columns: &[Vec<BaseElement>]) -> Vec<u8> {
    let n = main_columns[0].len();
    let main = compute_main_degrees(main_columns, n);
    let aux = compute_aux_degrees(main_columns, n);

    main.iter()
        .chain(aux.iter())
        .map(|&d| u8::try_from(d).expect("constraint degree fits in u8"))
        .collect()
}

/// Decodes the per-constraint degree arrays from trace metadata.
///
/// When `meta` is shorter than [`DEGREE_BYTES`] (e.g. for AIR instances
/// constructed by tests without a prover), the original static degrees are
/// returned unchanged.
pub fn decode_degrees(meta: &[u8]) -> ([usize; NUM_CONSTRAINTS], [usize; NUM_AUX_CONSTRAINTS]) {
    if meta.len() < DEGREE_BYTES {
        return (CONSTRAINT_DEGREES, AUX_CONSTRAINT_DEGREES);
    }

    let main = core::array::from_fn(|i| meta[i] as usize);
    let aux = core::array::from_fn(|i| meta[NUM_CONSTRAINTS + i] as usize);
    (main, aux)
}

/// Computes the effective transition degree for every main-segment
/// constraint by interpolating the constraint quotient polynomial and
/// reading off its degree.
fn compute_main_degrees(main_columns: &[Vec<BaseElement>], n: usize) -> [usize; NUM_CONSTRAINTS] {
    let max_declared = *CONSTRAINT_DEGREES.iter().max().expect("non-empty");
    let ce_blowup = max_declared.next_power_of_two().max(2);
    let ce_size = n * ce_blowup;

    let inv_twiddles_n = fft::get_inv_twiddles::<BaseElement>(n);
    let twiddles_n = fft::get_twiddles::<BaseElement>(n);
    let domain_offset = BaseElement::GENERATOR;

    let lde_columns = (0..TRACE_WIDTH)
        .map(|c| {
            let mut coeffs = main_columns[c].clone();
            fft::interpolate_poly(&mut coeffs, &inv_twiddles_n);
            fft::evaluate_poly_with_offset(&coeffs, &twiddles_n, domain_offset, ce_blowup)
        })
        .collect::<Vec<Vec<BaseElement>>>();

    let trace_generator = BaseElement::get_root_of_unity(n.ilog2());
    let exemption = trace_generator.exp((n as u64) - 1);
    let ce_generator = BaseElement::get_root_of_unity(ce_size.ilog2());
    let div_values = (0..ce_size)
        .map(|i| {
            let x = domain_offset * ce_generator.exp(i as u64);
            (x.exp(n as u64) - BaseElement::ONE) / (x - exemption)
        })
        .collect::<Vec<BaseElement>>();

    let mut constraint_evals = vec![vec![BaseElement::ZERO; ce_size]; NUM_CONSTRAINTS];
    let mut current = vec![BaseElement::ZERO; TRACE_WIDTH];
    let mut next = vec![BaseElement::ZERO; TRACE_WIDTH];
    let mut row_buf = vec![BaseElement::ZERO; NUM_CONSTRAINTS];

    for i in 0..ce_size {
        let next_i = (i + ce_blowup) & (ce_size - 1);
        for c in 0..TRACE_WIDTH {
            current[c] = lde_columns[c][i];
            next[c] = lde_columns[c][next_i];
        }
        main_segment::evaluate(&current, &next, &mut row_buf);
        for (c, &val) in row_buf.iter().enumerate() {
            constraint_evals[c][i] = val;
        }
    }

    let inv_twiddles_ce = fft::get_inv_twiddles::<BaseElement>(ce_size);
    let mut degrees = [DEGENERATE_DEGREE; NUM_CONSTRAINTS];
    for (c, slot) in degrees.iter_mut().enumerate() {
        let mut quotient = constraint_evals[c]
            .iter()
            .zip(div_values.iter())
            .map(|(&v, &d)| v / d)
            .collect::<Vec<BaseElement>>();
        fft::interpolate_poly_with_offset(&mut quotient, &inv_twiddles_ce, domain_offset);
        let quotient_degree = polynom::degree_of(&quotient);
        *slot = quotient_degree_to_declared(quotient_degree, n).min(CONSTRAINT_DEGREES[c]);
    }

    degrees
}

/// Maps a quotient polynomial degree to the corresponding declared
/// transition degree.
///
/// For a transition constraint declared with degree `D`, Winterfell expects
/// a quotient polynomial of degree `(D - 1) * (n - 1)`. Inverting that
/// relationship: a quotient polynomial of degree `q > 0` requires
/// `D = ceil(q / (n - 1)) + 1`. A zero-degree quotient corresponds to the
/// degenerate `D = 1`.
fn quotient_degree_to_declared(quotient_degree: usize, n: usize) -> usize {
    if quotient_degree == 0 {
        return DEGENERATE_DEGREE;
    }
    let denom = n.saturating_sub(1).max(1);
    quotient_degree.div_ceil(denom) + 1
}

/// Deterministic placeholder challenges used to materialize the auxiliary
/// trace for degree detection.
///
/// The auxiliary trace structure depends on three verifier-supplied random
/// elements `[z, alpha, z_rc]`. Real proving derives them via Fiat-Shamir
/// after committing to the main trace, which is unavailable at AIR
/// construction time. For the sole purpose of measuring polynomial degree
/// in `x`, any non-zero triple suffices: the polynomial degree of a
/// multivariate constraint is the maximum over all monomial degree
/// signatures in `x`, and a generic placeholder reveals it.
const AUX_DEGREE_PROBE: [u64; NUM_AUX_RANDS] = [
    0xa8d2_f0d4_ec99_3b1d,
    0x6b7e_a31a_4f01_22e9,
    0xd71f_44b8_94c5_06af,
];

/// Computes the effective transition degree for every auxiliary-segment
/// constraint by interpolating the constraint quotient polynomial and
/// reading off its degree.
fn compute_aux_degrees(
    main_columns: &[Vec<BaseElement>],
    n: usize,
) -> [usize; NUM_AUX_CONSTRAINTS] {
    let max_declared = *AUX_CONSTRAINT_DEGREES
        .iter()
        .chain(CONSTRAINT_DEGREES.iter())
        .max()
        .expect("non-empty");
    let ce_blowup = max_declared.next_power_of_two().max(2);
    let ce_size = n * ce_blowup;

    let inv_twiddles_n = fft::get_inv_twiddles::<BaseElement>(n);
    let twiddles_n = fft::get_twiddles::<BaseElement>(n);
    let domain_offset = BaseElement::GENERATOR;

    let main_lde = (0..TRACE_WIDTH)
        .map(|c| {
            let mut coeffs = main_columns[c].clone();
            fft::interpolate_poly(&mut coeffs, &inv_twiddles_n);
            fft::evaluate_poly_with_offset(&coeffs, &twiddles_n, domain_offset, ce_blowup)
        })
        .collect::<Vec<Vec<BaseElement>>>();

    let rand_elements = AUX_DEGREE_PROBE.map(BaseElement::new);
    let aux_columns = aux_segment::build_aux_columns::<BaseElement>(main_columns, &rand_elements);
    let aux_lde = (0..AUX_WIDTH)
        .map(|c| {
            let mut coeffs = aux_columns[c].clone();
            fft::interpolate_poly(&mut coeffs, &inv_twiddles_n);
            fft::evaluate_poly_with_offset(&coeffs, &twiddles_n, domain_offset, ce_blowup)
        })
        .collect::<Vec<Vec<BaseElement>>>();

    let trace_generator = BaseElement::get_root_of_unity(n.ilog2());
    let exemption = trace_generator.exp((n as u64) - 1);
    let ce_generator = BaseElement::get_root_of_unity(ce_size.ilog2());
    let div_values = (0..ce_size)
        .map(|i| {
            let x = domain_offset * ce_generator.exp(i as u64);
            (x.exp(n as u64) - BaseElement::ONE) / (x - exemption)
        })
        .collect::<Vec<BaseElement>>();

    let mut constraint_evals = vec![vec![BaseElement::ZERO; ce_size]; NUM_AUX_CONSTRAINTS];
    let mut main_curr = vec![BaseElement::ZERO; TRACE_WIDTH];
    let mut main_next = vec![BaseElement::ZERO; TRACE_WIDTH];
    let mut aux_curr = vec![BaseElement::ZERO; AUX_WIDTH];
    let mut aux_next = vec![BaseElement::ZERO; AUX_WIDTH];
    let mut row_buf = vec![BaseElement::ZERO; NUM_AUX_CONSTRAINTS];

    for i in 0..ce_size {
        let next_i = (i + ce_blowup) & (ce_size - 1);
        for c in 0..TRACE_WIDTH {
            main_curr[c] = main_lde[c][i];
            main_next[c] = main_lde[c][next_i];
        }
        for c in 0..AUX_WIDTH {
            aux_curr[c] = aux_lde[c][i];
            aux_next[c] = aux_lde[c][next_i];
        }
        aux_segment::evaluate(
            &main_curr,
            &main_next,
            &aux_curr,
            &aux_next,
            &rand_elements,
            &mut row_buf,
        );
        for (c, &val) in row_buf.iter().enumerate() {
            constraint_evals[c][i] = val;
        }
    }

    let inv_twiddles_ce = fft::get_inv_twiddles::<BaseElement>(ce_size);
    let mut degrees = [DEGENERATE_DEGREE; NUM_AUX_CONSTRAINTS];
    for (c, slot) in degrees.iter_mut().enumerate() {
        let mut quotient = constraint_evals[c]
            .iter()
            .zip(div_values.iter())
            .map(|(&v, &d)| v / d)
            .collect::<Vec<BaseElement>>();
        fft::interpolate_poly_with_offset(&mut quotient, &inv_twiddles_ce, domain_offset);
        let quotient_degree = polynom::degree_of(&quotient);
        *slot = quotient_degree_to_declared(quotient_degree, n).min(AUX_CONSTRAINT_DEGREES[c]);
    }

    degrees
}

#[cfg(test)]
mod tests {
    use maat_trace::COL_SEL_BASE;

    use super::*;

    fn padded_trace(rows: usize) -> Vec<Vec<BaseElement>> {
        let n = rows.next_power_of_two().max(8);
        let mut cols = vec![vec![BaseElement::ZERO; n]; TRACE_WIDTH];
        for slot in cols[COL_SEL_BASE].iter_mut() {
            *slot = BaseElement::ONE;
        }
        cols
    }

    #[test]
    fn nop_only_trace_marks_main_constraints_degenerate() {
        let cols = padded_trace(8);
        let degrees = compute_main_degrees(&cols, cols[0].len());
        for (i, &d) in degrees.iter().enumerate() {
            // Only constraint 17 (selector sum) is statically degree 1; every
            // other constraint must reduce to degree 1 on a NOP-only trace.
            assert!(d <= CONSTRAINT_DEGREES[i], "constraint {i} over-declared");
            if i != 17 {
                assert_eq!(d, 1, "constraint {i} should be degenerate on NOP trace");
            }
        }
    }

    #[test]
    fn decode_short_meta_returns_static_degrees() {
        let (main, aux) = decode_degrees(&[]);
        assert_eq!(main, CONSTRAINT_DEGREES);
        assert_eq!(aux, AUX_CONSTRAINT_DEGREES);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let cols = padded_trace(8);
        let bytes = encode_degrees(&cols);
        assert_eq!(bytes.len(), DEGREE_BYTES);
        let (main, aux) = decode_degrees(&bytes);
        for (i, &d) in main.iter().enumerate() {
            assert!((1..=CONSTRAINT_DEGREES[i]).contains(&d));
        }
        for (i, &d) in aux.iter().enumerate() {
            assert!((1..=AUX_CONSTRAINT_DEGREES[i]).contains(&d));
        }
    }
}
