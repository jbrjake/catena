//! The scene graph: the complete display list a layout emits (plan §7.4, owner ruling A3).

use crate::geometry::cell::{CellBox, CellPt, SubPt};
use crate::graph::{EdgeIx, NodeIx};

/// The compositor's fixed z-order, bottom to top. Within a layer, items draw in insertion order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// Under everything.
    Background,
    /// Edges drawn under the others.
    EdgesUnder,
    /// Edges drawn over the others: highlighted or hover-lifted ones.
    EdgesOver,
    /// Glow halos, which tint backgrounds and keep the glyphs over them.
    Glow,
    /// Node boxes.
    Nodes,
    /// Text over the nodes.
    Labels,
    /// Over everything.
    Annotations,
}

impl Layer {
    /// Every layer, bottom to top.
    pub const ALL: [Layer; 7] = [
        Layer::Background,
        Layer::EdgesUnder,
        Layer::EdgesOver,
        Layer::Glow,
        Layer::Nodes,
        Layer::Labels,
        Layer::Annotations,
    ];
}

/// Which resolved style an item draws in; the renderer maps it to colors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StyleId(pub u16);

/// A shared segment's identity within its scene; only the scene makes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SegmentId(pub(super) u32);

/// Route geometry (plan §7.4).
#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    /// Points with sub-cell precision, joined by straight segments; any blitter rasterizes it.
    Polyline(Vec<SubPt>),
    /// Cells joined by axis-aligned runs, drawn as box-drawing glyphs whatever the blitter.
    Orthogonal(Vec<CellPt>),
}

impl Route {
    /// The cells of its first and last points, or `None` for a route with no points.
    #[must_use]
    pub fn ends(&self) -> Option<(CellPt, CellPt)> {
        match self {
            Route::Polyline(points) => Some((points.first()?.cell(), points.last()?.cell())),
            Route::Orthogonal(cells) => Some((*cells.first()?, *cells.last()?)),
        }
    }

    /// The smallest box holding every cell the route passes through, or `None` for no points.
    /// A point lies in the cell it rounds to, so the vertices' cells bound the whole route.
    #[must_use]
    pub fn bounds(&self) -> Option<CellBox> {
        match self {
            Route::Polyline(points) => CellBox::around(points.iter().map(|p| p.cell())),
            Route::Orthogonal(cells) => CellBox::around(cells.iter().copied()),
        }
    }

    /// The distance in cells from `at` to the nearest point on the route; infinite for a route
    /// with no points.
    #[must_use]
    pub fn distance_to(&self, at: SubPt) -> f64 {
        match self {
            Route::Polyline(points) => polyline_distance(points.iter().copied(), at),
            Route::Orthogonal(cells) => polyline_distance(cells.iter().map(|&c| c.into()), at),
        }
    }
}

fn polyline_distance(points: impl Iterator<Item = SubPt>, at: SubPt) -> f64 {
    let mut best = f64::INFINITY;
    let mut previous: Option<SubPt> = None;
    for point in points {
        let from = previous.unwrap_or(point);
        best = best.min(segment_distance(from, point, at));
        previous = Some(point);
    }
    best
}

fn segment_distance(a: SubPt, b: SubPt, p: SubPt) -> f64 {
    let (ux, uy) = (b.x - a.x, b.y - a.y);
    let len2 = ux * ux + uy * uy;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * ux + (p.y - a.y) * uy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (dx, dy) = (a.x + t * ux - p.x, a.y + t * uy - p.y);
    (dx * dx + dy * dy).sqrt()
}

/// An edge's route (owner ruling A3): its own path, or an ordered chain of shared segments
/// that connect its source anchor to its target anchor.
#[derive(Debug, Clone, PartialEq)]
pub enum EdgeRoute {
    /// The edge's own geometry, drawn by its item.
    Path(Route),
    /// Shared segments in order from source to target, each drawn once by its own item.
    Chain(Vec<SegmentId>),
}

/// A count drawn as `×n` with its `×` at cell `at`, which lies in the carrying item's bounds:
/// the parallel-edge badge (plan §7.3), made a decoration any item can carry (owner ruling A3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CountBadge {
    /// The number shown.
    pub count: u32,
    /// Where the badge starts.
    pub at: CellPt,
}

/// Geometry that belongs to no node or edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Decoration {
    /// A glow halo: the item's bounds take the style's background, keeping their glyphs.
    Glow,
}

/// What a scene item is.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Payload {
    /// A node's box, drawn with the node's label.
    NodeBox {
        /// The node.
        ix: NodeIx,
    },
    /// An edge and its route.
    EdgePath {
        /// The edge.
        ix: EdgeIx,
        /// Its geometry, or the shared segments that carry it.
        route: EdgeRoute,
    },
    /// A segment shared by several edges, drawn once (owner ruling A3).
    Segment {
        /// Its identity within the scene.
        id: SegmentId,
        /// Its geometry.
        route: Route,
        /// The edges it carries, ascending, each once: what hitting it selects.
        members: Vec<EdgeIx>,
    },
    /// A decoration.
    Decoration(Decoration),
}

