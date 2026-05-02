//! Auxiliary trace segment: unified memory permutation argument plus the
//! aux-column slices owned by registered builtinss.

use maat_field::{BaseElement, ExtensionOf, FieldElement};
use maat_trace::table::{COL_MEM_ADDR, COL_MEM_VAL};
use winter_air::Assertion;

use crate::builtin::BuiltinSet;

/// Aux column index: sorted memory address (L2 address column).
pub const AUX_COL_L2_ADDR: usize = 0;
/// Aux column index: sorted memory value (L2 value column).
pub const AUX_COL_L2_VAL: usize = 1;
/// Aux column index: memory grand-product permutation accumulator.
pub const AUX_COL_MEM_ACC: usize = 2;

/// Number of aux columns owned by the unified memory permutation argument.
pub const MEMORY_AUX_WIDTH: usize = 3;

/// Number of verifier challenges consumed by the memory permutation argument.
///
/// `z` is the linear-combination base; `alpha` compresses the `(addr, val)`
/// tuple into a single field element.
pub const MEMORY_NUM_AUX_RANDS: usize = 2;

/// Per-constraint degrees for the memory permutation argument.
const MEMORY_AUX_CONSTRAINT_DEGREES: [usize; MEMORY_NUM_CONSTRAINTS] = [2, 2, 2];

/// Number of memory transition constraints.
const MEMORY_NUM_CONSTRAINTS: usize = 3;

/// Number of memory boundary assertions (`mem_acc[0]`, `mem_acc[last]`).
const MEMORY_NUM_ASSERTIONS: usize = 2;

/// Index of verifier challenge `z`.
const RAND_Z: usize = 0;
/// Index of verifier challenge `alpha`.
const RAND_ALPHA: usize = 1;

/// Total width of the auxiliary trace segment (memory + every registered builtin).
pub const AUX_WIDTH: usize = MEMORY_AUX_WIDTH + BuiltinSet::TOTAL_AUX_WIDTH;

/// Total verifier challenges drawn for auxiliary column construction.
pub const NUM_AUX_RANDS: usize = MEMORY_NUM_AUX_RANDS + BuiltinSet::TOTAL_NUM_AUX_RANDS;

/// Total auxiliary transition-constraint count.
pub const NUM_AUX_CONSTRAINTS: usize =
    MEMORY_NUM_CONSTRAINTS + BuiltinSet::TOTAL_NUM_AUX_CONSTRAINTS;

/// Total auxiliary boundary-assertion count.
pub const NUM_AUX_ASSERTIONS: usize = MEMORY_NUM_ASSERTIONS + BuiltinSet::TOTAL_NUM_AUX_ASSERTIONS;

pub fn evaluate<F, E>(
    main_current: &[F],
    main_next: &[F],
    aux_current: &[E],
    aux_next: &[E],
    rand_elements: &[E],
    result: &mut [E],
) where
    F: FieldElement<BaseField = BaseElement>,
    E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
{
    debug_assert_eq!(result.len(), NUM_AUX_CONSTRAINTS);

    let (mem_result, builtin_result) = result.split_at_mut(MEMORY_NUM_CONSTRAINTS);
    let (memory_rands, _) = rand_elements.split_at(MEMORY_NUM_AUX_RANDS);
    evaluate_memory::<F, E>(main_next, aux_current, aux_next, memory_rands, mem_result);

    BuiltinSet::evaluate_aux_transition::<F, E>(
        main_current,
        main_next,
        aux_current,
        aux_next,
        rand_elements,
        builtin_result,
    );
}

pub fn aux_constraint_degrees() -> Vec<usize> {
    let mut out = Vec::with_capacity(NUM_AUX_CONSTRAINTS);
    out.extend_from_slice(&MEMORY_AUX_CONSTRAINT_DEGREES);
    out.extend(BuiltinSet::aux_constraint_degrees());
    out
}

