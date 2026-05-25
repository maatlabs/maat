//! Builtin-segment ABI for the Maat AIR.
//!
//! A *builtin* is a self-contained mini-AIR that owns a slice of the
//! auxiliary trace segment together with the verifier challenges and
//! transition constraints needed to prove its operation semantics.
//! Operations expensive to arithmetize, e.g., range check, bitwise,
//! ordering, hash builtins, etc. are implemented as builtins.
//!
//! # Composition
//!
//! [`BuiltinSet`] aggregates all builtins. Aux columns are laid out in
//! registration order, immediately after the unified memory permutation columns
//! owned by the CPU AIR. Verifier challenges are partitioned the same way:
//! the memory permutation consumes the first `MEMORY_NUM_AUX_RANDS` entries, then each
//! builtin consumes its share in registration order.
//!
//! # Adding a new builtin
//!
//! 1. Define a unit struct (e.g. `BitwiseBuiltin`).
//! 2. Implement the [`Builtin`] trait (the trait carries the entire
//!    public surface--aux width, randomness, constraint count,
//!    constraint degrees, boundary-assertion count, reserved address
//!    range--so the layout adapts to runtime-shaped builtins).
//! 3. Register the struct as a field of [`BuiltinSet`] and extend
//!    [`Layout::compute`] + the dispatch lines in
//!    `evaluate_aux_transition`, `build_aux_columns`, and
//!    `aux_assertions`.

pub mod bitwise;
pub mod diluted;
pub mod identity;
pub mod logup;
pub mod range_check;

pub use bitwise::BitwiseBuiltin;
pub use diluted::{
    CHUNKS_PER_OPERAND, ChunkBitwiseWitness, DILUTED_BITS, NATIVE_BITS, POOL_SIZE, POOL_TABLE_ID,
    SPREAD_MASK, STRIDE, bitwise_identity_residuals, chunk_decompose, chunk_recompose,
    chunk_weight, chunk_witness, dilute, is_in_pool, pool_entries, undilute,
};
pub use identity::IdentityBuiltin;
pub use logup::{
    AirPool, Channel, ChannelSource, LogUpBuiltin, LogUpColumns, LookupTable, TableId, TableSpec,
    evaluate_transition_step,
};
use maat_field::{BaseElement, ExtensionOf, FieldElement};
pub use range_check::RangeCheckBuiltin;
use winter_air::Assertion;

use crate::aux_segment::{MEMORY_AUX_WIDTH, MEMORY_NUM_AUX_RANDS};
use crate::builtin::bitwise::{D_A_OFFSET, D_AND_OFFSET, D_B_OFFSET, D_OUT_OFFSET, POW_K_OFFSET};
use crate::builtin::range_check::{
    RC_B0_HI, RC_B0_LO, RC_B1_HI, RC_B1_LO, RC_B2_HI, RC_B2_LO, RC_B3_HI, RC_B3_LO, TABLE_SIZE,
};

/// Byte-table LogUp pool ID consumed by [`RangeCheckBuiltin`].
pub const BYTE_TABLE_ID: TableId = 0;

/// Pow2-paired LogUp pool ID. Pinned table entries are
/// `{delta * k + 2^k : k = 0..NUM_POW2_ENTRIES}` where `delta` is the
/// pool's Fiat-Shamir compression challenge.
pub const POW2_TABLE_ID: TableId = 2;

/// Number of pow2 table entries: `{1, 2, 4, ..., 2^63}` covers every
/// 64-bit shift amount.
pub const NUM_POW2_ENTRIES: usize = 64;

/// Diluted-paired LogUp pool ID.
pub const DILUTED_TABLE_ID: TableId = 3;

/// Number of diluted-paired table entries: one per 8-bit chunk value.
pub const NUM_DILUTED_ENTRIES: usize = 256;

/// Builds the byte-table [`AirPool`] consumed by [`RangeCheckBuiltin`].
fn build_air_pool_for_range_check(rc_aux_base: usize) -> AirPool {
    use maat_trace::table::{COL_RC_L0, COL_RC_L1, COL_RC_L2, COL_RC_L3};

    let table_entries: Vec<BaseElement> = (0..TABLE_SIZE as u64).map(BaseElement::new).collect();

    let channel_specs = [
        (ChannelSource::HighByte(COL_RC_L0), RC_B0_HI),
        (ChannelSource::LowByte(COL_RC_L0), RC_B0_LO),
        (ChannelSource::HighByte(COL_RC_L1), RC_B1_HI),
        (ChannelSource::LowByte(COL_RC_L1), RC_B1_LO),
        (ChannelSource::HighByte(COL_RC_L2), RC_B2_HI),
        (ChannelSource::LowByte(COL_RC_L2), RC_B2_LO),
        (ChannelSource::HighByte(COL_RC_L3), RC_B3_HI),
        (ChannelSource::LowByte(COL_RC_L3), RC_B3_LO),
    ];

    let channels = channel_specs
        .into_iter()
        .map(|(source, local_off)| Channel {
            source,
            aux_column: rc_aux_base + local_off,
            gate_main_cols: Vec::new(),
        })
        .collect();

    AirPool {
        table_id: BYTE_TABLE_ID,
        spec: TableSpec::Fixed(table_entries),
        channels,
    }
}

