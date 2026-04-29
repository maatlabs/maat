//! Execution trace table for the Maat ZK backend.
//!
//! The [`TraceTable`] records every instruction step as a row of [`TRACE_WIDTH`]
//! Goldilocks field elements. The trace is the algebraic witness that the STARK
//! prover commits to and the verifier checks against the AIR constraints.
//!
//! # Range-check columns
//!
//! Columns 30--35 carry the range-check sub-AIR witness data:
//!
//! - **`rc_val`**: the value being range-checked (zero on non-trigger rows).
//! - **`rc_l0`..`rc_l3`**: four 16-bit limbs satisfying
//!   `rc_val = l0 + 2^16 * l1 + 2^32 * l2 + 2^48 * l3`.
//! - **`nonzero_inv`**: multiplicative inverse of the divisor on `sel_div_mod`
//!   rows, proving the divisor is non-zero.
//!
//! # Heap-segment columns
//!
//! Columns [`COL_HEAP_ADDR`], [`COL_HEAP_VAL`], and [`COL_HEAP_IS_READ`]
//! together with the dedicated allocation flag [`COL_HEAP_ALLOC_FLAG`]
//! carry the heap-access witness for composite-type tracing. Every row
//! records one heap interaction; non-heap rows perform a dummy read of the
//! last accessed address (mirroring the main-memory pattern). The heap
//! permutation argument in the auxiliary segment ties execution-order
//! heap accesses to the sorted-by-address list, enforcing single-value
//! consistency over the heap address space.

use std::fmt;
use std::io::{self, Write};

use maat_bytecode::{NUM_SELECTORS, NUM_SUB_SELECTORS, SEL_NOP};
use maat_errors::{Result, VmError};
use maat_field::{Felt, FieldElement};

/// Program counter.
pub const COL_PC: usize = 0;
/// Stack pointer (depth of operand stack).
pub const COL_SP: usize = 1;
/// Frame pointer (base address in flat memory).
pub const COL_FP: usize = 2;
/// Current opcode byte.
pub const COL_OPCODE: usize = 3;
/// First opcode operand (zero if absent).
pub const COL_OPERAND_0: usize = 4;
/// Second opcode operand (zero if absent).
pub const COL_OPERAND_1: usize = 5;
/// Stack top before instruction.
pub const COL_S0: usize = 6;
/// Stack second element before instruction.
pub const COL_S1: usize = 7;
/// Stack third element before instruction.
pub const COL_S2: usize = 8;
/// Result value pushed to stack (or zero for store/jump).
pub const COL_OUT: usize = 9;
/// Flat memory address accessed (for loads/stores).
pub const COL_MEM_ADDR: usize = 10;
/// Memory value at `mem_addr`.
pub const COL_MEM_VAL: usize = 11;
/// `1` if memory read, `0` if memory write.
pub const COL_IS_READ: usize = 12;
/// Base column index for the 17 selector flags (`sel_0..sel_16`).
pub const COL_SEL_BASE: usize = 13;

/// Base index of the range-check columns (immediately after selectors).
const COL_RC_BASE: usize = COL_SEL_BASE + NUM_SELECTORS;

/// Value being range-checked. Zero on non-trigger rows.
pub const COL_RC_VAL: usize = COL_RC_BASE;
/// Range-check limb 0 (bits 0--15).
pub const COL_RC_L0: usize = COL_RC_BASE + 1;
/// Range-check limb 1 (bits 16--31).
pub const COL_RC_L1: usize = COL_RC_BASE + 2;
/// Range-check limb 2 (bits 32--47).
pub const COL_RC_L2: usize = COL_RC_BASE + 3;
/// Range-check limb 3 (bits 48--63).
pub const COL_RC_L3: usize = COL_RC_BASE + 4;
/// Multiplicative inverse of the divisor on `sel_div_mod` rows.
/// Proves the divisor is non-zero: `divisor * nonzero_inv = 1`.
pub const COL_NONZERO_INV: usize = COL_RC_BASE + 5;

/// Operand-byte width of the current opcode (`operand_widths().sum() + 1`).
pub const COL_OP_WIDTH: usize = COL_NONZERO_INV + 1;

