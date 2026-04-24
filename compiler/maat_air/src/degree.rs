//! Transition constraint degree computation.
//!
//! Winterfell requires that declared constraint degrees exactly match the
//! actual polynomial degrees observed during proof generation. A selector-gated
//! constraint whose opcode never fires in the trace produces the zero
//! polynomial (degree 0), but a static degree declaration would claim
//! degree 2 or higher, triggering a `debug_assert` failure and producing
//! an incorrectly-sized quotient polynomial.
//!
//! This module computes a per-constraint *activity mask* from the main trace
//! data, then uses that mask to emit the correct degree for each constraint.
//! Degenerate (zero-polynomial) constraints are declared with degree 1,
//! yielding an expected quotient degree of 0, which matches the zero polynomial.
//!
//! # Soundness
//!
//! The activity mask is embedded in [`winter_air::TraceInfo::meta`] and travels
//! with the proof. The verifier reconstructs the same degrees from the mask.
//!
//! If a malicious prover falsely declares a constraint as degenerate (degree 1)
//! but actually activates the corresponding opcode, the constraint polynomial
//! has degree > 1(n−1), producing a quotient of degree > 0. FRI expects
//! degree 0 and rejects the proof.
//!
//! Conversely, declaring an inactive constraint as active is harmless: the
//! quotient has lower degree than expected, and FRI accepts (it checks `<=`).

use maat_trace::{
    COL_CMP_INV, COL_DIV_AUX, COL_FP, COL_IS_READ, COL_MEM_ADDR, COL_MEM_VAL, COL_NONZERO_INV,
    COL_OUT, COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3, COL_S0, COL_SEL_BASE, COL_SUB_SEL_BASE,
    SUB_SEL_ADD, SUB_SEL_DIV, SUB_SEL_EQ, SUB_SEL_FELT_ADD, SUB_SEL_FELT_MUL, SUB_SEL_FELT_SUB,
    SUB_SEL_NEG, SUB_SEL_NEQ, SUB_SEL_SUB,
};
use winter_math::FieldElement;
use winter_math::fields::f64::BaseElement;

use crate::aux_segment::{AUX_CONSTRAINT_DEGREES, NUM_AUX_CONSTRAINTS};
use crate::main_segment::{
    CONSTRAINT_DEGREES, NUM_CONSTRAINTS, NUM_SELECTORS, SEL_ARITH, SEL_BITWISE, SEL_CALL, SEL_CMP,
    SEL_COND_JUMP, SEL_CONVERT, SEL_DIV_MOD, SEL_FELT, SEL_JUMP, SEL_LOAD, SEL_NOP, SEL_PUSH,
    SEL_RETURN, SEL_STORE, SEL_UNARY,
};

/// Degree assigned to degenerate (zero-polynomial) constraints.
///
/// A transition constraint with degree 1 produces an expected quotient degree
/// of `(1 − 1) * (n − 1) = 0`, which matches the zero polynomial exactly.
const DEGENERATE_DEGREE: usize = 1;

/// Number of bytes used to serialize the activity mask in
/// [`winter_air::TraceInfo::meta`].
///
/// The mask is `u128`-wide because the AIR currently declares
/// `NUM_CONSTRAINTS + NUM_AUX_CONSTRAINTS = 64 + 8 = 72` transition
/// constraints, exceeding the 64-bit envelope.
pub const MASK_BYTES: usize = 16;

/// Per-row activity flags derived once from the main trace.
struct TraceActivity {
    active_sels: u32,
    fp_changes: bool,
    out_changes: bool,
    mem_val_equals_s0: bool,
    is_read_varies: bool,
    nonzero_inv_varies: bool,
    sub_sel_active: [bool; 9],
    cmp_inv_varies: bool,
    div_aux_varies: bool,
}