pub fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
    last_step: usize,
) -> Vec<Assertion<E>> {
    let mut out = Vec::with_capacity(NUM_AUX_ASSERTIONS);
    out.extend(memory_aux_assertions::<E>(last_step));
    out.extend(BuiltinSet::aux_assertions::<E>(last_step));
    out
}

fn memory_aux_assertions<E: FieldElement<BaseField = BaseElement>>(
    last_step: usize,
) -> Vec<Assertion<E>> {
    vec![
        Assertion::single(AUX_COL_MEM_ACC, 0, E::ONE),
        Assertion::single(AUX_COL_MEM_ACC, last_step, E::ONE),
    ]
}

pub fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
    main_columns: &[&[BaseElement]],
    rand_elements: &[E],
) -> Vec<Vec<E>> {
    let memory_rands = &rand_elements[..MEMORY_NUM_AUX_RANDS];

    let mut columns = Vec::with_capacity(AUX_WIDTH);
    columns.extend(build_memory_columns(main_columns, memory_rands));
    columns.extend(BuiltinSet::build_aux_columns(main_columns, rand_elements));
    columns
}

fn evaluate_memory<F, E>(
    main_next: &[F],
    aux_curr: &[E],
    aux_next: &[E],
    rand_elements: &[E],
    result: &mut [E],
) where
    F: FieldElement,
    E: FieldElement<BaseField = F::BaseField> + ExtensionOf<F>,
{
    debug_assert_eq!(result.len(), MEMORY_NUM_CONSTRAINTS);

    let one = E::ONE;

    let l2_addr = aux_curr[AUX_COL_L2_ADDR];
    let l2_addr_next = aux_next[AUX_COL_L2_ADDR];
    let l2_val = aux_curr[AUX_COL_L2_VAL];
    let l2_val_next = aux_next[AUX_COL_L2_VAL];
    let mem_acc = aux_curr[AUX_COL_MEM_ACC];
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
}

fn build_memory_columns<E: FieldElement<BaseField = BaseElement>>(
    main_columns: &[&[BaseElement]],
    rand_elements: &[E],
) -> Vec<Vec<E>> {
    let n = main_columns[COL_MEM_ADDR].len();
    let z = rand_elements[RAND_Z];
    let alpha = rand_elements[RAND_ALPHA];

    let mut pairs = (0..n)
        .map(|i| (main_columns[COL_MEM_ADDR][i], main_columns[COL_MEM_VAL][i]))
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
        let l1_addr_i = E::from(main_columns[COL_MEM_ADDR][i]);
        let l1_val_i = E::from(main_columns[COL_MEM_VAL][i]);
        let l1_tuple = l1_addr_i + alpha * l1_val_i;
        let l2_tuple = l2_addr[i] + alpha * l2_val[i];
        let numerator = z - l1_tuple;
        let denominator = z - l2_tuple;
        mem_acc.push(mem_acc[i - 1] * numerator * denominator.inv());
    }

    vec![l2_addr, l2_val, mem_acc]
}

#[cfg(test)]
mod tests {
    use maat_trace::table::{COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3, TRACE_WIDTH};

    use super::*;
    use crate::builtin::range_check::{RC_ACC, RC_SORTED_0, RC_SORTED_1, RC_SORTED_2, RC_SORTED_3};

    type F = BaseElement;

    fn rc_aux_col(builtin_offset: usize) -> usize {
        BuiltinSet::RANGE_CHECK_AUX_BASE + builtin_offset
    }

    /// Creates a column-major main trace with the given memory access pairs.
    fn mock_main_trace(mem_pairs: &[(u64, u64)]) -> Vec<Vec<F>> {
        let n = mem_pairs.len();
        let mut columns = vec![vec![F::ZERO; n]; TRACE_WIDTH];
        for (i, &(addr, val)) in mem_pairs.iter().enumerate() {
            columns[COL_MEM_ADDR][i] = F::new(addr);
            columns[COL_MEM_VAL][i] = F::new(val);
        }
        columns
    }