/// Multiplicative inverse witness for the equality output constraint.
pub const COL_CMP_INV: usize = COL_OP_WIDTH + 1;

/// Auxiliary witness for the division/modulo identity.
pub const COL_DIV_AUX: usize = COL_CMP_INV + 1;

/// Heap address accessed on this row (zero on rows with no real heap access).
pub const COL_HEAP_ADDR: usize = COL_DIV_AUX + 1;

/// Heap value at [`COL_HEAP_ADDR`].
pub const COL_HEAP_VAL: usize = COL_HEAP_ADDR + 1;

/// `1` if the row reads the heap, `0` if it writes the heap.
pub const COL_HEAP_IS_READ: usize = COL_HEAP_VAL + 1;

/// Heap-allocation flag: `1` on rows that allocate a fresh heap cell,
/// `0` otherwise. Used by the AIR to constrain monotonic heap-pointer
/// growth and to distinguish allocations from in-place reads/writes.
pub const COL_HEAP_ALLOC_FLAG: usize = COL_HEAP_IS_READ + 1;

/// Base column index for the per-opcode sub-selector flags. The sub-selector
/// indices themselves (`SUB_SEL_ADD`, `SUB_SEL_SUB`, ...) live in
/// `maat_bytecode` so the trace recorder and the AIR read them from the
/// same source.
pub const COL_SUB_SEL_BASE: usize = COL_HEAP_ALLOC_FLAG + 1;

/// Total number of columns in the main execution trace.
pub const TRACE_WIDTH: usize = COL_SUB_SEL_BASE + NUM_SUB_SELECTORS;

/// Column names for CSV header and debugging.
pub const COLUMN_NAMES: [&str; TRACE_WIDTH] = [
    "pc",
    "sp",
    "fp",
    "opcode",
    "operand_0",
    "operand_1",
    "s0",
    "s1",
    "s2",
    "out",
    "mem_addr",
    "mem_val",
    "is_read",
    "sel_0",
    "sel_1",
    "sel_2",
    "sel_3",
    "sel_4",
    "sel_5",
    "sel_6",
    "sel_7",
    "sel_8",
    "sel_9",
    "sel_10",
    "sel_11",
    "sel_12",
    "sel_13",
    "sel_14",
    "sel_15",
    "sel_16",
    "sel_17",
    "sel_18",
    "sel_19",
    "rc_val",
    "rc_l0",
    "rc_l1",
    "rc_l2",
    "rc_l3",
    "nonzero_inv",
    "op_width",
    "cmp_inv",
    "div_aux",
    "heap_addr",
    "heap_val",
    "heap_is_read",
    "heap_alloc_flag",
    "sub_sel_add",
    "sub_sel_sub",
    "sub_sel_div",
    "sub_sel_neg",
    "sub_sel_felt_add",
    "sub_sel_felt_sub",
    "sub_sel_felt_mul",
    "sub_sel_eq",
    "sub_sel_neq",
];

/// A single trace row: an array of [`TRACE_WIDTH`] field elements.
pub type TraceRow = [Felt; TRACE_WIDTH];

/// Execution trace matrix.
///
/// Each row records the machine state for one instruction step. After
/// execution, the trace is padded to the next power of two (minimum 8 rows)
/// as required by the Winterfell FRI prover.
pub struct TraceTable {
    rows: Vec<TraceRow>,
}

impl TraceTable {
    const MIN_ROWS: usize = 8;

    /// Creates an empty trace table.
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Appends a row to the trace.
    pub fn push_row(&mut self, row: TraceRow) {
        self.rows.push(row);
    }

    /// Returns the number of rows (before or after padding).
    pub fn num_rows(&self) -> usize {
        self.rows.len()
    }

    /// Returns a reference to the row at the given index.
    pub fn row(&self, index: usize) -> &TraceRow {
        &self.rows[index]
    }

    /// Returns a mutable reference to the row at the given index.
    pub fn row_mut(&mut self, index: usize) -> &mut TraceRow {
        &mut self.rows[index]
    }