/// Computes a bitmask indicating which transition constraints are non-degenerate
/// (achieve their declared polynomial degree) on the given main trace.
///
/// Bits `0..NUM_CONSTRAINTS` correspond to the main-segment constraints; bits
/// `NUM_CONSTRAINTS..NUM_CONSTRAINTS + NUM_AUX_CONSTRAINTS` correspond to the
/// auxiliary-segment constraints. A set bit means the constraint is active;
/// a clear bit means it is the zero polynomial.
///
/// The returned value must be encoded as [`MASK_BYTES`] little-endian bytes in
/// [`winter_air::TraceInfo::meta`] so that `decode_mask` can reconstruct the
/// correct declarations during AIR construction.
pub fn encode_mask(main_columns: &[Vec<BaseElement>]) -> u128 {
    let n = main_columns[0].len();
    let mut mask = 0u128;

    let activity = compute_activity(main_columns, n);

    for i in 0..NUM_CONSTRAINTS {
        if main_constraint_active(i, &activity, main_columns, n) {
            mask |= 1u128 << i;
        }
    }

    let aux_flags = aux_non_degenerate(main_columns, n);
    for (i, active) in aux_flags.iter().enumerate() {
        if *active {
            mask |= 1u128 << (NUM_CONSTRAINTS + i);
        }
    }

    mask
}

/// Decodes the activity mask from trace metadata and returns the degree
/// arrays for main and auxiliary constraints.
///
/// When `meta` is empty (e.g. in unit tests that construct the AIR directly
/// without a prover), the original static degrees are returned unchanged.
pub fn decode_mask(meta: &[u8]) -> ([usize; NUM_CONSTRAINTS], [usize; NUM_AUX_CONSTRAINTS]) {
    if meta.len() < MASK_BYTES {
        return (CONSTRAINT_DEGREES, AUX_CONSTRAINT_DEGREES);
    }

    let mask = u128::from_le_bytes(
        meta[..MASK_BYTES]
            .try_into()
            .expect("meta slice must be at least MASK_BYTES bytes"),
    );

    let main = core::array::from_fn(|i| {
        if mask & (1u128 << i) != 0 {
            CONSTRAINT_DEGREES[i]
        } else {
            DEGENERATE_DEGREE
        }
    });

    let aux = core::array::from_fn(|i| {
        if mask & (1u128 << (NUM_CONSTRAINTS + i)) != 0 {
            AUX_CONSTRAINT_DEGREES[i]
        } else {
            DEGENERATE_DEGREE
        }
    });

    (main, aux)
}

/// Builds a bitmask of which selector columns contain at least one non-zero entry.
fn active_selectors(cols: &[Vec<BaseElement>], n: usize) -> u32 {
    let mut mask = 0u32;
    let all_active = (1u32 << NUM_SELECTORS) - 1;

    for i in 0..NUM_SELECTORS {
        if cols[COL_SEL_BASE + i]
            .iter()
            .take(n)
            .any(|&v| v != BaseElement::ZERO)
        {
            mask |= 1 << i;
        }
        if mask == all_active {
            break;
        }
    }
    mask
}

/// Returns `true` if any consecutive pair of values in a column differs.
fn column_changes(col: &[BaseElement]) -> bool {
    col.windows(2).any(|w| w[0] != w[1])
}

/// Returns `true` if any value in the column differs from the first.
fn column_varies(col: &[BaseElement]) -> bool {
    let first = col[0];
    col.iter().any(|&v| v != first)
}

/// Returns `true` if two columns have identical values at every row.
fn columns_equal(a: &[BaseElement], b: &[BaseElement]) -> bool {
    a.iter().zip(b.iter()).all(|(&x, &y)| x == y)
}

/// Returns `true` if any value in the column is non-zero.
fn column_non_zero(col: &[BaseElement]) -> bool {
    col.iter().any(|&v| v != BaseElement::ZERO)
}

