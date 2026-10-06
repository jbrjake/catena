//! Braille sub-cell rendering for high-resolution terminal graphics.
//!
//! Each terminal cell maps to a 2x4 grid of Braille dots (Unicode U+2800-U+28FF),
//! giving 8 individually addressable dots per cell -- 2x horizontal and 4x vertical
//! resolution compared to regular character rendering.
//!
//! Lines and curves are drawn as polylines (owner ruling A1): a line is two vertices, a curve is
//! its tessellation, and solid, dashed and hop-gapped drawing all walk the polyline's pixels.
//! Coordinates are signed sub-pixels so callers can pass off-screen endpoints; arithmetic runs in
//! `i64` so no input overflows, and pixels outside the canvas are silently clipped. M1
//! generalizes this canvas to `SubCellCanvas` (plan §7.2).

use super::polyline::{Walk, dash, snap};
use crate::geometry::curve::{Bezier, tessellate};

#[cfg(doc)]
use super::polyline::polyline_pixels;

/// Bit positions for Braille dots within a cell.
/// Layout:  (col=0, col=1)
///   row 0:  bit0   bit3
///   row 1:  bit1   bit4
///   row 2:  bit2   bit5
///   row 3:  bit6   bit7
const BRAILLE_MAP: [[u8; 2]; 4] = [
    [0x01, 0x08], // row 0: bit0, bit3
    [0x02, 0x10], // row 1: bit1, bit4
    [0x04, 0x20], // row 2: bit2, bit5
    [0x40, 0x80], // row 3: bit6, bit7
];

/// Unicode Braille base character (empty pattern).
const BRAILLE_BASE: u32 = 0x2800;

/// A canvas that renders pixels at Braille sub-cell resolution.
/// Each terminal cell holds 2×4 = 8 individually addressable dots.
pub struct BrailleCanvas {
    /// Width in terminal cells.
    cell_width: usize,
    /// Height in terminal cells.
    cell_height: usize,
    /// Flat bitmask buffer indexed by `cell_y * cell_width + cell_x`.
    buffer: Vec<u8>,
    /// Reused by every curve for its tessellated polyline.
    points: Vec<(f64, f64)>,
    /// Reused by every dashed primitive for its vertices' cumulative arc lengths.
    arc: Vec<f64>,
}

impl BrailleCanvas {
    /// Create a canvas sized to fill `cell_width × cell_height` terminal cells.
    /// Pixel resolution is `(cell_width * 2) × (cell_height * 4)`.
    #[must_use]
    pub fn new(cell_width: usize, cell_height: usize) -> Self {
        Self {
            cell_width,
            cell_height,
            buffer: vec![0u8; cell_width * cell_height],
            points: Vec::new(),
            arc: Vec::new(),
        }
    }

    /// Pixel width (2× cell width).
    #[must_use]
    pub fn pixel_width(&self) -> usize {
        self.cell_width * 2
    }

    /// Pixel height (4× cell height).
    #[must_use]
    pub fn pixel_height(&self) -> usize {
        self.cell_height * 4
    }

    /// Set a single pixel at sub-cell coordinates.
    pub fn set_pixel(&mut self, x: usize, y: usize) {
        let cell_x = x / 2;
        let cell_y = y / 4;
        if cell_x >= self.cell_width || cell_y >= self.cell_height {
            return; // out of bounds — silently clip
        }
        let dot_x = x % 2;
        let dot_y = y % 4;
        self.buffer[cell_y * self.cell_width + cell_x] |= BRAILLE_MAP[dot_y][dot_x];
    }

    /// Clear a single pixel at sub-cell coordinates.
    pub fn clear_pixel(&mut self, x: usize, y: usize) {
        let cell_x = x / 2;
        let cell_y = y / 4;
        if cell_x >= self.cell_width || cell_y >= self.cell_height {
            return;
        }
        let dot_x = x % 2;
        let dot_y = y % 4;
        self.buffer[cell_y * self.cell_width + cell_x] &= !BRAILLE_MAP[dot_y][dot_x];
    }

