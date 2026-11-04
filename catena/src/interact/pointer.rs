//! The pointer (plan §10.1, §10.2): a press selects what it hits and can start a node drag,
//! a move hovers or drags, and a release ends a drag.
//!
//! A press on a node selects it, on an edge selects the edge (the first of a shared
//! segment's), and on nothing clears both selections. A drag (new in catena: the seed dragged
//! windows, never nodes) pins the node under the pointer for its duration, keeping the
//! pointer's offset from the node's anchor, and on release puts its pin back as it was and
//! reports where the node ended. A hover change redraws the hovered node's edges, or the
//! hovered edges, lifted (owner ruling A4); a pointer moving within one target changes
//! nothing and, once warm, allocates nothing.

use super::controller::Outcome;
use super::hit::Hit;
use crate::GraphView;
use crate::geometry::cell::SubPt;
use crate::graph::{EdgeIx, Key, NodeIx};

/// A node being dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Drag {
    pub(super) node: NodeIx,
    /// The node's pin before the drag.
    pin: Option<(f64, f64)>,
    /// The node's anchor less the pointer, in cells.
    grab: (i32, i32),
    /// The pointer's last cell.
    at: (u16, u16),
    /// Whether the node moved: a press and release that never left one cell is a click.
    moved: bool,
}

/// What the pointer is on, copied out of the scene.
enum Target {
    Node(NodeIx),
    /// The first of the edges under the pointer.
    Edge(EdgeIx),
}

impl<K: Key> GraphView<K> {
    /// A press of the primary button at cell `(x, y)`.
    pub(super) fn press(&mut self, x: u16, y: u16, out: &mut Vec<Outcome>) {
        let target = self.hit(x, y).and_then(|hit| match hit {
            Hit::Node(ix) => Some(Target::Node(ix)),
            Hit::Edges(edges) => edges.first().copied().map(Target::Edge),
        });
        match target {
            Some(Target::Node(ix)) => {
                self.choose(Some(ix), None, out);
                let pin = self.store.node(ix).and_then(|node| node.spec.pinned);
                self.controller.drag = self.viewport.cell(ix).map(|anchor| Drag {
                    node: ix,
                    pin,
                    grab: (anchor.x - i32::from(x), anchor.y - i32::from(y)),
                    at: (x, y),
                    moved: false,
                });
            }
            Some(Target::Edge(ix)) => {
                let id = self.store.edge(ix).map(|edge| edge.id);
                self.choose(None, id, out);
            }
            None => self.choose(None, None, out),
        }
    }

    /// The pointer moved to cell `(x, y)`: drags the dragged node there, or hovers.
    pub(super) fn point(&mut self, x: u16, y: u16, out: &mut Vec<Outcome>) {
        if let Some(drag) = self.controller.drag {
            if drag.at != (x, y)
                && let Some(drag) = self.drag_to(drag, x, y)
            {
                self.controller.drag = Some(drag);
            }
            return;
        }
        self.refresh();
        let hit = self.drawing.hits.hit(&self.drawing.scene, (x, y));
        let (node, edges): (Option<NodeIx>, &[EdgeIx]) = match hit {
            Some(Hit::Node(ix)) => (Some(ix), &[]),
            Some(Hit::Edges(edges)) => (None, edges),
            None => (None, &[]),
        };
        let controller = &mut self.controller;
        let node_changed = controller.hovered != node;
        let edges_changed = controller.hovered_edges != edges;
        if edges_changed {
            controller.hovered_edges.clear();
            controller.hovered_edges.extend_from_slice(edges);
        }
        if node_changed {
            controller.hovered = node;
            out.push(Outcome::Hovered(node));
        }
        if node_changed || edges_changed {
            self.drawing.invalidate();
        }
    }

    /// A release of the primary button at cell `(x, y)`: ends a drag there.
    pub(super) fn release(&mut self, x: u16, y: u16, out: &mut Vec<Outcome>) {
        let Some(mut drag) = self.controller.drag.take() else {
            return;
        };
        if drag.at != (x, y) {
            drag = self.drag_to(drag, x, y).unwrap_or(drag);
        }
        if !drag.moved {
            return;
        }
        let Some(world) = self.store.node(drag.node).and_then(|node| node.spec.pinned) else {
            return;
        };
        if drag.pin.is_none()
            && let Some(key) = self.store.key(drag.node).cloned()
        {
            // Unpinned where it was dropped: the layout's position stays at the pin. The key
            // was just read from the store, so the edit cannot fail.
            let _ = self.update(|tx| tx.set_node(&key, |n| n.pinned = None));
        }
        out.push(Outcome::DragCompleted(drag.node, world));
    }

    /// Pins the dragged node at the world point under cell `(x, y)` plus its grab. Returns
    /// the drag as it now is, or `None` when the node could not move.
    fn drag_to(&mut self, mut drag: Drag, x: u16, y: u16) -> Option<Drag> {
        let at = SubPt::new(
            f64::from(i32::from(x) + drag.grab.0),
            f64::from(i32::from(y) + drag.grab.1),
        );
        let world = self.viewport.world_at(at)?;
        let key = self.store.key(drag.node).cloned()?;
        self.update(|tx| tx.set_node(&key, |n| n.pinned = Some(world)))
            .ok()?;
        drag.at = (x, y);
        drag.moved = true;
        Some(drag)
    }
}

#[cfg(test)]
#[path = "pointer_tests.rs"]
mod tests;
