//! [`CellGrid`], the core's in-memory [`Surface`] (plan §7.1). Every golden, visual and
//! oscillation test reads its frames back from one, and static export renders through one.

use std::fmt;

use super::ansi::{self, Pen};
use super::text::text_cells;
use super::{Attrs, CellStyle, PaletteColor, Surface};

/// The most cells a grid holds. A larger request keeps its width and loses rows from the
/// bottom. 2²² cells is 2048 × 2048, past any terminal; the ceiling exists because the
/// dimensions are the host's `u16`s, and 65 535² cells would not fit in memory.
pub const MAX_CELLS: usize = 1 << 22;

/// Bytes a cell's symbol holds. A glyph takes at most four; zero-width followers that do not
/// fit are dropped, which changes no width. A ceiling, because labels are untrusted input.
pub(crate) const SYMBOL_BYTES: usize = 15;

/// A cell's content, stored inline so a grid is one allocation and a frame allocates nothing.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Symbol {
    len: u8,
    bytes: [u8; SYMBOL_BYTES],
}

impl Symbol {
    const EMPTY: Symbol = Symbol {
        len: 0,
        bytes: [0; SYMBOL_BYTES],
    };

    const SPACE: Symbol = {
        let mut bytes = [0; SYMBOL_BYTES];
        bytes[0] = b' ';
        Symbol { len: 1, bytes }
    };

    /// The leading whole characters of `text` that fit.
    fn new(text: &str) -> Symbol {
        let mut symbol = Symbol::EMPTY;
        let mut len = 0;
        for c in text.chars() {
            let end = len + c.len_utf8();
            if end > SYMBOL_BYTES {
                break;
            }
            c.encode_utf8(&mut symbol.bytes[len..end]);
            len = end;
        }
        symbol.len = u8::try_from(len).expect("at most SYMBOL_BYTES");
        symbol
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .expect("a symbol holds whole characters copied from a str")
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

/// Which part of a glyph a cell holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Span {
    /// A whole glyph of width 1, or a blank.
    Narrow,
    /// The first cell of a width-2 glyph.
    Lead,
    /// The second cell of a width-2 glyph, drawn by the glyph in the cell before it.
    Continuation,
}

/// One cell of a [`CellGrid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cell {
    symbol: Symbol,
    fg: Option<PaletteColor>,
    bg: Option<PaletteColor>,
    attrs: Attrs,
    span: Span,
}

impl Cell {
    const BLANK: Cell = Cell {
        symbol: Symbol::SPACE,
        fg: None,
        bg: None,
        attrs: Attrs::NONE,
        span: Span::Narrow,
    };

    /// The cell's content: a glyph and its zero-width followers, `" "` when blank, and `""` in
    /// the continuation cell of a width-2 glyph.
    #[must_use]
    pub fn symbol(&self) -> &str {
        self.symbol.as_str()
    }

    /// The glyph's color; `None`, the terminal's default, until something is written here.
    #[must_use]
    pub fn fg(&self) -> Option<PaletteColor> {
        self.fg
    }

    /// The background; `None` is the terminal's default.
    #[must_use]
    pub fn bg(&self) -> Option<PaletteColor> {
        self.bg
    }

    /// The text attributes.
    #[must_use]
    pub fn attrs(&self) -> Attrs {
        self.attrs
    }

    /// Columns the cell's glyph covers from here: 1, or 2 for a width-2 glyph, and 0 in the
    /// continuation cell after one.
    #[must_use]
    pub fn width(&self) -> u16 {
        match self.span {
            Span::Narrow => 1,
            Span::Lead => 2,
            Span::Continuation => 0,
        }
    }

    /// Whether this is the second cell of a width-2 glyph. It carries the glyph's style and an
    /// empty symbol.
    #[must_use]
    pub fn is_continuation(&self) -> bool {
        self.span == Span::Continuation
    }

    /// A space, in text: styles do not show there.
    fn is_blank_text(&self) -> bool {
        self.span == Span::Narrow && self.symbol == Symbol::SPACE
    }

    /// A space with no background showing.
    fn is_blank_on_screen(&self) -> bool {
        self.is_blank_text() && self.bg.is_none() && !self.attrs.contains(Attrs::REVERSED)
    }

    /// This cell with its glyph gone and its style kept: what remains of one half of a width-2
    /// glyph when the other half is overwritten.
    fn blanked(self) -> Cell {
        Cell {
            symbol: Symbol::SPACE,
            span: Span::Narrow,
            ..self
        }
    }
}