/// One entry of the display list. If it is on screen it is in the scene; if it is in the scene
/// it has bounds and a payload (plan §7.4).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneItem {
    /// Its layer.
    pub z: Layer,
    /// Every cell it draws, decorations included, in derived cells; may extend past the
    /// viewport, which clips at render.
    pub bounds: CellBox,
    /// What it is.
    pub payload: Payload,
    /// How it is drawn.
    pub style: StyleId,
    /// A count badge it carries.
    pub badge: Option<CountBadge>,
}

impl SceneItem {
    /// An item with no badge.
    #[must_use]
    pub fn new(z: Layer, bounds: CellBox, payload: Payload, style: StyleId) -> Self {
        SceneItem {
            z,
            bounds,
            payload,
            style,
            badge: None,
        }
    }

    /// The item with `badge`.
    #[must_use]
    pub fn with_badge(self, badge: CountBadge) -> Self {
        SceneItem {
            badge: Some(badge),
            ..self
        }
    }

    /// The edges the item stands for: its edge, a shared segment's members, or none.
    #[must_use]
    pub fn edges(&self) -> &[EdgeIx] {
        match &self.payload {
            Payload::EdgePath { ix, .. } => std::slice::from_ref(ix),
            Payload::Segment { members, .. } => members,
            Payload::NodeBox { .. } | Payload::Decoration(_) => &[],
        }
    }

    /// The geometry the item draws itself, if it is a route.
    fn own_route(&self) -> Option<&Route> {
        match &self.payload {
            Payload::EdgePath {
                route: EdgeRoute::Path(route),
                ..
            }
            | Payload::Segment { route, .. } => Some(route),
            _ => None,
        }
    }
}

/// The edge hit threshold, in cells (plan §10.2).
pub const EDGE_HIT_CELLS: f64 = 1.5;

/// The complete display list. Not generic over the host's key: items carry indices, so building
/// a scene never clones a key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneGraph {
    pub(super) items: Vec<SceneItem>,
    /// For each segment id, its item's index.
    pub(super) segments: Vec<usize>,
}

impl SceneGraph {
    /// An empty scene.
    #[must_use]
    pub fn new() -> Self {
        SceneGraph::default()
    }

    /// Empties the scene, keeping its allocations.
    pub fn clear(&mut self) {
        self.items.clear();
        self.segments.clear();
    }

    /// The items in insertion order.
    #[must_use]
    pub fn items(&self) -> &[SceneItem] {
        &self.items
    }

    /// Adds `item`. A segment item gets the scene's next segment id, whatever its payload said
    /// ([`SceneGraph::push_segment`] returns it), and its members sorted, each kept once.
    pub fn push(&mut self, mut item: SceneItem) {
        if let Payload::Segment { id, members, .. } = &mut item.payload {
            *id = self.next_segment();
            members.sort_unstable();
            members.dedup();
            self.segments.push(self.items.len());
        }
        self.items.push(item);
    }

    /// Adds a segment carrying `members`, bounded by its route, and returns its id.
    pub fn push_segment(
        &mut self,
        z: Layer,
        style: StyleId,
        route: Route,
        members: Vec<EdgeIx>,
    ) -> SegmentId {
        let bounds = route.bounds().unwrap_or_default();
        let id = self.next_segment();
        self.push(SceneItem::new(
            z,
            bounds,
            Payload::Segment { id, route, members },
            style,
        ));
        id
    }

    fn next_segment(&self) -> SegmentId {
        SegmentId(u32::try_from(self.segments.len()).unwrap_or(u32::MAX))
    }

    /// The item of segment `id`.
    #[must_use]
    pub fn segment(&self, id: SegmentId) -> Option<&SceneItem> {
        let index = *self.segments.get(usize::try_from(id.0).ok()?)?;
        self.items.get(index)
    }

    /// The items in draw order: by layer, then insertion.
    pub fn in_draw_order(&self) -> impl Iterator<Item = &SceneItem> {
        Layer::ALL
            .into_iter()
            .flat_map(move |layer| self.items.iter().filter(move |item| item.z == layer))
    }

    /// The edges that the route item nearest the top within `threshold` cells of `at` stands
    /// for: one edge, or every member of a shared segment (owner ruling A3). The topmost layer
    /// wins, then the smallest bounds, then the earliest item (plan §10.2). Empty on a miss.
    #[must_use]
    pub fn edges_at(&self, at: SubPt, threshold: f64) -> &[EdgeIx] {
        let mut best: Option<(std::cmp::Reverse<Layer>, u64, usize)> = None;
        for (index, item) in self.items.iter().enumerate() {
            let Some(route) = item.own_route() else {
                continue;
            };
            if route.distance_to(at) <= threshold {
                let rank = (std::cmp::Reverse(item.z), item.bounds.area(), index);
                if best.is_none_or(|b| rank < b) {
                    best = Some(rank);
                }
            }
        }
        best.map_or(&[], |(_, _, index)| self.items[index].edges())
    }
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
