//! Drawing a view (plan §7.4): its layout as a [`SceneGraph`], and that scene onto a surface.
//!
//! Every edge becomes an `EdgePath` on [`Layer::EdgesUnder`] whose route runs straight from its
//! source's anchor to its target's; a self-loop's route is its node's anchor alone. Every node
//! becomes a `NodeBox` on [`Layer::Nodes`], bounded by its box as drawn. Both go in the store's
//! canonical order (plan §4.1), so the scene depends on the graph alone, never on insertion
//! order or slot numbers. The compositor's label mask keeps each line off the boxes it joins,
//! and draws boxes over lines.
//!
//! What the controller marks draws marked (plan §10.1, owner ruling A4): the selected node
//! reversed, the hovered node bold, and the hovered node's edges, or the hovered edges, lifted
//! onto [`Layer::EdgesOver`] in a highlight, so a lifted edge is drawn whole over any edge it
//! crosses. The selected edge is lifted too, in a highlight of its own.
//!
//! Not yet drawn as plan §7.3 has it: parallel edges share one line until they fan out, a
//! self-loop shows no arc, curved and dashed classes draw straight and solid, and no edge has
//! an arrowhead or crossing hop. Until plan §12's themes, items draw in placeholder styles.

use std::fmt;

use crate::geometry::{ResolvedMetrics, Viewport};
use crate::graph::{EdgeIx, GraphStore, Key, NodeIx};
use crate::interact::hit::HitMap;
use crate::raster::{Attrs, CellStyle, PaletteColor};
use crate::scene::{Compositor, EdgeRoute, Layer, Payload, Route, SceneGraph, SceneItem, StyleId};

/// The placeholder style of node boxes and their text.
pub(crate) const NODE: StyleId = StyleId(0);
/// The placeholder style of edges.
pub(crate) const EDGE: StyleId = StyleId(1);
/// The placeholder style of the selected node.
pub(crate) const NODE_SELECTED: StyleId = StyleId(2);
/// The placeholder style of the hovered node.
pub(crate) const NODE_HOVERED: StyleId = StyleId(3);
/// The placeholder style of hover-lifted edges.
pub(crate) const EDGE_LIFTED: StyleId = StyleId(4);
/// The placeholder style of the selected edge.
pub(crate) const EDGE_SELECTED: StyleId = StyleId(5);
/// The placeholder style of the overview's rectangle of what the view shows.
pub(crate) const VIEWPORT: StyleId = StyleId(6);

/// The placeholder styles, in 16-color codes, which every color terminal has: nodes in the
/// terminal's white, reversed when selected and bright and bold when hovered; edges in its
/// grey, bright white when lifted and bright yellow when selected; the overview's rectangle in
/// bright cyan.
pub(crate) fn style(id: StyleId) -> CellStyle {
    let with = |color: u8, attrs: Attrs| CellStyle {
        attrs,
        ..CellStyle::new(PaletteColor::Ansi(color))
    };
    match id {
        EDGE => with(8, Attrs::NONE),
        NODE_SELECTED => with(7, Attrs::REVERSED),
        NODE_HOVERED => with(15, Attrs::BOLD),
        EDGE_LIFTED => with(15, Attrs::NONE),
        EDGE_SELECTED => with(11, Attrs::BOLD),
        VIEWPORT => with(14, Attrs::NONE),
        _ => with(7, Attrs::NONE),
    }
}

/// What the controller marks for drawing: by index, valid for the commit it was taken at.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Marks<'a> {
    pub(crate) selected: Option<NodeIx>,
    pub(crate) hovered: Option<NodeIx>,
    /// The edges under the pointer, when it is over edges and not a node.
    pub(crate) hovered_edges: &'a [EdgeIx],
    pub(crate) selected_edge: Option<EdgeIx>,
}

/// What a view keeps from frame to frame to draw: its scene and compositor, whose buffers a
/// steady frame reuses (plan §15.2). Not state: a clone starts empty.
#[derive(Default)]
pub(crate) struct Drawing {
    pub(crate) scene: SceneGraph,
    pub(crate) compositor: Compositor,
    /// The scene indexed for hit-testing (plan §10.2), rebuilt with it.
    pub(crate) hits: HitMap,
    /// Whether `scene` and `hits` show the view as it is now. Anything that changes the graph,
    /// a box, the viewport or a mark clears it, and the next frame or hit-test rebuilds both.
    pub(crate) current: bool,
    /// Whether the last frame drawn shows the view as it is now: `current`, and drawn since.
    /// A hit-test rebuilds the scene without drawing it, so the two differ (plan §10.4's
    /// `Redraw`).
    pub(crate) shown: bool,
}

impl Drawing {
    /// The view changed: the next frame rebuilds its scene.
    pub(crate) fn invalidate(&mut self) {
        self.current = false;
        self.shown = false;
    }
}

impl Clone for Drawing {
    fn clone(&self) -> Self {
        Drawing::default()
    }
}

impl fmt::Debug for Drawing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Drawing")
            .field("items", &self.scene.items().len())
            .finish_non_exhaustive()
    }
}

/// Fills `scene` with `store` as `viewport` draws it, every box as `metrics` measures it and
/// what `marks` marks drawn marked.
pub(crate) fn build<K: Key>(
    store: &GraphStore<K>,
    metrics: &ResolvedMetrics,
    viewport: &Viewport,
    marks: Marks<'_>,
    scene: &mut SceneGraph,
) {
    scene.clear();
    for &ix in store.edges_in_order() {
        let Some(edge) = store.edge(ix) else {
            continue;
        };
        let lifted = marks
            .hovered
            .is_some_and(|node| edge.from == node || edge.to == node)
            || marks.hovered_edges.contains(&ix);
        let (layer, style) = if marks.selected_edge == Some(ix) {
            (Layer::EdgesOver, EDGE_SELECTED)
        } else if lifted {
            (Layer::EdgesOver, EDGE_LIFTED)
        } else {
            (Layer::EdgesUnder, EDGE)
        };
        let (Some(from), Some(to)) = (viewport.cell(edge.from), viewport.cell(edge.to)) else {
            continue;
        };
        let points = if from == to {
            vec![from.into()]
        } else {
            vec![from.into(), to.into()]
        };
        let route = Route::Polyline(points);
        let bounds = route.bounds().unwrap_or_default();
        scene.push(SceneItem::new(
            layer,
            bounds,
            Payload::EdgePath {
                ix,
                route: EdgeRoute::Path(route),
            },
            style,
        ));
    }

    let mut boxes = vec![None; store.nodes.len()];
    for (ix, bounds) in viewport.boxes(metrics) {
        if let Some(slot) = boxes.get_mut(ix.slot()) {
            *slot = Some(bounds);
        }
    }
    for &ix in store.nodes_in_order() {
        if let Some(bounds) = boxes.get(ix.slot()).copied().flatten() {
            let style = if marks.selected == Some(ix) {
                NODE_SELECTED
            } else if marks.hovered == Some(ix) {
                NODE_HOVERED
            } else {
                NODE
            };
            scene.push(SceneItem::new(
                Layer::Nodes,
                bounds,
                Payload::NodeBox { ix },
                style,
            ));
        }
    }
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod tests;
