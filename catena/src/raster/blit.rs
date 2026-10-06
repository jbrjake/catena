//! Blitters: how a sub-cell canvas becomes one glyph per terminal cell (plan §7.2).
//!
//! A canvas stores, per cell, a mask of lit sub-pixels in its blitter's bit layout and the color
//! slot of the last pen to touch the cell. The blitter only encodes: braille and sextant masks
//! are table lookups, half blocks pick a glyph and two colors, and the ascii blitter's "pixels"
//! are line directions, so lines crossing in one canvas merge into a junction glyph.

use super::{CellStyle, Surface};

#[cfg(doc)]
use super::SubCellCanvas;

/// How a canvas's sub-pixels become glyphs. Braille is the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Blitter {
    /// 2×4 dots per cell, one color: the highest resolution, Unicode braille.
    #[default]
    Braille,
    /// 1×2 per cell, a color per half: color fidelity over resolution.
    HalfBlock,
    /// 2×3 per cell, one color: Unicode 13 sextants, which some fonts draw better than braille.
    Sextant,
    /// One cell per pixel, drawn as a line glyph in the line's direction, `─│╱╲`, with junctions
    /// where lines cross: the universal fallback, and the readable renderer of text goldens.
    Ascii,
}

/// An index into the styles a canvas is blitted with: the scene's style of whatever drew there.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ColorSlot(pub u16);

/// The glyphs the ascii blitter draws lines with. Every glyph is a `char` of width 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineGlyphs {
    /// A horizontal line.
    pub horizontal: char,
    /// A vertical line.
    pub vertical: char,
    /// A line rising to the right.
    pub rising: char,
    /// A line falling to the right.
    pub falling: char,
    /// Lines crossing, unless both are diagonals.
    pub cross: char,
    /// Two diagonals crossing.
    pub diagonal_cross: char,
    /// A point with no direction.
    pub dot: char,
}

impl LineGlyphs {
    /// Unicode box drawing.
    pub const BOX: LineGlyphs = LineGlyphs {
        horizontal: '─',
        vertical: '│',
        rising: '╱',
        falling: '╲',
        cross: '┼',
        diagonal_cross: '╳',
        dot: '·',
    };

    /// Plain ASCII, for terminals that draw nothing else.
    pub const ASCII: LineGlyphs = LineGlyphs {
        horizontal: '-',
        vertical: '|',
        rising: '/',
        falling: '\\',
        cross: '+',
        diagonal_cross: 'X',
        dot: '.',
    };
}

impl Default for LineGlyphs {
    fn default() -> Self {
        LineGlyphs::BOX
    }
}

/// The ascii blitter's direction bits, which take the place of sub-pixel bits in its masks.
pub(crate) mod ascii_bits {
    /// Horizontal, within 22.5° of level as the cell shape shows it.
    pub(crate) const H: u8 = 0x01;
    /// Vertical, within 22.5° of upright.
    pub(crate) const V: u8 = 0x02;
    /// Rising to the right.
    pub(crate) const RISING: u8 = 0x04;
    /// Falling to the right (y grows downward).
    pub(crate) const FALLING: u8 = 0x08;
    /// A point with no direction.
    pub(crate) const DOT: u8 = 0x10;
}

/// tan 22.5° and tan 67.5°, the bounds between a level, a diagonal and an upright line.
const TAN_22_5: f64 = std::f64::consts::SQRT_2 - 1.0;
const TAN_67_5: f64 = std::f64::consts::SQRT_2 + 1.0;

/// The direction bit of a line along `(dx, dy)` cells, quantized by the angle it shows on
/// screen: a cell is `cell_aspect` times as wide as it is tall (plan §6), so `(dx, dy)` looks
/// like `(dx · cell_aspect, dy)`. No length is a point.
pub(crate) fn line_direction(dx: f64, dy: f64, cell_aspect: f64) -> u8 {
    let (run, rise) = ((dx * cell_aspect).abs(), dy.abs());
    if run == 0.0 && rise == 0.0 {
        ascii_bits::DOT
    } else if rise <= run * TAN_22_5 {
        ascii_bits::H
    } else if rise >= run * TAN_67_5 {
        ascii_bits::V
    } else if (dx > 0.0) == (dy > 0.0) {
        ascii_bits::FALLING
    } else {
        ascii_bits::RISING
    }
}

