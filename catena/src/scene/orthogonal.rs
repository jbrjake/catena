//! `Orthogonal` routes (plan §7.4): runs of whole cells drawn as box-drawing glyphs whatever the
//! blitter. Each cell records the arms (north, east, south, west) the routes through it leave
//! by, so runs meeting in a cell merge into the corner, tee or cross they form.

use super::StyleId;
use crate::geometry::cell::CellPt;

/// The arm toward the row above.
pub(crate) const N: u8 = 0b0001;
/// The arm toward the next column.
pub(crate) const E: u8 = 0b0010;
/// The arm toward the row below.
pub(crate) const S: u8 = 0b0100;
/// The arm toward the previous column.
pub(crate) const W: u8 = 0b1000;

/// The glyphs `Orthogonal` routes and boxed nodes' borders are drawn with, indexed by a cell's
/// arms. Every glyph is a `char` of width 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoxGlyphs {
    /// Glyph for each arm set, bit 0 north, 1 east, 2 south, 3 west. A cell with one arm draws
    /// the full line along it.
    pub by_arms: [char; 16],
}

impl BoxGlyphs {
    /// Unicode light box drawing.
    pub const BOX: BoxGlyphs = BoxGlyphs {
        //        ∅    N    E    NE   S    NS   ES   NES  W    NW   EW   NEW  SW   NSW  ESW  NESW
        by_arms: [
            ' ', '│', '─', '└', '│', '│', '┌', '├', '─', '┘', '─', '┴', '┐', '┤', '┬', '┼',
        ],
    };

    /// Plain ASCII: lines and a `+` wherever runs turn or meet.
    pub const ASCII: BoxGlyphs = BoxGlyphs {
        by_arms: [
            ' ', '|', '-', '+', '|', '|', '+', '+', '-', '+', '-', '+', '+', '+', '+', '+',
        ],
    };

    /// The glyph for `arms`.
    #[must_use]
    pub fn glyph(&self, arms: u8) -> char {
        self.by_arms[usize::from(arms & 0x0f)]
    }
}

impl Default for BoxGlyphs {
    fn default() -> Self {
        BoxGlyphs::BOX
    }
}

/// The arms and the style of every cell `Orthogonal` routes pass through, over a
/// `width × height` grid; the last route through a cell styles it.
#[derive(Debug, Default)]
pub(crate) struct Arms {
    pub(super) width: u16,
    pub(super) height: u16,
    /// Per cell, its arm bits.
    masks: Vec<u8>,
    styles: Vec<StyleId>,
}

impl Arms {
    /// Empties the grid and sizes it, reusing its allocations.
    pub(crate) fn reset(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        let cells = usize::from(width) * usize::from(height);
        self.masks.clear();
        self.masks.resize(cells, 0);
        self.styles.clear();
        self.styles.resize(cells, StyleId::default());
    }

    /// The arms of cell `(x, y)`, or `None` when no route passes through it.
    pub(crate) fn arms(&self, x: u16, y: u16) -> Option<u8> {
        let arms = *self.masks.get(self.index(x, y)?)?;
        (arms != 0).then_some(arms)
    }

    /// The style of the last route through cell `(x, y)`.
    pub(crate) fn style(&self, x: u16, y: u16) -> Option<StyleId> {
        self.arms(x, y)?;
        self.styles.get(self.index(x, y)?).copied()
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        (x < self.width && y < self.height)
            .then(|| usize::from(y) * usize::from(self.width) + usize::from(x))
    }

    /// Adds a route: a run between each pair of consecutive cells, a pair on neither the same
    /// row nor the same column bending through the cell level with the first and in line with
    /// the second (horizontal first). Only the cells on the grid are visited.
    pub(crate) fn add_route(&mut self, cells: &[CellPt], style: StyleId) {
        for pair in cells.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let bend = CellPt::new(to.x, from.y);
            self.add_run(from, bend, style);
            self.add_run(bend, to, style);
        }
    }

    /// Adds the arms of a straight run from `from` to `to` (on one row or one column): each cell
    /// gets the arms toward its neighbours along the run.
    fn add_run(&mut self, from: CellPt, to: CellPt, style: StyleId) {
        if from == to {
            return;
        }
        let horizontal = from.y == to.y;
        let (fixed, a, b) = if horizontal {
            (from.y, from.x, to.x)
        } else {
            (from.x, from.y, to.y)
        };
        let (lo, hi) = (a.min(b), a.max(b));
        let (lo_arm, hi_arm) = if horizontal { (W, E) } else { (N, S) };
        let limit = i64::from(if horizontal { self.width } else { self.height }) - 1;
        let (first, last) = (i64::from(lo).max(0), i64::from(hi).min(limit));
        for along in first..=last {
            let mut arms = 0;
            if along > i64::from(lo) {
                arms |= lo_arm;
            }
            if along < i64::from(hi) {
                arms |= hi_arm;
            }
            let (x, y) = if horizontal {
                (along, i64::from(fixed))
            } else {
                (i64::from(fixed), along)
            };
            if let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y))
                && let Some(i) = self.index(x, y)
            {
                self.masks[i] |= arms;
                self.styles[i] = style;
            }
        }
    }
}

#[cfg(test)]
#[path = "orthogonal_tests.rs"]
mod tests;
