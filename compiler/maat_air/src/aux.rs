//! Auxiliary trace segment for the memory permutation argument.
//!
//! This module implements a two-list permutation argument that
//! enforces memory consistency across the execution trace. The argument works
//! by comparing two representations of the same memory access sequence:
//!
//! - **L1** (execution order): `(mem_addr, mem_val)` columns already present in
//!   the main trace at [`COL_MEM_ADDR`] and [`COL_MEM_VAL`].
//! - **L2** (sorted order): the same `(addr, val)` pairs sorted by address,
//!   stored in the auxiliary segment at [`AUX_COL_L2_ADDR`] and [`AUX_COL_L2_VAL`].
//!
//! A grand-product accumulator column ([`AUX_COL_PERM_ACC`]) proves that L1 and
//! L2 are permutations of each other using verifier-supplied random challenges
//! `z` and `alpha`.
//!
//! # Auxiliary transition constraints
//!
//! | Index | Description                 | Degree |
//! |-------|-----------------------------|--------|
//! | 0     | Address continuity          | 2      |
//! | 1     | Single-value consistency    | 2      |
//! | 2     | Grand-product accumulator   | 2      |
//!
//! # Auxiliary boundary assertions
//!
//! - `perm_acc[0] = 1`
//! - `perm_acc[n-1] = 1`
//!
//! # Soundness note
//!
//! The grand-product transition uses "next" row indices, excluding entry 0 from
//! the accumulated product. This is sound because every Maat trace begins with
//! `mem_addr = 0, mem_val = 0` in both L1 and L2, making the excluded ratio
//! trivially equal to one.

use maat_trace::{COL_MEM_ADDR, COL_MEM_VAL};
use winter_math::fields::f64::BaseElement;
use winter_math::{ExtensionOf, FieldElement};

/// Auxiliary column index: sorted memory address.
pub const AUX_COL_L2_ADDR: usize = 0;

/// Auxiliary column index: sorted memory value.
pub const AUX_COL_L2_VAL: usize = 1;

/// Auxiliary column index: grand-product permutation accumulator.
pub const AUX_COL_PERM_ACC: usize = 2;

/// Auxiliary column index: range-check limb 0.
pub const AUX_COL_LIMB_0: usize = 3;

/// Auxiliary column index: range-check limb 1.
pub const AUX_COL_LIMB_1: usize = 4;

/// Total width of the auxiliary trace segment.
pub const AUX_WIDTH: usize = 5;

/// Number of random field elements drawn for auxiliary column construction.
///
/// The verifier supplies two challenges: `z` (linear combination base) and
/// `alpha` (tuple compression coefficient).
pub const NUM_AUX_RANDS: usize = 2;

/// Number of auxiliary transition constraints.
pub const NUM_AUX_CONSTRAINTS: usize = 3;

/// Degree of each auxiliary transition constraint.
pub const AUX_CONSTRAINT_DEGREES: [usize; NUM_AUX_CONSTRAINTS] = [2, 2, 2];

/// Index of verifier challenge `z` within the random elements slice.
const RAND_Z: usize = 0;

/// Index of verifier challenge `alpha` within the random elements slice.
const RAND_ALPHA: usize = 1;

/// Evaluates the three auxiliary transition constraints.
///
/// All constraints evaluate to zero on valid auxiliary trace rows.
///
/// # Parameters
///
/// - `main_current`, `main_next`: consecutive rows from the main trace segment.
/// - `aux_current`, `aux_next`: consecutive rows from the auxiliary trace segment.
/// - `rand_elements`: verifier challenges `[z, alpha]`.
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
    let perm_acc = aux_current[AUX_COL_PERM_ACC];
    let perm_acc_next = aux_next[AUX_COL_PERM_ACC];

    let z = rand_elements[RAND_Z];
    let alpha = rand_elements[RAND_ALPHA];

    let addr_delta = l2_addr_next - l2_addr;

    result[0] = addr_delta * (addr_delta - one);
    result[1] = (l2_val_next - l2_val) * (addr_delta - one);

    let l1_addr_next = E::from(main_next[COL_MEM_ADDR]);
    let l1_val_next = E::from(main_next[COL_MEM_VAL]);
    let l1_tuple_next = l1_addr_next + alpha * l1_val_next;
    let l2_tuple_next = l2_addr_next + alpha * l2_val_next;
    result[2] = (z - l2_tuple_next) * perm_acc_next - (z - l1_tuple_next) * perm_acc;
}

