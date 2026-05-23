//! LogUp lookup-argument primitive.
//!
//! Proves that a stream of values is a multi-subset of a pinned lookup
//! table via Häböck's logarithmic-derivative argument (2022). The argument
//! reduces to the polynomial identity over a
//! verifier-supplied extension-field challenge `α` (Fiat-Shamir).
//! Soundness follows by Schwartz-Zippel; the witness consists of a
//! per-table multiplicity column `m_v` and a single grand-sum accumulator
//! `s_i` whose boundary values pin the multisets equal.

use std::collections::HashMap;

use maat_errors::LogUpError;
use maat_field::{BaseElement, ExtensionOf, FieldElement};
use winter_air::Assertion;

use super::Builtin;

pub type TableId = u32;

/// A pinned lookup table: a fixed multiset of base-field entries.
#[derive(Clone, Debug)]
pub struct LookupTable {
    id: TableId,
    entries: Vec<BaseElement>,
    index: HashMap<u64, usize>,
}

impl LookupTable {
    pub fn new(id: TableId, entries: Vec<BaseElement>) -> Self {
        let mut index = HashMap::with_capacity(entries.len());
        for (i, e) in entries.iter().enumerate() {
            index.entry(e.as_int()).or_insert(i);
        }
        Self { id, entries, index }
    }

    pub fn id(&self) -> TableId {
        self.id
    }

    pub fn entries(&self) -> &[BaseElement] {
        &self.entries
    }

    /// Returns the index of `value` in the table, or `None` if absent.
    pub fn position(&self, value: BaseElement) -> Option<usize> {
        self.index.get(&value.as_int()).copied()
    }
}

/// Per-table witness columns produced by [`LogUpBuiltin::build_columns`].
#[derive(Clone, Debug)]
pub struct LogUpColumns<E: FieldElement<BaseField = BaseElement>> {
    pub table_id: TableId,
    pub lookup_column: Vec<BaseElement>,
    pub table_column: Vec<BaseElement>,
    pub multiplicities: Vec<E>,
    pub grand_sum: Vec<E>,
}

/// Describes how a channel's per-row value is derived from the main trace.
#[derive(Clone, Copy, Debug)]
pub enum ChannelSource {
    /// High byte of a main-trace column (`(col >> 8) & 0xff`).
    HighByte(usize),
    /// Low byte of a main-trace column (`col & 0xff`).
    LowByte(usize),
}

impl ChannelSource {
    /// Extracts the channel value at `row` from `main_cols`.
    pub fn extract(&self, main_cols: &[&[BaseElement]], row: usize) -> u64 {
        match *self {
            ChannelSource::HighByte(col) => (main_cols[col][row].as_int() >> 8) & 0xff,
            ChannelSource::LowByte(col) => main_cols[col][row].as_int() & 0xff,
        }
    }
}

/// One LogUp channel: the consumer-side witness column (`aux_column`)
/// plus a recipe for recomputing the same value from the main trace
/// (`source`) at witness-build time.
#[derive(Clone, Copy, Debug)]
pub struct Channel {
    pub source: ChannelSource,
    pub aux_column: usize,
}

/// One AIR-active LogUp pool: a pinned lookup table plus the channels
/// that consume from it.
#[derive(Clone, Debug)]
pub struct AirPool {
    pub table_id: TableId,
    pub table_entries: Vec<BaseElement>,
    pub channels: Vec<Channel>,
}

impl AirPool {
    pub fn aux_width(&self) -> usize {
        4 + self.channels.len()
    }

    pub fn num_aux_rands(&self) -> usize {
        1
    }

    pub fn num_aux_constraints(&self) -> usize {
        self.channels.len() + 2
    }

    pub fn num_aux_assertions(&self) -> usize {
        self.channels.len() + 2
    }

    pub fn aux_constraint_degrees(&self) -> Vec<usize> {
        let mut out = vec![2usize; self.channels.len() + 1];
        out.push(1);
        out
    }

    pub fn min_trace_len(&self) -> usize {
        self.table_entries.len() + 1
    }

    fn t_offset(&self) -> usize {
        0
    }

    fn m_offset(&self) -> usize {
        1
    }

    fn s_base(&self) -> usize {
        2
    }

    fn sm_offset(&self) -> usize {
        2 + self.channels.len()
    }

    fn bal_offset(&self) -> usize {
        3 + self.channels.len()
    }
}