fn compute_activity(main_columns: &[Vec<BaseElement>], n: usize) -> TraceActivity {
    let active_sels = active_selectors(main_columns, n);
    let fp_changes = column_changes(&main_columns[COL_FP]);
    let out_changes = column_changes(&main_columns[COL_OUT]);
    let mem_val_equals_s0 = columns_equal(&main_columns[COL_MEM_VAL], &main_columns[COL_S0]);

    let is_read_varies = column_varies(&main_columns[COL_IS_READ]);
    let nonzero_inv_varies = column_varies(&main_columns[COL_NONZERO_INV]);

    let sub_sel_active = [
        column_non_zero(&main_columns[COL_SUB_SEL_BASE + SUB_SEL_ADD]),
        column_non_zero(&main_columns[COL_SUB_SEL_BASE + SUB_SEL_SUB]),
        column_non_zero(&main_columns[COL_SUB_SEL_BASE + SUB_SEL_DIV]),
        column_non_zero(&main_columns[COL_SUB_SEL_BASE + SUB_SEL_NEG]),
        column_non_zero(&main_columns[COL_SUB_SEL_BASE + SUB_SEL_FELT_ADD]),
        column_non_zero(&main_columns[COL_SUB_SEL_BASE + SUB_SEL_FELT_SUB]),
        column_non_zero(&main_columns[COL_SUB_SEL_BASE + SUB_SEL_FELT_MUL]),
        column_non_zero(&main_columns[COL_SUB_SEL_BASE + SUB_SEL_EQ]),
        column_non_zero(&main_columns[COL_SUB_SEL_BASE + SUB_SEL_NEQ]),
    ];

    let cmp_inv_varies = column_non_zero(&main_columns[COL_CMP_INV]);
    let div_aux_varies = column_non_zero(&main_columns[COL_DIV_AUX]);

    TraceActivity {
        active_sels,
        fp_changes,
        out_changes,
        mem_val_equals_s0,
        is_read_varies,
        nonzero_inv_varies,
        sub_sel_active,
        cmp_inv_varies,
        div_aux_varies,
    }
}

