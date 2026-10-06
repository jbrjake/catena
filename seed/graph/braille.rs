//! Braille sub-cell rendering for high-resolution terminal graphics.
//! See PRD-021-TUI.md §3.1.3 for the algorithm specification.
//!
//! Each terminal cell maps to a 2x4 grid of Braille dots (Unicode U+2800-U+28FF),
//! giving 8 individually addressable dots per cell -- 2x horizontal and 4x vertical
//! resolution compared to regular character rendering.

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
}

impl BrailleCanvas {
    /// Create a canvas sized to fill `cell_width × cell_height` terminal cells.
    /// Pixel resolution is `(cell_width * 2) × (cell_height * 4)`.
    pub fn new(cell_width: usize, cell_height: usize) -> Self {
        Self {
            cell_width,
            cell_height,
            buffer: vec![0u8; cell_width * cell_height],
        }
    }

    /// Pixel width (2× cell width).
    #[allow(dead_code)] // Used by tests; available for future canvas consumers.
    pub fn pixel_width(&self) -> usize {
        self.cell_width * 2
    }

    /// Pixel height (4× cell height).
    #[allow(dead_code)] // Used by tests; available for future canvas consumers.
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

    /// Draw a line between two pixel coordinates using Bresenham's algorithm.
    /// Accepts `i32` so callers can pass off-screen (negative) endpoints;
    /// pixels outside the canvas are silently clipped.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (mut x0, mut y0) = (x0, y0);
        let (x1, y1) = (x1, y1);

        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if x0 >= 0 && y0 >= 0 {
                self.set_pixel(x0 as usize, y0 as usize);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
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
        let (mut cx, mut cy) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if cx >= 0 && cy >= 0 {
                // Always draw endpoints; hops only suppress interior crossing pixels.
                let is_endpoint = cx == x0 && cy == y0 || cx == x1 && cy == y1;
                let in_hop = !is_endpoint
                    && hops.iter().any(|&(hx, hy, r)| {
                        let ddx = cx - hx;
                        let ddy = cy - hy;
                        ddx * ddx + ddy * ddy <= r * r
                    });
                if !in_hop {
                    self.set_pixel(cx as usize, cy as usize);
                }
            }
            if cx == x1 && cy == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                cx += sx;
            }
            if e2 <= dx {
                err += dx;
                cy += sy;
            }
        }
    }

    /// Draw a dashed line between two pixel coordinates.
    /// `dash_on` pixels are drawn, then `dash_off` pixels are skipped, repeating.
    pub fn draw_dashed_line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        dash_on: u32,
        dash_off: u32,
    ) {
        let period = dash_on + dash_off;
        if period == 0 {
            // Nothing to draw with a zero-length dash period.
            return;
        }

        let (mut x0, mut y0) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut step = 0u32;

        loop {
            if step % period < dash_on && x0 >= 0 && y0 >= 0 {
                self.set_pixel(x0 as usize, y0 as usize);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
            step += 1;
        }
    }

    /// Draw a dashed Bézier curve between two pixel coordinates.
    /// `dash_on` pixels are drawn, then `dash_off` pixels are skipped, repeating.
    pub fn draw_dashed_bezier(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        dash_on: u32,
        dash_off: u32,
    ) {
        let mx = (x0 + x1) as f64 / 2.0;
        let my = (y0 + y1) as f64 / 2.0;
        let dx = (x1 - x0) as f64;
        let dy = (y1 - y0) as f64;
        let len = (dx * dx + dy * dy).sqrt();
        let offset = len * 0.2;
        let (nx, ny) = if len > 0.0 {
            (-dy / len, dx / len)
        } else {
            (0.0, 0.0)
        };
        let cx = mx + nx * offset;
        let cy = my + ny * offset;

        let period = dash_on + dash_off;
        if period == 0 {
            return;
        }

        // Low-severity note: dash phase is counted by sample index, not arc-length.
        // For very short curves (len < 20 px) the clamp to 10 steps means multiple
        // samples land on the same pixel, so the visible dash pattern may appear
        // coarser or fully solid regardless of period.  Acceptable for the current
        // use-case (self-loop / near-coincident node edges).
        let steps = (len as usize / 2).clamp(10, 200);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let one_minus_t = 1.0 - t;
            let px = one_minus_t * one_minus_t * x0 as f64
                + 2.0 * one_minus_t * t * cx
                + t * t * x1 as f64;
            let py = one_minus_t * one_minus_t * y0 as f64
                + 2.0 * one_minus_t * t * cy
                + t * t * y1 as f64;

            if (i as u32) % period < dash_on {
                let ix = px.round() as i32;
                let iy = py.round() as i32;
                if ix >= 0 && iy >= 0 {
                    self.set_pixel(ix as usize, iy as usize);
                }
            }
        }
    }

    /// Draw a quadratic Bezier curve between two pixel coordinates.
    /// The control point is offset perpendicular to the midpoint of the line,
    /// creating a gentle arc that visually distinguishes overlay edges.
    pub fn draw_bezier(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        // Compute midpoint
        let mx = (x0 + x1) as f64 / 2.0;
        let my = (y0 + y1) as f64 / 2.0;

        // Perpendicular offset (20% of line length)
        let dx = (x1 - x0) as f64;
        let dy = (y1 - y0) as f64;
        let len = (dx * dx + dy * dy).sqrt();
        let offset = len * 0.2;

        // Control point: perpendicular to midpoint
        let (nx, ny) = if len > 0.0 {
            (-dy / len, dx / len)
        } else {
            (0.0, 0.0)
        };
        let cx = mx + nx * offset;
        let cy = my + ny * offset;

        // Sample the quadratic Bezier: B(t) = (1-t)^2 P0 + 2(1-t)t C + t^2 P1
        let steps = (len as usize / 2).clamp(10, 200);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let one_minus_t = 1.0 - t;
            let px = one_minus_t * one_minus_t * x0 as f64
                + 2.0 * one_minus_t * t * cx
                + t * t * x1 as f64;
            let py = one_minus_t * one_minus_t * y0 as f64
                + 2.0 * one_minus_t * t * cy
                + t * t * y1 as f64;

            let ix = px.round() as i32;
            let iy = py.round() as i32;
            if ix >= 0 && iy >= 0 {
                self.set_pixel(ix as usize, iy as usize);
            }
        }
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
        let dx = (x1 - x0) as f64;
        let dy = (y1 - y0) as f64;
        let len = (dx * dx + dy * dy).sqrt();
        let steps = (len as usize / 2).clamp(10, 200);

        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let one_minus_t = 1.0 - t;
            let px = one_minus_t * one_minus_t * x0 as f64
                + 2.0 * one_minus_t * t * ctrl_x as f64
                + t * t * x1 as f64;
            let py = one_minus_t * one_minus_t * y0 as f64
                + 2.0 * one_minus_t * t * ctrl_y as f64
                + t * t * y1 as f64;

            let ix = px.round() as i32;
            let iy = py.round() as i32;
            if ix >= 0 && iy >= 0 {
                self.set_pixel(ix as usize, iy as usize);
            }
        }
    }

    /// Draw a dashed quadratic Bézier curve with an explicit control point.
    /// Combines the explicit control point of `draw_bezier_ctrl` with a dash
    /// pattern: `dash_on` pixels are drawn, then `dash_off` pixels are skipped.
    #[allow(clippy::too_many_arguments)]
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
        let period = dash_on + dash_off;
        if period == 0 {
            return;
        }

        let dx = (x1 - x0) as f64;
        let dy = (y1 - y0) as f64;
        let len = (dx * dx + dy * dy).sqrt();
        let steps = (len as usize / 2).clamp(10, 200);

        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let one_minus_t = 1.0 - t;
            let px = one_minus_t * one_minus_t * x0 as f64
                + 2.0 * one_minus_t * t * ctrl_x as f64
                + t * t * x1 as f64;
            let py = one_minus_t * one_minus_t * y0 as f64
                + 2.0 * one_minus_t * t * ctrl_y as f64
                + t * t * y1 as f64;

            if (i as u32) % period < dash_on {
                let ix = px.round() as i32;
                let iy = py.round() as i32;
                if ix >= 0 && iy >= 0 {
                    self.set_pixel(ix as usize, iy as usize);
                }
            }
        }
    }

    /// Draw a circle outline using the Midpoint Circle algorithm.
    /// Accepts `i32` center coordinates; off-screen pixels are silently clipped.
    pub fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32) {
        if radius <= 0 {
            if radius == 0 && cx >= 0 && cy >= 0 {
                self.set_pixel(cx as usize, cy as usize);
            }
            return;
        }
        let mut x = radius;
        let mut y: i32 = 0;
        let mut err = 1 - radius;

        while x >= y {
            // Plot 8 octant-symmetric points.
            for &(px, py) in &[
                (cx + x, cy + y),
                (cx - x, cy + y),
                (cx + x, cy - y),
                (cx - x, cy - y),
                (cx + y, cy + x),
                (cx - y, cy + x),
                (cx + y, cy - x),
                (cx - y, cy - x),
            ] {
                if px >= 0 && py >= 0 {
                    self.set_pixel(px as usize, py as usize);
                }
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
    pub fn render(&self) -> Vec<Vec<char>> {
        (0..self.cell_height)
            .map(|row| {
                let start = row * self.cell_width;
                self.buffer[start..start + self.cell_width]
                    .iter()
                    .map(|&bits| char::from_u32(BRAILLE_BASE + bits as u32).unwrap_or(' '))
                    .collect()
            })
            .collect()
    }

    /// Clear the canvas.
    #[allow(dead_code)] // Used by tests; available for future canvas consumers.
    pub fn clear(&mut self) {
        self.buffer.fill(0);
    }

    /// Clear and resize to new dimensions, reusing the allocation when possible.
    #[allow(dead_code)] // Available for callers that persist canvases across frames.
    pub fn clear_and_resize(&mut self, cell_width: usize, cell_height: usize) {
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        let len = cell_width * cell_height;
        self.buffer.clear();
        self.buffer.resize(len, 0);
    }

    /// Width in terminal cells (read-only).
    pub fn cell_cols(&self) -> usize {
        self.cell_width
    }

    /// Height in terminal cells (read-only).
    pub fn cell_rows(&self) -> usize {
        self.cell_height
    }

    /// Raw 8-bit dot-pattern for the cell at `(row, col)`.
    pub fn get_cell(&self, row: usize, col: usize) -> u8 {
        self.buffer[row * self.cell_width + col]
    }
}

#[cfg(test)]
#[path = "braille_tests.rs"]
mod tests;
