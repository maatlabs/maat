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

/// Describes how a channel's per-row value is derived from the trace.
#[derive(Clone, Copy, Debug)]
pub enum ChannelSource {
    /// High byte of a main-trace column (`(col >> 8) & 0xff`).
    HighByte(usize),
    /// Low byte of a main-trace column (`col & 0xff`).
    LowByte(usize),
    /// Pow2-paired key: `(s0, 2^s0)` where `s0` lives in `main[main_col]`
    /// and `2^s0` lives in `aux[aux_col]` as a witness. Compressed by
    /// the pool's per-pool challenge.
    Pow2Paired { main_col: usize, aux_col: usize },
    /// Diluted-paired key: `(v, dilute(v))` where `v` (an 8-bit chunk
    /// byte) lives in `main[main_col]` and `dilute(v)` (the spread-bit
    /// 29-bit form) lives in `aux[aux_col]` as a witness. Compressed by
    /// the pool's per-pool challenge. Used by AND/OR/XOR chunked rows.
    DilutedPaired { main_col: usize, aux_col: usize },
}

impl ChannelSource {
    pub fn is_paired(&self) -> bool {
        matches!(
            self,
            ChannelSource::Pow2Paired { .. } | ChannelSource::DilutedPaired { .. }
        )
    }
}

/// Describes the pinned lookup table associated with an [`AirPool`].
///
/// `Fixed` carries precomputed entries (used by the 8-bit byte table).
/// `Pow2Paired` derives entries from the pool's per-pool compression
/// challenge `delta` at witness-build time: `t[k] = delta * k + 2^k`.
/// `DilutedPaired` derives entries from the per-pool challenge `gamma`:
/// `t[v] = gamma * v + dilute(v)` for `v = 0..num_entries`.
#[derive(Clone, Debug)]
pub enum TableSpec {
    Fixed(Vec<BaseElement>),
    Pow2Paired { num_entries: usize },
    DilutedPaired { num_entries: usize },
}