/// Builds the pow2-paired [`AirPool`] consumed by [`BitwiseBuiltin`]'s
/// SHL/SHR rows.
fn build_air_pool_pow2(bitwise_aux_base: usize) -> AirPool {
    use maat_trace::selector::{SUB_SEL_SHL, SUB_SEL_SHR};
    use maat_trace::table::{COL_S0, COL_SUB_SEL_BASE};

    let pow_k_col = bitwise_aux_base + POW_K_OFFSET;
    AirPool {
        table_id: POW2_TABLE_ID,
        spec: TableSpec::Pow2Paired {
            num_entries: NUM_POW2_ENTRIES,
        },
        channels: vec![Channel {
            source: ChannelSource::Pow2Paired {
                main_col: COL_S0,
                aux_col: pow_k_col,
            },
            aux_column: pow_k_col,
            gate_main_cols: vec![
                COL_SUB_SEL_BASE + SUB_SEL_SHL,
                COL_SUB_SEL_BASE + SUB_SEL_SHR,
            ],
        }],
    }
}

/// Builds the diluted-paired [`AirPool`] consumed by the chunked AND/OR/XOR rows.
fn build_air_pool_diluted(bitwise_aux_base: usize) -> AirPool {
    use maat_trace::selector::{SUB_SEL_AND, SUB_SEL_OR, SUB_SEL_XOR};
    use maat_trace::table::{
        COL_CHUNK_A, COL_CHUNK_AND, COL_CHUNK_B, COL_CHUNK_OUT, COL_SUB_SEL_BASE,
    };

    let gate = vec![
        COL_SUB_SEL_BASE + SUB_SEL_AND,
        COL_SUB_SEL_BASE + SUB_SEL_OR,
        COL_SUB_SEL_BASE + SUB_SEL_XOR,
    ];

    let channel_specs = [
        (COL_CHUNK_A, bitwise_aux_base + D_A_OFFSET),
        (COL_CHUNK_B, bitwise_aux_base + D_B_OFFSET),
        (COL_CHUNK_AND, bitwise_aux_base + D_AND_OFFSET),
        (COL_CHUNK_OUT, bitwise_aux_base + D_OUT_OFFSET),
    ];

    let channels = channel_specs
        .into_iter()
        .map(|(main_col, aux_col)| Channel {
            source: ChannelSource::DilutedPaired { main_col, aux_col },
            aux_column: aux_col,
            gate_main_cols: gate.clone(),
        })
        .collect();

    AirPool {
        table_id: DILUTED_TABLE_ID,
        spec: TableSpec::DilutedPaired {
            num_entries: NUM_DILUTED_ENTRIES,
        },
        channels,
    }
}

pub trait Builtin {
    fn name(&self) -> &'static str;

    fn aux_width(&self) -> usize;

    fn num_aux_rands(&self) -> usize;

    fn num_aux_constraints(&self) -> usize;

    fn num_aux_assertions(&self) -> usize;

    fn aux_constraint_degrees(&self) -> Vec<usize>;

    fn reserved_address_range(&self) -> (u64, u64);

    #[allow(clippy::too_many_arguments)]
    fn evaluate_aux_transition<F, E>(
        &self,
        main_curr: &[F],
        main_next: &[F],
        aux_curr: &[E],
        aux_next: &[E],
        base_offset: usize,
        rand_elements: &[E],
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>;

    fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_columns: &[&[BaseElement]],
        rand_elements: &[E],
    ) -> Vec<Vec<E>>;

    fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
        &self,
        column_base: usize,
        last_step: usize,
    ) -> Vec<Assertion<E>>;
}

