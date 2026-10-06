//! The sub-cell canvas: pixels finer than a terminal cell, drawn by one set of primitives
//! whatever the blitter (plan §7.2).
//!
//! Each terminal cell holds the blitter's grid of sub-pixels: 2×4 braille dots, 2×3 sextants,
//! two half blocks, or for the ascii blitter one pixel that records the directions of the
//! lines through it. The canvas keeps a mask per cell in the blitter's bit layout and the color
//! slot of the last pen to touch the cell, one slot per half for half blocks; the blitter only
//! encodes masks as glyphs.
//!
//! Lines and curves are drawn as polylines (owner ruling A1): a line is two vertices, a curve is
//! its tessellation, and solid, dashed and hop-gapped drawing all walk the polyline's pixels.
//! Coordinates are signed sub-pixels so callers can pass off-screen endpoints; arithmetic runs in
//! `i64` so no input overflows, and pixels outside the canvas are silently clipped.

use super::blit::{self, Blitter, ColorSlot, LineGlyphs, line_direction};
use super::grid::MAX_CELLS;
use super::polyline::{Clip, PathPixel, Walk, dash, snap};
use super::{CellStyle, Surface};
use crate::geometry::curve::{Bezier, tessellate};

#[cfg(doc)]
use super::polyline::polyline_pixels;

/// A canvas of sub-cell pixels over `cols × rows` terminal cells, encoded by one [`Blitter`].
pub struct SubCellCanvas {
    blitter: Blitter,
    /// Width in terminal cells.
    cell_width: usize,
    /// Height in terminal cells.
    cell_height: usize,
    /// Per cell, the lit sub-pixels in the blitter's bit layout, indexed by
    /// `cell_y * cell_width + cell_x`.
    buffer: Vec<u8>,
    /// Per cell (per half, for half blocks), the slot of the last pen to light a pixel there.
    slots: Vec<ColorSlot>,
    /// The slot the next pixels are lit with.
    pen: ColorSlot,
    /// The ascii blitter's cell shape, width over height (plan §6).
    cell_aspect: f64,
    /// The ascii blitter's glyphs.
    lines: LineGlyphs,
    /// Reused by every curve for its tessellated polyline.
    points: Vec<(f64, f64)>,
    /// Reused by every dashed primitive for its vertices' cumulative arc lengths.
    arc: Vec<f64>,
}

/// The default cell shape, width over height: a cell about twice as tall as it is wide.
const DEFAULT_CELL_ASPECT: f64 = 0.5;

impl SubCellCanvas {
    /// A blank canvas over `cols × rows` terminal cells, bounded like a `CellGrid` by
    /// [`MAX_CELLS`](super::MAX_CELLS): a larger request keeps its width and loses rows.
    #[must_use]
    pub fn new(blitter: Blitter, cols: u16, rows: u16) -> Self {
        let mut canvas = Self {
            blitter,
            cell_width: 0,
            cell_height: 0,
            buffer: Vec::new(),
            slots: Vec::new(),
            pen: ColorSlot::default(),
            cell_aspect: DEFAULT_CELL_ASPECT,
            lines: LineGlyphs::BOX,
            points: Vec::new(),
            arc: Vec::new(),
        };
        canvas.clear_and_resize(cols, rows);
        canvas
    }

    /// The blitter whose bit layout the canvas stores.
    #[must_use]
    pub fn blitter(&self) -> Blitter {
        self.blitter
    }

    /// Pixel width: cells times the blitter's sub-pixel columns.
    #[must_use]
    pub fn pixel_width(&self) -> usize {
        self.cell_width * usize::from(self.blitter.sub_size().0)
    }

    /// Pixel height: cells times the blitter's sub-pixel rows.
    #[must_use]
    pub fn pixel_height(&self) -> usize {
        self.cell_height * usize::from(self.blitter.sub_size().1)
    }

    /// Lights the next pixels with `slot`: within one canvas, the last pen to touch a cell
    /// colors it.
    pub fn set_pen(&mut self, slot: ColorSlot) {
        self.pen = slot;
    }

    /// The ascii blitter's cell shape, width over height, by which it judges a line's direction
    /// as drawn (0.5 unless set). A value that is not positive and finite is ignored.
    pub fn set_cell_aspect(&mut self, cell_aspect: f64) {
        if cell_aspect.is_finite() && cell_aspect > 0.0 {
            self.cell_aspect = cell_aspect;
        }
    }

