//! The [`Legend`] widget (plan §3), behind `widget-extras`: rows of a colored swatch and a
//! label, which the host supplies, since only it knows what its colors mean.

use catena::raster::{CellStyle, PaletteColor, Surface, text_cells};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::Widget;

use crate::BufferSurface;

/// The labels' color.
const TEXT: PaletteColor = PaletteColor::Ansi(7);

/// One row a label, after a swatch in its color: `Legend::new(&[("person", red)])`. Rows past
/// the area's height are not drawn, and a label is cut at the area's edge, never through a wide
/// glyph. It clears its area first, so it can float over a graph.
#[derive(Debug, Clone, Copy)]
pub struct Legend<'a> {
    rows: &'a [(&'a str, PaletteColor)],
    swatch: char,
}

impl<'a> Legend<'a> {
    /// A legend of `rows`, each swatch a `■`.
    #[must_use]
    pub const fn new(rows: &'a [(&'a str, PaletteColor)]) -> Self {
        Legend {
            rows, swatch: '■'
        }
    }

    /// Draws each swatch as `swatch` instead: `#`, say, for a plain ASCII terminal.
    #[must_use]
    pub const fn swatch(mut self, swatch: char) -> Self {
        self.swatch = swatch;
        self
    }
}

impl Widget for Legend<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        crate::widget::clear(buf, area);
        let mut surface = BufferSurface::new(buf, area);
        let (width, height) = surface.size();
        for (y, &(label, color)) in (0..height).zip(self.rows) {
            surface.put_char(0, y, self.swatch, CellStyle::new(color));
            let mut x = 2u16;
            for cell in text_cells(label) {
                if u32::from(x) + u32::from(cell.width) > u32::from(width) {
                    break;
                }
                surface.put(x, y, cell.symbol, CellStyle::new(TEXT));
                x += cell.width;
            }
        }
    }
}

#[cfg(test)]
#[path = "legend_tests.rs"]
mod tests;
