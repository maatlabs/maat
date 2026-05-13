//! Memory segments for the Maat runtime.

use std::collections::HashMap;
use std::fmt;

use maat_errors::MemoryError;
use maat_field::{Felt, from_i64};
use serde::{Deserialize, Serialize};

/// Segment ID reserved for the program bytecode.
pub const SEG_PROGRAM: u32 = 0;
/// Segment ID reserved for the execution segment (locals, globals, saved
/// frame pointers).
pub const SEG_EXECUTION: u32 = 1;

/// First flat address used by [`MemorySegmentManager::relocate_segments`].
pub const RELOCATION_BASE: u32 = 1;

type Result<T> = std::result::Result<T, MemoryError>;

/// A logical address within a segment, decoupled from the flat address space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Relocatable {
    pub segment_index: u32,
    pub offset: u32,
}

impl Relocatable {
    pub const fn new(segment_index: u32, offset: u32) -> Self {
        Self {
            segment_index,
            offset,
        }
    }

    pub fn add_offset(self, n: Felt) -> Result<Self> {
        let raw = n.as_int();
        let addend_u32 = u32::try_from(raw).map_err(|_| MemoryError::OffsetOverflow {
            segment: self.segment_index,
            offset: self.offset,
            addend: raw,
        })?;
        let offset = self
            .offset
            .checked_add(addend_u32)
            .ok_or(MemoryError::OffsetOverflow {
                segment: self.segment_index,
                offset: self.offset,
                addend: raw,
            })?;
        Ok(Self {
            segment_index: self.segment_index,
            offset,
        })
    }

    /// Returns the signed difference of two same-segment relocatables encoded
    /// as a Goldilocks field element.
    pub fn sub_r(self, other: Self) -> Result<Felt> {
        if self.segment_index != other.segment_index {
            return Err(MemoryError::CrossSegmentSubtraction {
                lhs_segment: self.segment_index,
                rhs_segment: other.segment_index,
            });
        }
        let diff = i64::from(self.offset) - i64::from(other.offset);
        Ok(from_i64(diff))
    }
}

impl fmt::Display for Relocatable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.segment_index, self.offset)
    }
}

/// A memory cell value: either a field element or a logical address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaybeRelocatable {
    Felt(Felt),
    Relocatable(Relocatable),
}

impl MaybeRelocatable {
    pub fn as_felt(self) -> Option<Felt> {
        match self {
            Self::Felt(f) => Some(f),
            Self::Relocatable(_) => None,
        }
    }

    pub fn as_relocatable(self) -> Option<Relocatable> {
        match self {
            Self::Relocatable(r) => Some(r),
            Self::Felt(_) => None,
        }
    }
}

impl From<Felt> for MaybeRelocatable {
    fn from(value: Felt) -> Self {
        Self::Felt(value)
    }
}

impl From<Relocatable> for MaybeRelocatable {
    fn from(value: Relocatable) -> Self {
        Self::Relocatable(value)
    }
}

impl fmt::Display for MaybeRelocatable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Felt(v) => v.fmt(f),
            Self::Relocatable(r) => r.fmt(f),
        }
    }
}

/// Allocates segments on demand and tracks their contents.
#[derive(Debug, Clone, Default)]
pub struct MemorySegmentManager {
    data: Vec<Vec<Option<MaybeRelocatable>>>,
    segment_sizes: HashMap<u32, u32>,
}

