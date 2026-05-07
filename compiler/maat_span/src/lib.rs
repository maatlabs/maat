#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// A zero-length span at position 0, used as a placeholder in tests
    /// and span-less contexts.
    pub const ZERO: Self = Self { start: 0, end: 0 };

    #[inline]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Merges two spans into one covering both ranges.
    ///
    /// The resulting span starts at the minimum of the two starts
    /// and ends at the maximum of the two ends.
    #[inline]
    pub const fn merge(self, other: Self) -> Self {
        let start = if self.start < other.start {
            self.start
        } else {
            other.start
        };
        let end = if self.end > other.end {
            self.end
        } else {
            other.end
        };
        Self { start, end }
    }
}

impl Default for Span {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SourceMap {
    entries: Vec<(usize, Span)>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, offset: usize, span: Span) {
        debug_assert!(
            self.entries.last().is_none_or(|&(prev, _)| offset >= prev),
            "SourceMap::add: offset {offset} is less than previous entry offset {}",
            self.entries.last().map_or(0, |&(prev, _)| prev),
        );
        self.entries.push((offset, span));
    }

    pub fn lookup(&self, offset: usize) -> Option<Span> {
        if self.entries.is_empty() {
            return None;
        }
        match self.entries.binary_search_by_key(&offset, |&(o, _)| o) {
            Ok(idx) => Some(self.entries[idx].1),
            Err(0) => None,
            Err(idx) => Some(self.entries[idx - 1].1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_merge() {
        let a = Span::new(5, 10);
        let b = Span::new(3, 12);
        assert_eq!(a.merge(b), Span::new(3, 12));

        let c = Span::new(0, 5);
        let d = Span::new(10, 15);
        assert_eq!(c.merge(d), Span::new(0, 15));
    }

    #[test]
    fn source_map_lookup() {
        let mut sm = SourceMap::new();
        sm.add(0, Span::new(0, 5));
        sm.add(3, Span::new(6, 10));
        sm.add(7, Span::new(11, 15));

        assert_eq!(sm.lookup(0), Some(Span::new(0, 5)));
        assert_eq!(sm.lookup(3), Some(Span::new(6, 10)));
        assert_eq!(sm.lookup(7), Some(Span::new(11, 15)));

        // Falls back to nearest preceding entry
        assert_eq!(sm.lookup(5), Some(Span::new(6, 10)));
        assert_eq!(sm.lookup(1), Some(Span::new(0, 5)));
        assert_eq!(sm.lookup(100), Some(Span::new(11, 15)));
    }

    #[test]
    fn source_map_empty() {
        let sm = SourceMap::new();
        assert_eq!(sm.lookup(0), None);
    }

    #[test]
    fn source_map_accepts_equal_offsets() {
        let mut sm = SourceMap::new();
        sm.add(5, Span::new(0, 3));
        sm.add(5, Span::new(4, 7));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SourceMap::add: offset 2 is less than previous entry offset 5")]
    fn source_map_rejects_out_of_order() {
        let mut sm = SourceMap::new();
        sm.add(5, Span::new(0, 3));
        sm.add(2, Span::new(4, 7));
    }
}