impl TableSpec {
    pub fn len(&self) -> usize {
        match self {
            TableSpec::Fixed(v) => v.len(),
            TableSpec::Pow2Paired { num_entries } => *num_entries,
            TableSpec::DilutedPaired { num_entries } => *num_entries,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_paired(&self) -> bool {
        matches!(
            self,
            TableSpec::Pow2Paired { .. } | TableSpec::DilutedPaired { .. }
        )
    }

    pub fn entries<E>(&self, rand_elements: &[E]) -> Vec<E>
    where
        E: FieldElement<BaseField = BaseElement>,
    {
        match self {
            TableSpec::Fixed(v) => v.iter().copied().map(E::from).collect(),
            TableSpec::Pow2Paired { num_entries } => {
                let delta = rand_elements[1];
                (0..*num_entries)
                    .map(|k| {
                        let k_e = E::from(BaseElement::new(k as u64));
                        let pow = E::from(BaseElement::new(1u64 << k));
                        delta * k_e + pow
                    })
                    .collect()
            }
            TableSpec::DilutedPaired { num_entries } => {
                let gamma = rand_elements[1];
                (0..*num_entries)
                    .map(|v| {
                        let v_e = E::from(BaseElement::new(v as u64));
                        let d = E::from(BaseElement::new(super::diluted::dilute(v as u64)));
                        gamma * v_e + d
                    })
                    .collect()
            }
        }
    }
}

/// One LogUp channel: the consumer-side witness column (`aux_column`)
/// plus a recipe for recomputing the same value from the main trace.
#[derive(Clone, Debug)]
pub struct Channel {
    pub source: ChannelSource,
    pub aux_column: usize,
    pub gate_main_cols: Vec<usize>,
}

impl Channel {
    /// Returns true if this channel is unconditionally active.
    pub fn is_ungated(&self) -> bool {
        self.gate_main_cols.is_empty()
    }
}

/// One AIR-active LogUp pool: a pinned lookup table plus the channels
/// that consume from it.
#[derive(Clone, Debug)]
pub struct AirPool {
    pub table_id: TableId,
    pub spec: TableSpec,
    pub channels: Vec<Channel>,
}

impl AirPool {
    pub fn aux_width(&self) -> usize {
        4 + self.channels.len()
    }

    pub fn num_aux_rands(&self) -> usize {
        if self.spec.is_paired() { 2 } else { 1 }
    }

    pub fn num_aux_constraints(&self) -> usize {
        self.channels.len() + 2
    }

    pub fn num_aux_assertions(&self) -> usize {
        self.channels.len() + 2
    }

    pub fn aux_constraint_degrees(&self) -> Vec<usize> {
        let mut out: Vec<usize> = self
            .channels
            .iter()
            .map(|ch| if ch.is_ungated() { 2 } else { 3 })
            .collect();
        out.push(2); // m-side transition
        out.push(1); // balance binding
        out
    }

    pub fn min_trace_len(&self) -> usize {
        self.spec.len() + 1
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

fn compute_channel_values<E>(
    channel: &Channel,
    main_columns: &[&[BaseElement]],
    pool_rands: &[E],
    trace_len: usize,
) -> Vec<E>
where
    E: FieldElement<BaseField = BaseElement>,
{
    let delta = if pool_rands.len() > 1 {
        pool_rands[1]
    } else {
        E::ZERO
    };
    (0..trace_len)
        .map(|row| match channel.source {
            ChannelSource::HighByte(col) => {
                let byte = (main_columns[col][row].as_int() >> 8) & 0xff;
                E::from(BaseElement::new(byte))
            }
            ChannelSource::LowByte(col) => {
                let byte = main_columns[col][row].as_int() & 0xff;
                E::from(BaseElement::new(byte))
            }
            ChannelSource::Pow2Paired { main_col, .. } => {
                let s0 = main_columns[main_col][row].as_int();
                let pow_k = if s0 < 64 { 1u64 << s0 } else { 0 };
                delta * E::from(BaseElement::new(s0)) + E::from(BaseElement::new(pow_k))
            }
            ChannelSource::DilutedPaired { main_col, .. } => {
                let v = main_columns[main_col][row].as_int();
                let d = if v < (super::diluted::POOL_SIZE as u64) {
                    super::diluted::dilute(v)
                } else {
                    0
                };
                delta * E::from(BaseElement::new(v)) + E::from(BaseElement::new(d))
            }
        })
        .collect()
}

/// Computes a per-row mask indicating whether a gated channel is active
/// on each row. Ungated channels are always active.
fn compute_gate_activity(
    channel: &Channel,
    main_columns: &[&[BaseElement]],
    trace_len: usize,
) -> Vec<bool> {
    if channel.is_ungated() {
        return vec![true; trace_len];
    }
    (0..trace_len)
        .map(|row| {
            channel
                .gate_main_cols
                .iter()
                .any(|&col| main_columns[col][row] != BaseElement::ZERO)
        })
        .collect()
}

fn position_in_fixed_table<E>(spec: &TableSpec, value: E) -> Option<usize>
where
    E: FieldElement<BaseField = BaseElement>,
{
    let TableSpec::Fixed(entries) = spec else {
        return None;
    };
    entries.iter().position(|e| E::from(*e) == value)
}

fn position_in_paired_table<E>(table_entries: &[E], value: E) -> Option<usize>
where
    E: FieldElement<BaseField = BaseElement>,
{
    table_entries.iter().position(|e| *e == value)
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
        main_next: &[F],
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
            let delta = if pool.spec.is_paired() {
                rand_elements[rand_off + 1]
            } else {
                E::ZERO
            };

            for (k, channel) in pool.channels.iter().enumerate() {
                let s_curr = local_curr[pool.s_base() + k];
                let s_next = local_next[pool.s_base() + k];
                let b_next = match channel.source {
                    ChannelSource::HighByte(_) | ChannelSource::LowByte(_) => {
                        aux_next[channel.aux_column]
                    }
                    ChannelSource::Pow2Paired { main_col, aux_col }
                    | ChannelSource::DilutedPaired { main_col, aux_col } => {
                        delta * E::from(main_next[main_col]) + aux_next[aux_col]
                    }
                };
                let s_delta = s_next - s_curr;
                if channel.is_ungated() {
                    result[res_off + k] = s_delta * (alpha - b_next) - one;
                } else {
                    let gate = channel
                        .gate_main_cols
                        .iter()
                        .fold(E::ZERO, |acc, &col| acc + E::from(main_next[col]));
                    result[res_off + k] =
                        gate * (s_delta * (alpha - b_next) - one) + (one - gate) * s_delta;
                }
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
            let active = n.saturating_sub(1);
            let table_size = pool.spec.len();
            let embedding_rows = table_size.min(active);
            let pool_rands = &rand_elements[rand_off..rand_off + pool.num_aux_rands()];
            let alpha = pool_rands[0];
            let table_entries: Vec<E> = pool.spec.entries(pool_rands);

            let channel_values = pool
                .channels
                .iter()
                .map(|ch| compute_channel_values(ch, main_columns, pool_rands, n))
                .collect::<Vec<Vec<E>>>();

            let gate_active = pool
                .channels
                .iter()
                .map(|ch| compute_gate_activity(ch, main_columns, n))
                .collect::<Vec<Vec<bool>>>();

            let mut counts = vec![0u64; table_size];
            for (k, channel) in channel_values.iter().enumerate() {
                for (i, v) in channel.iter().enumerate().skip(1) {
                    if !gate_active[k][i] {
                        continue;
                    }
                    let idx = if pool.spec.is_paired() {
                        position_in_paired_table(&table_entries, *v)
                    } else {
                        position_in_fixed_table(&pool.spec, *v)
                    };
                    if let Some(idx) = idx {
                        counts[idx] += 1;
                    }
                }
            }

            let mut t_col = vec![E::ZERO; n];
            let mut m_col = vec![E::ZERO; n];
            for v in 0..embedding_rows {
                t_col[1 + v] = table_entries[v];
                m_col[1 + v] = E::from(BaseElement::new(counts[v]));
            }

            let mut s_cols = pool
                .channels
                .iter()
                .map(|_| vec![E::ZERO; n])
                .collect::<Vec<Vec<E>>>();
            for (k, channel_vals) in channel_values.iter().enumerate() {
                for i in 1..n {
                    s_cols[k][i] = if gate_active[k][i] {
                        let b = channel_vals[i];
                        s_cols[k][i - 1] + (alpha - b).inv()
                    } else {
                        s_cols[k][i - 1]
                    };
                }
            }

            let mut sm_col = vec![E::ZERO; n];
            for i in 1..n {
                sm_col[i] = sm_col[i - 1] + m_col[i] * (alpha - t_col[i]).inv();
            }

            let bal_col = (0..n)
                .map(|i| {
                    let sum_s = (0..pool.channels.len())
                        .map(|k| s_cols[k][i])
                        .fold(E::ZERO, |acc, v| acc + v);
                    sum_s - sm_col[i]
                })
                .collect::<Vec<E>>();

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
    use crate::builtin::{BitwiseBuiltin, RangeCheckBuiltin};

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

    fn run_air_pool(
        pool: AirPool,
        main_columns: Vec<Vec<F>>,
        rand_elements: &[F],
        s1_witness: Option<Vec<u64>>,
    ) -> Vec<Vec<F>> {
        let n = main_columns[0].len();
        let pool_width = pool.aux_width();
        let extra_aux_cols = pool
            .channels
            .iter()
            .filter(|ch| ch.source.is_paired())
            .count();
        let total_aux = pool_width + extra_aux_cols;

        let main_slices = main_columns
            .iter()
            .map(|c| c.as_slice())
            .collect::<Vec<&[F]>>();

        let logup = LogUpBuiltin::with_pools(vec![pool.clone()]);
        let pool_cols = logup.build_aux_columns::<F>(&main_slices, rand_elements);

        let mut aux: Vec<Vec<F>> = pool_cols;
        if let Some(witness) = s1_witness {
            assert_eq!(witness.len(), n);
            let witness_col = witness.into_iter().map(F::new).collect::<Vec<F>>();
            aux.push(witness_col);
        }
        assert_eq!(aux.len(), total_aux);

        // Sanity-check the AIR at every row transition.
        for i in 0..n - 1 {
            let main_curr = main_columns.iter().map(|c| c[i]).collect::<Vec<F>>();
            let main_next = main_columns.iter().map(|c| c[i + 1]).collect::<Vec<F>>();
            let aux_curr = aux.iter().map(|c| c[i]).collect::<Vec<F>>();
            let aux_next = aux.iter().map(|c| c[i + 1]).collect::<Vec<F>>();
            let mut result = vec![F::ZERO; logup.num_aux_constraints()];
            logup.evaluate_aux_transition::<F, F>(
                &main_curr,
                &main_next,
                &aux_curr,
                &aux_next,
                0,
                rand_elements,
                &mut result,
            );
            for (j, &r) in result.iter().enumerate() {
                assert_eq!(r, F::ZERO, "paired-pool constraint {j} violated at row {i}");
            }
        }

        aux
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
    fn pow2_paired_table_entries_match_delta_times_k_plus_two_pow_k() {
        let spec = TableSpec::Pow2Paired { num_entries: 8 };
        let alpha_v = F::new(7919);
        let delta_v = F::new(31);
        let entries = spec.entries::<F>(&[alpha_v, delta_v]);
        assert_eq!(entries.len(), 8);
        for (k, &entry) in entries.iter().enumerate() {
            let expected = delta_v * F::new(k as u64) + F::new(1u64 << k);
            assert_eq!(entry, expected, "pow2 paired entry {k} mismatch");
        }
    }

    #[test]
    fn diluted_paired_table_entries_match_gamma_times_v_plus_dilute_v() {
        use crate::builtin::diluted::dilute;

        let spec = TableSpec::DilutedPaired { num_entries: 16 };
        let alpha_v = F::new(7919);
        let gamma_v = F::new(37);
        let entries = spec.entries::<F>(&[alpha_v, gamma_v]);
        assert_eq!(entries.len(), 16);
        for (v, &entry) in entries.iter().enumerate() {
            let expected = gamma_v * F::new(v as u64) + F::new(dilute(v as u64));
            assert_eq!(entry, expected, "diluted paired entry {v} mismatch");
        }
    }

    #[test]
    fn diluted_paired_pool_with_no_channels_closes_balance() {
        use maat_trace::table::TRACE_WIDTH;

        let pool = AirPool {
            table_id: 97,
            spec: TableSpec::DilutedPaired { num_entries: 256 },
            channels: Vec::new(),
        };
        let n = 512usize;
        let main = (0..TRACE_WIDTH)
            .map(|_| vec![F::ZERO; n])
            .collect::<Vec<Vec<F>>>();
        let rands = vec![F::new(7919), F::new(37)];

        let aux = run_air_pool(pool, main, &rands, None);
        assert_eq!(aux.len(), 4);
        assert_eq!(
            aux[3][n - 1],
            F::ZERO,
            "no-channel diluted paired pool balance must close to zero",
        );
    }

    #[test]
    fn diluted_paired_pool_with_synthetic_channel_closes_balance() {
        use maat_trace::table::{COL_S0, TRACE_WIDTH};

        use crate::builtin::diluted::dilute;

        let pool_width = 5usize;
        let pool = AirPool {
            table_id: 97,
            spec: TableSpec::DilutedPaired { num_entries: 256 },
            channels: vec![Channel {
                source: ChannelSource::DilutedPaired {
                    main_col: COL_S0,
                    aux_col: pool_width,
                },
                aux_column: pool_width,
                gate_main_cols: Vec::new(),
            }],
        };

        let n = 512usize;
        let mut main = (0..TRACE_WIDTH)
            .map(|_| vec![F::ZERO; n])
            .collect::<Vec<Vec<F>>>();
        for (i, cell) in main[COL_S0].iter_mut().enumerate().take(n).skip(1) {
            *cell = F::new(((i - 1) % 256) as u64);
        }

        let mut d_witness = vec![0u64; n];
        for (i, cell) in d_witness.iter_mut().enumerate().take(n).skip(1) {
            *cell = dilute(main[COL_S0][i].as_int());
        }

        let rands = vec![F::new(7919), F::new(37)];
        let aux = run_air_pool(pool, main, &rands, Some(d_witness));
        // Pool aux_width = 4 + 1 channel = 5; plus the paired witness col = 6.
        assert_eq!(aux.len(), 6);
        let bal_col_idx = 4;
        assert_eq!(
            aux[bal_col_idx][n - 1],
            F::ZERO,
            "diluted-channel balance must close to zero on correct witness",
        );
    }

    #[test]
    fn diluted_paired_pool_tampered_aux_breaks_channel_transition() {
        use maat_trace::table::{COL_S0, TRACE_WIDTH};

        use crate::builtin::diluted::dilute;

        let pool_width = 5usize;
        let pool = AirPool {
            table_id: 97,
            spec: TableSpec::DilutedPaired { num_entries: 256 },
            channels: vec![Channel {
                source: ChannelSource::DilutedPaired {
                    main_col: COL_S0,
                    aux_col: pool_width,
                },
                aux_column: pool_width,
                gate_main_cols: Vec::new(),
            }],
        };

        let n = 512usize;
        let mut main = (0..TRACE_WIDTH)
            .map(|_| vec![F::ZERO; n])
            .collect::<Vec<Vec<F>>>();
        for (i, cell) in main[COL_S0].iter_mut().enumerate().take(n).skip(1) {
            *cell = F::new(((i - 1) % 256) as u64);
        }
        let mut d_witness = vec![0u64; n];
        for (i, cell) in d_witness.iter_mut().enumerate().take(n).skip(1) {
            *cell = dilute(main[COL_S0][i].as_int());
        }
        let tamper_row = 7usize;
        d_witness[tamper_row] = d_witness[tamper_row].wrapping_add(1);

        let rands = vec![F::new(7919), F::new(37)];
        let logup = LogUpBuiltin::with_pools(vec![pool.clone()]);
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();
        let pool_cols = logup.build_aux_columns::<F>(&main_slices, &rands);
        let mut aux: Vec<Vec<F>> = pool_cols;
        aux.push(d_witness.into_iter().map(F::new).collect());

        let main_curr = main.iter().map(|c| c[tamper_row - 1]).collect::<Vec<F>>();
        let main_next = main.iter().map(|c| c[tamper_row]).collect::<Vec<F>>();
        let aux_curr = aux.iter().map(|c| c[tamper_row - 1]).collect::<Vec<F>>();
        let aux_next = aux.iter().map(|c| c[tamper_row]).collect::<Vec<F>>();
        let mut result = vec![F::ZERO; logup.num_aux_constraints()];
        logup.evaluate_aux_transition::<F, F>(
            &main_curr,
            &main_next,
            &aux_curr,
            &aux_next,
            0,
            &rands,
            &mut result,
        );
        assert_ne!(
            result[0],
            F::ZERO,
            "tampered diluted aux witness must fire the channel transition at the tampered row",
        );
    }

    #[test]
    fn paired_pool_with_no_channels_closes_balance() {
        use maat_trace::table::TRACE_WIDTH;

        let pool = AirPool {
            table_id: 99,
            spec: TableSpec::Pow2Paired { num_entries: 64 },
            channels: Vec::new(),
        };
        let n = 128usize;
        let main = (0..TRACE_WIDTH)
            .map(|_| vec![F::ZERO; n])
            .collect::<Vec<Vec<F>>>();
        let rands = vec![F::new(7919), F::new(31)];

        let aux = run_air_pool(pool, main, &rands, None);
        // Width = 4 (t, m, sm, bal) with zero channels.
        assert_eq!(aux.len(), 4);
        assert_eq!(
            aux[3][n - 1],
            F::ZERO,
            "no-channel paired pool balance must close to zero",
        );
    }

    #[test]
    fn paired_pool_with_synthetic_pow2_channel_closes_balance() {
        use maat_trace::table::{COL_S0, TRACE_WIDTH};

        let pool_with_no_aux_yet = AirPool {
            table_id: 99,
            spec: TableSpec::Pow2Paired { num_entries: 64 },
            channels: vec![Channel {
                source: ChannelSource::Pow2Paired {
                    main_col: COL_S0,
                    aux_col: 0,
                },
                aux_column: 0,
                gate_main_cols: Vec::new(),
            }],
        };
        let pool_width = pool_with_no_aux_yet.aux_width();
        let pool = AirPool {
            table_id: 99,
            spec: TableSpec::Pow2Paired { num_entries: 64 },
            channels: vec![Channel {
                source: ChannelSource::Pow2Paired {
                    main_col: COL_S0,
                    aux_col: pool_width,
                },
                aux_column: pool_width,
                gate_main_cols: Vec::new(),
            }],
        };

        let n = 128usize;
        let mut main = (0..TRACE_WIDTH)
            .map(|_| vec![F::ZERO; n])
            .collect::<Vec<Vec<F>>>();
        for (i, cell) in main[COL_S0].iter_mut().enumerate().take(n).skip(1) {
            *cell = F::new(((i - 1) % 64) as u64);
        }

        let mut pow_k_witness = vec![0u64; n];
        for (i, cell) in pow_k_witness.iter_mut().enumerate().take(n).skip(1) {
            *cell = 1u64 << main[COL_S0][i].as_int();
        }

        let rands = vec![F::new(7919), F::new(31)];
        let aux = run_air_pool(pool, main, &rands, Some(pow_k_witness));

        assert_eq!(aux.len(), 6);
        let bal_col_idx = 4; // sm at offset 3, bal at offset 4 within pool slice
        assert_eq!(
            aux[bal_col_idx][n - 1],
            F::ZERO,
            "paired-channel balance must close to zero on correct witness",
        );
    }

    #[test]
    fn paired_pool_tampered_aux_witness_breaks_channel_transition() {
        use maat_trace::table::{COL_S0, TRACE_WIDTH};

        let pool_width = 5usize;
        let pool = AirPool {
            table_id: 99,
            spec: TableSpec::Pow2Paired { num_entries: 64 },
            channels: vec![Channel {
                source: ChannelSource::Pow2Paired {
                    main_col: COL_S0,
                    aux_col: pool_width,
                },
                aux_column: pool_width,
                gate_main_cols: Vec::new(),
            }],
        };

        let n = 128usize;
        let mut main = (0..TRACE_WIDTH)
            .map(|_| vec![F::ZERO; n])
            .collect::<Vec<Vec<F>>>();
        for (i, cell) in main[COL_S0].iter_mut().enumerate().take(n).skip(1) {
            *cell = F::new(((i - 1) % 64) as u64);
        }

        let mut pow_k_witness = vec![0u64; n];
        for (i, cell) in pow_k_witness.iter_mut().enumerate().take(n).skip(1) {
            *cell = 1u64 << main[COL_S0][i].as_int();
        }
        let tamper_row = 5usize;
        pow_k_witness[tamper_row] = pow_k_witness[tamper_row].wrapping_add(1);

        let rands = vec![F::new(7919), F::new(31)];
        let logup = LogUpBuiltin::with_pools(vec![pool.clone()]);
        let main_slices = main.iter().map(|c| c.as_slice()).collect::<Vec<&[F]>>();
        let pool_cols = logup.build_aux_columns::<F>(&main_slices, &rands);
        let mut aux: Vec<Vec<F>> = pool_cols;
        aux.push(pow_k_witness.into_iter().map(F::new).collect());

        let main_curr = main.iter().map(|c| c[tamper_row - 1]).collect::<Vec<F>>();
        let main_next = main.iter().map(|c| c[tamper_row]).collect::<Vec<F>>();
        let aux_curr = aux.iter().map(|c| c[tamper_row - 1]).collect::<Vec<F>>();
        let aux_next = aux.iter().map(|c| c[tamper_row]).collect::<Vec<F>>();
        let mut result = vec![F::ZERO; logup.num_aux_constraints()];
        logup.evaluate_aux_transition::<F, F>(
            &main_curr,
            &main_next,
            &aux_curr,
            &aux_next,
            0,
            &rands,
            &mut result,
        );
        assert_ne!(
            result[0],
            F::ZERO,
            "tampered paired aux witness must fire the channel transition at the tampered row",
        );
    }

    #[test]
    fn reserved_address_range_is_disjoint_from_existing_builtins() {
        let ranges = [
            RangeCheckBuiltin::RESERVED_ADDRESS_RANGE,
            BitwiseBuiltin::RESERVED_ADDRESS_RANGE,
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