impl MemorySegmentManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes a value at `addr`. Write-once is enforced: rewriting a cell
    /// with a different value is an error.
    pub fn write(&mut self, addr: Relocatable, value: MaybeRelocatable) -> Result<()> {
        let segment = self
            .data
            .get_mut(addr.segment_index as usize)
            .ok_or(MemoryError::SegmentNotFound(addr.segment_index))?;
        let off = addr.offset as usize;
        if segment.len() <= off {
            segment.resize(
                off.checked_add(1)
                    .ok_or(MemoryError::SegmentTooLarge(addr.segment_index))?,
                None,
            );
        }
        match segment[off] {
            Some(existing) if existing != value => Err(MemoryError::WriteOnceViolation {
                segment: addr.segment_index,
                offset: addr.offset,
            }),
            _ => {
                segment[off] = Some(value);
                Ok(())
            }
        }
    }

    /// Reads the value at `addr`, or `None` if the cell is unallocated.
    pub fn read(&self, addr: Relocatable) -> Option<MaybeRelocatable> {
        self.data
            .get(addr.segment_index as usize)?
            .get(addr.offset as usize)
            .copied()
            .flatten()
    }

    pub fn add_with_size(&mut self, size: u32) -> Result<Relocatable> {
        let base = self.add()?;
        self.segment_sizes.insert(base.segment_index, size);
        Ok(base)
    }

    pub fn add(&mut self) -> Result<Relocatable> {
        let seg_index =
            u32::try_from(self.data.len()).map_err(|_| MemoryError::SegmentCountOverflow)?;
        self.data.push(Vec::new());
        Ok(Relocatable::new(seg_index, 0))
    }

    pub fn num_segments(&self) -> usize {
        self.data.len()
    }

    /// Returns the effective size of each segment in registration order.
    pub fn compute_sizes(&self) -> Result<Vec<u32>> {
        self.data
            .iter()
            .enumerate()
            .map(|(idx, seg)| {
                let seg_id = u32::try_from(idx).map_err(|_| MemoryError::SegmentCountOverflow)?;
                let highest = seg
                    .iter()
                    .rposition(Option::is_some)
                    .map(|p| p.saturating_add(1))
                    .unwrap_or(0);
                let written =
                    u32::try_from(highest).map_err(|_| MemoryError::SegmentTooLarge(seg_id))?;
                match self.segment_sizes.get(&seg_id) {
                    Some(&declared) if declared < written => {
                        Err(MemoryError::DeclaredSizeUnderflow {
                            segment: seg_id,
                            declared,
                            actual: written,
                        })
                    }
                    Some(&declared) => Ok(declared),
                    None => Ok(written),
                }
            })
            .collect()
    }

    /// Computes the relocation table mapping segment IDs to flat base addresses.
    pub fn relocate_segments(&self) -> Result<Vec<u32>> {
        let sizes = self.compute_sizes()?;
        let mut bases = Vec::with_capacity(sizes.len());
        let mut cursor = RELOCATION_BASE;
        for size in sizes {
            bases.push(cursor);
            cursor = cursor
                .checked_add(size)
                .ok_or(MemoryError::RelocationOverflow)?;
        }
        Ok(bases)
    }
}

#[cfg(test)]
mod tests {
    use maat_field::FieldElement;

    use super::*;

    #[test]
    fn add_assigns_sequential_segment_ids() {
        let mut mgr = MemorySegmentManager::new();
        assert_eq!(mgr.add().unwrap(), Relocatable::new(0, 0));
        assert_eq!(mgr.add().unwrap(), Relocatable::new(1, 0));
        assert_eq!(mgr.add().unwrap(), Relocatable::new(2, 0));
        assert_eq!(mgr.num_segments(), 3);
    }

    #[test]
    fn add_with_size_records_declared_size() {
        let mut mgr = MemorySegmentManager::new();
        let base = mgr.add_with_size(4).unwrap();
        assert_eq!(base, Relocatable::new(0, 0));
        let sizes = mgr.compute_sizes().unwrap();
        assert_eq!(sizes, vec![4]);
    }

    #[test]
    fn declared_size_takes_precedence_over_written_offsets() {
        let mut mgr = MemorySegmentManager::new();
        let base = mgr.add_with_size(8).unwrap();
        mgr.write(base, Felt::new(42).into()).unwrap();
        let sizes = mgr.compute_sizes().unwrap();
        assert_eq!(sizes, vec![8]);
    }

    #[test]
    fn declared_size_below_high_water_mark_errors() {
        let mut mgr = MemorySegmentManager::new();
        let base = mgr.add_with_size(2).unwrap();
        mgr.write(Relocatable::new(base.segment_index, 5), Felt::new(1).into())
            .unwrap();
        let err = mgr.compute_sizes().unwrap_err();
        assert_eq!(
            err,
            MemoryError::DeclaredSizeUnderflow {
                segment: 0,
                declared: 2,
                actual: 6,
            }
        );
    }

    #[test]
    fn effective_size_uses_high_water_mark_for_undeclared_segments() {
        let mut mgr = MemorySegmentManager::new();
        let base = mgr.add().unwrap();
        mgr.write(Relocatable::new(base.segment_index, 0), Felt::new(1).into())
            .unwrap();
        mgr.write(Relocatable::new(base.segment_index, 9), Felt::new(2).into())
            .unwrap();
        assert_eq!(mgr.compute_sizes().unwrap(), vec![10]);
    }

    #[test]
    fn read_returns_written_value() {
        let mut mgr = MemorySegmentManager::new();
        let base = mgr.add().unwrap();
        mgr.write(base, Felt::new(99).into()).unwrap();
        assert_eq!(mgr.read(base), Some(MaybeRelocatable::Felt(Felt::new(99))));
    }

    #[test]
    fn read_returns_none_for_unwritten_cell() {
        let mut mgr = MemorySegmentManager::new();
        let base = mgr.add().unwrap();
        assert_eq!(mgr.read(base), None);
        assert_eq!(mgr.read(Relocatable::new(99, 0)), None);
    }

    #[test]
    fn rewrite_with_same_value_succeeds() {
        let mut mgr = MemorySegmentManager::new();
        let base = mgr.add().unwrap();
        mgr.write(base, Felt::new(7).into()).unwrap();
        mgr.write(base, Felt::new(7).into()).unwrap();
    }