/// A [`Surface`] in memory that can be read back: [`CellGrid::cell`], `to_string` (the glyphs,
/// for text goldens) and [`CellGrid::to_ansi_string`] (with colors, for a terminal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellGrid {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl CellGrid {
    /// A blank grid of `width × height` cells, bounded by [`MAX_CELLS`].
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let mut grid = CellGrid {
            width: 0,
            height: 0,
            cells: Vec::new(),
        };
        grid.resize(width, height);
        grid
    }

    /// Makes the grid `width × height` blank cells, bounded by [`MAX_CELLS`], reusing its
    /// allocation.
    pub fn resize(&mut self, width: u16, height: u16) {
        let max_rows = MAX_CELLS / usize::from(width.max(1));
        self.width = width;
        self.height = height.min(u16::try_from(max_rows).unwrap_or(u16::MAX));
        self.cells.clear();
        self.cells.resize(
            usize::from(self.width) * usize::from(self.height),
            Cell::BLANK,
        );
    }

    /// Blanks every cell.
    pub fn clear(&mut self) {
        self.cells.fill(Cell::BLANK);
    }

    /// The cell at `(x, y)`, or `None` outside the grid.
    #[must_use]
    pub fn cell(&self, x: u16, y: u16) -> Option<&Cell> {
        self.index(x, y).map(|i| &self.cells[i])
    }

    /// The rows, top to bottom, each its cells left to right.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &[Cell]> {
        (0..self.height).map(|y| self.row(y))
    }

    /// The grid as text with SGR escapes: one line per row, a sequence wherever the colors or
    /// attributes change, a reset at the end of any row left styled, and trailing cells that
    /// show nothing (a space with no background) trimmed.
    #[must_use]
    pub fn to_ansi_string(&self) -> String {
        let mut out = String::new();
        for y in 0..self.height {
            if y > 0 {
                out.push('\n');
            }
            let row = self.row(y);
            let end = row
                .iter()
                .rposition(|cell| !cell.is_blank_on_screen())
                .map_or(0, |last| last + 1);
            let mut pen = Pen::DEFAULT;
            for cell in row[..end].iter().filter(|cell| !cell.is_continuation()) {
                let next = Pen {
                    fg: cell.fg,
                    bg: cell.bg,
                    attrs: cell.attrs,
                };
                if next != pen {
                    next.write_sgr(&mut out);
                    pen = next;
                }
                out.push_str(cell.symbol());
            }
            if pen != Pen::DEFAULT {
                out.push_str(ansi::RESET);
            }
        }
        out
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        (x < self.width && y < self.height)
            .then(|| usize::from(y) * usize::from(self.width) + usize::from(x))
    }

    fn row(&self, y: u16) -> &[Cell] {
        let width = usize::from(self.width);
        let start = usize::from(y) * width;
        &self.cells[start..start + width]
    }

    /// The index of the other half of the width-2 glyph cell `i` is half of, if it is.
    fn partner(&self, i: usize) -> Option<usize> {
        match self.cells[i].span {
            Span::Narrow => None,
            Span::Lead => Some(i + 1),
            Span::Continuation => i.checked_sub(1),
        }
        .filter(|&j| j < self.cells.len())
    }

    /// Blanks the other half of any width-2 glyph cell `i` is half of, before `i` is
    /// overwritten, so no glyph is left with one half.
    fn release(&mut self, i: usize) {
        if let Some(j) = self.partner(i) {
            self.cells[j] = self.cells[j].blanked();
        }
    }
}

impl Surface for CellGrid {
    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    fn put(&mut self, x: u16, y: u16, symbol: &str, style: CellStyle) {
        let Some(text) = text_cells(symbol).next() else {
            return;
        };
        let Some(i) = self.index(x, y) else {
            return;
        };
        let wide = text.width == 2;
        if wide && u32::from(x) + 1 >= u32::from(self.width) {
            return;
        }
        self.release(i);
        if wide {
            self.release(i + 1);
        }
        let cell = Cell {
            symbol: Symbol::new(text.symbol),
            fg: Some(style.fg),
            bg: style.bg.or(self.cells[i].bg),
            attrs: style.attrs,
            span: if wide { Span::Lead } else { Span::Narrow },
        };
        self.cells[i] = cell;
        if wide {
            self.cells[i + 1] = Cell {
                symbol: Symbol::EMPTY,
                span: Span::Continuation,
                ..cell
            };
        }
    }

    fn patch_bg(&mut self, x: u16, y: u16, bg: PaletteColor) {
        let Some(i) = self.index(x, y) else {
            return;
        };
        self.cells[i].bg = Some(bg);
        if let Some(j) = self.partner(i) {
            self.cells[j].bg = Some(bg);
        }
    }
}

impl fmt::Display for CellGrid {
    /// The glyphs, one line per row with trailing spaces trimmed: the form of the text goldens
    /// (plan §16.3). A width-2 glyph prints once, so every line is at most the grid's width in
    /// display columns.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for y in 0..self.height {
            if y > 0 {
                f.write_str("\n")?;
            }
            let row = self.row(y);
            let end = row
                .iter()
                .rposition(|cell| !cell.is_blank_text())
                .map_or(0, |last| last + 1);
            for cell in &row[..end] {
                f.write_str(cell.symbol())?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
