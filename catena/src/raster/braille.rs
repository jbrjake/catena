//! Braille sub-cell rendering for high-resolution terminal graphics.
//!
//! Each terminal cell maps to a 2x4 grid of Braille dots (Unicode U+2800-U+28FF),
//! giving 8 individually addressable dots per cell -- 2x horizontal and 4x vertical
//! resolution compared to regular character rendering.
//!
//! Every primitive takes signed `i32` sub-pixel coordinates so callers can pass off-screen
//! endpoints; arithmetic runs in `i64` so no input overflows, and pixels outside the canvas are
//! silently clipped. M1 generalizes this canvas to `SubCellCanvas` (plan §7.2).

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
    #[must_use]
    pub fn new(cell_width: usize, cell_height: usize) -> Self {
        Self {
            cell_width,
            cell_height,
            buffer: vec![0u8; cell_width * cell_height],
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

    /// Draw a line between two pixel coordinates using Bresenham's algorithm.
    /// Accepts `i32` so callers can pass off-screen (negative) endpoints;
    /// pixels outside the canvas are silently clipped.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        walk_line(x0, y0, x1, y1, |x, y, _| self.plot(x, y));
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
        let start = (i64::from(x0), i64::from(y0));
        let end = (i64::from(x1), i64::from(y1));
        walk_line(x0, y0, x1, y1, |x, y, _| {
            // Always draw endpoints; hops only suppress interior crossing pixels.
            let is_endpoint = (x, y) == start || (x, y) == end;
            let in_hop = !is_endpoint && hops.iter().any(|&hop| in_disc(x, y, hop));
            if !in_hop {
                self.plot(x, y);
            }
        });
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
        let Some(period) = dash_period(dash_on, dash_off) else {
            // Nothing to draw with a zero-length dash period.
            return;
        };
        walk_line(x0, y0, x1, y1, |x, y, step| {
            if step % period < u64::from(dash_on) {
                self.plot(x, y);
            }
        });
    }

    /// Draw a dashed Bézier curve between two pixel coordinates.
    /// `dash_on` pixels are drawn, then `dash_off` pixels are skipped, repeating.
    ///
    /// Low-severity note: dash phase is counted by sample index, not arc-length.
    /// For very short curves (len < 20 px) the clamp to 10 steps means multiple
    /// samples land on the same pixel, so the visible dash pattern may appear
    /// coarser or fully solid regardless of period.  Acceptable for the current
    /// use-case (self-loop / near-coincident node edges).
    pub fn draw_dashed_bezier(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        dash_on: u32,
        dash_off: u32,
    ) {
        let Some(period) = dash_period(dash_on, dash_off) else {
            return;
        };
        let ctrl = bow_control(x0, y0, x1, y1);
        sample_quadratic((x0, y0), ctrl, (x1, y1), |x, y, i| {
            if i % period < u64::from(dash_on) {
                self.plot(x, y);
            }
        });
    }

    /// Draw a quadratic Bezier curve between two pixel coordinates.
    /// The control point is offset perpendicular to the midpoint of the line,
    /// creating a gentle arc that visually distinguishes overlay edges.
    pub fn draw_bezier(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let ctrl = bow_control(x0, y0, x1, y1);
        sample_quadratic((x0, y0), ctrl, (x1, y1), |x, y, _| self.plot(x, y));
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
        let ctrl = (f64::from(ctrl_x), f64::from(ctrl_y));
        sample_quadratic((x0, y0), ctrl, (x1, y1), |x, y, _| self.plot(x, y));
    }

    /// Draw a dashed quadratic Bézier curve with an explicit control point.
    /// Combines the explicit control point of `draw_bezier_ctrl` with a dash
    /// pattern: `dash_on` pixels are drawn, then `dash_off` pixels are skipped.
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
        let Some(period) = dash_period(dash_on, dash_off) else {
            return;
        };
        let ctrl = (f64::from(ctrl_x), f64::from(ctrl_y));
        sample_quadratic((x0, y0), ctrl, (x1, y1), |x, y, i| {
            if i % period < u64::from(dash_on) {
                self.plot(x, y);
            }
        });
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

/// `dash_on + dash_off`, or `None` for a zero period (nothing to draw). A sum past `u32::MAX`
/// cannot overflow: the period is `u64`.
fn dash_period(dash_on: u32, dash_off: u32) -> Option<u64> {
    let period = u64::from(dash_on) + u64::from(dash_off);
    (period > 0).then_some(period)
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

/// Walks the Bresenham line from `(x0, y0)` to `(x1, y1)` inclusive, calling `visit` with each
/// pixel and its 0-based step index. Runs in `i64`, so no `i32` endpoints overflow.
fn walk_line(x0: i32, y0: i32, x1: i32, y1: i32, mut visit: impl FnMut(i64, i64, u64)) {
    let (mut x, mut y) = (i64::from(x0), i64::from(y0));
    let (x1, y1) = (i64::from(x1), i64::from(y1));
    let dx = (x1 - x).abs();
    let dy = -(y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut step = 0u64;

    loop {
        visit(x, y, step);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        step += 1;
    }
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

/// Samples the quadratic Bézier `B(t) = (1-t)^2 P0 + 2(1-t)t C + t^2 P1` and calls `plot` with
/// each sample rounded to the nearest pixel, plus the sample's index (the dash phase). The one
/// sampler behind every curve primitive.
///
/// Sampling density: one sample per 2 sub-pixels of chord length, clamped to 10..=200 samples
/// (plus the closing one).
fn sample_quadratic(
    (x0, y0): (i32, i32),
    (ctrl_x, ctrl_y): (f64, f64),
    (x1, y1): (i32, i32),
    mut plot: impl FnMut(i64, i64, u64),
) {
    let (x0, y0, x1, y1) = (f64::from(x0), f64::from(y0), f64::from(x1), f64::from(y1));
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = sample_count((dx * dx + dy * dy).sqrt());
    for i in 0..=steps {
        let t = f64::from(i) / f64::from(steps);
        let one_minus_t = 1.0 - t;
        let px = one_minus_t * one_minus_t * x0 + 2.0 * one_minus_t * t * ctrl_x + t * t * x1;
        let py = one_minus_t * one_minus_t * y0 + 2.0 * one_minus_t * t * ctrl_y + t * t * y1;
        plot(round_px(px), round_px(py), u64::from(i));
    }
}

/// `(len / 2).clamp(10, 200)` for a chord of `len` sub-pixels, truncating like the seed did.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "len is a finite, non-negative chord length, and the float-to-int cast saturates"
)]
fn sample_count(len: f64) -> u32 {
    let half = (len as u64) / 2;
    u32::try_from(half.clamp(10, 200)).expect("clamped to at most 200")
}

/// Rounds a sample to the nearest pixel.
#[expect(
    clippy::cast_possible_truncation,
    reason = "samples lie between i32 endpoints and control points, far inside i64; the cast \
              saturates anyway"
)]
fn round_px(v: f64) -> i64 {
    v.round() as i64
}

#[cfg(test)]
#[path = "braille_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "braille_seed_rasters_tests.rs"]
mod seed_rasters;
