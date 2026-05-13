//! Memory-hole filling pass.

use maat_errors::{Result, VmError};
use maat_field::{Felt, FieldElement};
use maat_runtime::{MemorySegmentManager, Relocatable};

use crate::relocation::Relocator;
use crate::selector::SEL_NOP;
use crate::table::{
    COL_FP, COL_IS_READ, COL_MEM_ADDR, COL_MEM_VAL, COL_OUT, COL_PC, COL_SEL_BASE, COL_SP,
    TRACE_WIDTH, TraceRow, TraceTable,
};

/// Appends a dummy-read row for every unaccessed offset in every user
/// segment. Returns the number of hole rows injected.
pub fn fill_memory_holes(
    trace: &mut TraceTable,
    segments: &MemorySegmentManager,
    relocator: &Relocator,
) -> Result<usize> {
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

/// Snapshot of the prior row's CPU state propagated into every hole row.
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
}
