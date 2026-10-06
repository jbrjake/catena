//! Drawing a node's measured form (plan §5, §7.4): what `ResolvedMetrics` measured, cell for
//! cell, so the drawn box is the measured one (invariant N).

use super::orthogonal::{BoxGlyphs, E, N, S, W};
use crate::geometry::cell::CellBox;
use crate::geometry::{FormShape, Mark, NodeForm};
use crate::raster::text::display_width;
use crate::raster::{CellStyle, Surface};

/// The glyphs node forms are drawn with. Each is a `char` of width 1, which is what node
/// measurement counts it as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeGlyphs {
    /// Opens a label: `[`.
    pub open: char,
    /// Closes a label: `]`.
    pub close: char,
    /// Ends a cut label: `…`.
    pub ellipsis: char,
    /// Marks a pinned node: `*`.
    pub pin: char,
    /// Stands for a node that has no glyph of its own to show: `•`.
    pub dot: char,
}

impl NodeGlyphs {
    /// Unicode: `[label…]`, `*` and `•`.
    pub const UNICODE: NodeGlyphs = NodeGlyphs {
        open: '[',
        close: ']',
        ellipsis: '…',
        pin: '*',
        dot: '•',
    };

    /// Plain ASCII: `[label~]`, `*` and `o`.
    pub const ASCII: NodeGlyphs = NodeGlyphs {
        open: '[',
        close: ']',
        ellipsis: '~',
        pin: '*',
        dot: 'o',
    };
}

impl Default for NodeGlyphs {
    fn default() -> Self {
        NodeGlyphs::UNICODE
    }
}

/// Draws `form` from the top left of `bounds`, keeping to `bounds` and to the surface.
pub(super) fn draw_form<S: Surface + ?Sized>(
    surface: &mut S,
    bounds: CellBox,
    form: &NodeForm,
    style: CellStyle,
    (glyphs, boxes): (&NodeGlyphs, &BoxGlyphs),
) {
    let mut pen = Pen {
        surface,
        bounds,
        style,
        x: i64::from(bounds.x),
        y: i64::from(bounds.y),
    };
    match form.shape() {
        FormShape::Glyph(mark) => pen.mark(mark, glyphs),
        FormShape::Row { pin, icon, label } => {
            if *pin {
                pen.glyph(glyphs.pin);
            }
            if let Some(icon) = icon {
                pen.mark(icon, glyphs);
            }
            if let Some(label) = label {
                pen.glyph(glyphs.open);
                pen.text(&label.text);
                if label.cut {
                    pen.glyph(glyphs.ellipsis);
                }
                pen.glyph(glyphs.close);
            }
        }
        FormShape::Boxed { pin, lines } => {
            let (width, height) = (i64::from(form.width()), i64::from(form.height()));
            let (left, top) = (pen.x, pen.y);
            for row in 0..height {
                let (start, middle, end) = match row {
                    0 => (E | S, E | W, S | W),
                    r if r == height - 1 => (N | E, E | W, N | W),
                    _ => (N | S, 0, N | S),
                };
                pen.x = left;
                pen.y = top + row;
                pen.glyph(boxes.glyph(start));
                for _ in 1..width - 1 {
                    pen.glyph(boxes.glyph(middle));
                }
                pen.glyph(boxes.glyph(end));
            }
            if *pin {
                (pen.x, pen.y) = (left + 1, top);
                pen.glyph(glyphs.pin);
            }
            let (inner_w, inner_h) = (width - 2, height - 2);
            let first = top + 1 + (inner_h - to_i64(lines.len())).max(0) / 2;
            for (row, line) in (first..).zip(lines) {
                pen.x = left + 1 + (inner_w - to_i64(display_width(line))).max(0) / 2;
                pen.y = row;
                pen.text(line);
            }
        }
    }
}

fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Writes left to right from `(x, y)`, dropping any cell outside `bounds` or the surface.
struct Pen<'s, S: ?Sized> {
    surface: &'s mut S,
    bounds: CellBox,
    style: CellStyle,
    x: i64,
    y: i64,
}

impl<S: Surface + ?Sized> Pen<'_, S> {
    fn glyph(&mut self, c: char) {
        self.text(c.encode_utf8(&mut [0; 4]));
    }

    fn mark(&mut self, mark: &Mark, glyphs: &NodeGlyphs) {
        match mark {
            Mark::Text(text) => self.text(text),
            Mark::Dot => self.glyph(glyphs.dot),
        }
    }

    /// Writes `text` cell by cell as `text_cells` splits it; a wide glyph that would cross the
    /// edge of the bounds or the surface is dropped, and the pen still moves past it.
    fn text(&mut self, text: &str) {
        for cell in crate::raster::text_cells(text) {
            let end = self.x + i64::from(cell.width);
            let inside = self.x >= i64::from(self.bounds.x)
                && end <= self.bounds.right()
                && self.y >= i64::from(self.bounds.y)
                && self.y < self.bounds.bottom();
            if inside && let (Ok(x), Ok(y)) = (u16::try_from(self.x), u16::try_from(self.y)) {
                self.surface.put(x, y, cell.symbol, self.style);
            }
            self.x = end;
        }
    }
}

#[cfg(test)]
#[path = "nodes_tests.rs"]
mod tests;
