//! Utilities for trace relocation and
//! appending/filling semantics for memory holes and dummy rows.

use maat_errors::{MemoryError, VmError};
use maat_field::{Felt, FieldElement};
use maat_runtime::{MemorySegmentManager, Relocatable};

use crate::recorder::RowRelocPlan;
use crate::selector::SEL_NOP;
use crate::table::{
    COL_FP, COL_IS_READ, COL_MEM_ADDR, COL_MEM_VAL, COL_OUT, COL_PC, COL_S0, COL_S1, COL_S2,
    COL_SEL_BASE, COL_SP, TRACE_WIDTH, TraceRow, TraceTable,
};

type Result<T> = std::result::Result<T, MemoryError>;

/// Relocation table mapping logical segment IDs to flat base addresses.
#[derive(Debug, Clone)]
pub struct Relocator {
    table: Vec<u32>,
}

impl Relocator {
    pub fn new(segment_sizes: &[u32], base: u32) -> Result<Self> {
        let mut table = Vec::with_capacity(segment_sizes.len());
        let mut cursor = base;
        for &size in segment_sizes {
            table.push(cursor);
            cursor = cursor
                .checked_add(size)
                .ok_or(MemoryError::RelocationOverflow)?;
        }
        Ok(Self { table })
    }

    /// Resolves a logical [`Relocatable`] to its flat field element address.
    pub fn flatten(&self, addr: Relocatable) -> Result<Felt> {
        let base = self
            .table
            .get(addr.segment_index as usize)
            .copied()
            .ok_or(MemoryError::SegmentNotFound(addr.segment_index))?;
        let flat = base
            .checked_add(addr.offset)
            .ok_or(MemoryError::RelocationOverflow)?;
        Ok(Felt::new(u64::from(flat)))
    }

    pub fn table(&self) -> &[u32] {
        &self.table
    }
}

/// Rewrites every relocatable-bearing trace cell to its flat form, using the
/// per-row plans the recorder produced in lock-step with the trace rows.
pub fn relocate_trace(
    trace: &mut TraceTable,
    plans: &[RowRelocPlan],
    relocator: &Relocator,
) -> maat_errors::Result<()> {
    if plans.len() != trace.num_rows() {
        return Err(VmError::new(format!(
            "relocation plans length mismatch: trace has {} rows, plans have {}",
            trace.num_rows(),
            plans.len(),
        ))
        .into());
    }
    for (row_idx, plan) in plans.iter().enumerate() {
        let row = trace.row_mut(row_idx);
        for (col, slot) in [
            (COL_MEM_ADDR, plan.mem_addr),
            (COL_MEM_VAL, plan.mem_val),
            (COL_S0, plan.s0),
            (COL_S1, plan.s1),
            (COL_S2, plan.s2),
            (COL_OUT, plan.out),
        ] {
            if let Some(r) = slot {
                row[col] = relocator.flatten(r).map_err(|e| {
                    VmError::new(format!(
                        "relocation failed for row {row_idx} col {col}: {e}"
                    ))
                })?;
            }
        }
    }
    Ok(())
}

/// Appends a dummy-read row for every unaccessed offset in every user
/// segment. Returns the number of hole rows injected.
pub fn fill_memory_holes(
    trace: &mut TraceTable,
    segments: &MemorySegmentManager,
    relocator: &Relocator,
) -> maat_errors::Result<usize> {
    let holes = segments
        .holes()
        .map_err(|e| VmError::new(format!("hole computation failed: {e}")))?;
    let total = holes.iter().map(Vec::len).sum::<usize>();
    if total == 0 {
        return Ok(0);
    }
    let template = base_template(trace).ok_or_else(|| {
        VmError::new("cannot fill memory holes: trace has no rows to inherit from")
    })?;
    for (seg_idx, hole_offsets) in holes.iter().enumerate() {
        let seg_id = u32::try_from(seg_idx)
            .map_err(|_| VmError::new("hole filler: segment index exceeds u32"))?;
        for &offset in hole_offsets {
            let flat = relocator
                .flatten(Relocatable::new(seg_id, offset))
                .map_err(|e| VmError::new(format!("hole filler relocation failed: {e}")))?;
            trace.push_row(make_hole_row(&template, flat));
        }
    }
    Ok(total)
}

/// Appends `count` `(0, 0)` public-memory dummy rows.
/// No-op when `count` is zero.
pub fn append_pubmem_dummies(trace: &mut TraceTable, count: usize) -> maat_errors::Result<()> {
    if count == 0 {
        return Ok(());
    }
    let template = base_template(trace).ok_or_else(|| {
        VmError::new("cannot append public-memory dummies: trace has no rows to inherit from")
    })?;
    for _ in 0..count {
        trace.push_row(make_dummy_row(&template));
    }
    Ok(())
}

/// Snapshot of the prior row's CPU state propagated into every hole/dummy row.
#[derive(Clone, Copy)]
struct BaseState {
    pc: Felt,
    sp: Felt,
    fp: Felt,
    out: Felt,
}

