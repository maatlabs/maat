//! Public-memory segments bound into the AIR's public-memory accumulator.

use maat_field::Felt;

/// A single public-memory segment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublicSegment {
    /// Flat (relocated) base address of the segment's first cell.
    pub base: u32,
    /// The segment's cell values, in offset order.
    pub cells: Vec<Felt>,
}

impl PublicSegment {
    pub fn new(base: u32, cells: Vec<Felt>) -> Self {
        Self { base, cells }
    }
}

/// The three public-memory segments the verifier binds, in accumulator order:
/// program inputs, program output cells, and the pinned program image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublicMemory {
    /// `fn main`'s `pub` input cells.
    pub input: PublicSegment,
    /// The program's public-output cells.
    pub output: PublicSegment,
    /// The pinned program image.
    pub program: PublicSegment,
}

impl PublicMemory {
    pub fn segments(&self) -> [&PublicSegment; 3] {
        [&self.input, &self.output, &self.program]
    }

    pub fn total_cells(&self) -> usize {
        self.input.cells.len() + self.output.cells.len() + self.program.cells.len()
    }
}
