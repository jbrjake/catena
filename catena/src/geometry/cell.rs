//! Points and boxes in cell space: the derived, viewport-relative coordinates a scene is laid
//! out in (plan §6, §7.4). Coordinates are signed, so off-screen geometry keeps its position and
//! edges still aim at it.

/// A terminal cell, column `x` and row `y`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellPt {
    /// Column.
    pub x: i32,
    /// Row.
    pub y: i32,
}

impl CellPt {
    /// The cell at column `x`, row `y`.
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        CellPt { x, y }
    }
}

/// A point in cell units with sub-cell precision. An integer-valued point names a cell, and a
/// point lies in the cell it rounds to, whatever the blitter.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SubPt {
    /// Column, fractional.
    pub x: f64,
    /// Row, fractional.
    pub y: f64,
}

impl SubPt {
    /// The point at column `x`, row `y`.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        SubPt { x, y }
    }

    /// The cell the point lies in: its coordinates rounded, saturating to the `i32` range (NaN
    /// to 0).
    #[must_use]
    pub fn cell(self) -> CellPt {
        CellPt::new(round_cell(self.x), round_cell(self.y))
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the float-to-int cast saturates, and NaN becomes 0"
)]
fn round_cell(v: f64) -> i32 {
    v.round() as i32
}

impl From<CellPt> for SubPt {
    fn from(cell: CellPt) -> Self {
        SubPt::new(f64::from(cell.x), f64::from(cell.y))
    }
}

/// A rectangle of cells: `width × height` cells from `(x, y)`. Empty when either side is zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CellBox {
    /// The left column.
    pub x: i32,
    /// The top row.
    pub y: i32,
    /// Columns covered.
    pub width: u32,
    /// Rows covered.
    pub height: u32,
}

impl CellBox {
    /// The box of `width × height` cells from `(x, y)`.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        CellBox {
            x,
            y,
            width,
            height,
        }
    }

    /// The smallest box holding every cell in `cells`, or `None` for none.
    pub fn around(cells: impl IntoIterator<Item = CellPt>) -> Option<CellBox> {
        let mut cells = cells.into_iter();
        let first = cells.next()?;
        let (mut lo, mut hi) = (first, first);
        for cell in cells {
            lo = CellPt::new(lo.x.min(cell.x), lo.y.min(cell.y));
            hi = CellPt::new(hi.x.max(cell.x), hi.y.max(cell.y));
        }
        Some(CellBox::spanning(lo, hi))
    }

    /// The box from cell `lo` to cell `hi`, both inclusive (`lo ≤ hi` on both axes).
    fn spanning(lo: CellPt, hi: CellPt) -> CellBox {
        let side =
            |lo: i32, hi: i32| u32::try_from(i64::from(hi) - i64::from(lo) + 1).unwrap_or(u32::MAX);
        CellBox::new(lo.x, lo.y, side(lo.x, hi.x), side(lo.y, hi.y))
    }

    /// Whether the box covers no cell.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Cells covered.
    #[must_use]
    pub const fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// One past the right column, exact in `i64`.
    #[must_use]
    pub fn right(&self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    /// One past the bottom row, exact in `i64`.
    #[must_use]
    pub fn bottom(&self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    /// Whether `cell` lies in the box.
    #[must_use]
    pub fn contains(&self, cell: CellPt) -> bool {
        i64::from(cell.x) >= i64::from(self.x)
            && i64::from(cell.x) < self.right()
            && i64::from(cell.y) >= i64::from(self.y)
            && i64::from(cell.y) < self.bottom()
    }
}

#[cfg(test)]
#[path = "cell_tests.rs"]
mod tests;
