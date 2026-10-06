//! Grid snapping (plan §6): world positions to canonical cells, and canonical cells to the
//! derived cells a frame draws.
//!
//! The force layout runs in isotropic world space; a snap maps it onto the viewport's grid in
//! two steps. [`fit`] scales the world into the grid, applying the cell's aspect once, here,
//! and the zoom the snap runs at (`ref_zoom`); [`resolve`] then rounds each node's anchor to a
//! cell and moves the nodes whose boxes would collide, hubs first, along one bounded spiral.
//! The result is canonical: pan-independent, at `ref_zoom`. Every frame [`derive`]s view cells
//! from it in O(n), so pan and zoom never run a layout.
//!
//! Harvested from the seed's `snap_to_grid` (`seed/graph/layout_fr.rs`), with plan §6's fixes:
//! one scale for both axes (`Fit::Contain`), where the seed normalized x and y separately and
//! stretched the layout to fill the viewport (ledger row 9); off-screen nodes take part in
//! collision resolution, where the seed's `on_screen` test reserved their cells but never
//! resolved them (row 7); one spiral bounded by 50 rings, where the seed had two copies bounded
//! by 50 attempts; and the 2-row gap between stacked nodes enforced, where the seed promised it
//! in a comment and tested label cells only (row 28).

use std::collections::HashSet;
use std::hash::BuildHasherDefault;

use super::ResolvedMetrics;
use super::cell::{CellBox, CellPt, SubPt};
use crate::graph::{GraphStore, Key, NodeIx};

/// Cells kept clear along each edge of the grid, and the point zoom scales about.
pub(crate) const MARGIN: i32 = 1;

/// Empty rows kept between stacked boxes, so an edge always has a row to pass between them:
/// every box is grown by one row above and below for the collision test (plan §6).
const GAP: i32 = 2;

/// Rings the collision spiral walks before a node keeps its last candidate (plan §6).
const RINGS: u32 = 50;

/// The grid a snap fills: the viewport in cells, and a cell's width over its height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Grid {
    pub(crate) cols: u16,
    pub(crate) rows: u16,
    /// A cell's width over its height (plan §6): one world unit down is this many rows.
    pub(crate) cell_aspect: f64,
}

/// How the world maps onto the grid (plan §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Fit {
    /// One scale for both axes, the largest that shows every node, centered: the layout's
    /// distances survive, and the slack axis is letterboxed.
    #[default]
    Contain,
    /// Each axis scaled to fill the grid: an explicit opt-in, since it distorts distances.
    Stretch,
}

/// Where a snap put each node.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Snap {
    /// Per slot, the node's anchor cell (its box's middle, `NodeForm::anchor`); `None` for a
    /// vacant slot or a node with no position.
    pub(crate) anchors: Vec<Option<CellPt>>,
    /// The nodes moved off the cell their fit rounds to, in placement order.
    pub(crate) displaced: Vec<NodeIx>,
    /// The nodes the spiral found no free place for, which keep its last candidate and
    /// overlap: a subset of `displaced`.
    pub(crate) overlapping: Vec<NodeIx>,
}

impl Snap {
    /// Each placed node's box: its form's size, around its anchor.
    pub(crate) fn boxes<'a>(
        &'a self,
        metrics: &'a ResolvedMetrics,
    ) -> impl Iterator<Item = (NodeIx, CellBox)> + 'a {
        self.anchors
            .iter()
            .enumerate()
            .filter_map(|(slot, anchor)| {
                let ix = NodeIx::new(u32::try_from(slot).ok()?);
                let form = metrics.form(ix)?;
                Some((ix, Footprint::of(form).at((*anchor)?)))
            })
    }

    /// The box around every node's box; `None` with no node placed.
    pub(crate) fn bounds(&self, metrics: &ResolvedMetrics) -> Option<CellBox> {
        CellBox::around(self.boxes(metrics).flat_map(|(_, b)| {
            let far = CellPt::new(last(b.x, b.width), last(b.y, b.height));
            [CellPt::new(b.x, b.y), far]
        }))
    }
}

/// The last cell of a run of `len` cells from `start`.
fn last(start: i32, len: u32) -> i32 {
    let end = i64::from(start) + i64::from(len) - 1;
    i32::try_from(end).unwrap_or(i32::MAX)
}

/// Fits the world positions `world` (node centers, by slot) into `grid` at `zoom`, returning
/// each node's anchor in fractional canonical cells: [`Transform::of`] then
/// [`Transform::wanted`].
pub(crate) fn fit(
    world: &[Option<(f64, f64)>],
    metrics: &ResolvedMetrics,
    grid: Grid,
    fit: Fit,
    zoom: f64,
) -> Vec<Option<SubPt>> {
    Transform::of(world, metrics, grid, fit).map_or_else(
        || vec![None; world.len()],
        |transform| transform.wanted(world, metrics, zoom),
    )
}

