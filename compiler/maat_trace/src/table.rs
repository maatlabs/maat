//! Execution trace table for the Maat STARK prover/verifier.
//!
//! The [`TraceTable`] records every instruction step as a row of [`TRACE_WIDTH`]
//! Goldilocks field elements. The trace is the algebraic witness that the STARK
//! prover commits to and the verifier checks against the AIR constraints.

use std::fmt;
use std::io::{self, Write};

use maat_bytecode::selector::{NUM_SELECTORS, NUM_SUB_SELECTORS, SEL_NOP};
use maat_field::{Felt, FieldElement};

/// Program counter.
pub const COL_PC: usize = 0;
/// Stack pointer (depth of operand stack).
pub const COL_SP: usize = 1;
/// Frame pointer (base address in flat memory).
pub const COL_FP: usize = 2;
/// First opcode operand (zero if absent). Read by the unconditional and
/// conditional jump constraints.
pub const COL_OPERAND_0: usize = 3;
/// Stack top before instruction.
pub const COL_S0: usize = 4;
/// Stack second element before instruction.
pub const COL_S1: usize = 5;
/// Stack third element before instruction.
pub const COL_S2: usize = 6;
/// Result value pushed to stack (or zero for store/jump).
pub const COL_OUT: usize = 7;
/// Flat memory address accessed (for loads/stores and heap accesses).
pub const COL_MEM_ADDR: usize = 8;
/// Memory value at `mem_addr`.
pub const COL_MEM_VAL: usize = 9;
/// `1` if memory read, `0` if memory write.
pub const COL_IS_READ: usize = 10;
/// Base column index for the 17 selector flags (`sel_0..sel_16`).
pub const COL_SEL_BASE: usize = 11;

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

/// Base column index for the per-opcode sub-selector flags.
pub const COL_SUB_SEL_BASE: usize = COL_DIV_AUX + 1;

/// Total number of columns in the main execution trace.
pub const TRACE_WIDTH: usize = COL_SUB_SEL_BASE + NUM_SUB_SELECTORS;

/// Column names for CSV header and debugging.
pub const COLUMN_NAMES: [&str; TRACE_WIDTH] = [
    "pc",
    "sp",
    "fp",
    "operand_0",
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
    "sub_sel_add",
    "sub_sel_sub",
    "sub_sel_div",
    "sub_sel_neg",
    "sub_sel_felt_add",
    "sub_sel_felt_sub",
    "sub_sel_felt_mul",
    "sub_sel_eq",
    "sub_sel_neq",
    "sub_sel_and",
    "sub_sel_or",
    "sub_sel_xor",
    "sub_sel_shl",
    "sub_sel_shr",
    "sub_sel_lt",
    "sub_sel_gt",
];

pub type TraceRow = [Felt; TRACE_WIDTH];

pub struct TraceTable {
    rows: Vec<TraceRow>,
}

impl TraceTable {
    const MIN_ROWS: usize = 8;

    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn push_row(&mut self, row: TraceRow) {
        self.rows.push(row);
    }

    pub fn num_rows(&self) -> usize {
        self.rows.len()
    }

    pub fn row(&self, index: usize) -> &TraceRow {
        &self.rows[index]
    }

    pub fn row_mut(&mut self, index: usize) -> &mut TraceRow {
        &mut self.rows[index]
    }

    pub fn stamp_output(&mut self, output: Felt) {
        if let Some(last) = self.rows.last_mut() {
            last[COL_OUT] = output;
        }
    }

    pub fn pad_to_power_of_two(&mut self) {
        let target = if self.rows.len() <= Self::MIN_ROWS {
            Self::MIN_ROWS
        } else {
            self.rows.len().next_power_of_two()
        };

        let pad_row = self.make_padding_row();
        self.rows.resize(target, pad_row);
    }

    fn make_padding_row(&self) -> TraceRow {
        let mut row = [Felt::ZERO; TRACE_WIDTH];
        if let Some(last) = self.rows.last() {
            row[COL_PC] = last[COL_PC];
            row[COL_SP] = last[COL_SP];
            row[COL_FP] = last[COL_FP];
            row[COL_OUT] = last[COL_OUT];
            // Dummy read: repeat the last memory access so the unified memory
            // permutation argument stays consistent across padding rows.
            row[COL_MEM_ADDR] = last[COL_MEM_ADDR];
            row[COL_MEM_VAL] = last[COL_MEM_VAL];
            row[COL_IS_READ] = Felt::ONE;
        }
        row[COL_SEL_BASE + SEL_NOP] = Felt::ONE;
        row
    }

    pub fn write_csv<W: Write>(&self, mut w: W) -> io::Result<()> {
        for (i, name) in COLUMN_NAMES.iter().enumerate() {
            if i > 0 {
                write!(w, ",")?;
            }
            write!(w, "{name}")?;
        }
        writeln!(w)?;

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

    pub fn to_csv(&self) -> String {
        let mut buf = Vec::new();
        self.write_csv(&mut buf)
            .expect("writing to Vec<u8> cannot fail");
        String::from_utf8(buf).expect("CSV is valid UTF-8")
    }

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
        assert_eq!(cols[TRACE_WIDTH - 1], "sub_sel_gt");
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
