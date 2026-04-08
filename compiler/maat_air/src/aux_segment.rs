//! Auxiliary trace segment: memory permutation argument and range-check sub-AIR.
//!
//! This module implements two independent verification sub-systems that share
//! the auxiliary trace segment:
//!
//! ## Memory permutation argument
//!
//! A two-list permutation argument enforces memory consistency:
//!
//! - **L1** (execution order): `(mem_addr, mem_val)` from the main trace.
//! - **L2** (sorted order): the same pairs sorted by address, stored in
//!   [`AUX_COL_L2_ADDR`] and [`AUX_COL_L2_VAL`].
//! - **Grand-product accumulator** ([`AUX_COL_MEM_ACC`]): proves L1 and L2
//!   are permutations using challenges `z` and `alpha`.
//!
//! ## Range-check sub-AIR
//!
//! Proves that every 16-bit limb emitted by range-check trigger rows lies in
//! `[0, 2^16)`. The mechanism:
//!
//! 1. **Limb collection**: the main trace decomposes each range-checked value
//!    into four 16-bit limbs (`COL_RC_L0..L3`). Every row contributes four
//!    limb values (zero on non-trigger rows).
//! 2. **Sorted pool**: all 4(T) limbs are globally sorted and distributed
//!    across four auxiliary columns ([`AUX_COL_RC_SORTED_0`] through
//!    [`AUX_COL_RC_SORTED_3`]), interleaved so that
//!    `sorted_0[i] <= sorted_1[i] <= sorted_2[i] <= sorted_3[i] <= sorted_0[i+1]`.
//! 3. **Continuity constraints**: four degree-2 constraints enforce that
//!    consecutive sorted values differ by at most 1, proving every limb is in
//!    `[0, 2^16)`.
//! 4. **Permutation accumulator** ([`AUX_COL_RC_ACC`]): a degree-5 grand-product
//!    constraint proves the sorted pool is a permutation of the execution-order
//!    limbs from the main trace.
//!
//! # Auxiliary constraint summary
//!
//! | Index | Description                           | Degree |
//! |-------|---------------------------------------|--------|
//! | 0     | Memory: address continuity            | 2      |
//! | 1     | Memory: single-value consistency      | 2      |
//! | 2     | Memory: grand-product accumulator     | 2      |
//! | 3     | RC: sorted continuity 0-->1           | 2      |
//! | 4     | RC: sorted continuity 1-->2           | 2      |
//! | 5     | RC: sorted continuity 2-->3           | 2      |
//! | 6     | RC: sorted continuity 3-->0(next)     | 2      |
//! | 7     | RC: permutation accumulator           | 5      |

use maat_trace::{
    COL_MEM_ADDR, COL_MEM_VAL, COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3, TRACE_WIDTH,
};
use winter_math::fields::f64::BaseElement;
use winter_math::{ExtensionOf, FieldElement};

/// Auxiliary column index: sorted memory address.
pub const AUX_COL_L2_ADDR: usize = 0;

/// Auxiliary column index: sorted memory value.
pub const AUX_COL_L2_VAL: usize = 1;

/// Auxiliary column index: memory grand-product permutation accumulator.
pub const AUX_COL_MEM_ACC: usize = 2;

/// Auxiliary column index: range-check sorted limb pool, column 0.
pub const AUX_COL_RC_SORTED_0: usize = 3;

/// Auxiliary column index: range-check sorted limb pool, column 1.
pub const AUX_COL_RC_SORTED_1: usize = 4;

/// Auxiliary column index: range-check sorted limb pool, column 2.
pub const AUX_COL_RC_SORTED_2: usize = 5;

/// Auxiliary column index: range-check sorted limb pool, column 3.
pub const AUX_COL_RC_SORTED_3: usize = 6;

/// Auxiliary column index: range-check permutation accumulator.
pub const AUX_COL_RC_ACC: usize = 7;

/// Total width of the auxiliary trace segment.
pub const AUX_WIDTH: usize = 8;