    /// Returns a reference to the last row, or `None` if empty.
    pub fn last_row(&self) -> Option<&TraceRow> {
        self.rows.last()
    }

    /// Writes the program output into [`COL_OUT`] on the last row.
    ///
    /// This must be called before [`pad_to_power_of_two`](Self::pad_to_power_of_two)
    /// so that the NOP padding rows inherit the output value, satisfying the
    /// boundary assertion `out[last] = public_output` in the AIR.
    pub fn stamp_output(&mut self, output: Felt) {
        if let Some(last) = self.rows.last_mut() {
            last[COL_OUT] = output;
        }
    }

    /// Pads the trace to the next power of two (minimum 8 rows).
    ///
    /// Padding rows use `sel_nop` (selector 0 = 1, all others = 0) and
    /// repeat the final `pc`, `sp`, `fp`, and `out` values from the last
    /// real row.
    pub fn pad_to_power_of_two(&mut self) {
        let target = if self.rows.len() <= Self::MIN_ROWS {
            Self::MIN_ROWS
        } else {
            self.rows.len().next_power_of_two()
        };

        let pad_row = self.make_padding_row();
        self.rows.resize(target, pad_row);
    }

    /// Constructs a NOP padding row from the final execution state.
    fn make_padding_row(&self) -> TraceRow {
        let mut row = [Felt::ZERO; TRACE_WIDTH];
        if let Some(last) = self.rows.last() {
            row[COL_PC] = last[COL_PC];
            row[COL_SP] = last[COL_SP];
            row[COL_FP] = last[COL_FP];
            row[COL_OUT] = last[COL_OUT];
            // Dummy reads: repeat the last main-memory and heap accesses to
            // preserve address/value consistency in both permutation arguments.
            row[COL_MEM_ADDR] = last[COL_MEM_ADDR];
            row[COL_MEM_VAL] = last[COL_MEM_VAL];
            row[COL_IS_READ] = Felt::ONE;
            row[COL_HEAP_ADDR] = last[COL_HEAP_ADDR];
            row[COL_HEAP_VAL] = last[COL_HEAP_VAL];
            row[COL_HEAP_IS_READ] = Felt::ONE;
        }
        row[COL_SEL_BASE + SEL_NOP] = Felt::ONE;
        row
    }

    /// Writes the trace as CSV to the given writer.
    ///
    /// Field elements are serialized as decimal `u64` values.
    pub fn write_csv<W: Write>(&self, mut w: W) -> io::Result<()> {
        // Header
        for (i, name) in COLUMN_NAMES.iter().enumerate() {
            if i > 0 {
                write!(w, ",")?;
            }
            write!(w, "{name}")?;
        }
        writeln!(w)?;

        // Rows
        for row in &self.rows {
            for (i, felt) in row.iter().enumerate() {
                if i > 0 {
                    write!(w, ",")?;
                }
                write!(w, "{}", felt.as_int())?;
            }
            writeln!(w)?;
        }
        Ok(())
    }

    /// Serializes the trace to a CSV string.
    pub fn to_csv(&self) -> String {
        let mut buf = Vec::new();
        self.write_csv(&mut buf)
            .expect("writing to Vec<u8> cannot fail");
        String::from_utf8(buf).expect("CSV is valid UTF-8")
    }

    /// Transposes the row-major table into column-major vectors.
    ///
    /// Returns a vector of `TRACE_WIDTH` columns, each containing one
    /// field element per row. This is the layout expected by Winterfell's
    /// `TraceTable::init`.
    pub fn into_columns(self) -> Vec<Vec<Felt>> {
        let num_rows = self.rows.len();
        let mut columns = (0..TRACE_WIDTH)
            .map(|_| Vec::with_capacity(num_rows))
            .collect::<Vec<Vec<Felt>>>();
        for row in &self.rows {
            for (col_idx, felt) in row.iter().enumerate() {
                columns[col_idx].push(*felt);
            }
        }
        columns
    }