    /// The glyphs the ascii blitter draws lines with ([`LineGlyphs::BOX`] unless set).
    pub fn set_line_glyphs(&mut self, lines: LineGlyphs) {
        self.lines = lines;
    }

    /// Set a single pixel at sub-cell coordinates. On an ascii canvas it is a point, with no
    /// direction.
    pub fn set_pixel(&mut self, x: usize, y: usize) {
        if let Some((cell, sx, sy)) = self.locate(x, y) {
            self.light(cell, sy, self.blitter.bit(sx, sy));
        }
    }

    /// Clear a single pixel at sub-cell coordinates (on an ascii canvas, the whole cell).
    pub fn clear_pixel(&mut self, x: usize, y: usize) {
        if let Some((cell, sx, sy)) = self.locate(x, y) {
            match self.blitter {
                Blitter::Ascii => self.buffer[cell] = 0,
                blitter => self.buffer[cell] &= !blitter.bit(sx, sy),
            }
        }
    }

    /// The cell index and the sub-pixel within it of pixel `(x, y)`, or `None` off the canvas.
    fn locate(&self, x: usize, y: usize) -> Option<(usize, usize, usize)> {
        let (sub_w, sub_h) = self.blitter.sub_size();
        let (sub_w, sub_h) = (usize::from(sub_w), usize::from(sub_h));
        let (cell_x, cell_y) = (x / sub_w, y / sub_h);
        if cell_x >= self.cell_width || cell_y >= self.cell_height {
            return None; // out of bounds — silently clip
        }
        Some((cell_y * self.cell_width + cell_x, x % sub_w, y % sub_h))
    }

    /// ORs `bit` into `cell` and records the pen in the slot of sub-pixel row `sy`.
    fn light(&mut self, cell: usize, sy: usize, bit: u8) {
        self.buffer[cell] |= bit;
        let per_cell = self.blitter.color_slots();
        let half = if per_cell == 2 { sy } else { 0 };
        self.slots[cell * per_cell + half] = self.pen;
    }

