//! Auxiliary trace segment: unified memory permutation argument plus the
//! aux-column slices owned by registered builtinss.

use maat_field::{BaseElement, ExtensionOf, FieldElement};
use maat_trace::table::{COL_MEM_ADDR, COL_MEM_VAL};
use winter_air::Assertion;

use crate::builtin::BUILTIN_SET;

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
pub fn aux_width() -> usize {
    MEMORY_AUX_WIDTH + BUILTIN_SET.total_aux_width()
}

/// Total verifier challenges drawn for auxiliary column construction.
pub fn num_aux_rands() -> usize {
    MEMORY_NUM_AUX_RANDS + BUILTIN_SET.total_num_aux_rands()
}

/// Total auxiliary transition-constraint count.
pub fn num_aux_constraints() -> usize {
    MEMORY_NUM_CONSTRAINTS + BUILTIN_SET.total_num_aux_constraints()
}

/// Total auxiliary boundary-assertion count.
pub fn num_aux_assertions() -> usize {
    MEMORY_NUM_ASSERTIONS + BUILTIN_SET.total_num_aux_assertions()
}

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
    debug_assert_eq!(result.len(), num_aux_constraints());

    let (mem_result, builtin_result) = result.split_at_mut(MEMORY_NUM_CONSTRAINTS);
    let (memory_rands, _) = rand_elements.split_at(MEMORY_NUM_AUX_RANDS);
    evaluate_memory::<F, E>(main_next, aux_current, aux_next, memory_rands, mem_result);

    BUILTIN_SET.evaluate_aux_transition::<F, E>(
        main_current,
        main_next,
        aux_current,
        aux_next,
        rand_elements,
        builtin_result,
    );
}

pub fn aux_constraint_degrees() -> Vec<usize> {
    let mut out = Vec::with_capacity(num_aux_constraints());
    out.extend_from_slice(&MEMORY_AUX_CONSTRAINT_DEGREES);
    out.extend(BUILTIN_SET.aux_constraint_degrees());
    out
}

pub fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
    last_step: usize,
    rand_elements: &[E],
    output_base: u32,
    output_segment: &[BaseElement],
) -> Vec<Assertion<E>> {
    let mut out = Vec::with_capacity(num_aux_assertions());
    out.extend(memory_aux_assertions::<E>(
        last_step,
        rand_elements,
        output_base,
        output_segment,
    ));
    out.extend(BUILTIN_SET.aux_assertions::<E>(last_step));
    out
}

fn memory_aux_assertions<E: FieldElement<BaseField = BaseElement>>(
    last_step: usize,
    rand_elements: &[E],
    output_base: u32,
    output_segment: &[BaseElement],
) -> Vec<Assertion<E>> {
    let memory_rands = &rand_elements[..MEMORY_NUM_AUX_RANDS];
    let endpoint = public_memory_endpoint::<E>(memory_rands, output_base, output_segment);
    vec![
        Assertion::single(AUX_COL_MEM_ACC, 0, E::ONE),
        Assertion::single(AUX_COL_MEM_ACC, last_step, endpoint),
    ]
}

pub fn public_memory_endpoint<E: FieldElement<BaseField = BaseElement>>(
    memory_rands: &[E],
    output_base: u32,
    output_segment: &[BaseElement],
) -> E {
    if output_segment.is_empty() {
        return E::ONE;
    }
    let z = memory_rands[RAND_Z];
    let alpha = memory_rands[RAND_ALPHA];
    let mut prod = E::ONE;
    for (off, &val) in output_segment.iter().enumerate() {
        let off_u32 = u32::try_from(off).expect("output segment longer than u32::MAX");
        let addr_u64 = u64::from(output_base) + u64::from(off_u32);
        let addr = E::from(BaseElement::new(addr_u64));
        let val_e = E::from(val);
        prod *= z - (addr + alpha * val_e);
    }
    let z_pow_l = z.exp((output_segment.len() as u64).into());
    z_pow_l * prod.inv()
}

