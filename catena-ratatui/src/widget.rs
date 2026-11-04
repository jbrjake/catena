//! The [`Graph`] widget and the [`BufferSurface`] it draws through (plan §3, §7.1).

use std::fmt;
use std::marker::PhantomData;

use catena::GraphView;
use catena::graph::Key;
use catena::raster::{Attrs, CellStyle, PaletteColor, Surface, text_cells};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier};
use ratatui_core::widgets::StatefulWidget;

/// Draws a [`GraphView`] in an area, as its state:
/// `frame.render_stateful_widget(Graph::new(), area, &mut gv)`. The area's size is the view's
/// (a new size lays the graph out for it), and its corner is where the view reads the pointer
/// from.
pub struct Graph<K> {
    key: PhantomData<fn() -> K>,
}

impl<K> Graph<K> {
    /// The widget.
    #[must_use]
    pub const fn new() -> Self {
        Graph { key: PhantomData }
    }
}

impl<K> Default for Graph<K> {
    fn default() -> Self {
        Graph::new()
    }
}

impl<K> Clone for Graph<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for Graph<K> {}

impl<K> fmt::Debug for Graph<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Graph")
    }
}

impl<K: Key> StatefulWidget for Graph<K> {
    type State = GraphView<K>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut GraphView<K>) {
        state.render(&mut BufferSurface::new(buf, area));
    }
}

/// An area of a ratatui [`Buffer`] as a catena [`Surface`].
pub struct BufferSurface<'b> {
    buf: &'b mut Buffer,
    area: Rect,
}

impl<'b> BufferSurface<'b> {
    /// The part of `area` inside `buf`.
    pub fn new(buf: &'b mut Buffer, area: Rect) -> Self {
        let area = area.intersection(buf.area);
        BufferSurface { buf, area }
    }
}

impl fmt::Debug for BufferSurface<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufferSurface")
            .field("area", &self.area)
            .finish_non_exhaustive()
    }
}

impl Surface for BufferSurface<'_> {
    fn size(&self) -> (u16, u16) {
        (self.area.width, self.area.height)
    }

    fn origin(&self) -> (u16, u16) {
        (self.area.x, self.area.y)
    }

    fn put(&mut self, x: u16, y: u16, symbol: &str, style: CellStyle) {
        let Some(text) = text_cells(symbol).next() else {
            return;
        };
        let Some(at) = self.at(x, y) else {
            return;
        };
        let wide = text.width == 2;
        let trailing = if wide {
            let Some(next) = self.at(x + 1, y) else {
                return;
            };
            Some(next)
        } else {
            None
        };
        self.release(x, y);
        if wide {
            self.release(x + 1, y);
        }
        let cell = &mut self.buf[at];
        cell.set_symbol(text.symbol);
        cell.fg = color(style.fg);
        if let Some(bg) = style.bg {
            cell.bg = color(bg);
        }
        cell.modifier = modifier(style.attrs);
        let lead = cell.clone();
        if let Some(next) = trailing {
            // The terminal draws the glyph over this cell, and ratatui's diff skips it; it
            // carries the glyph's style, as `CellGrid`'s continuation does.
            let cell = &mut self.buf[next];
            *cell = lead;
            cell.set_symbol(" ");
        }
    }

    fn patch_bg(&mut self, x: u16, y: u16, bg: PaletteColor) {
        let Some(at) = self.at(x, y) else {
            return;
        };
        let bg = color(bg);
        self.buf[at].bg = bg;
        if let Some(other) = self.partner(x, y) {
            self.buf[other].bg = bg;
        }
    }
}

impl BufferSurface<'_> {
    /// The buffer position of area cell `(x, y)`; `None` outside the area.
    fn at(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        (x < self.area.width && y < self.area.height).then(|| (self.area.x + x, self.area.y + y))
    }

    /// Whether area cell `(x, y)` holds a width-2 glyph.
    fn is_wide(&self, x: u16, y: u16) -> bool {
        self.at(x, y).is_some_and(|at| {
            text_cells(self.buf[at].symbol())
                .next()
                .is_some_and(|text| text.width == 2)
        })
    }

    /// The buffer position of the other half of the width-2 glyph area cell `(x, y)` is half
    /// of: its trailing cell, or the glyph before it. A `Buffer` keeps no spans, so the glyphs'
    /// widths say; only cells of the area count.
    fn partner(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        if self.is_wide(x, y) {
            self.at(x + 1, y)
        } else if x > 0 && self.is_wide(x - 1, y) {
            self.at(x - 1, y)
        } else {
            None
        }
    }

    /// Blanks the other half of any width-2 glyph area cell `(x, y)` is half of, before it is
    /// overwritten, keeping that half's style (as `CellGrid` does).
    fn release(&mut self, x: u16, y: u16) {
        if let Some(other) = self.partner(x, y) {
            self.buf[other].set_symbol(" ");
        }
    }
}

/// A catena color as ratatui's: the 16 ANSI colors by ratatui's names (the low four bits, as
/// catena reads them), the rest as they are.
pub(crate) fn color(color: PaletteColor) -> Color {
    const ANSI: [Color; 16] = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];
    match color {
        PaletteColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        PaletteColor::Ansi(n) => ANSI[usize::from(n & 0x0f)],
        PaletteColor::Indexed(n) => Color::Indexed(n),
    }
}

/// Catena's attributes as ratatui's modifier: the same bits (decision M1).
pub(crate) fn modifier(attrs: Attrs) -> Modifier {
    Modifier::from_bits_truncate(u16::from(attrs.bits()))
}

#[cfg(test)]
#[path = "widget_tests.rs"]
mod tests;