    #[test]
    fn rewrite_with_different_value_errors() {
        let mut mgr = MemorySegmentManager::new();
        let base = mgr.add().unwrap();
        mgr.write(base, Felt::new(7).into()).unwrap();
        let err = mgr.write(base, Felt::new(8).into()).unwrap_err();
        assert_eq!(
            err,
            MemoryError::WriteOnceViolation {
                segment: 0,
                offset: 0,
            }
        );
    }

    #[test]
    fn write_to_nonexistent_segment_errors() {
        let mut mgr = MemorySegmentManager::new();
        let err = mgr
            .write(Relocatable::new(0, 0), Felt::new(1).into())
            .unwrap_err();
        assert_eq!(err, MemoryError::SegmentNotFound(0));
    }

    #[test]
    fn relocate_lays_segments_back_to_back_starting_at_one() {
        let mut mgr = MemorySegmentManager::new();
        let _ = mgr.add_with_size(3).unwrap();
        let _ = mgr.add_with_size(5).unwrap();
        let _ = mgr.add_with_size(2).unwrap();
        // base[0] = 1; base[1] = 1+3 = 4; base[2] = 4+5 = 9; trailing cursor = 11
        assert_eq!(mgr.relocate_segments().unwrap(), vec![1, 4, 9]);
    }

    #[test]
    fn relocate_handles_empty_segments() {
        let mut mgr = MemorySegmentManager::new();
        let _ = mgr.add().unwrap();
        let _ = mgr.add_with_size(4).unwrap();
        let _ = mgr.add().unwrap();
        assert_eq!(mgr.relocate_segments().unwrap(), vec![1, 1, 5]);
    }

    #[test]
    fn relocate_overflow_detected() {
        let mut mgr = MemorySegmentManager::new();
        let _ = mgr.add_with_size(u32::MAX).unwrap();
        let _ = mgr.add_with_size(2).unwrap();
        let err = mgr.relocate_segments().unwrap_err();
        assert_eq!(err, MemoryError::RelocationOverflow);
    }

    #[test]
    fn relocatable_add_offset_advances_within_segment() {
        let r = Relocatable::new(3, 10);
        assert_eq!(r.add_offset(Felt::new(5)).unwrap(), Relocatable::new(3, 15));
    }

    #[test]
    fn relocatable_add_offset_zero_is_identity() {
        let r = Relocatable::new(3, 10);
        assert_eq!(r.add_offset(Felt::ZERO).unwrap(), r);
    }

    #[test]
    fn relocatable_add_offset_rejects_u32_overflow() {
        let r = Relocatable::new(3, u32::MAX - 1);
        let err = r.add_offset(Felt::new(5)).unwrap_err();
        assert!(matches!(err, MemoryError::OffsetOverflow { .. }));
    }

    #[test]
    fn relocatable_add_offset_rejects_addend_above_u32() {
        let r = Relocatable::new(3, 0);
        let err = r
            .add_offset(Felt::new(u64::from(u32::MAX) + 1))
            .unwrap_err();
        assert!(matches!(err, MemoryError::OffsetOverflow { .. }));
    }

    #[test]
    fn relocatable_sub_same_segment_returns_signed_difference() {
        let high = Relocatable::new(2, 20);
        let low = Relocatable::new(2, 5);
        assert_eq!(high.sub_r(low).unwrap(), Felt::new(15));
        // Reverse direction encodes negative as Goldilocks signed felt.
        assert_eq!(low.sub_r(high).unwrap(), from_i64(-15));
    }

    #[test]
    fn relocatable_sub_cross_segment_rejected() {
        let lhs = Relocatable::new(2, 20);
        let rhs = Relocatable::new(3, 5);
        let err = lhs.sub_r(rhs).unwrap_err();
        assert_eq!(
            err,
            MemoryError::CrossSegmentSubtraction {
                lhs_segment: 2,
                rhs_segment: 3,
            }
        );
    }

    #[test]
    fn maybe_relocatable_accessors_round_trip() {
        let f: MaybeRelocatable = Felt::new(42).into();
        assert_eq!(f.as_felt(), Some(Felt::new(42)));
        assert_eq!(f.as_relocatable(), None);

        let r: MaybeRelocatable = Relocatable::new(1, 2).into();
        assert_eq!(r.as_relocatable(), Some(Relocatable::new(1, 2)));
        assert_eq!(r.as_felt(), None);
    }

    #[test]
    fn maybe_relocatable_display_matches_payload() {
        assert_eq!(
            format!("{}", MaybeRelocatable::Felt(Felt::new(7))),
            format!("{}", Felt::new(7)),
        );
        assert_eq!(
            format!("{}", MaybeRelocatable::Relocatable(Relocatable::new(4, 9))),
            "4:9"
        );
    }

    #[test]
    fn reserved_segment_constants_are_distinct() {
        assert_ne!(SEG_PROGRAM, SEG_EXECUTION);
        assert_eq!(SEG_PROGRAM, 0);
        assert_eq!(SEG_EXECUTION, 1);
    }
}