/// Memoized aux-column / randomness layout for a [`BuiltinSet`].
#[derive(Clone, Copy, Debug)]
struct Layout {
    range_check_aux_base: usize,
    bitwise_aux_base: usize,
    identity_aux_base: usize,
    logup_aux_base: usize,
    aux_end: usize,
    range_check_rand_base: usize,
    bitwise_rand_base: usize,
    identity_rand_base: usize,
    logup_rand_base: usize,
    rand_end: usize,
    total_num_aux_constraints: usize,
    total_num_aux_assertions: usize,
}

impl Layout {
    fn compute(
        range_check: &RangeCheckBuiltin,
        bitwise: &BitwiseBuiltin,
        identity: &IdentityBuiltin,
        logup: &LogUpBuiltin,
    ) -> Self {
        let range_check_aux_base = MEMORY_AUX_WIDTH;
        let bitwise_aux_base = range_check_aux_base + range_check.aux_width();
        let identity_aux_base = bitwise_aux_base + bitwise.aux_width();
        let logup_aux_base = identity_aux_base + identity.aux_width();
        let aux_end = logup_aux_base + logup.aux_width();

        let range_check_rand_base = MEMORY_NUM_AUX_RANDS;
        let bitwise_rand_base = range_check_rand_base + range_check.num_aux_rands();
        let identity_rand_base = bitwise_rand_base + bitwise.num_aux_rands();
        let logup_rand_base = identity_rand_base + identity.num_aux_rands();
        let rand_end = logup_rand_base + logup.num_aux_rands();

        let total_num_aux_constraints = range_check.num_aux_constraints()
            + bitwise.num_aux_constraints()
            + identity.num_aux_constraints()
            + logup.num_aux_constraints();
        let total_num_aux_assertions = range_check.num_aux_assertions()
            + bitwise.num_aux_assertions()
            + identity.num_aux_assertions()
            + logup.num_aux_assertions();

        Self {
            range_check_aux_base,
            bitwise_aux_base,
            identity_aux_base,
            logup_aux_base,
            aux_end,
            range_check_rand_base,
            bitwise_rand_base,
            identity_rand_base,
            logup_rand_base,
            rand_end,
            total_num_aux_constraints,
            total_num_aux_assertions,
        }
    }
}

pub static BUILTIN_SET: std::sync::LazyLock<BuiltinSet> = std::sync::LazyLock::new(BuiltinSet::new);

#[derive(Clone, Debug)]
pub struct BuiltinSet {
    pub range_check: RangeCheckBuiltin,
    pub bitwise: BitwiseBuiltin,
    pub identity: IdentityBuiltin,
    pub logup: LogUpBuiltin,
    layout: Layout,
}

impl Default for BuiltinSet {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinSet {
    pub fn new() -> Self {
        let range_check = RangeCheckBuiltin;
        let bitwise = BitwiseBuiltin;
        let identity = IdentityBuiltin;

        let layout_without_logup =
            Layout::compute(&range_check, &bitwise, &identity, &LogUpBuiltin::default());
        let rc_base = layout_without_logup.range_check_aux_base;

        let byte_table_pool = build_air_pool_for_range_check(rc_base);
        let pow2_pool = build_air_pool_pow2(layout_without_logup.bitwise_aux_base);
        let diluted_pool = build_air_pool_diluted(layout_without_logup.bitwise_aux_base);
        let logup = LogUpBuiltin::with_pools(vec![byte_table_pool, pow2_pool, diluted_pool]);

        let layout = Layout::compute(&range_check, &bitwise, &identity, &logup);

        Self {
            range_check,
            bitwise,
            identity,
            logup,
            layout,
        }
    }

    pub fn range_check_aux_base(&self) -> usize {
        self.layout.range_check_aux_base
    }

    pub fn bitwise_aux_base(&self) -> usize {
        self.layout.bitwise_aux_base
    }

    pub fn identity_aux_base(&self) -> usize {
        self.layout.identity_aux_base
    }

    pub fn logup_aux_base(&self) -> usize {
        self.layout.logup_aux_base
    }

    pub fn range_check_rand_base(&self) -> usize {
        self.layout.range_check_rand_base
    }

    pub fn bitwise_rand_base(&self) -> usize {
        self.layout.bitwise_rand_base
    }

    pub fn identity_rand_base(&self) -> usize {
        self.layout.identity_rand_base
    }

    pub fn logup_rand_base(&self) -> usize {
        self.layout.logup_rand_base
    }

    pub fn total_aux_width(&self) -> usize {
        self.layout.aux_end - MEMORY_AUX_WIDTH
    }

    pub fn total_num_aux_rands(&self) -> usize {
        self.layout.rand_end - MEMORY_NUM_AUX_RANDS
    }