/// LogUp lookup-argument engine and shared-pool aux-state owner.
#[derive(Clone, Debug, Default)]
pub struct LogUpBuiltin {
    tables: Vec<LookupTable>,
    table_index: HashMap<TableId, usize>,
    lookups: HashMap<TableId, Vec<BaseElement>>,
    pools: Vec<AirPool>,
}

impl LogUpBuiltin {
    pub const NAME: &'static str = "logup";

    /// Reserved memory-segment range for the LogUp builtin, sitting
    /// directly above [`BitwiseBuiltin`](super::BitwiseBuiltin).
    pub const RESERVED_ADDRESS_RANGE: (u64, u64) = (1u64 << 36, (1u64 << 37) - 1);

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pools(pools: Vec<AirPool>) -> Self {
        Self {
            pools,
            ..Self::default()
        }
    }

    pub fn pools(&self) -> &[AirPool] {
        &self.pools
    }

    /// Returns the offset of the `idx`-th pool's slice within this
    /// builtin's aux footprint (i.e. relative to `base_offset`).
    fn pool_local_offset(&self, idx: usize) -> usize {
        self.pools[..idx].iter().map(AirPool::aux_width).sum()
    }

    pub fn register_table(
        &mut self,
        id: TableId,
        entries: Vec<BaseElement>,
    ) -> Result<(), LogUpError> {
        if self.table_index.contains_key(&id) {
            return Err(LogUpError::DuplicateTable(id));
        }
        let pos = self.tables.len();
        self.tables.push(LookupTable::new(id, entries));
        self.table_index.insert(id, pos);
        self.lookups.entry(id).or_default();
        Ok(())
    }

    pub fn register_lookup(
        &mut self,
        table_id: TableId,
        value: BaseElement,
    ) -> Result<(), LogUpError> {
        let table_pos = *self
            .table_index
            .get(&table_id)
            .ok_or(LogUpError::UnknownTable(table_id))?;
        let table = &self.tables[table_pos];
        if table.position(value).is_none() {
            return Err(LogUpError::ValueNotInTable {
                table: table_id,
                value: value.as_int(),
            });
        }
        self.lookups.entry(table_id).or_default().push(value);
        Ok(())
    }

    pub fn tables(&self) -> &[LookupTable] {
        &self.tables
    }

    /// Returns the recorded lookups for `table_id`, or `None` if no
    /// table has been registered under that identifier.
    pub fn lookups_for(&self, table_id: TableId) -> Option<&[BaseElement]> {
        self.lookups.get(&table_id).map(Vec::as_slice)
    }

    /// Builds the per-table witness columns over a trace of `trace_len` rows.
    /// `alpha` is the Fiat-Shamir verifier challenge.
    pub fn build_columns<E>(
        &self,
        trace_len: usize,
        alpha: E,
    ) -> Result<Vec<LogUpColumns<E>>, LogUpError>
    where
        E: FieldElement<BaseField = BaseElement>,
    {
        self.tables
            .iter()
            .map(|table| build_table_columns(table, self.lookups_for(table.id()), trace_len, alpha))
            .collect()
    }
}