    /// Creates a column-major main trace with both memory pairs and limb data.
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

    fn column_slices(columns: &[Vec<F>]) -> Vec<&[F]> {
        columns.iter().map(|c| c.as_slice()).collect()
    }

    fn make_aux_row(
        l2_addr: u64,
        l2_val: u64,
        mem_acc: F,
        sorted: [u64; 4],
        rc_acc: F,
        identity: F,
    ) -> Vec<F> {
        let mut row = vec![F::ZERO; AUX_WIDTH];
        row[AUX_COL_L2_ADDR] = F::new(l2_addr);
        row[AUX_COL_L2_VAL] = F::new(l2_val);
        row[AUX_COL_MEM_ACC] = mem_acc;
        row[rc_aux_col(RC_SORTED_0)] = F::new(sorted[0]);
        row[rc_aux_col(RC_SORTED_1)] = F::new(sorted[1]);
        row[rc_aux_col(RC_SORTED_2)] = F::new(sorted[2]);
        row[rc_aux_col(RC_SORTED_3)] = F::new(sorted[3]);
        row[rc_aux_col(RC_ACC)] = rc_acc;
        row[BuiltinSet::IDENTITY_AUX_BASE] = identity;
        row
    }

    fn rands(z: F, alpha: F, z_rc: F) -> Vec<F> {
        vec![z, alpha, z_rc]
    }

    #[test]
    fn address_continuity_same_addr_passes() {
        let main = vec![F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 10, F::ONE, [0, 0, 0, 0], F::ONE, F::ONE);
        let aux_next = make_aux_row(5, 10, F::ONE, [0, 0, 0, 0], F::ONE, F::ONE);
        let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
        evaluate(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            &rands(F::new(7), F::new(3), F::new(11)),
            &mut result,
        );
        assert_eq!(result[0], F::ZERO);
    }

    #[test]
    fn address_continuity_increment_by_two_fails() {
        let main = vec![F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 10, F::ONE, [0, 0, 0, 0], F::ONE, F::ONE);
        let aux_next = make_aux_row(7, 20, F::ONE, [0, 0, 0, 0], F::ONE, F::ONE);
        let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
        evaluate(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            &rands(F::new(7), F::new(3), F::new(11)),
            &mut result,
        );
        assert_ne!(result[0], F::ZERO);
    }

    #[test]
    fn single_value_same_addr_different_val_fails() {
        let main = vec![F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 42, F::ONE, [0, 0, 0, 0], F::ONE, F::ONE);
        let aux_next = make_aux_row(5, 99, F::ONE, [0, 0, 0, 0], F::ONE, F::ONE);
        let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
        evaluate(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            &rands(F::new(7), F::new(3), F::new(11)),
            &mut result,
        );
        assert_ne!(result[1], F::ZERO);
    }