/// How a fit maps world space onto the grid at zoom 1: a world point `(x, y)` lands on column
/// `left + (x − x0) · sx` and row `top + (y − y0) · sy`, where `sy` includes the cell aspect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Transform {
    origin: (f64, f64),
    scale: (f64, f64),
    offset: (f64, f64),
}

impl Transform {
    /// The fit of the world positions `world` (node centers, by slot) into `grid`; `None`
    /// when no node has both a position and a form.
    ///
    /// The nodes' centers span the grid less [`MARGIN`] and half the widest and tallest box
    /// on each side, so every box fits. A world coordinate that is not finite reads as 0.
    pub(crate) fn of(
        world: &[Option<(f64, f64)>],
        metrics: &ResolvedMetrics,
        grid: Grid,
        fit: Fit,
    ) -> Option<Transform> {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let (mut widest, mut tallest) = (0u16, 0u16);
        let mut any = false;
        for (ix, (x, y)) in live(world, metrics) {
            any = true;
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            if let Some(form) = metrics.form(ix) {
                widest = widest.max(form.width());
                tallest = tallest.max(form.height());
            }
        }
        if !any {
            return None;
        }
        let margin = f64::from(MARGIN);
        let usable = |side: u16, box_side: u16| {
            (f64::from(side) - 2.0 * margin - f64::from(box_side)).max(1.0)
        };
        let (usable_w, usable_h) = (usable(grid.cols, widest), usable(grid.rows, tallest));
        let aspect = if grid.cell_aspect > 0.0 && grid.cell_aspect.is_finite() {
            grid.cell_aspect
        } else {
            0.5
        };
        // The world's extent in cells at scale 1: columns across, rows down.
        let (span_x, span_y) = (x1 - x0, (y1 - y0) * aspect);
        let scale = |usable: f64, span: f64| {
            if span > 0.0 {
                usable / span
            } else {
                f64::INFINITY
            }
        };
        let finite_or_zero = |s: f64| if s.is_finite() { s } else { 0.0 };
        let (sx, sy) = match fit {
            Fit::Contain => {
                let s = finite_or_zero(scale(usable_w, span_x).min(scale(usable_h, span_y)));
                (s, s)
            }
            Fit::Stretch => (
                finite_or_zero(scale(usable_w, span_x)),
                finite_or_zero(scale(usable_h, span_y)),
            ),
        };
        Some(Transform {
            origin: (x0, y0),
            scale: (sx, sy * aspect),
            offset: (
                margin + f64::from(widest) / 2.0 + (usable_w - span_x * sx) / 2.0,
                margin + f64::from(tallest) / 2.0 + (usable_h - span_y * sy) / 2.0,
            ),
        })
    }

    /// Where the world point `(x, y)` lands at `zoom`, in fractional cells: the fit at zoom 1,
    /// scaled about the margin as [`derive`] scales. A coordinate that is not finite reads as 0.
    pub(crate) fn place(&self, (x, y): (f64, f64), zoom: f64) -> SubPt {
        let margin = f64::from(MARGIN);
        let (x, y) = (finite(x), finite(y));
        let col = self.offset.0 + (x - self.origin.0) * self.scale.0;
        let row = self.offset.1 + (y - self.origin.1) * self.scale.1;
        SubPt::new(
            margin + (col - margin) * zoom,
            margin + (row - margin) * zoom,
        )
    }

    /// Each node's anchor in fractional canonical cells at `zoom`, by slot: where its center
    /// in `world` lands.
    pub(crate) fn wanted(
        &self,
        world: &[Option<(f64, f64)>],
        metrics: &ResolvedMetrics,
        zoom: f64,
    ) -> Vec<Option<SubPt>> {
        let mut out = vec![None; world.len()];
        for (ix, p) in live(world, metrics) {
            out[ix.slot()] = Some(self.place(p, zoom));
        }
        out
    }
}

fn finite(v: f64) -> f64 {
    if v.is_finite() { v } else { 0.0 }
}

/// The nodes with both a world position and a form, and their positions made finite.
fn live<'a>(
    world: &'a [Option<(f64, f64)>],
    metrics: &'a ResolvedMetrics,
) -> impl Iterator<Item = (NodeIx, (f64, f64))> + 'a {
    world.iter().enumerate().filter_map(|(slot, p)| {
        let (x, y) = (*p)?;
        let ix = NodeIx::new(u32::try_from(slot).ok()?);
        metrics.form(ix).map(|_| (ix, (finite(x), finite(y))))
    })
}

/// Rounds each wanted anchor (by slot) to a cell and resolves collisions, placing the nodes in
/// `order`: a node whose box, grown by one row above and below, meets one already placed walks
/// the [`spiral`] to the first free candidate, or keeps its last one and overlaps.
pub(crate) fn resolve(
    wanted: &[Option<SubPt>],
    metrics: &ResolvedMetrics,
    order: &[NodeIx],
) -> Snap {
    let mut snap = Snap {
        anchors: vec![None; wanted.len()],
        ..Snap::default()
    };
    let mut taken = Taken::default();
    for &ix in order {
        let (Some(want), Some(form)) = (wanted.get(ix.slot()).copied().flatten(), metrics.form(ix))
        else {
            continue;
        };
        let footprint = Footprint::of(form);
        let start = want.cell();
        let mut at = start;
        if taken.meets(footprint.at(at)) {
            snap.displaced.push(ix);
            let step = (i32::from(form.width()), i32::from(form.height()) + GAP);
            let mut free = None;
            for candidate in spiral(start, step) {
                at = candidate;
                if !taken.meets(footprint.at(candidate)) {
                    free = Some(candidate);
                    break;
                }
            }
            if free.is_none() {
                snap.overlapping.push(ix);
            }
        }
        taken.claim(footprint.at(at));
        snap.anchors[ix.slot()] = Some(at);
    }
    snap
}

