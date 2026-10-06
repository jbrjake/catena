//! The compositor (plan §7.4): it draws a scene onto a surface and adds nothing. It owns
//! z-order: polylines go into one sub-cell canvas per layer, OR-merged bottom to top with each
//! cell colored by the topmost layer that lights it, then blitted once around the label mask;
//! `Orthogonal` routes become box-drawing glyphs; glow halos tint backgrounds; node forms and
//! count badges draw last, in draw order.

use super::nodes::{NodeGlyphs, draw_form};
use super::orthogonal::{Arms, BoxGlyphs};
use super::{Decoration, EdgeRoute, Layer, Payload, Route, SceneGraph, StyleId};
use crate::geometry::NodeForm;
use crate::geometry::cell::CellBox;
use crate::graph::NodeIx;
use crate::raster::{
    Blitter, CellStyle, ColorSlot, LineGlyphs, SubCellCanvas, Surface, text_cells,
};

/// How a scene is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    /// The encoder of polyline routes.
    pub blitter: Blitter,
    /// A cell's width over its height (plan §6), by which the ascii blitter judges directions.
    pub cell_aspect: f64,
    /// The ascii blitter's line glyphs.
    pub lines: LineGlyphs,
    /// The glyphs of `Orthogonal` routes and of a boxed node's border.
    pub boxes: BoxGlyphs,
    /// The brackets, ellipsis, pin marker and dot of node forms.
    pub nodes: NodeGlyphs,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            blitter: Blitter::Braille,
            cell_aspect: 0.5,
            lines: LineGlyphs::BOX,
            boxes: BoxGlyphs::BOX,
            nodes: NodeGlyphs::UNICODE,
        }
    }
}

/// Draws scenes, keeping its buffers from frame to frame so a steady frame allocates nothing.
pub struct Compositor {
    /// One canvas per layer, in `Layer::ALL` order.
    layers: Vec<SubCellCanvas>,
    /// Per cell, whether text owns it, so no edge draws there.
    masked: Vec<bool>,
    arms: Arms,
    /// A polyline's points in sub-pixels.
    points: Vec<(f64, f64)>,
    width: u16,
    height: u16,
}

impl Default for Compositor {
    fn default() -> Self {
        Compositor::new()
    }
}

impl Compositor {
    /// A compositor with nothing allocated yet.
    #[must_use]
    pub fn new() -> Self {
        Compositor {
            layers: Vec::new(),
            masked: Vec::new(),
            arms: Arms::default(),
            points: Vec::new(),
            width: 0,
            height: 0,
        }
    }