    pub fn total_num_aux_constraints(&self) -> usize {
        self.layout.total_num_aux_constraints
    }

    pub fn total_num_aux_assertions(&self) -> usize {
        self.layout.total_num_aux_assertions
    }

    pub fn aux_constraint_degrees(&self) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.total_num_aux_constraints());
        out.extend(self.range_check.aux_constraint_degrees());
        out.extend(self.bitwise.aux_constraint_degrees());
        out.extend(self.identity.aux_constraint_degrees());
        out.extend(self.logup.aux_constraint_degrees());
        out
    }

    pub fn evaluate_aux_transition<F, E>(
        &self,
        main_curr: &[F],
        main_next: &[F],
        aux_curr: &[E],
        aux_next: &[E],
        rand_elements: &[E],
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
    {
        debug_assert_eq!(result.len(), self.total_num_aux_constraints());

        let (rc_result, rest) = result.split_at_mut(self.range_check.num_aux_constraints());
        let (bw_result, rest) = rest.split_at_mut(self.bitwise.num_aux_constraints());
        let (id_result, lu_result) = rest.split_at_mut(self.identity.num_aux_constraints());

        let rc_rands =
            &rand_elements[self.layout.range_check_rand_base..self.layout.bitwise_rand_base];

        self.range_check.evaluate_aux_transition::<F, E>(
            main_curr,
            main_next,
            aux_curr,
            aux_next,
            self.layout.range_check_aux_base,
            rc_rands,
            rc_result,
        );

        let bw_rands =
            &rand_elements[self.layout.bitwise_rand_base..self.layout.identity_rand_base];

        self.bitwise.evaluate_aux_transition::<F, E>(
            main_curr,
            main_next,
            aux_curr,
            aux_next,
            self.layout.bitwise_aux_base,
            bw_rands,
            bw_result,
        );

        let id_rands = &rand_elements[self.layout.identity_rand_base..self.layout.logup_rand_base];

        self.identity.evaluate_aux_transition::<F, E>(
            main_curr,
            main_next,
            aux_curr,
            aux_next,
            self.layout.identity_aux_base,
            id_rands,
            id_result,
        );

        let lu_rands = &rand_elements[self.layout.logup_rand_base..self.layout.rand_end];

        self.logup.evaluate_aux_transition::<F, E>(
            main_curr,
            main_next,
            aux_curr,
            aux_next,
            self.layout.logup_aux_base,
            lu_rands,
            lu_result,
        );
    }

    pub fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_columns: &[&[BaseElement]],
        rand_elements: &[E],
    ) -> Vec<Vec<E>> {
        let rc_rands =
            &rand_elements[self.layout.range_check_rand_base..self.layout.bitwise_rand_base];
        let bw_rands =
            &rand_elements[self.layout.bitwise_rand_base..self.layout.identity_rand_base];
        let id_rands = &rand_elements[self.layout.identity_rand_base..self.layout.logup_rand_base];
        let lu_rands = &rand_elements[self.layout.logup_rand_base..self.layout.rand_end];

        let mut cols = Vec::with_capacity(self.total_aux_width());
        cols.extend(self.range_check.build_aux_columns(main_columns, rc_rands));
        cols.extend(self.bitwise.build_aux_columns(main_columns, bw_rands));
        cols.extend(self.identity.build_aux_columns(main_columns, id_rands));
        cols.extend(self.logup.build_aux_columns(main_columns, lu_rands));
        cols
    }

    pub fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
        &self,
        last_step: usize,
    ) -> Vec<Assertion<E>> {
        let mut out = Vec::with_capacity(self.total_num_aux_assertions());
        out.extend(
            self.range_check
                .aux_assertions::<E>(self.layout.range_check_aux_base, last_step),
        );
        out.extend(
            self.bitwise
                .aux_assertions::<E>(self.layout.bitwise_aux_base, last_step),
        );
        out.extend(
            self.identity
                .aux_assertions::<E>(self.layout.identity_aux_base, last_step),
        );
        out.extend(
            self.logup
                .aux_assertions::<E>(self.layout.logup_aux_base, last_step),
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use maat_trace::table::TRACE_WIDTH;

    use super::*;
    use crate::aux_segment::{MEMORY_AUX_WIDTH, MEMORY_NUM_AUX_RANDS};

    type F = BaseElement;

    #[test]
    fn four_builtins_active_compose() {
        let set = BuiltinSet::new();
        assert_eq!(
            set.total_aux_width(),
            set.range_check.aux_width()
                + set.bitwise.aux_width()
                + set.identity.aux_width()
                + set.logup.aux_width()
        );
        // Byte-table pool draws 1 rand (alpha); pow2 paired pool draws 2
        // (alpha, delta); diluted paired pool draws 2 (alpha, gamma).
        assert_eq!(set.total_num_aux_rands(), 5);
        assert_eq!(
            set.total_num_aux_constraints(),
            set.range_check.num_aux_constraints()
                + set.bitwise.num_aux_constraints()
                + set.identity.num_aux_constraints()
                + set.logup.num_aux_constraints()
        );

        let aux_full_width = MEMORY_AUX_WIDTH + set.total_aux_width();
        let n = 8usize;
        let main = vec![vec![F::ZERO; n]; TRACE_WIDTH];
        let main_slices: Vec<&[F]> = main.iter().map(|c| c.as_slice()).collect();

        let total_rands = MEMORY_NUM_AUX_RANDS + set.total_num_aux_rands();
        let rands: Vec<F> = (0..total_rands).map(|i| F::new(7 + i as u64)).collect();

        let builtin_cols = set.build_aux_columns(&main_slices, &rands);
        assert_eq!(builtin_cols.len(), set.total_aux_width());

        let mut aux_full = vec![vec![F::ZERO; n]; aux_full_width];
        for (i, col) in builtin_cols.into_iter().enumerate() {
            aux_full[MEMORY_AUX_WIDTH + i] = col;
        }

        for i in 0..n - 1 {
            let main_curr: Vec<F> = (0..TRACE_WIDTH).map(|c| main[c][i]).collect();
            let main_next: Vec<F> = (0..TRACE_WIDTH).map(|c| main[c][i + 1]).collect();
            let aux_curr: Vec<F> = (0..aux_full_width).map(|c| aux_full[c][i]).collect();
            let aux_next: Vec<F> = (0..aux_full_width).map(|c| aux_full[c][i + 1]).collect();

            let mut result = vec![F::ZERO; set.total_num_aux_constraints()];
            set.evaluate_aux_transition(
                &main_curr,
                &main_next,
                &aux_curr,
                &aux_next,
                &rands,
                &mut result,
            );

            for (j, r) in result.iter().enumerate() {
                assert_eq!(*r, F::ZERO, "builtin constraint {j} non-zero at row {i}");
            }
        }
    }

    #[test]
    fn registry_layout_matches_concrete_builtins() {
        let set = BuiltinSet::new();
        assert_eq!(set.range_check_aux_base(), MEMORY_AUX_WIDTH);
        assert_eq!(
            set.bitwise_aux_base(),
            MEMORY_AUX_WIDTH + set.range_check.aux_width()
        );
        assert_eq!(
            set.identity_aux_base(),
            MEMORY_AUX_WIDTH + set.range_check.aux_width() + set.bitwise.aux_width()
        );
        assert_eq!(
            set.logup_aux_base(),
            MEMORY_AUX_WIDTH
                + set.range_check.aux_width()
                + set.bitwise.aux_width()
                + set.identity.aux_width()
        );
        assert_eq!(set.range_check_rand_base(), MEMORY_NUM_AUX_RANDS);
        assert_eq!(
            set.bitwise_rand_base(),
            MEMORY_NUM_AUX_RANDS + set.range_check.num_aux_rands()
        );
        assert_eq!(
            set.identity_rand_base(),
            MEMORY_NUM_AUX_RANDS + set.range_check.num_aux_rands() + set.bitwise.num_aux_rands()
        );
        assert_eq!(
            set.logup_rand_base(),
            MEMORY_NUM_AUX_RANDS
                + set.range_check.num_aux_rands()
                + set.bitwise.num_aux_rands()
                + set.identity.num_aux_rands()
        );
    }

    #[test]
    fn reserved_address_ranges_are_disjoint() {
        let ranges = [
            RangeCheckBuiltin::RESERVED_ADDRESS_RANGE,
            BitwiseBuiltin::RESERVED_ADDRESS_RANGE,
            IdentityBuiltin::RESERVED_ADDRESS_RANGE,
            LogUpBuiltin::RESERVED_ADDRESS_RANGE,
        ];
        for (lo, hi) in ranges {
            assert!(lo <= hi);
        }
        for i in 0..ranges.len() {
            for j in (i + 1)..ranges.len() {
                let (a_lo, a_hi) = ranges[i];
                let (b_lo, b_hi) = ranges[j];
                assert!(a_hi < b_lo || b_hi < a_lo, "ranges {i} and {j} overlap");
            }
        }
    }
}