/// Number of random field elements drawn for auxiliary column construction.
///
/// The verifier supplies three challenges:
/// - `z`: linear combination base for the memory permutation.
/// - `alpha`: tuple compression coefficient for the memory permutation.
/// - `z_rc`: linear combination base for the range-check permutation.
pub const NUM_AUX_RANDS: usize = 3;

/// Number of auxiliary transition constraints.
pub const NUM_AUX_CONSTRAINTS: usize = 8;

/// Degree of each auxiliary transition constraint.
pub const AUX_CONSTRAINT_DEGREES: [usize; NUM_AUX_CONSTRAINTS] = [
    2, // memory address continuity
    2, // memory single-value consistency
    2, // memory grand-product accumulator
    2, // RC sorted continuity 0-->1
    2, // RC sorted continuity 1-->2
    2, // RC sorted continuity 2-->3
    2, // RC sorted continuity 3-->0(next)
    5, // RC permutation accumulator
];

/// Index of verifier challenge `z` (memory permutation).
const RAND_Z: usize = 0;

/// Index of verifier challenge `alpha` (memory tuple compression).
const RAND_ALPHA: usize = 1;

/// Index of verifier challenge `z_rc` (range-check permutation).
const RAND_Z_RC: usize = 2;

/// Evaluates all eight auxiliary transition constraints.
///
/// All constraints evaluate to zero on valid auxiliary trace rows.
///
/// # Parameters
///
/// - `main_current`, `main_next`: consecutive rows from the main trace segment.
/// - `aux_current`, `aux_next`: consecutive rows from the auxiliary trace segment.
/// - `rand_elements`: verifier challenges `[z, alpha, z_rc]`.
/// - `result`: mutable slice of length [`NUM_AUX_CONSTRAINTS`]; receives the
///   constraint evaluations.
pub fn evaluate<F, E>(
    _main_current: &[F],
    main_next: &[F],
    aux_current: &[E],
    aux_next: &[E],
    rand_elements: &[E],
    result: &mut [E],
) where
    F: FieldElement,
    E: FieldElement<BaseField = F::BaseField> + ExtensionOf<F>,
{
    debug_assert_eq!(result.len(), NUM_AUX_CONSTRAINTS);

    let one = E::ONE;

    let l2_addr = aux_current[AUX_COL_L2_ADDR];
    let l2_addr_next = aux_next[AUX_COL_L2_ADDR];
    let l2_val = aux_current[AUX_COL_L2_VAL];
    let l2_val_next = aux_next[AUX_COL_L2_VAL];
    let mem_acc = aux_current[AUX_COL_MEM_ACC];
    let mem_acc_next = aux_next[AUX_COL_MEM_ACC];

    let z = rand_elements[RAND_Z];
    let alpha = rand_elements[RAND_ALPHA];

    let addr_delta = l2_addr_next - l2_addr;

    result[0] = addr_delta * (addr_delta - one);
    result[1] = (l2_val_next - l2_val) * (addr_delta - one);

    let l1_addr_next = E::from(main_next[COL_MEM_ADDR]);
    let l1_val_next = E::from(main_next[COL_MEM_VAL]);
    let l1_tuple_next = l1_addr_next + alpha * l1_val_next;
    let l2_tuple_next = l2_addr_next + alpha * l2_val_next;
    result[2] = (z - l2_tuple_next) * mem_acc_next - (z - l1_tuple_next) * mem_acc;

    let s0 = aux_current[AUX_COL_RC_SORTED_0];
    let s1 = aux_current[AUX_COL_RC_SORTED_1];
    let s2 = aux_current[AUX_COL_RC_SORTED_2];
    let s3 = aux_current[AUX_COL_RC_SORTED_3];
    let s0_next = aux_next[AUX_COL_RC_SORTED_0];
    let s1_next = aux_next[AUX_COL_RC_SORTED_1];
    let s2_next = aux_next[AUX_COL_RC_SORTED_2];
    let s3_next = aux_next[AUX_COL_RC_SORTED_3];

    let rc_acc = aux_current[AUX_COL_RC_ACC];
    let rc_acc_next = aux_next[AUX_COL_RC_ACC];
    let z_rc = rand_elements[RAND_Z_RC];

    let d01 = s1 - s0;
    result[3] = d01 * (d01 - one);

    let d12 = s2 - s1;
    result[4] = d12 * (d12 - one);

    let d23 = s3 - s2;
    result[5] = d23 * (d23 - one);

    let d30 = s0_next - s3;
    result[6] = d30 * (d30 - one);

    let l0_next = E::from(main_next[COL_RC_L0]);
    let l1_next = E::from(main_next[COL_RC_L1]);
    let l2_next = E::from(main_next[COL_RC_L2]);
    let l3_next = E::from(main_next[COL_RC_L3]);

    let sorted_prod = (z_rc - s0_next) * (z_rc - s1_next) * (z_rc - s2_next) * (z_rc - s3_next);
    let limb_prod = (z_rc - l0_next) * (z_rc - l1_next) * (z_rc - l2_next) * (z_rc - l3_next);

    result[7] = sorted_prod * rc_acc_next - limb_prod * rc_acc;
}