    /// Draws `scene` onto `surface`, sized as the surface is. `styles` resolves each item's
    /// style; `forms` gives each node's measured form, drawn from the top left of its item's
    /// bounds, and a node with none draws nothing. Total: any scene draws, clipped to the
    /// surface.
    pub fn render<'f, S: Surface + ?Sized>(
        &mut self,
        scene: &SceneGraph,
        surface: &mut S,
        options: &RenderOptions,
        styles: impl Fn(StyleId) -> CellStyle,
        forms: impl Fn(NodeIx) -> Option<&'f NodeForm>,
    ) {
        let (width, height) = surface.size();
        self.prepare(options, width, height);
        self.mask_text(scene);

        for (layer, canvas) in Layer::ALL.into_iter().zip(&mut self.layers) {
            for item in scene.items().iter().filter(|item| item.z == layer) {
                let (Payload::EdgePath {
                    route: EdgeRoute::Path(route),
                    ..
                }
                | Payload::Segment { route, .. }) = &item.payload
                else {
                    continue;
                };
                match route {
                    Route::Polyline(points) => {
                        let (sub_w, sub_h) = options.blitter.sub_size();
                        let (sub_w, sub_h) = (f64::from(sub_w), f64::from(sub_h));
                        self.points.clear();
                        // A point lies in the cell it rounds to: centre it in that cell's
                        // sub-pixels.
                        self.points.extend(points.iter().map(|p| {
                            (
                                p.x * sub_w + (sub_w - 1.0) / 2.0,
                                p.y * sub_h + (sub_h - 1.0) / 2.0,
                            )
                        }));
                        canvas.set_pen(ColorSlot(item.style.0));
                        canvas.draw_polyline(&self.points);
                    }
                    Route::Orthogonal(cells) => self.arms.add_route(cells, item.style),
                }
            }
        }

        if let Some((merged, upper)) = self.layers.split_first_mut() {
            for layer in upper.iter() {
                merged.merge_from(layer);
            }
            merged.blit_masked(surface, |slot| styles(StyleId(slot.0)), &self.masked);
        }
        self.draw_arms(surface, options, &styles);

        for item in scene.in_draw_order() {
            let style = styles(item.style);
            match &item.payload {
                Payload::Decoration(Decoration::Glow) => {
                    for (x, y) in self.visible(item.bounds) {
                        surface.patch_bg(x, y, style.bg.unwrap_or(style.fg));
                    }
                }
                Payload::NodeBox { ix } => {
                    if let Some(form) = forms(*ix) {
                        let glyphs = (&options.nodes, &options.boxes);
                        draw_form(surface, item.bounds, form, style, glyphs);
                    }
                }
                _ => {}
            }
            if let Some(badge) = item.badge {
                let mut digits = [0u8; 10];
                draw_text(surface, item.bounds, badge.at.x, badge.at.y, "×", style);
                draw_text(
                    surface,
                    item.bounds,
                    badge.at.x.saturating_add(1),
                    badge.at.y,
                    decimal(badge.count, &mut digits),
                    style,
                );
            }
        }
    }

    /// Sizes and clears every buffer for a `width × height` frame in `options`' blitter.
    fn prepare(&mut self, options: &RenderOptions, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        if self
            .layers
            .first()
            .is_none_or(|canvas| canvas.blitter() != options.blitter)
        {
            self.layers.clear();
            self.layers
                .extend(Layer::ALL.map(|_| SubCellCanvas::new(options.blitter, width, height)));
        }
        for canvas in &mut self.layers {
            canvas.clear_and_resize(width, height);
            canvas.set_cell_aspect(options.cell_aspect);
            canvas.set_line_glyphs(options.lines);
        }
        let (cols, rows) = (self.layers[0].cell_cols(), self.layers[0].cell_rows());
        self.masked.clear();
        self.masked.resize(cols * rows, false);
        self.arms
            .reset(width, u16::try_from(rows).unwrap_or(height));
    }

    /// Marks the cells text will own: every node box, and every count badge.
    fn mask_text(&mut self, scene: &SceneGraph) {
        let cols = usize::from(self.width);
        for item in scene.items() {
            if let Payload::NodeBox { .. } = item.payload {
                for (x, y) in self.visible(item.bounds) {
                    self.masked[usize::from(y) * cols + usize::from(x)] = true;
                }
            }
            if let Some(badge) = item.badge {
                let digits = decimal(badge.count, &mut [0u8; 10]).chars().count();
                let width = u32::try_from(digits + 1).unwrap_or(u32::MAX);
                let cells = CellBox::new(badge.at.x, badge.at.y, width, 1);
                for (x, y) in self.visible(cells) {
                    if item
                        .bounds
                        .contains(crate::geometry::cell::CellPt::new(x.into(), y.into()))
                    {
                        self.masked[usize::from(y) * cols + usize::from(x)] = true;
                    }
                }
            }
        }
    }

    fn draw_arms<S: Surface + ?Sized>(
        &self,
        surface: &mut S,
        options: &RenderOptions,
        styles: &impl Fn(StyleId) -> CellStyle,
    ) {
        let cols = usize::from(self.width);
        for y in 0..self.arms.height {
            for x in 0..self.arms.width {
                let masked = self
                    .masked
                    .get(usize::from(y) * cols + usize::from(x))
                    .copied();
                if let (Some(arms), Some(style), Some(false)) =
                    (self.arms.arms(x, y), self.arms.style(x, y), masked)
                {
                    surface.put_char(x, y, options.boxes.glyph(arms), styles(style));
                }
            }
        }
    }

    /// The cells of `bounds` on the frame, row by row.
    fn visible(&self, bounds: CellBox) -> impl Iterator<Item = (u16, u16)> + use<> {
        let clamp = |lo: i64, hi: i64, size: u16| {
            let size = i64::from(size);
            let (lo, hi) = (lo.clamp(0, size), hi.clamp(0, size));
            (
                u16::try_from(lo).unwrap_or(0),
                u16::try_from(hi).unwrap_or(0),
            )
        };
        let (x0, x1) = clamp(i64::from(bounds.x), bounds.right(), self.width);
        let rows = u16::try_from(self.masked.len() / usize::from(self.width.max(1))).unwrap_or(0);
        let (y0, y1) = clamp(i64::from(bounds.y), bounds.bottom(), rows);
        (y0..y1).flat_map(move |y| (x0..x1).map(move |x| (x, y)))
    }
}

/// Writes `text` from cell `(x, y)` rightwards, cell by cell as `text_cells` splits it, keeping
/// to `bounds` and to the surface: a glyph that would cross either edge is dropped, and drawing
/// stops at the right.
fn draw_text<S: Surface + ?Sized>(
    surface: &mut S,
    bounds: CellBox,
    x: i32,
    y: i32,
    text: &str,
    style: CellStyle,
) {
    let row_inside = i64::from(y) >= i64::from(bounds.y) && i64::from(y) < bounds.bottom();
    let Ok(row) = u16::try_from(y) else {
        return;
    };
    if !row_inside {
        return;
    }
    let mut column = i64::from(x);
    for cell in text_cells(text) {
        let end = column + i64::from(cell.width);
        if end > bounds.right() {
            break;
        }
        if column >= i64::from(bounds.x)
            && let Ok(at) = u16::try_from(column)
        {
            surface.put(at, row, cell.symbol, style);
        }
        column = end;
    }
}

/// `n` in decimal, written into `buffer`.
fn decimal(mut n: u32, buffer: &mut [u8; 10]) -> &str {
    let mut start = buffer.len();
    loop {
        start -= 1;
        buffer[start] = b'0' + u8::try_from(n % 10).unwrap_or(0);
        n /= 10;
        if n == 0 {
            break;
        }
    }
    std::str::from_utf8(&buffer[start..]).unwrap_or("")
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