    /// Lights pixel `(x, y)`; on an ascii canvas, with the direction `direction` computes (from
    /// the canvas's cell aspect), which is not called otherwise.
    fn plot_with(&mut self, x: i64, y: i64, direction: impl FnOnce(f64) -> u8) {
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return; // negative coordinates are off-canvas
        };
        let Some((cell, sx, sy)) = self.locate(x, y) else {
            return;
        };
        let bit = match self.blitter {
            Blitter::Ascii => direction(self.cell_aspect),
            blitter => blitter.bit(sx, sy),
        };
        self.light(cell, sy, bit);
    }

    /// Lights a pixel of a polyline's walk, in the direction of the segment it lies on.
    fn plot_on(&mut self, points: &[(f64, f64)], pixel: PathPixel) {
        self.plot_with(pixel.x, pixel.y, |aspect| {
            let last = points.len().saturating_sub(1);
            let (from, to) = (
                points[pixel.segment.min(last)],
                points[(pixel.segment + 1).min(last)],
            );
            line_direction(to.0 - from.0, to.1 - from.1, aspect)
        });
    }

    /// Writes every lit cell to `surface`, in the style `style` gives its color slot: the glyph
    /// the blitter encodes from the cell's mask. A half-block cell with both halves lit in
    /// different styles draws the upper half in the upper style's foreground over the lower
    /// style's foreground as background. Unlit cells are not written, so the canvas composes
    /// over whatever the surface holds.
    pub fn blit<S: Surface + ?Sized>(
        &self,
        surface: &mut S,
        style: impl Fn(ColorSlot) -> CellStyle,
    ) {
        self.blit_masked(surface, style, &[]);
    }

    /// [`SubCellCanvas::blit`], except that a cell whose entry in `masked` (row-major, one per
    /// cell) is `true` is not written: the label mask, so edges never overprint text (plan
    /// §7.3). Cells past the end of `masked` are unmasked.
    pub fn blit_masked<S: Surface + ?Sized>(
        &self,
        surface: &mut S,
        style: impl Fn(ColorSlot) -> CellStyle,
        masked: &[bool],
    ) {
        let cols = u16::try_from(self.cell_width).unwrap_or(u16::MAX);
        blit::blit(
            self.blitter,
            &self.lines,
            (cols, &self.buffer, &self.slots, masked),
            surface,
            style,
        );
    }

    /// ORs `upper`'s lit sub-pixels into this canvas, each cell (each half, for half blocks)
    /// that `upper` lights taking `upper`'s color slot: the compositor's merge of one layer over
    /// the layers below, under which a lower layer's dots survive a crossing (plan §7.4, ledger
    /// row 15). A canvas of another blitter or size merges nothing.
    pub fn merge_from(&mut self, upper: &SubCellCanvas) {
        let same_shape = upper.blitter == self.blitter
            && upper.cell_width == self.cell_width
            && upper.cell_height == self.cell_height;
        if !same_shape {
            return;
        }
        let per_cell = self.blitter.color_slots();
        for (cell, &bits) in upper.buffer.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            self.buffer[cell] |= bits;
            for half in 0..per_cell {
                let lit = per_cell == 1 || bits & (1 << half) != 0;
                if lit {
                    self.slots[cell * per_cell + half] = upper.slots[cell * per_cell + half];
                }
            }
        }
    }

    /// The walk of `points`, clipped to this canvas: what lies far off it is skipped, not
    /// stepped through.
    fn walk<'a>(&self, points: &'a [(f64, f64)]) -> Walk<'a> {
        Walk::new(points).clipped_to(self.clip())
    }

    fn clip(&self) -> Clip {
        let side = |pixels: usize| i64::try_from(pixels).unwrap_or(i64::MAX);
        Clip::canvas(side(self.pixel_width()), side(self.pixel_height()))
    }

    /// Draw the polyline through `points` (sub-pixel coordinates; see [`polyline_pixels`]).
    /// Pixels outside the canvas are silently clipped, and the walk skips what lies far off it.
    pub fn draw_polyline(&mut self, points: &[(f64, f64)]) {
        for pixel in self.walk(points) {
            self.plot_on(points, pixel);
        }
    }

    /// Draw a dashed polyline: `dash_on` sub-pixels of arc length drawn, then `dash_off`
    /// skipped, repeating from a dash at the first vertex. Each dash and gap holds at least one
    /// pixel and spans within half a pixel step of its length; `dash_on = 0` draws nothing and
    /// `dash_off = 0` draws solid.
    pub fn draw_dashed_polyline(&mut self, points: &[(f64, f64)], dash_on: u32, dash_off: u32) {
        let mut walk =
            Walk::measured(points, std::mem::take(&mut self.arc)).clipped_to(self.clip());
        dash(
            &mut walk,
            f64::from(dash_on),
            f64::from(dash_off),
            |pixel| {
                self.plot_on(points, pixel);
            },
        );
        self.arc = walk.into_buffer();
    }

    /// Draw a polyline with hop gaps at crossing points. Each hop is `(center_x, center_y,
    /// radius)`: pixels within the disc are skipped, leaving a visible gap where another line
    /// crosses over this one. The polyline's first and last pixels are always drawn.
    pub fn draw_polyline_with_hops(&mut self, points: &[(f64, f64)], hops: &[(i32, i32, i32)]) {
        let ends = [points.first(), points.last()].map(|p| p.map(|&p| snap(p)));
        for pixel in self.walk(points) {
            let (x, y) = (pixel.x, pixel.y);
            let is_endpoint = ends.contains(&Some((x, y)));
            if is_endpoint || !hops.iter().any(|&hop| in_disc(x, y, hop)) {
                self.plot_on(points, pixel);
            }
        }
    }

    /// Draw a curve: a Bézier piece or a chain of them, tessellated into a polyline.
    pub fn draw_curve(&mut self, chain: &[Bezier]) {
        let mut points = std::mem::take(&mut self.points);
        tessellate(chain, &mut points);
        self.draw_polyline(&points);
        self.points = points;
    }

    /// Draw a dashed curve: [`SubCellCanvas::draw_dashed_polyline`] over its tessellation.
    pub fn draw_dashed_curve(&mut self, chain: &[Bezier], dash_on: u32, dash_off: u32) {
        let mut points = std::mem::take(&mut self.points);
        tessellate(chain, &mut points);
        self.draw_dashed_polyline(&points, dash_on, dash_off);
        self.points = points;
    }

    /// Draw a circle outline using the Midpoint Circle algorithm.
    /// Accepts `i32` center coordinates; off-screen pixels are silently clipped.
    pub fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32) {
        let (cx, cy) = (i64::from(cx), i64::from(cy));
        if radius <= 0 {
            if radius == 0 {
                self.plot_with(cx, cy, |_| blit::ascii_bits::DOT);
            }
            return;
        }
        let radius = i64::from(radius);
        let mut x = radius;
        let mut y: i64 = 0;
        let mut err = 1 - radius;

        while x >= y {
            // Plot 8 octant-symmetric points, each drawn along its tangent on an ascii canvas.
            for (ox, oy) in [
                (x, y),
                (-x, y),
                (x, -y),
                (-x, -y),
                (y, x),
                (-y, x),
                (y, -x),
                (-y, -x),
            ] {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "offsets are at most an i32 radius, which f64 holds exactly"
                )]
                let tangent = (-oy as f64, ox as f64);
                self.plot_with(cx + ox, cy + oy, |aspect| {
                    line_direction(tangent.0, tangent.1, aspect)
                });
            }
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    /// Each cell's glyph, ignoring color: the blitter's encoding of its mask, an empty cell
    /// being the encoding's blank (U+2800 for braille, a space otherwise).
    #[must_use]
    pub fn render(&self) -> Vec<Vec<char>> {
        (0..self.cell_height)
            .map(|row| {
                let start = row * self.cell_width;
                self.buffer[start..start + self.cell_width]
                    .iter()
                    .map(|&bits| self.blitter.glyph(bits, &self.lines))
                    .collect()
            })
            .collect()
    }

    /// Clear the canvas.
    pub fn clear(&mut self) {
        self.buffer.fill(0);
    }

    /// Clear and resize to `cols × rows` cells, bounded by [`MAX_CELLS`](super::MAX_CELLS) like
    /// [`SubCellCanvas::new`], reusing the allocations.
    pub fn clear_and_resize(&mut self, cols: u16, rows: u16) {
        let max_rows = MAX_CELLS / usize::from(cols.max(1));
        self.cell_width = usize::from(cols);
        self.cell_height = usize::from(rows).min(max_rows);
        let len = self.cell_width * self.cell_height;
        self.buffer.clear();
        self.buffer.resize(len, 0);
        self.slots.clear();
        self.slots
            .resize(len * self.blitter.color_slots(), ColorSlot::default());
    }

    /// Width in terminal cells (read-only).
    #[must_use]
    pub fn cell_cols(&self) -> usize {
        self.cell_width
    }

    /// Height in terminal cells (read-only).
    #[must_use]
    pub fn cell_rows(&self) -> usize {
        self.cell_height
    }

    /// The cell's mask at `(row, col)` in the blitter's bit layout (Unicode's dot bits for
    /// braille), or 0 (nothing lit) outside the canvas:
    /// like every other accessor, it clips rather than panicking (plan §14 row 13).
    #[must_use]
    pub fn get_cell(&self, row: usize, col: usize) -> u8 {
        self.try_get_cell(row, col).unwrap_or(0)
    }

    /// The cell's mask at `(row, col)`, or `None` outside the canvas.
    #[must_use]
    pub fn try_get_cell(&self, row: usize, col: usize) -> Option<u8> {
        if row >= self.cell_height || col >= self.cell_width {
            return None;
        }
        self.buffer.get(row * self.cell_width + col).copied()
    }
}

/// Whether `(x, y)` lies in the closed disc `(center_x, center_y, radius)`.
fn in_disc(x: i64, y: i64, (hx, hy, r): (i32, i32, i32)) -> bool {
    let dx = x - i64::from(hx);
    let dy = y - i64::from(hy);
    let r = i64::from(r);
    // |dx| and |dy| can approach 2^32, so their squares can pass i64::MAX; a saturated sum is
    // still larger than any r² <= 2^62, so the comparison stays exact.
    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)) <= r * r
}

mod integer;

#[cfg(test)]
#[path = "braille_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "braille_seed_rasters_tests.rs"]
mod seed_rasters;
