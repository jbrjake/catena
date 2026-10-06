//! The surface `catena` draws on (plan §7.1): a write-only grid of terminal cells, and the colors
//! and attributes a cell carries. Backends adapt it to their own buffer; [`CellGrid`] is the
//! core's in-memory implementation.

use std::ops::{BitOr, BitOrAssign};

#[cfg(doc)]
use super::{CellGrid, text_cells};

/// A color as the terminal sees it. A backend's adapter resolves it to its own color type; the
/// core never names one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteColor {
    /// A 24-bit color.
    Rgb(u8, u8, u8),
    /// One of the 16 ANSI colors, 0–7 normal and 8–15 bright. Only the low four bits are read,
    /// so an `Ansi` color never needs more than a 16-color terminal.
    Ansi(u8),
    /// An entry of the 256-color palette.
    Indexed(u8),
}

/// Text attributes, a set of flags. The bit values are ratatui's `Modifier` bits, so a cell's
/// attributes hash the same whichever side of the adapter they are read on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Attrs(u8);

impl Attrs {
    /// No attributes.
    pub const NONE: Attrs = Attrs(0);
    /// Bold, or increased intensity.
    pub const BOLD: Attrs = Attrs(0x01);
    /// Faint, or decreased intensity.
    pub const DIM: Attrs = Attrs(0x02);
    /// Italic.
    pub const ITALIC: Attrs = Attrs(0x04);
    /// Foreground and background swapped.
    pub const REVERSED: Attrs = Attrs(0x40);

    /// The flags as bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Whether every flag in `other` is set in `self`.
    #[must_use]
    pub const fn contains(self, other: Attrs) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether no flag is set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The flags set in either.
    #[must_use]
    pub const fn union(self, other: Attrs) -> Attrs {
        Attrs(self.0 | other.0)
    }
}

impl BitOr for Attrs {
    type Output = Attrs;

    fn bitor(self, other: Attrs) -> Attrs {
        self.union(other)
    }
}

impl BitOrAssign for Attrs {
    fn bitor_assign(&mut self, other: Attrs) {
        *self = self.union(other);
    }
}

/// How one cell is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellStyle {
    /// The glyph's color.
    pub fg: PaletteColor,
    /// The cell's background. `None` keeps whatever background the cell already has, so a glyph
    /// drawn after a glow halo sits on the halo (plan §7.4).
    pub bg: Option<PaletteColor>,
    /// Text attributes; a write replaces the cell's attributes with these.
    pub attrs: Attrs,
}

impl CellStyle {
    /// A style with foreground `fg`, no background of its own and no attributes.
    #[must_use]
    pub const fn new(fg: PaletteColor) -> Self {
        CellStyle {
            fg,
            bg: None,
            attrs: Attrs::NONE,
        }
    }
}

/// The only thing `catena` writes to. The widget crate adapts it to a ratatui `Buffer`;
/// [`CellGrid`] implements it in memory and can be read back.
///
/// Every method is total: a write outside the surface is ignored, never an error.
pub trait Surface {
    /// Width and height in cells.
    fn size(&self) -> (u16, u16);

    /// Writes one cell. `symbol` is one terminal cell's content: a glyph of display width 1 or 2
    /// plus any zero-width characters after it, as [`text_cells`] splits text (plan §7.1). Only
    /// its first cell is written; a symbol with none (empty, or only zero-width or control
    /// characters) writes nothing.
    ///
    /// A width-2 symbol owns `(x, y)` and `(x + 1, y)`, the second cell marked as a
    /// continuation; one whose second cell would fall outside the surface is not written.
    /// Writing over either half of a width-2 symbol blanks its other half. A style without a
    /// background keeps the cell's background (a width-2 symbol's two cells share the first's).
    fn put(&mut self, x: u16, y: u16, symbol: &str, style: CellStyle);

    /// Sets the background of the cell at `(x, y)`, keeping its glyph, foreground and
    /// attributes: how glow halos compose under edges (plan §7.4). Both cells of a width-2
    /// symbol share one background, so patching either half patches both.
    fn patch_bg(&mut self, x: u16, y: u16, bg: PaletteColor);

    /// Writes `glyph` as a one-character symbol; see [`Surface::put`].
    fn put_char(&mut self, x: u16, y: u16, glyph: char, style: CellStyle) {
        let mut utf8 = [0; 4];
        self.put(x, y, glyph.encode_utf8(&mut utf8), style);
    }
}
