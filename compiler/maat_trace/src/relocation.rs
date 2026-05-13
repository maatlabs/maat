//! Trace relocation pass.
//!
//! Between trace finalization and proof generation, the relocator rewrites
//! every cell that the recorder marked as carrying a logical
//! [`Relocatable`] to its flat field-element address.

use maat_errors::{MemoryError, VmError};
use maat_field::Felt;
use maat_runtime::Relocatable;

use crate::recorder::RowRelocPlan;
use crate::table::{COL_MEM_ADDR, COL_MEM_VAL, COL_OUT, COL_S0, COL_S1, COL_S2, TraceTable};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