    #[test]
    fn rc_sorted_continuity_valid() {
        let main = vec![F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(0, 0, F::ONE, [0, 0, 1, 1], F::ONE, F::ONE);
        let aux_next = make_aux_row(0, 0, F::ONE, [2, 2, 3, 3], F::ONE, F::ONE);
        let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
        evaluate(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            &rands(F::new(7), F::new(3), F::new(11)),
            &mut result,
        );
        // Memory: 3 constraints; range-check builtin: 5 constraints;
        // identity builtin: 1 constraint. Sorted continuity 0-->3 is at indices 3..6.
        assert_eq!(result[3], F::ZERO);
        assert_eq!(result[4], F::ZERO);
        assert_eq!(result[5], F::ZERO);
        assert_eq!(result[6], F::ZERO);
    }

    #[test]
    fn rc_sorted_continuity_gap_fails() {
        let main = vec![F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(0, 0, F::ONE, [0, 5, 5, 5], F::ONE, F::ONE);
        let aux_next = make_aux_row(0, 0, F::ONE, [5, 5, 5, 5], F::ONE, F::ONE);
        let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
        evaluate(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            &rands(F::new(7), F::new(3), F::new(11)),
            &mut result,
        );
        assert_ne!(result[3], F::ZERO);
    }

    #[test]
    fn identity_builtin_frozen_passes() {
        let main = vec![F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(0, 0, F::ONE, [0, 0, 0, 0], F::ONE, F::ONE);
        let aux_next = make_aux_row(0, 0, F::ONE, [0, 0, 0, 0], F::ONE, F::ONE);
        let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
        evaluate(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            &rands(F::new(7), F::new(3), F::new(11)),
            &mut result,
        );
        // Identity constraint is the last entry.
        assert_eq!(result[NUM_AUX_CONSTRAINTS - 1], F::ZERO);
    }

    #[test]
    fn identity_builtin_drift_fails() {
        let main = vec![F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(0, 0, F::ONE, [0, 0, 0, 0], F::ONE, F::ONE);
        let aux_next = make_aux_row(0, 0, F::ONE, [0, 0, 0, 0], F::ONE, F::new(2));
        let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
        evaluate(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            &rands(F::new(7), F::new(3), F::new(11)),
            &mut result,
        );
        assert_ne!(result[NUM_AUX_CONSTRAINTS - 1], F::ZERO);
    }

    #[test]
    fn build_aux_columns_identity_permutation() {
        let main = mock_main_trace(&[(0, 0); 8]);
        let rand_elements = rands(F::new(9999), F::new(13), F::new(7777));
        let aux = build_aux_columns(&column_slices(&main), &rand_elements);

        assert_eq!(aux.len(), AUX_WIDTH);
        assert_eq!(aux[AUX_COL_MEM_ACC][0], F::ONE);
        assert_eq!(aux[AUX_COL_MEM_ACC][7], F::ONE);
        assert_eq!(aux[rc_aux_col(RC_ACC)][0], F::ONE);
        assert_eq!(aux[rc_aux_col(RC_ACC)][7], F::ONE);
        // Identity column is constant one.
        for v in &aux[BuiltinSet::IDENTITY_AUX_BASE] {
            assert_eq!(*v, F::ONE);
        }
    }

    #[test]
    fn build_aux_columns_nontrivial_permutation_satisfies_all_constraints() {
        let mem = [
            (0, 0),
            (2, 20),
            (1, 10),
            (0, 0),
            (1, 10),
            (2, 20),
            (0, 0),
            (0, 0),
        ];
        let limbs: [[u64; 4]; 8] = [
            [0, 0, 0, 0],
            [4, 3, 2, 1],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        ];
        let main = mock_main_trace_with_limbs(&mem, &limbs);
        let rand_elements = rands(F::new(7777), F::new(31), F::new(5555));
        let slices = column_slices(&main);
        let aux = build_aux_columns(&slices, &rand_elements);

        let n = 8;
        for i in 0..n - 1 {
            let main_curr: Vec<F> = (0..TRACE_WIDTH).map(|c| main[c][i]).collect();
            let main_next: Vec<F> = (0..TRACE_WIDTH).map(|c| main[c][i + 1]).collect();
            let aux_curr: Vec<F> = (0..AUX_WIDTH).map(|c| aux[c][i]).collect();
            let aux_next: Vec<F> = (0..AUX_WIDTH).map(|c| aux[c][i + 1]).collect();

            let mut result = vec![F::ZERO; NUM_AUX_CONSTRAINTS];
            evaluate(
                &main_curr,
                &main_next,
                &aux_curr,
                &aux_next,
                &rand_elements,
                &mut result,
            );
            for (j, &r) in result.iter().enumerate() {
                assert_eq!(
                    r,
                    F::ZERO,
                    "aux constraint {j} violated at transition {i} -> {}",
                    i + 1
                );
            }
        }
    }
}