/// Bit values of the braille dots by `[row][column]`, Unicode's numbering: dots 1–3 and 7 down
/// the left column, 4–6 and 8 down the right, dot `n` bit `n − 1`.
pub(crate) const BRAILLE_MAP: [[u8; 2]; 4] =
    [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

/// The empty braille pattern; a mask's glyph is this plus the mask.
pub(crate) const BRAILLE_BASE: u32 = 0x2800;

/// Each braille mask's glyph: U+2800 plus the mask.
const BRAILLE: [char; 256] = {
    let mut glyphs = ['\u{2800}'; 256];
    let mut mask = 0u32;
    while mask < 256 {
        glyphs[mask as usize] = match char::from_u32(BRAILLE_BASE + mask) {
            Some(glyph) => glyph,
            None => panic!("U+2800..U+28FF are braille patterns"),
        };
        mask += 1;
    }
    glyphs
};

/// Each sextant mask's glyph, bit `k` lighting sextant `k + 1` (1 top-left, 2 top-right, then
/// down the rows). Unicode 13 encodes 60 patterns from U+1FB00 in increasing order; the empty
/// pattern, the two half columns and the full cell already existed as a space and block
/// elements.
const SEXTANT: [char; 64] = {
    let mut glyphs = [' '; 64];
    let mut mask = 1u32;
    while mask < 64 {
        glyphs[mask as usize] = match mask {
            0b01_0101 => '▌',
            0b10_1010 => '▐',
            0b11_1111 => '█',
            _ => {
                let skipped = (mask > 0b01_0101) as u32 + (mask > 0b10_1010) as u32;
                match char::from_u32(0x1FB00 + mask - 1 - skipped) {
                    Some(glyph) => glyph,
                    None => panic!("U+1FB00..U+1FB3B are sextants"),
                }
            }
        };
        mask += 1;
    }
    glyphs
};

impl Blitter {
    /// Sub-pixels per cell, `(columns, rows)`.
    #[must_use]
    pub const fn sub_size(self) -> (u8, u8) {
        match self {
            Blitter::Braille => (2, 4),
            Blitter::HalfBlock => (1, 2),
            Blitter::Sextant => (2, 3),
            Blitter::Ascii => (1, 1),
        }
    }

    /// The mask bit of sub-pixel `(sx, sy)` within its cell. The ascii blitter has none: its
    /// bits are directions.
    pub(crate) fn bit(self, sx: usize, sy: usize) -> u8 {
        match self {
            Blitter::Braille => BRAILLE_MAP[sy][sx],
            Blitter::HalfBlock | Blitter::Sextant => {
                1 << (sy * usize::from(self.sub_size().0) + sx)
            }
            Blitter::Ascii => ascii_bits::DOT,
        }
    }

    /// Color slots per cell: one, or one per half for half blocks.
    pub(crate) fn color_slots(self) -> usize {
        match self {
            Blitter::HalfBlock => 2,
            _ => 1,
        }
    }

    /// The glyph for a cell's mask, ignoring color. An empty mask is the encoding's blank: U+2800
    /// for braille, a space otherwise. Half blocks with both halves lit draw the upper half.
    #[must_use]
    pub fn glyph(self, mask: u8, lines: &LineGlyphs) -> char {
        match self {
            Blitter::Braille => BRAILLE[usize::from(mask)],
            Blitter::Sextant => SEXTANT[usize::from(mask & 0b11_1111)],
            Blitter::HalfBlock => match mask & 0b11 {
                0 => ' ',
                0b10 => '▄',
                _ => '▀',
            },
            Blitter::Ascii => ascii_glyph(mask, lines),
        }
    }
}

fn ascii_glyph(mask: u8, lines: &LineGlyphs) -> char {
    use ascii_bits::{DOT, FALLING, H, RISING, V};
    match mask & (H | V | RISING | FALLING) {
        0 if mask & DOT != 0 => lines.dot,
        0 => ' ',
        H => lines.horizontal,
        V => lines.vertical,
        RISING => lines.rising,
        FALLING => lines.falling,
        both if both == RISING | FALLING => lines.diagonal_cross,
        _ => lines.cross,
    }
}

/// Writes every lit cell of a canvas to `surface`: the glyph its mask encodes, in the style
/// `style` gives its color slot. A half-block cell with both halves lit in different styles
/// draws the upper half in the upper style's foreground over the lower style's foreground as
/// background. Unlit cells are not written.
pub(crate) fn blit<S: Surface + ?Sized>(
    blitter: Blitter,
    lines: &LineGlyphs,
    (cols, masks, slots, masked): (u16, &[u8], &[ColorSlot], &[bool]),
    surface: &mut S,
    style: impl Fn(ColorSlot) -> CellStyle,
) {
    if cols == 0 {
        return;
    }
    let per_cell = blitter.color_slots();
    for (i, &mask) in masks.iter().enumerate() {
        if mask == 0 || masked.get(i).copied().unwrap_or(false) {
            continue;
        }
        let (Ok(x), Ok(y)) = (
            u16::try_from(i % usize::from(cols)),
            u16::try_from(i / usize::from(cols)),
        ) else {
            continue;
        };
        let slot = |k: usize| slots.get(i * per_cell + k).copied().unwrap_or_default();
        if blitter == Blitter::HalfBlock && mask & 0b11 == 0b11 {
            let (upper, lower) = (style(slot(0)), style(slot(1)));
            if upper.fg == lower.fg {
                surface.put_char(x, y, '█', upper);
            } else {
                let split = CellStyle {
                    bg: Some(lower.fg),
                    ..upper
                };
                surface.put_char(x, y, '▀', split);
            }
            continue;
        }
        let lit_half = usize::from(blitter == Blitter::HalfBlock && mask & 0b01 == 0);
        surface.put_char(x, y, blitter.glyph(mask, lines), style(slot(lit_half)));
    }
}

#[cfg(test)]
#[path = "blit_tests.rs"]
mod tests;