/// Builds the four LogUp witness columns for a single pinned table.
fn build_table_columns<E>(
    table: &LookupTable,
    lookups: Option<&[BaseElement]>,
    trace_len: usize,
    alpha: E,
) -> Result<LogUpColumns<E>, LogUpError>
where
    E: FieldElement<BaseField = BaseElement>,
{
    let lookups = lookups.unwrap_or(&[]);
    let table_size = table.entries.len();
    let lookup_count = lookups.len();
    let active = trace_len.saturating_sub(1);

    if table_size == 0 {
        if lookup_count > 0 {
            return Err(LogUpError::EmptyTableWithLookups(table.id, lookup_count));
        }
        return Ok(LogUpColumns {
            table_id: table.id,
            lookup_column: vec![BaseElement::ZERO; trace_len],
            table_column: vec![BaseElement::ZERO; trace_len],
            multiplicities: vec![E::ZERO; trace_len],
            grand_sum: vec![E::ZERO; trace_len],
        });
    }

    if active < table_size.max(lookup_count) {
        return Err(LogUpError::TraceTooSmall {
            trace_len,
            table_size,
            lookup_count,
        });
    }

    let pad = table.entries[0];
    let table_column: Vec<BaseElement> = std::iter::once(BaseElement::ZERO)
        .chain(table.entries.iter().copied())
        .chain(std::iter::repeat_n(pad, active - table_size))
        .collect();
    let lookup_column: Vec<BaseElement> = std::iter::once(BaseElement::ZERO)
        .chain(lookups.iter().copied())
        .chain(std::iter::repeat_n(pad, active - lookup_count))
        .collect();

    let mut multiplicity_counts = vec![0u64; table_size];
    for value in lookups {
        let pos = table
            .position(*value)
            .expect("register_lookup guards membership");
        multiplicity_counts[pos] += 1;
    }
    let padding_lookups = (active - lookup_count) as u64;
    multiplicity_counts[0] += padding_lookups;

    let mut multiplicities = vec![E::ZERO; trace_len];
    for (i, count) in multiplicity_counts.iter().enumerate() {
        multiplicities[1 + i] = E::from(BaseElement::new(*count));
    }

    let mut grand_sum = vec![E::ZERO; trace_len];
    for i in 1..trace_len {
        let f = E::from(lookup_column[i]);
        let t = E::from(table_column[i]);
        let m = multiplicities[i];
        let step = m * (alpha - t).inv() - (alpha - f).inv();
        grand_sum[i] = grand_sum[i - 1] + step;
    }

    Ok(LogUpColumns {
        table_id: table.id,
        lookup_column,
        table_column,
        multiplicities,
        grand_sum,
    })
}

/// Evaluates the LogUp cross-multiplied transition residual at one step.
///
/// The returned value is `LHS − RHS`; a satisfied transition yields
/// [`FieldElement::ZERO`]. The constraint degree is three in the trace
/// polynomials.
pub fn evaluate_transition_step<F, E>(
    s_curr: E,
    s_next: E,
    m_next: E,
    f_next: F,
    t_next: F,
    alpha: E,
) -> E
where
    F: FieldElement<BaseField = BaseElement>,
    E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
{
    let f = E::from(f_next);
    let t = E::from(t_next);
    let lhs = (s_next - s_curr) * (alpha - f) * (alpha - t);
    let rhs = m_next * (alpha - f) - (alpha - t);
    lhs - rhs
}

impl Builtin for LogUpBuiltin {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn aux_width(&self) -> usize {
        self.pools.iter().map(AirPool::aux_width).sum()
    }

    fn num_aux_rands(&self) -> usize {
        self.pools.iter().map(AirPool::num_aux_rands).sum()
    }

    fn num_aux_constraints(&self) -> usize {
        self.pools.iter().map(AirPool::num_aux_constraints).sum()
    }

    fn num_aux_assertions(&self) -> usize {
        self.pools.iter().map(AirPool::num_aux_assertions).sum()
    }

    fn aux_constraint_degrees(&self) -> Vec<usize> {
        self.pools
            .iter()
            .flat_map(AirPool::aux_constraint_degrees)
            .collect()
    }

    fn reserved_address_range(&self) -> (u64, u64) {
        Self::RESERVED_ADDRESS_RANGE
    }