/// The cell a canonical cell is drawn at (plan §6): `(c − margin) × ratio + margin + pan`,
/// with `ratio` the zoom over the zoom the snap ran at, rounded once, half up.
///
/// Half up, `⌊v + ½⌋`, because it commutes with whole cells: a pan grown by `d` whole cells
/// moves every derived cell by exactly `d`, which is what lets anchor compensation and the
/// zoom-out re-snap leave a node exactly where it was drawn.
pub(crate) fn derive(canonical: CellPt, ratio: f64, pan: (f64, f64)) -> CellPt {
    let margin = f64::from(MARGIN);
    let axis = |c: i32, pan: f64| half_up((f64::from(c) - margin) * ratio + margin + pan);
    CellPt::new(axis(canonical.x, pan.0), axis(canonical.y, pan.1))
}

/// `⌊v + ½⌋`, saturating to the `i32` range, NaN to 0.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the float-to-int cast saturates, and NaN becomes 0"
)]
pub(crate) fn half_up(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}

/// The order a snap places nodes in: pinned nodes first, then by degree, most first (hubs
/// claim cells first), each in canonical order.
pub(crate) fn placement_order<K: Key>(store: &GraphStore<K>) -> Vec<NodeIx> {
    let mut order: Vec<(usize, NodeIx)> =
        store.nodes_in_order().iter().copied().enumerate().collect();
    let key = |&(place, ix): &(usize, NodeIx)| {
        let pinned = store
            .node(ix)
            .is_some_and(|node| node.spec.pinned.is_some());
        let degree = store.out_edges(ix).len() + store.in_edges(ix).len();
        (!pinned, std::cmp::Reverse(degree), place)
    };
    order.sort_by_key(key);
    order.into_iter().map(|(_, ix)| ix).collect()
}

/// The candidates a colliding node tries, from `start`: the seed's four-direction expanding
/// spiral, down, left, up and right by `step` (a box width across, its height plus the gap
/// down), each leg a step longer every other turn, for [`RINGS`] rings of two legs.
pub(crate) fn spiral(start: CellPt, step: (i32, i32)) -> impl Iterator<Item = CellPt> {
    (1..=2 * RINGS).scan(start, move |at, attempt| {
        let ring = i32::try_from(attempt.div_ceil(2)).unwrap_or(i32::MAX);
        let (dx, dy) = match attempt % 4 {
            1 => (0, step.1),
            2 => (-step.0, 0),
            3 => (0, -step.1),
            _ => (step.0, 0),
        };
        *at = CellPt::new(
            at.x.saturating_add(dx.saturating_mul(ring)),
            at.y.saturating_add(dy.saturating_mul(ring)),
        );
        Some(*at)
    })
}

/// A node's box, relative to its anchor.
#[derive(Debug, Clone, Copy)]
struct Footprint {
    size: (u16, u16),
    anchor: CellPt,
}

impl Footprint {
    fn of(form: &super::NodeForm) -> Footprint {
        Footprint {
            size: (form.width(), form.height()),
            anchor: form.anchor(),
        }
    }

    /// The box with its anchor at `at`.
    fn at(self, at: CellPt) -> CellBox {
        CellBox::new(
            at.x.saturating_sub(self.anchor.x),
            at.y.saturating_sub(self.anchor.y),
            u32::from(self.size.0),
            u32::from(self.size.1),
        )
    }
}

/// The cells placed boxes claim, each grown by one row above and below. A fixed hasher: the
/// set is only ever asked about, never iterated, and the core draws no random numbers.
#[derive(Default)]
struct Taken(HashSet<(i32, i32), BuildHasherDefault<std::hash::DefaultHasher>>);

impl Taken {
    /// The cells of `b` grown by one row above and below.
    fn grown(b: CellBox) -> impl Iterator<Item = (i32, i32)> {
        let rows = (b.y.saturating_sub(1))..=last(b.y, b.height).saturating_add(1);
        rows.flat_map(move |y| (b.x..=last(b.x, b.width)).map(move |x| (x, y)))
    }

    fn meets(&self, b: CellBox) -> bool {
        Taken::grown(b).any(|cell| self.0.contains(&cell))
    }

    fn claim(&mut self, b: CellBox) {
        self.0.extend(Taken::grown(b));
    }
}

#[cfg(test)]
#[path = "snap_tests.rs"]
mod tests;