/// Builds the eight auxiliary trace columns from the committed main trace and
/// verifier-supplied random challenges.
///
/// The returned vector contains columns in order:
/// `[L2_addr, L2_val, mem_acc, rc_sorted_0, rc_sorted_1, rc_sorted_2, rc_sorted_3, rc_acc]`.
///
/// # Panics
///
/// Panics if `rand_elements` does not contain at least [`NUM_AUX_RANDS`]
/// elements, or if `main_trace` does not contain the required columns.
pub fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
    main_trace: &[Vec<BaseElement>],
    rand_elements: &[E],
) -> Vec<Vec<E>> {
    debug_assert!(main_trace.len() >= TRACE_WIDTH);

    let n = main_trace[0].len();
    let z = rand_elements[RAND_Z];
    let alpha = rand_elements[RAND_ALPHA];
    let z_rc = rand_elements[RAND_Z_RC];

    let mut pairs = (0..n)
        .map(|i| (main_trace[COL_MEM_ADDR][i], main_trace[COL_MEM_VAL][i]))
        .collect::<Vec<(BaseElement, BaseElement)>>();
    pairs.sort_unstable_by(|a, b| {
        a.0.as_int()
            .cmp(&b.0.as_int())
            .then_with(|| a.1.as_int().cmp(&b.1.as_int()))
    });

    let l2_addr = pairs.iter().map(|(a, _)| E::from(*a)).collect::<Vec<E>>();
    let l2_val = pairs.iter().map(|(_, v)| E::from(*v)).collect::<Vec<E>>();

    let mut mem_acc = Vec::with_capacity(n);
    mem_acc.push(E::ONE);

    for i in 1..n {
        let l1_addr_i = E::from(main_trace[COL_MEM_ADDR][i]);
        let l1_val_i = E::from(main_trace[COL_MEM_VAL][i]);
        let l1_tuple = l1_addr_i + alpha * l1_val_i;
        let l2_tuple = l2_addr[i] + alpha * l2_val[i];

        let numerator = z - l1_tuple;
        let denominator = z - l2_tuple;
        mem_acc.push(mem_acc[i - 1] * numerator * denominator.inv());
    }

    // Collect all 4(T) limb values from the main trace.
    let mut limb_pool: Vec<u64> = Vec::with_capacity(4 * n);
    for (((l0, l1), l2), l3) in main_trace[COL_RC_L0]
        .iter()
        .zip(&main_trace[COL_RC_L1])
        .zip(&main_trace[COL_RC_L2])
        .zip(&main_trace[COL_RC_L3])
    {
        limb_pool.push(l0.as_int());
        limb_pool.push(l1.as_int());
        limb_pool.push(l2.as_int());
        limb_pool.push(l3.as_int());
    }

    limb_pool.sort_unstable();

    let mut rc_sorted: [Vec<E>; 4] = [
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    ];
    for i in 0..n {
        rc_sorted[0].push(E::from(BaseElement::new(limb_pool[4 * i])));
        rc_sorted[1].push(E::from(BaseElement::new(limb_pool[4 * i + 1])));
        rc_sorted[2].push(E::from(BaseElement::new(limb_pool[4 * i + 2])));
        rc_sorted[3].push(E::from(BaseElement::new(limb_pool[4 * i + 3])));
    }

    let mut rc_acc = Vec::with_capacity(n);
    rc_acc.push(E::ONE);

    for i in 1..n {
        let l0 = E::from(main_trace[COL_RC_L0][i]);
        let l1 = E::from(main_trace[COL_RC_L1][i]);
        let l2 = E::from(main_trace[COL_RC_L2][i]);
        let l3 = E::from(main_trace[COL_RC_L3][i]);

        let limb_prod = (z_rc - l0) * (z_rc - l1) * (z_rc - l2) * (z_rc - l3);
        let sorted_prod = (z_rc - rc_sorted[0][i])
            * (z_rc - rc_sorted[1][i])
            * (z_rc - rc_sorted[2][i])
            * (z_rc - rc_sorted[3][i]);

        rc_acc.push(rc_acc[i - 1] * limb_prod * sorted_prod.inv());
    }

    let mut columns = vec![vec![E::ZERO; n]; AUX_WIDTH];
    columns[AUX_COL_L2_ADDR] = l2_addr;
    columns[AUX_COL_L2_VAL] = l2_val;
    columns[AUX_COL_MEM_ACC] = mem_acc;
    columns[AUX_COL_RC_SORTED_0] = rc_sorted[0].clone();
    columns[AUX_COL_RC_SORTED_1] = rc_sorted[1].clone();
    columns[AUX_COL_RC_SORTED_2] = rc_sorted[2].clone();
    columns[AUX_COL_RC_SORTED_3] = rc_sorted[3].clone();
    columns[AUX_COL_RC_ACC] = rc_acc;
    columns
}

