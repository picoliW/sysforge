//! A selected row index for a navigable table.
//!
//! Every domain with a scrollable table (containers, processes,
//! services, pods) tracks which row is selected and moves that
//! selection up and down, clamping it to the number of rows currently
//! shown. That logic was copied per domain until it appeared four
//! times identically; this type holds it once.

/// A selected row index, with saturating navigation and clamping.
///
/// The index only grows or shrinks through [`Self::up`], [`Self::down`]
/// and [`Self::clamp`]; it never panics on an empty or shorter table.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    index: usize,
}

impl Selection {
    /// The currently selected row.
    #[must_use]
    pub fn index(self) -> usize {
        self.index
    }

    /// Moves one row toward the top, stopping at the first row.
    pub fn up(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    /// Moves one row toward the bottom. The upper bound is enforced
    /// separately by [`Self::clamp`], against the live row count.
    pub fn down(&mut self) {
        self.index = self.index.saturating_add(1);
    }

    /// Clamps the selection to the last valid row for `row_count`
    /// rows. An empty table clamps the selection to zero.
    pub fn clamp(&mut self, row_count: usize) {
        self.index = self.index.min(row_count.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_saturates_at_zero() {
        let mut sel = Selection::default();
        sel.up();
        assert_eq!(sel.index(), 0);
    }

    #[test]
    fn down_then_clamp_respects_row_count() {
        let mut sel = Selection::default();
        sel.down();
        sel.down();
        sel.down();
        assert_eq!(sel.index(), 3);
        sel.clamp(2); // only 2 rows: valid indices are 0, 1
        assert_eq!(sel.index(), 1);
    }

    #[test]
    fn clamp_on_empty_is_zero() {
        let mut sel = Selection::default();
        sel.down();
        sel.clamp(0);
        assert_eq!(sel.index(), 0);
    }
}
