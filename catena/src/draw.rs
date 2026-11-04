//! Drawing a view (plan §7.4): its layout as a [`SceneGraph`], and that scene onto a surface.
//!
//! Every edge becomes an `EdgePath` on [`Layer::EdgesUnder`] whose route runs straight from its
//! source's anchor to its target's; a self-loop's route is its node's anchor alone. Every node
//! becomes a `NodeBox` on [`Layer::Nodes`], bounded by its box as drawn. Both go in the store's
//! canonical order (plan §4.1), so the scene depends on the graph alone, never on insertion
//! order or slot numbers. The compositor's label mask keeps each line off the boxes it joins,
//! and draws boxes over lines.
//!
//! Not yet drawn as plan §7.3 has it: parallel edges share one line until they fan out, a
//! self-loop shows no arc, curved and dashed classes draw straight and solid, and no edge has
//! an arrowhead or crossing hop. Until plan §12's themes, items draw in two placeholder
//! styles.

use std::fmt;

use crate::geometry::{ResolvedMetrics, Viewport};
use crate::graph::{GraphStore, Key};
use crate::interact::hit::HitMap;
use crate::raster::{CellStyle, PaletteColor};
use crate::scene::{Compositor, EdgeRoute, Layer, Payload, Route, SceneGraph, SceneItem, StyleId};

/// The placeholder style of node boxes and their text.
pub(crate) const NODE: StyleId = StyleId(0);
/// The placeholder style of edges.
pub(crate) const EDGE: StyleId = StyleId(1);

/// The placeholder styles: nodes in the terminal's white, edges in its grey (16-color codes,
/// which every color terminal has).
pub(crate) fn style(id: StyleId) -> CellStyle {
    match id {
        EDGE => CellStyle::new(PaletteColor::Ansi(8)),
        _ => CellStyle::new(PaletteColor::Ansi(7)),
    }
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
    /// a box or the viewport clears it, and the next frame or hit-test rebuilds both.
    pub(crate) current: bool,
}

impl Drawing {
    /// The view changed: the next frame rebuilds its scene.
    pub(crate) fn invalidate(&mut self) {
        self.current = false;
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

/// Fills `scene` with `store` as `viewport` draws it, every box as `metrics` measures it.
pub(crate) fn build<K: Key>(
    store: &GraphStore<K>,
    metrics: &ResolvedMetrics,
    viewport: &Viewport,
    scene: &mut SceneGraph,
) {
    scene.clear();
    for &ix in store.edges_in_order() {
        let Some(edge) = store.edge(ix) else {
            continue;
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
            Layer::EdgesUnder,
            bounds,
            Payload::EdgePath {
                ix,
                route: EdgeRoute::Path(route),
            },
            EDGE,
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
            scene.push(SceneItem::new(
                Layer::Nodes,
                bounds,
                Payload::NodeBox { ix },
                NODE,
            ));
        }
    }
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod tests;