fn base_template(trace: &TraceTable) -> Option<BaseState> {
    let last_idx = trace.num_rows().checked_sub(1)?;
    let last = trace.row(last_idx);
    Some(BaseState {
        pc: last[COL_PC],
        sp: last[COL_SP],
        fp: last[COL_FP],
        out: last[COL_OUT],
    })
}

fn make_hole_row(template: &BaseState, flat_addr: Felt) -> TraceRow {
    let mut row = [Felt::ZERO; TRACE_WIDTH];
    row[COL_PC] = template.pc;
    row[COL_SP] = template.sp;
    row[COL_FP] = template.fp;
    row[COL_OUT] = template.out;
    row[COL_MEM_ADDR] = flat_addr;
    row[COL_MEM_VAL] = Felt::ZERO;
    row[COL_IS_READ] = Felt::ONE;
    row[COL_SEL_BASE + SEL_NOP] = Felt::ONE;
    row
}

fn make_dummy_row(template: &BaseState) -> TraceRow {
    let mut row = [Felt::ZERO; TRACE_WIDTH];
    row[COL_PC] = template.pc;
    row[COL_SP] = template.sp;
    row[COL_FP] = template.fp;
    row[COL_OUT] = template.out;
    row[COL_MEM_ADDR] = Felt::ZERO;
    row[COL_MEM_VAL] = Felt::ZERO;
    row[COL_IS_READ] = Felt::ONE;
    row[COL_SEL_BASE + SEL_NOP] = Felt::ONE;
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_trace_with_one_row() -> TraceTable {
        let mut trace = TraceTable::new();
        let mut row = [Felt::ZERO; TRACE_WIDTH];
        row[COL_PC] = Felt::new(42);
        row[COL_SP] = Felt::new(7);
        row[COL_FP] = Felt::new(3);
        row[COL_OUT] = Felt::new(11);
        row[COL_SEL_BASE + SEL_NOP] = Felt::ONE;
        trace.push_row(row);
        trace
    }

    fn seed_trace() -> TraceTable {
        let mut trace = TraceTable::new();
        let mut row = [Felt::ZERO; TRACE_WIDTH];
        row[COL_PC] = Felt::new(11);
        row[COL_SP] = Felt::new(5);
        row[COL_FP] = Felt::new(3);
        row[COL_OUT] = Felt::new(99);
        row[COL_MEM_ADDR] = Felt::new(17);
        row[COL_MEM_VAL] = Felt::new(42);
        row[COL_IS_READ] = Felt::ONE;
        row[COL_SEL_BASE + SEL_NOP] = Felt::ONE;
        trace.push_row(row);
        trace
    }

    #[test]
    fn relocator_lays_segments_back_to_back() {
        let r = Relocator::new(&[3, 5, 2], 1).unwrap();
        assert_eq!(r.table(), &[1, 4, 9]);
    }

    #[test]
    fn relocator_supports_custom_base() {
        let r = Relocator::new(&[3, 5, 2], 100).unwrap();
        assert_eq!(r.table(), &[100, 103, 108]);
    }

    #[test]
    fn relocator_handles_empty_segments() {
        let r = Relocator::new(&[0, 4, 0], 10).unwrap();
        assert_eq!(r.table(), &[10, 10, 14]);
    }

    #[test]
    fn relocator_detects_overflow() {
        let err = Relocator::new(&[u32::MAX, 2], 0).unwrap_err();
        assert_eq!(err, MemoryError::RelocationOverflow);
    }

    #[test]
    fn flatten_resolves_segment_offset() {
        let r = Relocator::new(&[3, 5, 2], 1).unwrap();
        assert_eq!(r.flatten(Relocatable::new(1, 2)).unwrap(), Felt::new(4 + 2),);
        assert_eq!(r.flatten(Relocatable::new(2, 0)).unwrap(), Felt::new(9),);
    }

    #[test]
    fn flatten_rejects_unknown_segment() {
        let r = Relocator::new(&[3, 5], 1).unwrap();
        let err = r.flatten(Relocatable::new(7, 0)).unwrap_err();
        assert_eq!(err, MemoryError::SegmentNotFound(7));
    }

    #[test]
    fn flatten_detects_offset_overflow() {
        let r = Relocator::new(&[2], u32::MAX - 5).unwrap();
        let err = r.flatten(Relocatable::new(0, 10)).unwrap_err();
        assert_eq!(err, MemoryError::RelocationOverflow);
    }

    #[test]
    fn no_segments_no_holes() {
        let mut trace = seed_trace_with_one_row();
        let segments = MemorySegmentManager::new();
        let relocator = Relocator::new(&[], 1).unwrap();
        let added = fill_memory_holes(&mut trace, &segments, &relocator).unwrap();
        assert_eq!(added, 0);
        assert_eq!(trace.num_rows(), 1);
    }

    #[test]
    fn dense_segment_produces_no_holes() {
        let mut trace = seed_trace_with_one_row();
        let mut segments = MemorySegmentManager::new();
        let base = segments.add().unwrap();
        segments
            .write(base, maat_runtime::MaybeRelocatable::Felt(Felt::new(1)))
            .unwrap();
        segments
            .write(
                Relocatable::new(base.segment_index, 1),
                maat_runtime::MaybeRelocatable::Felt(Felt::new(2)),
            )
            .unwrap();
        let relocator = Relocator::new(&[2], 100).unwrap();
        let added = fill_memory_holes(&mut trace, &segments, &relocator).unwrap();
        assert_eq!(added, 0);
        assert_eq!(trace.num_rows(), 1);
    }

    #[test]
    fn fills_intra_segment_holes_with_inherited_state() {
        let mut trace = seed_trace_with_one_row();
        let mut segments = MemorySegmentManager::new();
        let base = segments.add().unwrap();
        segments
            .write(base, maat_runtime::MaybeRelocatable::Felt(Felt::new(10)))
            .unwrap();
        segments
            .write(
                Relocatable::new(base.segment_index, 3),
                maat_runtime::MaybeRelocatable::Felt(Felt::new(20)),
            )
            .unwrap();
        let relocator = Relocator::new(&[4], 100).unwrap();
        let added = fill_memory_holes(&mut trace, &segments, &relocator).unwrap();
        assert_eq!(added, 2);
        assert_eq!(trace.num_rows(), 3);

        let row1 = trace.row(1);
        assert_eq!(row1[COL_PC], Felt::new(42));
        assert_eq!(row1[COL_SP], Felt::new(7));
        assert_eq!(row1[COL_FP], Felt::new(3));
        assert_eq!(row1[COL_OUT], Felt::new(11));
        assert_eq!(row1[COL_MEM_ADDR], Felt::new(101));
        assert_eq!(row1[COL_MEM_VAL], Felt::ZERO);
        assert_eq!(row1[COL_IS_READ], Felt::ONE);
        assert_eq!(row1[COL_SEL_BASE + SEL_NOP], Felt::ONE);

        let row2 = trace.row(2);
        assert_eq!(row2[COL_MEM_ADDR], Felt::new(102));
    }

    #[test]
    fn fills_holes_across_multiple_segments_in_declaration_order() {
        let mut trace = seed_trace_with_one_row();
        let mut segments = MemorySegmentManager::new();
        let a = segments.add().unwrap();
        segments
            .write(a, maat_runtime::MaybeRelocatable::Felt(Felt::new(1)))
            .unwrap();
        segments
            .write(
                Relocatable::new(a.segment_index, 2),
                maat_runtime::MaybeRelocatable::Felt(Felt::new(2)),
            )
            .unwrap();
        let b = segments.add().unwrap();
        segments
            .write(
                Relocatable::new(b.segment_index, 1),
                maat_runtime::MaybeRelocatable::Felt(Felt::new(3)),
            )
            .unwrap();
        let relocator = Relocator::new(&[3, 2], 50).unwrap();
        let added = fill_memory_holes(&mut trace, &segments, &relocator).unwrap();
        assert_eq!(added, 2);
        // Seg A base = 50, hole at offset 1 -> flat 51.
        // Seg B base = 53, hole at offset 0 -> flat 53.
        assert_eq!(trace.row(1)[COL_MEM_ADDR], Felt::new(51));
        assert_eq!(trace.row(2)[COL_MEM_ADDR], Felt::new(53));
    }

    #[test]
    fn zero_count_is_a_no_op() {
        let mut trace = seed_trace();
        append_pubmem_dummies(&mut trace, 0).unwrap();
        assert_eq!(trace.num_rows(), 1);
    }

    #[test]
    fn appended_rows_inherit_cpu_state_and_zero_mem() {
        let mut trace = seed_trace();
        append_pubmem_dummies(&mut trace, 3).unwrap();
        assert_eq!(trace.num_rows(), 4);
        for i in 1..4 {
            let r = trace.row(i);
            assert_eq!(r[COL_PC], Felt::new(11));
            assert_eq!(r[COL_SP], Felt::new(5));
            assert_eq!(r[COL_FP], Felt::new(3));
            assert_eq!(r[COL_OUT], Felt::new(99));
            assert_eq!(r[COL_MEM_ADDR], Felt::ZERO);
            assert_eq!(r[COL_MEM_VAL], Felt::ZERO);
            assert_eq!(r[COL_IS_READ], Felt::ONE);
            assert_eq!(r[COL_SEL_BASE + SEL_NOP], Felt::ONE);
        }
    }

    #[test]
    fn errors_when_trace_is_empty() {
        let mut trace = TraceTable::new();
        let err = append_pubmem_dummies(&mut trace, 2).unwrap_err();
        assert!(err.to_string().contains("public-memory dummies"));
    }
}
