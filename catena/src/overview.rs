//! The overview (plan §18's minimap, "rewritten thin over core"): the whole graph small, from
//! the view's layout, with the rectangle the view shows; the minimap widget draws it.
//!
//! The seed's `render_graph_minimap` was read for this and deleted. Kept: nothing under two
//! cells a side is drawn; a node is one sub-cell dot, and nodes may share a cell, so a crowded
//! minimap stays a picture of the layout rather than a re-spaced one; the selected node lights
//! its whole cell; and the view's frame is drawn over the dots. Changed: the fit is the view's
//! world positions, aspect-corrected so a world unit is as long across as down (the seed
//! stretched snapped cells to fill both axes), inside a one-cell margin; colors come from the
//! styles of the drawing, not a palette function, and every cell's color from the last pen to
//! touch it rather than hash-map order. Dropped: the seed's floating-window caller and its
//! colors by entity type.

use crate::GraphView;
use crate::draw::{self, NODE, NODE_SELECTED, VIEWPORT};
use crate::geometry::cell::SubPt;
use crate::graph::Key;
use crate::raster::{ColorSlot, SubCellCanvas, Surface};
use crate::scene::StyleId;

/// The overview's margin, in cells on every side.
const MARGIN: f64 = 1.0;

/// How world points map onto an overview's sub-cell pixels: `px = left + (x − x0) · sx`.
struct Fit {
    origin: (f64, f64),
    scale: (f64, f64),
    offset: (f64, f64),
}

impl Fit {
    /// The fit of `points` into a `cols × rows` surface of `sub` pixels a cell, a cell being
    /// `aspect` times as wide as tall; `None` with no points.
    fn of(
        points: impl Iterator<Item = (f64, f64)> + Clone,
        (cols, rows): (u16, u16),
        sub: (u8, u8),
        aspect: f64,
    ) -> Option<Fit> {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut any = false;
        for (x, y) in points.filter(|p| p.0.is_finite() && p.1.is_finite()) {
            any = true;
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        if !any {
            return None;
        }
        // In column widths: a world unit is one (as the view draws it, plan §6), and a row is
        // 1 / aspect of them tall.
        let usable = (
            (f64::from(cols) - 2.0 * MARGIN).max(0.0),
            (f64::from(rows) - 2.0 * MARGIN).max(0.0) / aspect,
        );
        let span = (x1 - x0, y1 - y0);
        let fits = |usable: f64, span: f64| {
            if span > 0.0 {
                usable / span
            } else {
                f64::INFINITY
            }
        };
        let s = fits(usable.0, span.0).min(fits(usable.1, span.1));
        let s = if s.is_finite() { s } else { 0.0 };
        let (sx, sy) = (f64::from(sub.0), f64::from(sub.1));
        let scale = (s * sx, s * sy * aspect);
        let size = (f64::from(cols) * sx, f64::from(rows) * sy);
        Some(Fit {
            origin: (x0, y0),
            scale,
            offset: (
                (size.0 - span.0 * scale.0) / 2.0,
                (size.1 - span.1 * scale.1) / 2.0,
            ),
        })
    }

    /// Where world point `(x, y)` falls, in pixels.
    fn place(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (
            self.offset.0 + (x - self.origin.0) * self.scale.0,
            self.offset.1 + (y - self.origin.1) * self.scale.1,
        )
    }
}

/// The pixel holding continuous pixel coordinate `v`, if any of `0..len` does.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "floored, checked non-negative and below a canvas side"
)]
fn pixel(v: f64, len: usize) -> Option<usize> {
    let floor = v.floor();
    let len = f64::from(u32::try_from(len).ok()?);
    (floor >= 0.0 && floor < len).then_some(floor as usize)
}

impl<K: Key> GraphView<K> {
    /// Draws the whole graph small onto `surface` (plan §18's minimap): every node as one dot
    /// where the layout puts it, and under them the rectangle of what the view now shows, both
    /// fitted to the surface together with a one-cell margin, so the rectangle is always in
    /// sight; the selected node's whole cell is marked. Edges are not drawn, and nodes may
    /// share a cell. The view is not changed: its own frame,
    /// layout and pointer origin stay as they were. A surface under two cells a side, or a
    /// graph with nothing laid out, draws nothing.
    pub fn render_overview<S: Surface + ?Sized>(&self, surface: &mut S) {
        let (cols, rows) = surface.size();
        if cols < 2 || rows < 2 {
            return;
        }
        let options = &self.render_options;
        let world =
            |ix: crate::graph::NodeIx| self.force.positions.get(ix.slot()).copied().flatten();
        let placed = self
            .store
            .nodes_in_order()
            .iter()
            .filter_map(|&ix| world(ix));
        if placed.clone().next().is_none() {
            return;
        }
        let frame = self.shown_corners();
        let sub = options.blitter.sub_size();
        let everything = placed.clone().chain(frame.into_iter().flatten());
        let Some(fit) = Fit::of(everything, (cols, rows), sub, options.cell_aspect) else {
            return;
        };
        let new_canvas = || {
            let mut canvas = SubCellCanvas::new(options.blitter, cols, rows);
            canvas.set_cell_aspect(options.cell_aspect);
            canvas.set_line_glyphs(options.lines);
            canvas
        };
        let style = |slot: ColorSlot| draw::style(StyleId(slot.0));
        if let Some(corners) = frame {
            let points = corners.map(|at| {
                let (px, py) = fit.place(at);
                (px - 0.5, py - 0.5)
            });
            let mut under = new_canvas();
            under.set_pen(ColorSlot(VIEWPORT.0));
            under.draw_polyline(&[points[0], points[1], points[2], points[3], points[0]]);
            under.blit(surface, style);
        }
        // Nodes go over the rectangle, as boxes go over edges in the view: a cell with both
        // shows the node, which an ascii cell could not if the two merged into one glyph.
        let mut canvas = new_canvas();
        let (w, h) = (canvas.pixel_width(), canvas.pixel_height());
        canvas.set_pen(ColorSlot(NODE.0));
        for at in placed {
            let (px, py) = fit.place(at);
            if let (Some(x), Some(y)) = (pixel(px, w), pixel(py, h)) {
                canvas.set_pixel(x, y);
            }
        }
        if let Some(at) = self.controller.selected.and_then(world) {
            let (px, py) = fit.place(at);
            if let (Some(x), Some(y)) = (pixel(px, w), pixel(py, h)) {
                let (sx, sy) = (usize::from(sub.0), usize::from(sub.1));
                let (cx, cy) = (x / sx * sx, y / sy * sy);
                canvas.set_pen(ColorSlot(NODE_SELECTED.0));
                for dy in 0..sy {
                    for dx in 0..sx {
                        canvas.set_pixel(cx + dx, cy + dy);
                    }
                }
            }
        }
        canvas.blit(surface, style);
    }

    /// The world points at the corners of what the view shows, clockwise from its top left:
    /// its frame's outer edges, half a cell past its first and last cells' centers, back
    /// through its pan, zoom and fit. `None` before a first layout.
    fn shown_corners(&self) -> Option<[(f64, f64); 4]> {
        let grid = self.viewport.grid();
        let (right, bottom) = (f64::from(grid.cols) - 0.5, f64::from(grid.rows) - 0.5);
        let at = |x, y| self.viewport.world_at(SubPt::new(x, y));
        Some([
            at(-0.5, -0.5)?,
            at(right, -0.5)?,
            at(right, bottom)?,
            at(-0.5, bottom)?,
        ])
    }
}

#[cfg(test)]
#[path = "overview_tests.rs"]
mod tests;
