//! The seed's integer-coordinate primitives, each now a polyline or a curve (owner ruling A1).

use super::SubCellCanvas;
use crate::geometry::curve::Bezier;

impl SubCellCanvas {
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
    /// [`SubCellCanvas::draw_dashed_polyline`]).
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
    /// [`SubCellCanvas::draw_bezier`]: `dash_on` sub-pixels of arc length drawn, then
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
        reason = "the seed's signature; the scene draws through `draw_dashed_curve`"
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
}

/// An integer pixel coordinate as a polyline vertex.
fn pt(x: i32, y: i32) -> (f64, f64) {
    (f64::from(x), f64::from(y))
}

fn quadratic(from: (f64, f64), ctrl: (f64, f64), to: (f64, f64)) -> Bezier {
    Bezier::Quadratic { from, ctrl, to }
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