#[cfg(test)]
mod tests {
    use maat_trace::TRACE_WIDTH;
    use winter_math::fields::f64::BaseElement;

    use super::*;

    type F = BaseElement;

    /// Creates a minimal main trace (column-major) with the given memory
    /// access pairs. Non-memory columns are zero-filled.
    fn mock_main_trace(mem_pairs: &[(u64, u64)]) -> Vec<Vec<F>> {
        let n = mem_pairs.len();
        let mut columns = vec![vec![F::ZERO; n]; TRACE_WIDTH];
        for (i, &(addr, val)) in mem_pairs.iter().enumerate() {
            columns[COL_MEM_ADDR][i] = F::new(addr);
            columns[COL_MEM_VAL][i] = F::new(val);
        }
        columns
    }

    /// Creates a main trace with memory pairs and range-check limb data.
    fn mock_main_trace_with_limbs(mem_pairs: &[(u64, u64)], limbs: &[[u64; 4]]) -> Vec<Vec<F>> {
        let n = mem_pairs.len();
        assert_eq!(n, limbs.len());
        let mut columns = vec![vec![F::ZERO; n]; TRACE_WIDTH];
        for (i, &(addr, val)) in mem_pairs.iter().enumerate() {
            columns[COL_MEM_ADDR][i] = F::new(addr);
            columns[COL_MEM_VAL][i] = F::new(val);
        }
        for (i, ls) in limbs.iter().enumerate() {
            columns[COL_RC_L0][i] = F::new(ls[0]);
            columns[COL_RC_L1][i] = F::new(ls[1]);
            columns[COL_RC_L2][i] = F::new(ls[2]);
            columns[COL_RC_L3][i] = F::new(ls[3]);
        }
        columns
    }

    /// Evaluate auxiliary constraints on a single row pair.
    fn eval_aux(
        main_curr: &[F],
        main_next: &[F],
        aux_curr: &[F],
        aux_next: &[F],
        z: F,
        alpha: F,
        z_rc: F,
    ) -> Vec<F> {
        let rands = [z, alpha, z_rc];
        let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
        evaluate(
            main_curr,
            main_next,
            aux_curr,
            aux_next,
            &rands,
            &mut result,
        );
        result
    }