    /// Set a pixel at signed coordinates; negative coordinates are off-canvas and clipped.
    fn plot(&mut self, x: i64, y: i64) {
        if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) {
            self.set_pixel(x, y);
        }
    }

    /// Draw the polyline through `points` (sub-pixel coordinates; see [`polyline_pixels`]).
    /// Pixels outside the canvas are silently clipped.
    pub fn draw_polyline(&mut self, points: &[(f64, f64)]) {
        for pixel in Walk::new(points) {
            self.plot(pixel.x, pixel.y);
        }
    }

    /// Draw a dashed polyline: `dash_on` sub-pixels of arc length drawn, then `dash_off`
    /// skipped, repeating from a dash at the first vertex. Each dash and gap holds at least one
    /// pixel and spans within half a pixel step of its length; `dash_on = 0` draws nothing and
    /// `dash_off = 0` draws solid.
    pub fn draw_dashed_polyline(&mut self, points: &[(f64, f64)], dash_on: u32, dash_off: u32) {
        let mut walk = Walk::measured(points, std::mem::take(&mut self.arc));
        dash(
            &mut walk,
            f64::from(dash_on),
            f64::from(dash_off),
            |x, y| self.plot(x, y),
        );
        self.arc = walk.into_buffer();
    }

    /// Draw a polyline with hop gaps at crossing points. Each hop is `(center_x, center_y,
    /// radius)`: pixels within the disc are skipped, leaving a visible gap where another line
    /// crosses over this one. The polyline's first and last pixels are always drawn.
    pub fn draw_polyline_with_hops(&mut self, points: &[(f64, f64)], hops: &[(i32, i32, i32)]) {
        let ends = [points.first(), points.last()].map(|p| p.map(|&p| snap(p)));
        for pixel in Walk::new(points) {
            let (x, y) = (pixel.x, pixel.y);
            let is_endpoint = ends.contains(&Some((x, y)));
            if is_endpoint || !hops.iter().any(|&hop| in_disc(x, y, hop)) {
                self.plot(x, y);
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

    /// Draw a dashed curve: [`BrailleCanvas::draw_dashed_polyline`] over its tessellation.
    pub fn draw_dashed_curve(&mut self, chain: &[Bezier], dash_on: u32, dash_off: u32) {
        let mut points = std::mem::take(&mut self.points);
        tessellate(chain, &mut points);
        self.draw_dashed_polyline(&points, dash_on, dash_off);
        self.points = points;
    }

    /// Draw a line between two pixel coordinates using Bresenham's algorithm.
    /// Accepts `i32` so callers can pass off-screen (negative) endpoints;
    /// pixels outside the canvas are silently clipped.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.draw_polyline(&[pt(x0, y0), pt(x1, y1)]);
    }

    /// Draw a line with hop gaps at crossing points.
    /// Each hop is `(center_x, center_y, radius)` — pixels within the disc are
    /// skipped, creating a visible gap where another line crosses over this one.
    pub fn draw_line_with_hops(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        hops: &[(i32, i32, i32)],
    ) {
        self.draw_polyline_with_hops(&[pt(x0, y0), pt(x1, y1)], hops);
    }

    /// Draw a dashed line between two pixel coordinates: `dash_on` sub-pixels of arc length
    /// drawn, then `dash_off` skipped, repeating (see
    /// [`BrailleCanvas::draw_dashed_polyline`]).
    pub fn draw_dashed_line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        dash_on: u32,
        dash_off: u32,
    ) {
        self.draw_dashed_polyline(&[pt(x0, y0), pt(x1, y1)], dash_on, dash_off);
    }

    /// Draw a dashed Bézier curve between two pixel coordinates, bowed like
    /// [`BrailleCanvas::draw_bezier`]: `dash_on` sub-pixels of arc length drawn, then
    /// `dash_off` skipped, repeating.
    pub fn draw_dashed_bezier(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        dash_on: u32,
        dash_off: u32,
    ) {
        let ctrl = bow_control(x0, y0, x1, y1);
        self.draw_dashed_curve(
            &[quadratic(pt(x0, y0), ctrl, pt(x1, y1))],
            dash_on,
            dash_off,
        );
    }

    /// Draw a quadratic Bezier curve between two pixel coordinates.
    /// The control point is offset perpendicular to the midpoint of the line,
    /// creating a gentle arc that visually distinguishes overlay edges.
    pub fn draw_bezier(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let ctrl = bow_control(x0, y0, x1, y1);
        self.draw_curve(&[quadratic(pt(x0, y0), ctrl, pt(x1, y1))]);
    }

    /// Draw a quadratic Bezier curve with an explicit control point.
    /// Unlike `draw_bezier` (which auto-computes a perpendicular control point),
    /// this variant lets the caller specify the control point directly —
    /// needed for chord diagrams where chords curve toward the circle center.
    pub fn draw_bezier_ctrl(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        ctrl_x: i32,
        ctrl_y: i32,
    ) {
        self.draw_curve(&[quadratic(pt(x0, y0), pt(ctrl_x, ctrl_y), pt(x1, y1))]);
    }

    /// Draw a dashed quadratic Bézier curve with an explicit control point.
    /// Combines the explicit control point of `draw_bezier_ctrl` with a dash
    /// pattern: `dash_on` sub-pixels of arc length drawn, then `dash_off` skipped.
    #[expect(
        clippy::too_many_arguments,
        reason = "the seed's signature; M1's SubCellCanvas takes points and a dash pattern"
    )]
    pub fn draw_dashed_bezier_ctrl(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        ctrl_x: i32,
        ctrl_y: i32,
        dash_on: u32,
        dash_off: u32,
    ) {
        let curve = quadratic(pt(x0, y0), pt(ctrl_x, ctrl_y), pt(x1, y1));
        self.draw_dashed_curve(&[curve], dash_on, dash_off);
    }

    /// Draw a circle outline using the Midpoint Circle algorithm.
    /// Accepts `i32` center coordinates; off-screen pixels are silently clipped.
    pub fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32) {
        let (cx, cy) = (i64::from(cx), i64::from(cy));
        if radius <= 0 {
            if radius == 0 {
                self.plot(cx, cy);
            }
            return;
        }
        let radius = i64::from(radius);
        let mut x = radius;
        let mut y: i64 = 0;
        let mut err = 1 - radius;

        while x >= y {
            // Plot 8 octant-symmetric points.
            for (px, py) in [
                (cx + x, cy + y),
                (cx - x, cy + y),
                (cx + x, cy - y),
                (cx - x, cy - y),
                (cx + y, cy + x),
                (cx - y, cy + x),
                (cx + y, cy - x),
                (cx - y, cy - x),
            ] {
                self.plot(px, py);
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

    /// Render the canvas to a grid of Braille characters.
    #[must_use]
    pub fn render(&self) -> Vec<Vec<char>> {
        (0..self.cell_height)
            .map(|row| {
                let start = row * self.cell_width;
                self.buffer[start..start + self.cell_width]
                    .iter()
                    .map(|&bits| char::from_u32(BRAILLE_BASE + u32::from(bits)).unwrap_or(' '))
                    .collect()
            })
            .collect()
    }

    /// Clear the canvas.
    pub fn clear(&mut self) {
        self.buffer.fill(0);
    }

    /// Clear and resize to new dimensions, reusing the allocation when possible.
    pub fn clear_and_resize(&mut self, cell_width: usize, cell_height: usize) {
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        let len = cell_width * cell_height;
        self.buffer.clear();
        self.buffer.resize(len, 0);
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

    /// Raw 8-bit dot pattern for the cell at `(row, col)`, or 0 (no dots) outside the canvas:
    /// like every other accessor, it clips rather than panicking (plan §14 row 13).
    #[must_use]
    pub fn get_cell(&self, row: usize, col: usize) -> u8 {
        self.try_get_cell(row, col).unwrap_or(0)
    }

    /// Raw 8-bit dot pattern for the cell at `(row, col)`, or `None` outside the canvas.
    #[must_use]
    pub fn try_get_cell(&self, row: usize, col: usize) -> Option<u8> {
        if row >= self.cell_height || col >= self.cell_width {
            return None;
        }
        self.buffer.get(row * self.cell_width + col).copied()
    }
}

/// An integer pixel coordinate as a polyline vertex.
fn pt(x: i32, y: i32) -> (f64, f64) {
    (f64::from(x), f64::from(y))
}

fn quadratic(from: (f64, f64), ctrl: (f64, f64), to: (f64, f64)) -> Bezier {
    Bezier::Quadratic { from, ctrl, to }
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

/// The control point of the gentle arc `draw_bezier` draws: offset from the chord's midpoint,
/// perpendicular to it, by 20% of its length.
fn bow_control(x0: i32, y0: i32, x1: i32, y1: i32) -> (f64, f64) {
    let (x0, y0, x1, y1) = (f64::from(x0), f64::from(y0), f64::from(x1), f64::from(y1));
    // Compute midpoint
    let mx = f64::midpoint(x0, x1);
    let my = f64::midpoint(y0, y1);

    // Perpendicular offset (20% of line length)
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt();
    let offset = len * 0.2;

    // Control point: perpendicular to midpoint
    let (nx, ny) = if len > 0.0 {
        (-dy / len, dx / len)
    } else {
        (0.0, 0.0)
    };
    (mx + nx * offset, my + ny * offset)
}

#[cfg(test)]
#[path = "braille_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "braille_seed_rasters_tests.rs"]
mod seed_rasters;
