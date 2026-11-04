//! Hit-testing (plan §10.2): what a cell of the frame points at, read from the same scene the
//! renderer draws, so a target is exactly what is drawn.
//!
//! 1. A node: the point lies in its drawn box; or, missing every box, it lies within
//!    [`NODE_REACH`] cells of the node's anchor, by aspect-corrected distance, so a cell is as
//!    near across as down (`dx² + (dy / cell_aspect)²`). The seed measured raw Euclidean
//!    distance against the full label's width, so at low zoom a click took a neighbor.
//! 2. Edges: a route within [`EDGE_HIT_CELLS`] of the point, in cells. A shared segment
//!    stands for all its member edges (owner ruling A3).
//! 3. Nodes win over edges. Among nodes, or among edges, the topmost layer wins, then the
//!    smallest bounds (the most specific target), then the item first in canonical order,
//!    which is the scene's own order (the seed broke ties by hash-map order).
//!
//! Candidates come from a [`SpatialGrid`] over the frame, never from a scan of every item.

use std::cmp::Reverse;

use super::spatial::SpatialGrid;
use crate::geometry::cell::{CellBox, CellPt, SubPt};
use crate::graph::{EdgeIx, NodeIx};
use crate::scene::{EDGE_HIT_CELLS, EdgeRoute, Payload, Route, SceneGraph};

/// How far from its anchor a click still finds a node it missed, in aspect-corrected cells.
pub(crate) const NODE_REACH: f64 = 3.0;

/// What a point hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hit<'s> {
    Node(NodeIx),
    /// The edges one route item stands for: an edge, or a shared segment's members.
    Edges(&'s [EdgeIx]),
}

/// A scene's items, indexed for hit-testing.
#[derive(Debug, Clone, Default)]
pub(crate) struct HitMap {
    grid: SpatialGrid,
    /// Per scene item, a node's anchor cell.
    anchors: Vec<Option<CellPt>>,
    cell_aspect: f64,
}

/// `v` cells, rounded up to whole cells.
fn ceil_cells(v: f64) -> i32 {
    SubPt::new(v.ceil(), 0.0).cell().x
}

/// `b` grown by `dx` columns and `dy` rows on every side.
fn inflate(b: CellBox, dx: i32, dy: i32) -> CellBox {
    let grow = |len: u32, by: i32| len.saturating_add(u32::try_from(by).unwrap_or(0) * 2);
    CellBox::new(
        b.x.saturating_sub(dx),
        b.y.saturating_sub(dy),
        grow(b.width, dx),
        grow(b.height, dy),
    )
}

/// The route an item draws itself, if any.
fn route_of(payload: &Payload) -> Option<&Route> {
    match payload {
        Payload::EdgePath {
            route: EdgeRoute::Path(route),
            ..
        }
        | Payload::Segment { route, .. } => Some(route),
        _ => None,
    }
}

impl HitMap {
    /// Indexes `scene` over a `frame`-sized surface; `anchor` gives each node's anchor cell.
    pub(crate) fn rebuild(
        &mut self,
        scene: &SceneGraph,
        frame: (u16, u16),
        cell_aspect: f64,
        anchor: impl Fn(NodeIx) -> Option<CellPt>,
    ) {
        self.cell_aspect = if cell_aspect.is_finite() && cell_aspect > 0.0 {
            cell_aspect
        } else {
            0.5
        };
        // A node is reachable this far past its box; an edge its threshold past its route.
        let reach = (
            ceil_cells(NODE_REACH),
            ceil_cells(NODE_REACH * self.cell_aspect),
        );
        let margin = ceil_cells(EDGE_HIT_CELLS);
        self.anchors.clear();
        self.anchors
            .extend(scene.items().iter().map(|item| match item.payload {
                Payload::NodeBox { ix } => anchor(ix),
                _ => None,
            }));
        let items = scene.items().iter().enumerate().filter_map(|(i, item)| {
            let id = u32::try_from(i).ok()?;
            match &item.payload {
                Payload::NodeBox { .. } => Some((id, inflate(item.bounds, reach.0, reach.1))),
                payload => route_of(payload).map(|_| (id, inflate(item.bounds, margin, margin))),
            }
        });
        self.grid.rebuild(frame, items);
    }

    /// What cell `(x, y)` points at in `scene`, the scene this map was built from.
    pub(crate) fn hit<'s>(&self, scene: &'s SceneGraph, (x, y): (u16, u16)) -> Option<Hit<'s>> {
        let candidates = self.grid.query(x, y);
        let items = scene.items();
        let at = CellPt::new(i32::from(x), i32::from(y));
        let point = SubPt::from(at);
        let rank = |i: usize| (Reverse(items[i].z), items[i].bounds.area(), i);
        let index = |&id: &u32| usize::try_from(id).ok().filter(|&i| i < items.len());

        let in_box = candidates
            .iter()
            .filter_map(index)
            .filter(|&i| matches!(items[i].payload, Payload::NodeBox { .. }))
            .filter(|&i| items[i].bounds.contains(at))
            .min_by_key(|&i| rank(i));
        let node = in_box.or_else(|| {
            candidates
                .iter()
                .filter_map(index)
                .filter_map(|i| {
                    let a = self.anchors.get(i).copied().flatten()?;
                    let (dx, dy) = (
                        f64::from(a.x - at.x),
                        f64::from(a.y - at.y) / self.cell_aspect,
                    );
                    let d2 = dx * dx + dy * dy;
                    (d2 <= NODE_REACH * NODE_REACH).then_some((d2, i))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
                .map(|(_, i)| i)
        });
        if let Some(i) = node
            && let Payload::NodeBox { ix } = items[i].payload
        {
            return Some(Hit::Node(ix));
        }

        candidates
            .iter()
            .filter_map(index)
            .filter(|&i| {
                route_of(&items[i].payload).is_some_and(|r| r.distance_to(point) <= EDGE_HIT_CELLS)
            })
            .min_by_key(|&i| rank(i))
            .map(|i| Hit::Edges(items[i].edges()))
    }
}

#[cfg(test)]
#[path = "hit_tests.rs"]
mod tests;