    fn evaluate_aux_transition<F, E>(
        &self,
        _main_curr: &[F],
        _main_next: &[F],
        aux_curr: &[E],
        aux_next: &[E],
        base_offset: usize,
        rand_elements: &[E],
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = BaseElement>,
        E: FieldElement<BaseField = BaseElement> + ExtensionOf<F>,
    {
        let one = E::ONE;
        let mut rand_off = 0usize;
        let mut res_off = 0usize;

        for (idx, pool) in self.pools.iter().enumerate() {
            let pool_base = base_offset + self.pool_local_offset(idx);
            let local_curr = &aux_curr[pool_base..pool_base + pool.aux_width()];
            let local_next = &aux_next[pool_base..pool_base + pool.aux_width()];
            let alpha = rand_elements[rand_off];

            for (k, channel) in pool.channels.iter().enumerate() {
                let s_curr = local_curr[pool.s_base() + k];
                let s_next = local_next[pool.s_base() + k];
                let b_next = aux_next[channel.aux_column];
                result[res_off + k] = (s_next - s_curr) * (alpha - b_next) - one;
            }

            let m_idx = res_off + pool.channels.len();
            let sm_curr = local_curr[pool.sm_offset()];
            let sm_next = local_next[pool.sm_offset()];
            let t_next = local_next[pool.t_offset()];
            let m_next = local_next[pool.m_offset()];
            result[m_idx] = (sm_next - sm_curr) * (alpha - t_next) - m_next;

            let bal_idx = m_idx + 1;
            let bal_next = local_next[pool.bal_offset()];
            let sum_s_next = (0..pool.channels.len())
                .map(|k| local_next[pool.s_base() + k])
                .fold(E::ZERO, |acc, v| acc + v);
            result[bal_idx] = bal_next - (sum_s_next - sm_next);

            rand_off += pool.num_aux_rands();
            res_off += pool.num_aux_constraints();
        }
    }

    fn build_aux_columns<E: FieldElement<BaseField = BaseElement>>(
        &self,
        main_columns: &[&[BaseElement]],
        rand_elements: &[E],
    ) -> Vec<Vec<E>> {
        let mut cols: Vec<Vec<E>> = Vec::with_capacity(self.aux_width());
        let mut rand_off = 0usize;

        for pool in &self.pools {
            let n = main_columns[0].len();
            let alpha = rand_elements[rand_off];
            let active = n.saturating_sub(1);
            let table_size = pool.table_entries.len();
            let embedding_rows = table_size.min(active);

            let channel_values: Vec<Vec<u64>> = pool
                .channels
                .iter()
                .map(|ch| {
                    (0..n)
                        .map(|row| ch.source.extract(main_columns, row))
                        .collect()
                })
                .collect();

            let mut counts = vec![0u64; table_size];
            for channel in &channel_values {
                for &v in channel.iter().skip(1) {
                    if (v as usize) < table_size {
                        counts[v as usize] += 1;
                    }
                }
            }

            let mut t_col = vec![E::ZERO; n];
            let mut m_col = vec![E::ZERO; n];
            for v in 0..embedding_rows {
                t_col[1 + v] = E::from(pool.table_entries[v]);
                m_col[1 + v] = E::from(BaseElement::new(counts[v]));
            }

            let mut s_cols: Vec<Vec<E>> = pool.channels.iter().map(|_| vec![E::ZERO; n]).collect();
            for (k, channel_vals) in channel_values.iter().enumerate() {
                for i in 1..n {
                    let b = E::from(BaseElement::new(channel_vals[i]));
                    s_cols[k][i] = s_cols[k][i - 1] + (alpha - b).inv();
                }
            }

            let mut sm_col = vec![E::ZERO; n];
            for i in 1..n {
                sm_col[i] = sm_col[i - 1] + m_col[i] * (alpha - t_col[i]).inv();
            }

            let bal_col: Vec<E> = (0..n)
                .map(|i| {
                    let sum_s = (0..pool.channels.len())
                        .map(|k| s_cols[k][i])
                        .fold(E::ZERO, |acc, v| acc + v);
                    sum_s - sm_col[i]
                })
                .collect();

            cols.push(t_col);
            cols.push(m_col);
            cols.extend(s_cols);
            cols.push(sm_col);
            cols.push(bal_col);

            rand_off += pool.num_aux_rands();
        }

        cols
    }

    fn aux_assertions<E: FieldElement<BaseField = BaseElement>>(
        &self,
        column_base: usize,
        last_step: usize,
    ) -> Vec<Assertion<E>> {
        let mut out = Vec::with_capacity(self.num_aux_assertions());

        for (idx, pool) in self.pools.iter().enumerate() {
            let pool_base = column_base + self.pool_local_offset(idx);
            for k in 0..pool.channels.len() {
                out.push(Assertion::single(pool_base + pool.s_base() + k, 0, E::ZERO));
            }
            out.push(Assertion::single(pool_base + pool.sm_offset(), 0, E::ZERO));
            out.push(Assertion::single(
                pool_base + pool.bal_offset(),
                last_step,
                E::ZERO,
            ));
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::{BitwiseBuiltin, IdentityBuiltin, RangeCheckBuiltin};

    type F = BaseElement;

    fn alpha() -> F {
        F::new(7919)
    }

    fn felts(values: &[u64]) -> Vec<F> {
        values.iter().copied().map(F::new).collect()
    }

    fn assert_columns_satisfy_air(cols: &LogUpColumns<F>) {
        let n = cols.grand_sum.len();
        assert_eq!(cols.lookup_column.len(), n);
        assert_eq!(cols.table_column.len(), n);
        assert_eq!(cols.multiplicities.len(), n);

        assert_eq!(cols.grand_sum[0], F::ZERO, "s[0] boundary violated");
        assert_eq!(
            cols.grand_sum[n - 1],
            F::ZERO,
            "s[n-1] boundary violated -- lookup multiset disagrees with table"
        );

        let a = alpha();
        for i in 0..n - 1 {
            let residual = evaluate_transition_step::<F, F>(
                cols.grand_sum[i],
                cols.grand_sum[i + 1],
                cols.multiplicities[i + 1],
                cols.lookup_column[i + 1],
                cols.table_column[i + 1],
                a,
            );
            assert_eq!(
                residual,
                F::ZERO,
                "LogUp transition residual non-zero at step {i}",
            );
        }
    }

    #[test]
    fn round_trip_against_fixed_16_bit_table() {
        let mut engine = LogUpBuiltin::new();
        let table = (0..(1u64 << 16)).map(F::new).collect::<Vec<F>>();
        engine.register_table(0, table).unwrap();

        for value in [0u64, 1, 2, 255, 4096, 65_535, 32_768, 1] {
            engine.register_lookup(0, F::new(value)).unwrap();
        }

        let trace_len = (1usize << 16) + 1;
        let cols = engine
            .build_columns::<F>(trace_len, alpha())
            .unwrap()
            .pop()
            .expect("one table");

        assert_eq!(cols.table_id, 0);
        assert_columns_satisfy_air(&cols);
    }

    #[test]
    fn multi_table_queries_compose_independently() {
        let mut engine = LogUpBuiltin::new();
        engine.register_table(10, felts(&[1, 2, 3, 4])).unwrap();
        engine.register_table(20, felts(&[100, 200, 300])).unwrap();

        for v in [1u64, 2, 1, 3, 4, 2] {
            engine.register_lookup(10, F::new(v)).unwrap();
        }
        for v in [200u64, 300, 200, 100] {
            engine.register_lookup(20, F::new(v)).unwrap();
        }

        let trace_len = 16;
        let all = engine.build_columns::<F>(trace_len, alpha()).unwrap();
        assert_eq!(all.len(), 2);

        let by_id = all
            .iter()
            .map(|c| (c.table_id, c))
            .collect::<HashMap<TableId, &LogUpColumns<F>>>();
        assert_columns_satisfy_air(by_id[&10]);
        assert_columns_satisfy_air(by_id[&20]);
    }

    #[test]
    fn empty_table_with_zero_lookups_yields_trivial_columns() {
        let mut engine = LogUpBuiltin::new();
        engine.register_table(42, Vec::new()).unwrap();

        let trace_len = 8;
        let mut cols = engine.build_columns::<F>(trace_len, alpha()).unwrap();
        assert_eq!(cols.len(), 1);
        let cols = cols.pop().unwrap();

        assert!(cols.lookup_column.iter().all(|v| *v == F::ZERO));
        assert!(cols.table_column.iter().all(|v| *v == F::ZERO));
        assert!(cols.multiplicities.iter().all(|v| *v == F::ZERO));
        assert!(cols.grand_sum.iter().all(|v| *v == F::ZERO));
    }

    #[test]
    fn empty_table_with_lookups_is_rejected() {
        let mut engine = LogUpBuiltin::new();
        engine.register_table(7, Vec::new()).unwrap();
        let err = engine
            .register_lookup(7, F::new(1))
            .expect_err("lookup against empty table must fail at registration");
        assert!(matches!(err, LogUpError::ValueNotInTable { table: 7, .. }));
    }

    #[test]
    fn tampered_multiplicity_breaks_air() {
        let mut engine = LogUpBuiltin::new();
        engine.register_table(0, felts(&[10, 20, 30])).unwrap();
        for v in [10u64, 20, 10, 30, 20] {
            engine.register_lookup(0, F::new(v)).unwrap();
        }

        let trace_len = 8;
        let mut cols = engine
            .build_columns::<F>(trace_len, alpha())
            .unwrap()
            .pop()
            .unwrap();

        let tamper_row = 2;
        let original = cols.multiplicities[tamper_row];
        cols.multiplicities[tamper_row] = original + F::ONE;

        let a = alpha();
        let n = cols.grand_sum.len();
        let mut any_violation = false;
        for i in 0..n - 1 {
            let residual = evaluate_transition_step::<F, F>(
                cols.grand_sum[i],
                cols.grand_sum[i + 1],
                cols.multiplicities[i + 1],
                cols.lookup_column[i + 1],
                cols.table_column[i + 1],
                a,
            );
            if residual != F::ZERO {
                any_violation = true;
                break;
            }
        }
        assert!(
            any_violation,
            "tampering with the multiplicity column must violate at least one transition"
        );
    }

    #[test]
    fn duplicate_table_registration_is_rejected() {
        let mut engine = LogUpBuiltin::new();
        engine.register_table(5, felts(&[1, 2])).unwrap();
        let err = engine
            .register_table(5, felts(&[3, 4]))
            .expect_err("duplicate id");
        assert!(matches!(err, LogUpError::DuplicateTable(5)));
    }

    #[test]
    fn lookup_against_unknown_table_is_rejected() {
        let mut engine = LogUpBuiltin::new();
        let err = engine
            .register_lookup(99, F::new(1))
            .expect_err("unknown table id");
        assert!(matches!(err, LogUpError::UnknownTable(99)));
    }

    #[test]
    fn value_not_in_table_is_rejected_at_registration() {
        let mut engine = LogUpBuiltin::new();
        engine.register_table(0, felts(&[1, 2, 3])).unwrap();
        let err = engine
            .register_lookup(0, F::new(42))
            .expect_err("value not in table");
        assert!(matches!(
            err,
            LogUpError::ValueNotInTable {
                table: 0,
                value: 42
            }
        ));
    }

    #[test]
    fn trace_too_small_for_table_is_rejected() {
        let mut engine = LogUpBuiltin::new();
        engine.register_table(0, felts(&[1, 2, 3, 4])).unwrap();
        let err = engine
            .build_columns::<F>(3, alpha())
            .expect_err("trace too small");
        assert!(matches!(
            err,
            LogUpError::TraceTooSmall {
                trace_len: 3,
                table_size: 4,
                ..
            }
        ));
    }

    #[test]
    fn trace_too_small_for_lookups_is_rejected() {
        let mut engine = LogUpBuiltin::new();
        engine.register_table(0, felts(&[1, 2])).unwrap();
        for _ in 0..10 {
            engine.register_lookup(0, F::new(1)).unwrap();
        }
        let err = engine
            .build_columns::<F>(8, alpha())
            .expect_err("trace too small for lookup stream");
        assert!(matches!(
            err,
            LogUpError::TraceTooSmall {
                trace_len: 8,
                lookup_count: 10,
                ..
            }
        ));
    }

    #[test]
    fn no_op_builtin_impl_emits_zero_aux_state() {
        let builtin = LogUpBuiltin::new();
        assert_eq!(builtin.name(), LogUpBuiltin::NAME);
        assert_eq!(builtin.aux_width(), 0);
        assert_eq!(builtin.num_aux_rands(), 0);
        assert_eq!(builtin.num_aux_assertions(), 0);
        assert!(builtin.aux_constraint_degrees().is_empty());

        let main = (0..8).map(|_| vec![F::ZERO; 4]).collect::<Vec<Vec<F>>>();
        let main_slices = main.iter().map(Vec::as_slice).collect::<Vec<&[F]>>();
        assert!(builtin.build_aux_columns::<F>(&main_slices, &[]).is_empty());
        assert!(builtin.aux_assertions::<F>(0, 7).is_empty());

        let mut result: Vec<F> = Vec::new();
        builtin.evaluate_aux_transition::<F, F>(&[], &[], &[], &[], 0, &[], &mut result);
        assert!(result.is_empty());
    }

    #[test]
    fn reserved_address_range_is_disjoint_from_existing_builtins() {
        let ranges = [
            RangeCheckBuiltin::RESERVED_ADDRESS_RANGE,
            BitwiseBuiltin::RESERVED_ADDRESS_RANGE,
            IdentityBuiltin::RESERVED_ADDRESS_RANGE,
            LogUpBuiltin::RESERVED_ADDRESS_RANGE,
        ];
        for &(lo, hi) in &ranges {
            assert!(lo <= hi);
        }
        for i in 0..ranges.len() {
            for j in (i + 1)..ranges.len() {
                let (a_lo, a_hi) = ranges[i];
                let (b_lo, b_hi) = ranges[j];
                assert!(
                    a_hi < b_lo || b_hi < a_lo,
                    "reserved ranges {i} and {j} overlap"
                );
            }
        }
    }
}