/// Determines whether a main-segment constraint is non-degenerate.
///
/// A constraint is non-degenerate when its gating selector(s) are active in
/// the trace AND its inner expression is not the zero polynomial.
fn main_constraint_active(
    idx: usize,
    activity: &TraceActivity,
    main_columns: &[Vec<BaseElement>],
    _n: usize,
) -> bool {
    let sel = |s: usize| -> bool { activity.active_sels & (1 << s) != 0 };
    let any_sel = |sels: &[usize]| -> bool { sels.iter().any(|&s| sel(s)) };
    let sub = |i: usize| -> bool { activity.sub_sel_active[i] };

    let pc_uniform_classes = [
        SEL_ARITH,
        SEL_BITWISE,
        SEL_CMP,
        SEL_UNARY,
        SEL_LOAD,
        SEL_STORE,
        SEL_PUSH,
        SEL_CONVERT,
        SEL_FELT,
        SEL_DIV_MOD,
        12, // SEL_CONSTRUCT
        14, // SEL_COLLECTION
    ];
    let width_one_classes = [
        SEL_ARITH,
        SEL_BITWISE,
        SEL_CMP,
        SEL_UNARY,
        SEL_FELT,
        SEL_DIV_MOD,
    ];

    match idx {
        0..=16 => sel(idx),
        17 => true,
        18 => sel(SEL_PUSH),
        19 => any_sel(&[SEL_ARITH, SEL_BITWISE, SEL_CMP, SEL_DIV_MOD]),
        20 => sel(SEL_UNARY),
        21 => sel(SEL_STORE),
        22 => sel(SEL_LOAD),
        23 => any_sel(&pc_uniform_classes),
        24 => any_sel(&width_one_classes),
        25 => sel(SEL_CONVERT),
        26 => sel(SEL_JUMP),
        27..=29 => sel(SEL_COND_JUMP),
        30 => sel(SEL_LOAD) && activity.is_read_varies,
        31 => sel(SEL_LOAD),
        32 => sel(SEL_STORE) && activity.is_read_varies,
        33 => sel(SEL_STORE) && !activity.mem_val_equals_s0,
        34 => sel(SEL_CALL),
        35 => sel(SEL_RETURN),
        36 => sel(SEL_NOP),
        37 => sel(SEL_NOP),
        38 => sel(SEL_NOP) && activity.fp_changes,
        39 => {
            column_varies(&main_columns[COL_RC_L0])
                || column_varies(&main_columns[COL_RC_L1])
                || column_varies(&main_columns[COL_RC_L2])
                || column_varies(&main_columns[COL_RC_L3])
        }
        40 => sel(SEL_CONVERT),
        41 => sel(SEL_DIV_MOD) && activity.nonzero_inv_varies,
        42 => sel(SEL_NOP) && activity.out_changes,
        43 => sub(SUB_SEL_ADD),
        44 => sub(SUB_SEL_SUB),
        45 => sub(SUB_SEL_DIV),
        46 => sub(SUB_SEL_NEG),
        47 => sub(SUB_SEL_FELT_ADD),
        48 => sub(SUB_SEL_FELT_SUB),
        49 => sub(SUB_SEL_FELT_MUL),
        50 => sub(SUB_SEL_EQ),
        51 => sub(SUB_SEL_NEQ),
        52 => sub(SUB_SEL_ADD) && sub(SUB_SEL_SUB),
        53 => sub(SUB_SEL_FELT_ADD) && sub(SUB_SEL_FELT_SUB),
        54 => sub(SUB_SEL_FELT_ADD) && sub(SUB_SEL_FELT_MUL),
        55 => sub(SUB_SEL_FELT_SUB) && sub(SUB_SEL_FELT_MUL),
        56 => sub(SUB_SEL_EQ) && sub(SUB_SEL_NEQ),
        57 => sel(SEL_ARITH),
        58 => sel(SEL_DIV_MOD) && activity.div_aux_varies,
        59 => sel(SEL_UNARY),
        60 => sub(SUB_SEL_FELT_ADD) || sub(SUB_SEL_FELT_SUB) || sub(SUB_SEL_FELT_MUL),
        61..=63 => (sub(SUB_SEL_EQ) || sub(SUB_SEL_NEQ)) && activity.cmp_inv_varies,
        _ => unreachable!("constraint index {idx} out of range"),
    }
}

/// Determines which auxiliary constraints are non-degenerate, predicted
/// entirely from the main trace columns.
fn aux_non_degenerate(main_columns: &[Vec<BaseElement>], n: usize) -> [bool; NUM_AUX_CONSTRAINTS] {
    let mem_addrs = (0..n)
        .map(|i| main_columns[COL_MEM_ADDR][i].as_int())
        .collect::<Vec<u64>>();
    let mem_vals = (0..n)
        .map(|i| main_columns[COL_MEM_VAL][i].as_int())
        .collect::<Vec<u64>>();

    let exec_pairs = mem_addrs
        .iter()
        .copied()
        .zip(mem_vals.iter().copied())
        .collect::<Vec<(u64, u64)>>();

    let mut sorted_pairs = exec_pairs.clone();
    sorted_pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let sorted_addrs = sorted_pairs.iter().map(|p| p.0).collect::<Vec<u64>>();
    let sorted_vals = sorted_pairs.iter().map(|p| p.1).collect::<Vec<u64>>();
    let has_distinct_addrs = sorted_addrs.first() != sorted_addrs.last();
    let vals_vary = sorted_vals.windows(2).any(|w| w[0] != w[1]);
    let has_repeated_addr = sorted_addrs.windows(2).any(|w| w[0] == w[1]);

    let aux_1_active = vals_vary && has_repeated_addr;

    let mem_order_differs = exec_pairs != sorted_pairs;
    let limb_pool = (0..n)
        .flat_map(|i| {
            [
                main_columns[COL_RC_L0][i].as_int(),
                main_columns[COL_RC_L1][i].as_int(),
                main_columns[COL_RC_L2][i].as_int(),
                main_columns[COL_RC_L3][i].as_int(),
            ]
        })
        .collect::<Vec<u64>>();

    let mut sorted_pool = limb_pool.clone();
    sorted_pool.sort_unstable();

    let sc: [Vec<u64>; 4] =
        core::array::from_fn(|c| (0..n).map(|i| sorted_pool[4 * i + c]).collect());

    let continuity_active = |a: &[u64], b: &[u64]| -> bool {
        a.iter().zip(b.iter()).any(|(&x, &y)| {
            let d = y.wrapping_sub(x);
            d > 1
        })
    };
    let rc_01 = continuity_active(&sc[0], &sc[1]);
    let rc_12 = continuity_active(&sc[1], &sc[2]);
    let rc_23 = continuity_active(&sc[2], &sc[3]);
    let rc_30 = (0..n - 1).any(|i| {
        let d = sc[0][i + 1].wrapping_sub(sc[3][i]);
        d > 1
    });

    let rc_order_differs = limb_pool != sorted_pool;

    [
        has_distinct_addrs, // 0: address continuity
        aux_1_active,       // 1: single-value consistency
        mem_order_differs,  // 2: memory grand product
        rc_01,              // 3: RC sorted continuity 0-->1
        rc_12,              // 4: RC sorted continuity 1-->2
        rc_23,              // 5: RC sorted continuity 2-->3
        rc_30,              // 6: RC sorted continuity 3-->0(next)
        rc_order_differs,   // 7: RC permutation accumulator
    ]
}