pub fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
    main_columns: &[&[BaseElement]],
    rand_elements: &[E],
    output_base: u32,
    output_segment: &[BaseElement],
) -> Vec<Vec<E>> {
    let memory_rands = &rand_elements[..MEMORY_NUM_AUX_RANDS];

    let mut columns = Vec::with_capacity(aux_width());
    columns.extend(build_memory_columns(
        main_columns,
        memory_rands,
        output_base,
        output_segment,
    ));
    columns.extend(BUILTIN_SET.build_aux_columns(main_columns, rand_elements));
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
    output_base: u32,
    output_segment: &[BaseElement],
) -> Vec<Vec<E>> {
    let n = main_columns[COL_MEM_ADDR].len();
    let z = rand_elements[RAND_Z];
    let alpha = rand_elements[RAND_ALPHA];

    let l = output_segment.len();
    let mut pairs: Vec<(BaseElement, BaseElement)> = Vec::with_capacity(n);
    let mut zeros_to_remove = l;
    for (&addr, &val) in main_columns[COL_MEM_ADDR]
        .iter()
        .zip(main_columns[COL_MEM_VAL].iter())
    {
        if zeros_to_remove > 0 && addr == BaseElement::ZERO && val == BaseElement::ZERO {
            zeros_to_remove -= 1;
            continue;
        }
        pairs.push((addr, val));
    }
    for (off, &val) in output_segment.iter().enumerate() {
        let off_u32 = u32::try_from(off).expect("public-output segment longer than u32::MAX cells");
        let addr_u64 = u64::from(output_base) + u64::from(off_u32);
        pairs.push((BaseElement::new(addr_u64), val));
    }
    debug_assert_eq!(
        pairs.len(),
        n,
        "L2 multiset must equal trace length after public-memory swap",
    );

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
    use crate::builtin::Builtin;
    use crate::builtin::range_check::RangeCheckBuiltin;

    type F = BaseElement;

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

    fn make_aux_row(l2_addr: u64, l2_val: u64, mem_acc: F, identity: F) -> Vec<F> {
        let mut row = vec![F::ZERO; aux_width()];
        row[AUX_COL_L2_ADDR] = F::new(l2_addr);
        row[AUX_COL_L2_VAL] = F::new(l2_val);
        row[AUX_COL_MEM_ACC] = mem_acc;
        row[BUILTIN_SET.identity_aux_base()] = identity;
        row
    }

    fn rands(z: F, alpha: F, alpha_rc: F) -> Vec<F> {
        vec![z, alpha, alpha_rc]
    }

    fn identity_constraint_offset() -> usize {
        MEMORY_NUM_CONSTRAINTS
            + BUILTIN_SET.range_check.num_aux_constraints()
            + BUILTIN_SET.bitwise.num_aux_constraints()
    }

    #[test]
    fn address_continuity_same_addr_passes() {
        let main = vec![F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(5, 10, F::ONE, F::ONE);
        let aux_next = make_aux_row(5, 10, F::ONE, F::ONE);
        let mut result = vec![F::ZERO; num_aux_constraints()];
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
        let aux_curr = make_aux_row(5, 10, F::ONE, F::ONE);
        let aux_next = make_aux_row(7, 20, F::ONE, F::ONE);
        let mut result = vec![F::ZERO; num_aux_constraints()];
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
        let aux_curr = make_aux_row(5, 42, F::ONE, F::ONE);
        let aux_next = make_aux_row(5, 99, F::ONE, F::ONE);
        let mut result = vec![F::ZERO; num_aux_constraints()];
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
    fn identity_builtin_frozen_passes() {
        let main = vec![F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(0, 0, F::ONE, F::ONE);
        let aux_next = make_aux_row(0, 0, F::ONE, F::ONE);
        let mut result = vec![F::ZERO; num_aux_constraints()];
        evaluate(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            &rands(F::new(7), F::new(3), F::new(11)),
            &mut result,
        );
        assert_eq!(result[identity_constraint_offset()], F::ZERO);
    }

    #[test]
    fn identity_builtin_drift_fails() {
        let main = vec![F::ZERO; TRACE_WIDTH];
        let aux_curr = make_aux_row(0, 0, F::ONE, F::ONE);
        let aux_next = make_aux_row(0, 0, F::ONE, F::new(2));
        let mut result = vec![F::ZERO; num_aux_constraints()];
        evaluate(
            &main,
            &main,
            &aux_curr,
            &aux_next,
            &rands(F::new(7), F::new(3), F::new(11)),
            &mut result,
        );
        assert_ne!(result[identity_constraint_offset()], F::ZERO);
    }

    #[test]
    fn build_aux_columns_identity_permutation() {
        let n = RangeCheckBuiltin::MIN_TRACE_LEN.next_power_of_two();
        let main = mock_main_trace(&vec![(0, 0); n]);
        let rand_elements = rands(F::new(9999), F::new(13), F::new(7777));
        let aux = build_aux_columns(&column_slices(&main), &rand_elements, 0, &[]);

        assert_eq!(aux.len(), aux_width());
        assert_eq!(aux[AUX_COL_MEM_ACC][0], F::ONE);
        assert_eq!(aux[AUX_COL_MEM_ACC][n - 1], F::ONE);
        for v in &aux[BUILTIN_SET.identity_aux_base()] {
            assert_eq!(*v, F::ONE);
        }
    }

    #[test]
    fn build_aux_columns_nontrivial_permutation_satisfies_all_constraints() {
        let n = RangeCheckBuiltin::MIN_TRACE_LEN.next_power_of_two();
        let mut mem = vec![(0u64, 0u64); n];
        mem[1] = (2, 20);
        mem[2] = (1, 10);
        mem[4] = (1, 10);
        mem[5] = (2, 20);

        let mut limbs = vec![[0u64; 4]; n];
        limbs[1] = [4, 3, 2, 1];
        let main = mock_main_trace_with_limbs(&mem, &limbs);
        let rand_elements = rands(F::new(7777), F::new(31), F::new(5555));
        let slices = column_slices(&main);
        let aux = build_aux_columns(&slices, &rand_elements, 0, &[]);

        for i in 0..n - 1 {
            let main_curr: Vec<F> = (0..TRACE_WIDTH).map(|c| main[c][i]).collect();
            let main_next: Vec<F> = (0..TRACE_WIDTH).map(|c| main[c][i + 1]).collect();
            let aux_curr: Vec<F> = (0..aux_width()).map(|c| aux[c][i]).collect();
            let aux_next: Vec<F> = (0..aux_width()).map(|c| aux[c][i + 1]).collect();

            let mut result = vec![F::ZERO; num_aux_constraints()];
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