    /// Asserts that physical addresses in [`COL_MEM_ADDR`] and [`COL_HEAP_ADDR`]
    /// each form a contiguous range `[0, max_addr]` with no gaps.
    pub fn validate_address_contiguity(&self) -> Result<()> {
        self.assert_contiguous(COL_MEM_ADDR, "main-memory")?;
        self.assert_contiguous(COL_HEAP_ADDR, "heap")?;
        Ok(())
    }

    fn assert_contiguous(&self, column: usize, label: &str) -> Result<()> {
        let n = self.num_rows();
        let mut addrs = (0..n)
            .map(|i| self.row(i)[column].as_int())
            .collect::<Vec<u64>>();
        addrs.sort_unstable();
        addrs.dedup();
        for (expected, &actual) in addrs.iter().enumerate() {
            if actual != expected as u64 {
                return Err(VmError::new(format!(
                "physical address gap in {label} segment: expected address {expected}, found {actual}"
            ))
            .into());
            }
        }
        Ok(())
    }
}

impl Default for TraceTable {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TraceTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TraceTable({} rows, {} cols)",
            self.rows.len(),
            TRACE_WIDTH
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_empty_to_min() {
        let mut t = TraceTable::new();
        t.pad_to_power_of_two();
        assert_eq!(t.num_rows(), TraceTable::MIN_ROWS);
        for i in 0..t.num_rows() {
            assert_eq!(t.row(i)[COL_SEL_BASE], Felt::ONE, "sel_nop should be set");
        }
    }

    #[test]
    fn pad_single_row_to_min() {
        let mut t = TraceTable::new();
        let mut row = [Felt::ZERO; TRACE_WIDTH];
        row[COL_PC] = Felt::new(5);
        row[COL_SP] = Felt::new(2);
        t.push_row(row);
        t.pad_to_power_of_two();
        assert_eq!(t.num_rows(), TraceTable::MIN_ROWS);
        // Padding rows inherit pc/sp from last real row
        assert_eq!(t.row(1)[COL_PC], Felt::new(5));
        assert_eq!(t.row(1)[COL_SP], Felt::new(2));
    }

    #[test]
    fn pad_below_min_promoted_to_min() {
        let mut t = TraceTable::new();
        for _ in 0..(TraceTable::MIN_ROWS - 3) {
            t.push_row([Felt::ZERO; TRACE_WIDTH]);
        }
        t.pad_to_power_of_two();
        assert_eq!(t.num_rows(), TraceTable::MIN_ROWS);
    }

    #[test]
    fn pad_at_min_stays_at_min() {
        let mut t = TraceTable::new();
        for _ in 0..TraceTable::MIN_ROWS {
            t.push_row([Felt::ZERO; TRACE_WIDTH]);
        }
        t.pad_to_power_of_two();
        assert_eq!(t.num_rows(), TraceTable::MIN_ROWS);
    }

    #[test]
    fn pad_above_min_rounds_up_to_next_power_of_two() {
        let mut t = TraceTable::new();
        for _ in 0..(TraceTable::MIN_ROWS + 1) {
            t.push_row([Felt::ZERO; TRACE_WIDTH]);
        }
        t.pad_to_power_of_two();
        assert_eq!(t.num_rows(), (TraceTable::MIN_ROWS * 2).next_power_of_two());
    }

    #[test]
    fn csv_header_matches_columns() {
        let t = TraceTable::new();
        let csv = t.to_csv();
        let header = csv.lines().next().unwrap();
        let cols = header.split(',').collect::<Vec<_>>();
        assert_eq!(cols.len(), TRACE_WIDTH);
        assert_eq!(cols[0], "pc");
        assert_eq!(cols[TRACE_WIDTH - 1], "sub_sel_neq");
    }

    #[test]
    fn into_columns_transposes_correctly() {
        let mut t = TraceTable::new();
        let mut row = [Felt::ZERO; TRACE_WIDTH];
        row[COL_PC] = Felt::new(10);
        row[COL_SP] = Felt::new(20);
        t.push_row(row);
        let cols = t.into_columns();
        assert_eq!(cols.len(), TRACE_WIDTH);
        assert_eq!(cols[COL_PC][0], Felt::new(10));
        assert_eq!(cols[COL_SP][0], Felt::new(20));
    }
}