    fn make_aux_row(
        l2_addr: u64,
        l2_val: u64,
        mem_acc: F,
        sorted: [u64; 4],
        rc_acc: F,
    ) -> [F; AUX_WIDTH] {
        [
            F::new(l2_addr),
            F::new(l2_val),
            mem_acc,
            F::new(sorted[0]),
            F::new(sorted[1]),
            F::new(sorted[2]),
            F::new(sorted[3]),
            rc_acc,
        ]
    }

    #[test]
    fn address_continuity_same_addr_passes() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 10, F::ONE, [0, 0, 0, 0], F::ONE);
        let aux_next = make_aux_row(5, 10, F::ONE, [0, 0, 0, 0], F::ONE);
        let result = eval_aux(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            F::new(7),
            F::new(3),
            F::new(11),
        );
        assert_eq!(result[0], F::ZERO, "same address should satisfy continuity");
    }

    #[test]
    fn address_continuity_increment_by_one_passes() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 10, F::ONE, [0, 0, 0, 0], F::ONE);
        let aux_next = make_aux_row(6, 20, F::ONE, [0, 0, 0, 0], F::ONE);
        let result = eval_aux(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            F::new(7),
            F::new(3),
            F::new(11),
        );
        assert_eq!(result[0], F::ZERO, "addr+1 should satisfy continuity");
    }

    #[test]
    fn address_continuity_increment_by_two_fails() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 10, F::ONE, [0, 0, 0, 0], F::ONE);
        let aux_next = make_aux_row(7, 20, F::ONE, [0, 0, 0, 0], F::ONE);
        let result = eval_aux(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            F::new(7),
            F::new(3),
            F::new(11),
        );
        assert_ne!(result[0], F::ZERO, "addr+2 should violate continuity");
    }

    #[test]
    fn single_value_same_addr_same_val_passes() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 42, F::ONE, [0, 0, 0, 0], F::ONE);
        let aux_next = make_aux_row(5, 42, F::ONE, [0, 0, 0, 0], F::ONE);
        let result = eval_aux(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            F::new(7),
            F::new(3),
            F::new(11),
        );
        assert_eq!(result[1], F::ZERO, "same addr + same val should pass");
    }

    #[test]
    fn single_value_same_addr_different_val_fails() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 42, F::ONE, [0, 0, 0, 0], F::ONE);
        let aux_next = make_aux_row(5, 99, F::ONE, [0, 0, 0, 0], F::ONE);
        let result = eval_aux(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            F::new(7),
            F::new(3),
            F::new(11),
        );
        assert_ne!(result[1], F::ZERO, "same addr + different val should fail");
    }

    #[test]
    fn single_value_different_addr_different_val_passes() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 42, F::ONE, [0, 0, 0, 0], F::ONE);
        let aux_next = make_aux_row(6, 99, F::ONE, [0, 0, 0, 0], F::ONE);
        let result = eval_aux(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            F::new(7),
            F::new(3),
            F::new(11),
        );
        assert_eq!(result[1], F::ZERO, "different addr allows different val");
    }

    #[test]
    fn grand_product_correct_accumulator_passes() {
        let z = F::new(1000);
        let alpha = F::new(7);

        let main_curr = [F::ZERO; TRACE_WIDTH];
        let mut main_next = [F::ZERO; TRACE_WIDTH];
        main_next[COL_MEM_ADDR] = F::new(1);
        main_next[COL_MEM_VAL] = F::new(5);

        let aux_curr = make_aux_row(0, 0, F::ONE, [0, 0, 0, 0], F::ONE);
        let aux_next = make_aux_row(1, 5, F::ONE, [0, 0, 0, 0], F::ONE);

        let result = eval_aux(
            &main_curr,
            &main_next,
            &aux_curr,
            &aux_next,
            z,
            alpha,
            F::new(11),
        );
        assert_eq!(
            result[2],
            F::ZERO,
            "correct accumulator should satisfy grand product"
        );
    }

    #[test]
    fn grand_product_tampered_accumulator_fails() {
        let z = F::new(1000);
        let alpha = F::new(7);

        let mut main_next = [F::ZERO; TRACE_WIDTH];
        main_next[COL_MEM_ADDR] = F::new(1);
        main_next[COL_MEM_VAL] = F::new(5);

        let aux_curr = make_aux_row(0, 0, F::ONE, [0, 0, 0, 0], F::ONE);
        let aux_next = make_aux_row(1, 5, F::new(42), [0, 0, 0, 0], F::ONE);

        let main_curr = [F::ZERO; TRACE_WIDTH];
        let result = eval_aux(
            &main_curr,
            &main_next,
            &aux_curr,
            &aux_next,
            z,
            alpha,
            F::new(11),
        );
        assert_ne!(
            result[2],
            F::ZERO,
            "tampered accumulator should fail grand product"
        );
    }

    #[test]
    fn rc_sorted_continuity_valid() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(0, 0, F::ONE, [0, 0, 1, 1], F::ONE);
        let aux_next = make_aux_row(0, 0, F::ONE, [2, 2, 3, 3], F::ONE);
        let result = eval_aux(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            F::new(7),
            F::new(3),
            F::new(11),
        );

        assert_eq!(result[3], F::ZERO, "continuity 0→1 valid");
        assert_eq!(result[4], F::ZERO, "continuity 1→2 valid");
        assert_eq!(result[5], F::ZERO, "continuity 2→3 valid");

        assert_eq!(result[6], F::ZERO, "continuity 3-->0(next) valid");
    }

    #[test]
    fn rc_sorted_continuity_gap_fails() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(0, 0, F::ONE, [0, 5, 5, 5], F::ONE);
        let aux_next = make_aux_row(0, 0, F::ONE, [5, 5, 5, 5], F::ONE);
        let result = eval_aux(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            F::new(7),
            F::new(3),
            F::new(11),
        );

        assert_ne!(result[3], F::ZERO, "gap in sorted 0-->1 should fail");
    }

    #[test]
    fn build_aux_columns_identity_permutation() {
        let main = mock_main_trace(&[
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
        ]);
        let z = F::new(9999);
        let alpha = F::new(13);
        let z_rc = F::new(7777);
        let rands = [z, alpha, z_rc];

        let aux = build_aux_columns(&main, &rands);
        assert_eq!(aux.len(), AUX_WIDTH);
        assert_eq!(aux[AUX_COL_MEM_ACC][0], F::ONE, "mem_acc[0] must be 1");
        assert_eq!(
            aux[AUX_COL_MEM_ACC][7],
            F::ONE,
            "mem_acc[n-1] must be 1 for identity permutation"
        );
        assert_eq!(aux[AUX_COL_RC_ACC][0], F::ONE, "rc_acc[0] must be 1");
        assert_eq!(
            aux[AUX_COL_RC_ACC][7],
            F::ONE,
            "rc_acc[n-1] must be 1 for identity permutation"
        );
    }

    #[test]
    fn build_aux_columns_nontrivial_permutation() {
        let main = mock_main_trace(&[
            (0, 0),
            (2, 20),
            (1, 10),
            (0, 0),
            (1, 10),
            (2, 20),
            (0, 0),
            (0, 0),
        ]);
        let z = F::new(7777);
        let alpha = F::new(31);
        let z_rc = F::new(5555);
        let rands = [z, alpha, z_rc];

        let aux = build_aux_columns(&main, &rands);

        let n = 8;
        assert_eq!(aux[AUX_COL_MEM_ACC][0], F::ONE, "mem_acc[0] must be 1");
        assert_eq!(
            aux[AUX_COL_MEM_ACC][n - 1],
            F::ONE,
            "mem_acc[n-1] must be 1 for a valid permutation"
        );

        for i in 0..n - 1 {
            assert!(
                aux[AUX_COL_L2_ADDR][i].as_int() <= aux[AUX_COL_L2_ADDR][i + 1].as_int(),
                "L2 must be sorted by address at row {i}"
            );
        }

        for i in 0..n - 1 {
            let mut main_curr = vec![F::ZERO; TRACE_WIDTH];
            let mut main_next = vec![F::ZERO; TRACE_WIDTH];
            for col in 0..TRACE_WIDTH {
                main_curr[col] = main[col][i];
                main_next[col] = main[col][i + 1];
            }
            let aux_curr = (0..AUX_WIDTH).map(|c| aux[c][i]).collect::<Vec<F>>();
            let aux_next = (0..AUX_WIDTH).map(|c| aux[c][i + 1]).collect::<Vec<F>>();

            let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
            evaluate(
                &main_curr,
                &main_next,
                &aux_curr,
                &aux_next,
                &rands,
                &mut result,
            );
            for (j, &r) in result[..3].iter().enumerate() {
                assert_eq!(
                    r,
                    F::ZERO,
                    "memory constraint {j} violated at transition {i}-->{}",
                    i + 1
                );
            }
        }
    }

    #[test]
    fn build_aux_columns_with_limbs() {
        // Row 0: no range check (all zero limbs)
        // Row 1: range check value 0x0001_0002_0003_0004
        // Remaining rows: no range check
        let limbs: Vec<[u64; 4]> = vec![
            [0, 0, 0, 0],
            [4, 3, 2, 1],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        ];
        let mem_pairs: Vec<(u64, u64)> = vec![
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
        ];
        let main = mock_main_trace_with_limbs(&mem_pairs, &limbs);
        let z = F::new(9999);
        let alpha = F::new(13);
        let z_rc = F::new(7777);
        let rands = [z, alpha, z_rc];

        let aux = build_aux_columns(&main, &rands);

        // rc_acc boundaries
        assert_eq!(aux[AUX_COL_RC_ACC][0], F::ONE, "rc_acc[0] must be 1");
        assert_eq!(
            aux[AUX_COL_RC_ACC][7],
            F::ONE,
            "rc_acc[n-1] must be 1 for valid range-check permutation"
        );

        // Verify sorted columns are non-decreasing within and across columns.
        for (i, (((s0, s1), s2), s3)) in aux[AUX_COL_RC_SORTED_0]
            .iter()
            .zip(&aux[AUX_COL_RC_SORTED_1])
            .zip(&aux[AUX_COL_RC_SORTED_2])
            .zip(&aux[AUX_COL_RC_SORTED_3])
            .enumerate()
        {
            let (s0, s1, s2, s3) = (s0.as_int(), s1.as_int(), s2.as_int(), s3.as_int());
            assert!(s0 <= s1, "sorted_0 <= sorted_1 at row {i}");
            assert!(s1 <= s2, "sorted_1 <= sorted_2 at row {i}");
            assert!(s2 <= s3, "sorted_2 <= sorted_3 at row {i}");
        }
        let n = 8;
        for i in 0..n - 1 {
            let s3 = aux[AUX_COL_RC_SORTED_3][i].as_int();
            let s0_next = aux[AUX_COL_RC_SORTED_0][i + 1].as_int();
            assert!(s3 <= s0_next, "sorted_3[{i}] <= sorted_0[{}]", i + 1);
        }

        // Verify all 8 constraints evaluate to zero on every transition.
        for i in 0..n - 1 {
            let mut main_curr = vec![F::ZERO; TRACE_WIDTH];
            let mut main_next = vec![F::ZERO; TRACE_WIDTH];
            for col in 0..TRACE_WIDTH {
                main_curr[col] = main[col][i];
                main_next[col] = main[col][i + 1];
            }
            let aux_curr = (0..AUX_WIDTH).map(|c| aux[c][i]).collect::<Vec<F>>();
            let aux_next = (0..AUX_WIDTH).map(|c| aux[c][i + 1]).collect::<Vec<F>>();

            let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
            evaluate(
                &main_curr,
                &main_next,
                &aux_curr,
                &aux_next,
                &rands,
                &mut result,
            );
            for (j, &r) in result.iter().enumerate() {
                assert_eq!(
                    r,
                    F::ZERO,
                    "aux constraint {j} violated at transition {i}-->{}",
                    i + 1
                );
            }
        }
    }
}