#[cfg(test)]
mod tests {
    use maat_trace::TRACE_WIDTH;

    use super::*;

    /// Creates a minimal 8-row column-major trace with all NOP selectors.
    fn nop_trace() -> Vec<Vec<BaseElement>> {
        let n = 8;
        let mut cols = vec![vec![BaseElement::ZERO; n]; TRACE_WIDTH];
        for val in cols[COL_SEL_BASE].iter_mut() {
            *val = BaseElement::ONE; // sel_nop
        }
        cols
    }

    #[test]
    fn all_nop_trace_marks_nop_active() {
        let cols = nop_trace();
        let mask = encode_mask(&cols);

        // sel_nop (bit 0) should be active.
        assert_ne!(mask & 1, 0, "sel_nop binary constraint should be active");

        // sel_push (bit 1) should be inactive.
        assert_eq!(
            mask & (1u128 << 1),
            0,
            "sel_push should be inactive on NOP-only trace"
        );

        // Constraint 17 (selector sum) is always active.
        assert_ne!(mask & (1u128 << 17), 0, "constraint 17 always active");
    }

    #[test]
    fn decode_empty_meta_returns_static_degrees() {
        let (main, aux) = decode_mask(&[]);
        assert_eq!(main, CONSTRAINT_DEGREES);
        assert_eq!(aux, AUX_CONSTRAINT_DEGREES);
    }

    #[test]
    fn decode_all_active_returns_static_degrees() {
        let total = NUM_CONSTRAINTS + NUM_AUX_CONSTRAINTS;
        let mask: u128 = if total >= 128 {
            u128::MAX
        } else {
            (1u128 << total) - 1
        };
        let meta = mask.to_le_bytes();
        let (main, aux) = decode_mask(&meta);
        assert_eq!(main, CONSTRAINT_DEGREES);
        assert_eq!(aux, AUX_CONSTRAINT_DEGREES);
    }

    #[test]
    fn decode_all_inactive_returns_degenerate() {
        let meta = 0u128.to_le_bytes();
        let (main, aux) = decode_mask(&meta);
        for (i, &d) in main.iter().enumerate() {
            assert_eq!(
                d, DEGENERATE_DEGREE,
                "main constraint {i} should be degenerate"
            );
        }
        for (i, &d) in aux.iter().enumerate() {
            assert_eq!(
                d, DEGENERATE_DEGREE,
                "aux constraint {i} should be degenerate"
            );
        }
    }
}