/// Builds the five auxiliary trace columns from the committed main trace and
/// verifier-supplied random challenges.
///
/// The returned vector contains columns in order: `[L2_addr, L2_val, perm_acc,
/// limb_0, limb_1]`. Limb columns are currently zero-filled.
///
/// # Panics
///
/// Panics if `rand_elements` does not contain at least [`NUM_AUX_RANDS`]
/// elements, or if `main_trace` does not contain the required memory columns.
pub fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
    main_trace: &[Vec<BaseElement>],
    rand_elements: &[E],
) -> Vec<Vec<E>> {
    let n = main_trace[0].len();
    let z = rand_elements[RAND_Z];
    let alpha = rand_elements[RAND_ALPHA];

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

    let mut perm_acc = Vec::with_capacity(n);
    perm_acc.push(E::ONE);

    for i in 1..n {
        let l1_addr_i = E::from(main_trace[COL_MEM_ADDR][i]);
        let l1_val_i = E::from(main_trace[COL_MEM_VAL][i]);
        let l1_tuple = l1_addr_i + alpha * l1_val_i;
        let l2_tuple = l2_addr[i] + alpha * l2_val[i];

        let numerator = z - l1_tuple;
        let denominator = z - l2_tuple;
        perm_acc.push(perm_acc[i - 1] * numerator * denominator.inv());
    }

    let mut columns = vec![vec![E::ZERO; n]; AUX_WIDTH];
    columns[AUX_COL_L2_ADDR] = l2_addr;
    columns[AUX_COL_L2_VAL] = l2_val;
    columns[AUX_COL_PERM_ACC] = perm_acc;

    let _ = (AUX_COL_LIMB_0, AUX_COL_LIMB_1);
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

    /// Evaluate auxiliary constraints on a single row pair.
    fn eval_aux(
        main_curr: &[F],
        main_next: &[F],
        aux_curr: &[F],
        aux_next: &[F],
        z: F,
        alpha: F,
    ) -> Vec<F> {
        let rands = [z, alpha];
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

    fn make_aux_row(l2_addr: u64, l2_val: u64, perm_acc: F) -> [F; AUX_WIDTH] {
        [F::new(l2_addr), F::new(l2_val), perm_acc, F::ZERO, F::ZERO]
    }

    #[test]
    fn address_continuity_same_addr_passes() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 10, F::ONE);
        let aux_next = make_aux_row(5, 10, F::ONE);
        let result = eval_aux(&main, &main, &aux_curr, &aux_next, F::new(7), F::new(3));
        assert_eq!(result[0], F::ZERO, "same address should satisfy continuity");
    }

    #[test]
    fn address_continuity_increment_by_one_passes() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 10, F::ONE);
        let aux_next = make_aux_row(6, 20, F::ONE);
        let result = eval_aux(&main, &main, &aux_curr, &aux_next, F::new(7), F::new(3));
        assert_eq!(result[0], F::ZERO, "addr+1 should satisfy continuity");
    }

    #[test]
    fn address_continuity_increment_by_two_fails() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 10, F::ONE);
        let aux_next = make_aux_row(7, 20, F::ONE);
        let result = eval_aux(&main, &main, &aux_curr, &aux_next, F::new(7), F::new(3));
        assert_ne!(result[0], F::ZERO, "addr+2 should violate continuity");
    }

    #[test]
    fn single_value_same_addr_same_val_passes() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 42, F::ONE);
        let aux_next = make_aux_row(5, 42, F::ONE);
        let result = eval_aux(&main, &main, &aux_curr, &aux_next, F::new(7), F::new(3));
        assert_eq!(result[1], F::ZERO, "same addr + same val should pass");
    }

    #[test]
    fn single_value_same_addr_different_val_fails() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 42, F::ONE);
        let aux_next = make_aux_row(5, 99, F::ONE);
        let result = eval_aux(&main, &main, &aux_curr, &aux_next, F::new(7), F::new(3));
        assert_ne!(result[1], F::ZERO, "same addr + different val should fail");
    }

    #[test]
    fn single_value_different_addr_different_val_passes() {
        let main = [F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 42, F::ONE);
        let aux_next = make_aux_row(6, 99, F::ONE);
        let result = eval_aux(&main, &main, &aux_curr, &aux_next, F::new(7), F::new(3));
        assert_eq!(result[1], F::ZERO, "different addr allows different val");
    }

    #[test]
    fn grand_product_correct_accumulator_passes() {
        let z = F::new(1000);
        let alpha = F::new(7);

        // L1 (main trace): row 0 = (0,0), row 1 = (1,5)
        let main_curr = [F::ZERO; TRACE_WIDTH];
        let mut main_next = [F::ZERO; TRACE_WIDTH];
        main_next[COL_MEM_ADDR] = F::new(1);
        main_next[COL_MEM_VAL] = F::new(5);

        // L2 (aux trace): same data sorted — (0,0), (1,5)
        // perm_acc[0] = 1
        // perm_acc[1] = 1 * (z - L1[1]) / (z - L2[1]) = 1 (same data)
        let aux_curr = make_aux_row(0, 0, F::ONE);
        let aux_next = make_aux_row(1, 5, F::ONE);

        let result = eval_aux(&main_curr, &main_next, &aux_curr, &aux_next, z, alpha);
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

        let aux_curr = make_aux_row(0, 0, F::ONE);
        // Tamper: set perm_acc_next to an incorrect value.
        let aux_next = make_aux_row(1, 5, F::new(42));

        let main_curr = [F::ZERO; TRACE_WIDTH];
        let result = eval_aux(&main_curr, &main_next, &aux_curr, &aux_next, z, alpha);
        assert_ne!(
            result[2],
            F::ZERO,
            "tampered accumulator should fail grand product"
        );
    }

    #[test]
    fn build_aux_columns_identity_permutation() {
        // All rows access (0, 0): L1 == L2, perm_acc stays at 1.
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
        let rands = [z, alpha];

        let aux = build_aux_columns(&main, &rands);
        assert_eq!(aux.len(), AUX_WIDTH);
        assert_eq!(aux[AUX_COL_PERM_ACC][0], F::ONE, "perm_acc[0] must be 1");
        assert_eq!(
            aux[AUX_COL_PERM_ACC][7],
            F::ONE,
            "perm_acc[n-1] must be 1 for identity permutation"
        );
    }

    #[test]
    fn build_aux_columns_nontrivial_permutation() {
        // Execution order: (0,0), (2,20), (1,10), (0,0), (1,10), (2,20), (0,0), (0,0)
        // Sorted order:    (0,0), (0,0), (0,0), (0,0), (1,10), (1,10), (2,20), (2,20)
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
        let rands = [z, alpha];

        let aux = build_aux_columns(&main, &rands);

        // Verify boundaries.
        let n = 8;
        assert_eq!(aux[AUX_COL_PERM_ACC][0], F::ONE, "perm_acc[0] must be 1");
        assert_eq!(
            aux[AUX_COL_PERM_ACC][n - 1],
            F::ONE,
            "perm_acc[n-1] must be 1 for a valid permutation"
        );

        // Verify L2 is sorted.
        for i in 0..n - 1 {
            assert!(
                aux[AUX_COL_L2_ADDR][i].as_int() <= aux[AUX_COL_L2_ADDR][i + 1].as_int(),
                "L2 must be sorted by address at row {i}"
            );
        }

        // Verify all 3 constraints evaluate to zero on every transition.
        for i in 0..n - 1 {
            let mut main_curr = vec![F::ZERO; TRACE_WIDTH];
            let mut main_next = vec![F::ZERO; TRACE_WIDTH];
            for col in 0..TRACE_WIDTH {
                main_curr[col] = main[col][i];
                main_next[col] = main[col][i + 1];
            }
            let aux_curr: Vec<F> = (0..AUX_WIDTH).map(|c| aux[c][i]).collect();
            let aux_next: Vec<F> = (0..AUX_WIDTH).map(|c| aux[c][i + 1]).collect();

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
                    "aux constraint {j} violated at transition {i}→{}",
                    i + 1
                );
            }
        }
    }

    #[test]
    fn build_aux_columns_limbs_are_zero() {
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
        let rands = [F::new(100), F::new(200)];
        let aux = build_aux_columns(&main, &rands);

        for (limb0, limb1) in aux[AUX_COL_LIMB_0].iter().zip(&aux[AUX_COL_LIMB_1]) {
            assert_eq!(*limb0, F::ZERO);
            assert_eq!(*limb1, F::ZERO);
        }
    }
}
