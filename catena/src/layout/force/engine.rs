//! The force layout over a graph (plan §8): which nodes the simulation moves, which ring the
//! core, and where the islands of a disconnected graph go.
//!
//! Nodes joined by a layout edge (one that is `layout_participating` and not a self-loop) form
//! the core, and each connected component of it is simulated as its own island, so repulsion
//! between components cannot fling them apart. Islands new to the layout are packed left to
//! right, largest first, with gutters, after any island already placed; an island with a placed
//! or pinned node stays where its simulation leaves it, so a relayout moves no island wholesale.
//! Every other node goes to the peripheral ring (plan §8.2).
//!
//! A relayout follows one [`Change`] (the owner's "Relayout" ruling). After a topology change
//! or a semantic level change, it reaches `tether_reach` hops from what changed: the nodes
//! within reach move as their forces say, each tethered to where it was, loosely next to the
//! change and firmly at the edge of the reach, and every node beyond holds its place (see
//! [`mobility`]). After a resize every node is free, and islands and ring are placed afresh.
//!
//! Everything iterates in the store's canonical order, so the result depends on the graph alone,
//! never on insertion order or slot numbers.

use std::collections::BTreeSet;

use super::mobility::{self, Mobility};
use super::params::ForceParams;
use super::ring::{self, Placed, RingNode};
use super::simulation::{
    Body, Schedule, Spring, base_distance, centroid, ideal_distance, simulate,
};
use crate::fmath;
use crate::geometry::ResolvedMetrics;
use crate::graph::{GraphStore, Key, NodeIx};

/// Columns of empty space between islands (plan §8.2).
const GUTTER: f64 = 4.0;

/// The area a layout spreads over, and the cell's shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Frame {
    /// The usable viewport in world units: its columns, and its rows over `cell_aspect`.
    pub(crate) area: (f64, f64),
    /// A cell's width over its height (plan §6): a box `h` rows tall is `h / cell_aspect`
    /// world units tall.
    pub(crate) cell_aspect: f64,
}

/// What a relayout follows (plan §6, as the owner's "Relayout" ruling amends it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Change<'a> {
    /// Nodes or edges came or went, and these slots hold nodes new to the layout. A first
    /// layout is one whose every node is new.
    Topology(&'a BTreeSet<NodeIx>),
    /// The semantic zoom level changed, and these nodes' boxes collapsed or expanded with it.
    Level(&'a BTreeSet<NodeIx>),
    /// The frame changed size.
    Resize,
}

/// What the force layout keeps from one run to the next.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ForceState {
    /// Per slot, its node's world position (its center); `None` for a vacant slot.
    pub(crate) positions: Vec<Option<(f64, f64)>>,
    /// Per slot, its node's neighbors along layout edges, sorted, when it was last laid out:
    /// what tells a topology relayout which survivors gained or lost an edge.
    pub(super) neighbors: Vec<Vec<NodeIx>>,
    /// The base ideal distance of the last run, which a resize scales the layout by.
    pub(super) base: Option<f64>,
}

/// Lays out every node of `store` after `change` and writes each one's world position (its
/// center) into `state`, by slot; a vacant slot gets `None`.
///
/// What `state` held is the previous layout. A node new to it, or one that had no layout edge
/// and now has, starts beside its neighbors; the rest start where they were and move as
/// [`mobility`] allows. A pinned node sits at its pin.
pub(crate) fn lay_out<K: Key>(
    params: &ForceParams,
    store: &GraphStore<K>,
    metrics: &ResolvedMetrics,
    frame: Frame,
    change: Change<'_>,
    state: &mut ForceState,
) {
    let params = params.sanitized();
    let mut graph = Graph::of(store, metrics, frame);
    let base = graph.scale(&params, frame.area);
    let starts = mobility::starts(&params, &graph.view(), change, state, base);
    graph.previous = starts.previous;
    graph.mobility = starts.mobility;
    let resize = change == Change::Resize;

    let n = graph.order.len();
    let mut at = vec![(0.0, 0.0); n];
    let islands = graph.islands();
    let mut placed_box: Option<Bounds> = None;
    let mut free = Vec::new();
    for island in &islands {
        let (bodies, springs) = graph.bodies(island);
        let placed = bodies.iter().any(|b| b.previous.is_some());
        if bodies.iter().all(|b| b.pin.is_some()) {
            for (&i, b) in island.iter().zip(&bodies) {
                at[i] = b.pin.unwrap_or_default();
            }
        } else {
            let schedule = if placed && !resize {
                Schedule::Warm
            } else {
                Schedule::Cold
            };
            let result = simulate(&params, &bodies, &springs, frame.area, schedule);
            for (&i, &p) in island.iter().zip(&result) {
                at[i] = p;
            }
        }
        let pinned = island.iter().any(|&i| graph.pin[i].is_some());
        if pinned || (placed && !resize) {
            placed_box = Bounds::union(placed_box, graph.bounds(island, &at));
        } else {
            free.push(island);
        }
    }

    // Islands new to the layout follow the placed ones, left to right, centered on them.
    let (mut cursor, middle) = placed_box.map_or((0.0, 0.0), |b| (b.x1 + GUTTER, b.middle_y()));
    for island in free {
        let Some(b) = graph.bounds(island, &at) else {
            continue;
        };
        let (dx, dy) = (cursor - b.x0, middle - b.middle_y());
        for &i in island {
            at[i] = (at[i].0 + dx, at[i].1 + dy);
        }
        cursor += b.x1 - b.x0 + GUTTER;
    }

    graph.ring(&islands, &mut at, resize);
    state.positions.clear();
    state.positions.resize(store.nodes.len(), None);
    state.neighbors.clear();
    state.neighbors.resize(store.nodes.len(), Vec::new());
    for (i, &ix) in graph.order.iter().enumerate() {
        state.positions[ix.slot()] = Some(at[i]);
        state.neighbors[ix.slot()] = graph.neighbor_keys(i);
    }
    state.base = Some(base);
}

/// The graph as the layout sees it, every per-node vector in canonical order.
struct Graph<'s, K> {
    store: &'s GraphStore<K>,
    order: &'s [NodeIx],
    /// Per slot, the node's place in `order`.
    local: Vec<usize>,
    size: Vec<(f64, f64)>,
    degree: Vec<u32>,
    /// Layout edges between distinct nodes, by place, with their weights.
    springs: Vec<(usize, usize, f64)>,
    /// Per node, its neighbors along `springs`, by place, sorted and distinct.
    adjacent: Vec<Vec<usize>>,
    pin: Vec<Option<(f64, f64)>>,
    /// Per node, its ideal distance in this run.
    ideal: Vec<f64>,
    /// Per node, where it starts (see [`mobility::starts`]).
    previous: Vec<Option<(f64, f64)>>,
    mobility: Vec<Mobility>,
}

impl<'s, K: Key> Graph<'s, K> {
    /// The graph of `store`, with every node new to the layout.
    fn of(store: &'s GraphStore<K>, metrics: &ResolvedMetrics, frame: Frame) -> Self {
        let order = store.nodes_in_order();
        let n = order.len();
        let mut local = vec![usize::MAX; store.nodes.len()];
        for (i, ix) in order.iter().enumerate() {
            local[ix.slot()] = i;
        }
        let aspect = if frame.cell_aspect > 0.0 && frame.cell_aspect.is_finite() {
            frame.cell_aspect
        } else {
            0.5
        };
        let size = order
            .iter()
            .map(|&ix| {
                metrics.form(ix).map_or((1.0, 1.0 / aspect), |form| {
                    (f64::from(form.width()), f64::from(form.height()) / aspect)
                })
            })
            .collect();
        let mut degree = vec![0u32; n];
        let mut springs = Vec::new();
        let mut adjacent = vec![Vec::new(); n];
        for &e in store.edges_in_order() {
            let Some(edge) = store.edge(e) else { continue };
            if !edge.spec.layout_participating || edge.from == edge.to {
                continue;
            }
            let (a, b) = (local[edge.from.slot()], local[edge.to.slot()]);
            springs.push((a, b, f64::from(edge.spec.weight)));
            degree[a] = degree[a].saturating_add(1);
            degree[b] = degree[b].saturating_add(1);
            adjacent[a].push(b);
            adjacent[b].push(a);
        }
        for list in &mut adjacent {
            list.sort_unstable();
            list.dedup();
        }
        let pin = order
            .iter()
            .map(|&ix| store.node(ix).and_then(|node| node.spec.pinned))
            .collect();
        Graph {
            store,
            order,
            local,
            size,
            degree,
            springs,
            adjacent,
            pin,
            ideal: vec![1.0; n],
            previous: vec![None; n],
            mobility: vec![Mobility::Tethered(0.0); n],
        }
    }

    /// What [`mobility::starts`] reads.
    fn view(&self) -> mobility::View<'_> {
        mobility::View {
            order: self.order,
            adjacent: &self.adjacent,
        }
    }

    /// Sets each node's ideal distance for a layout over `area`, and returns the base
    /// distance they scale.
    fn scale(&mut self, params: &ForceParams, area: (f64, f64)) -> f64 {
        let core = self.degree.iter().filter(|&&d| d > 0).count();
        let base = base_distance(area, core);
        self.ideal = self
            .size
            .iter()
            .map(|&(width, _)| ideal_distance(params, base, width))
            .collect();
        base
    }

    /// Node `i`'s layout neighbors, as [`ForceState`] keeps them.
    fn neighbor_keys(&self, i: usize) -> Vec<NodeIx> {
        self.view().neighbor_keys(i)
    }

    /// The core's connected components, each in canonical order, largest first and then by
    /// their first node.
    fn islands(&self) -> Vec<Vec<usize>> {
        let n = self.order.len();
        let mut parent: Vec<usize> = (0..n).collect();
        let find = |parent: &mut Vec<usize>, mut i: usize| {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        };
        for &(a, b, _) in &self.springs {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            parent[ra.max(rb)] = ra.min(rb);
        }
        let mut island_of = vec![usize::MAX; n];
        let mut islands: Vec<Vec<usize>> = Vec::new();
        for i in (0..n).filter(|&i| self.degree[i] > 0) {
            let root = find(&mut parent, i);
            if island_of[root] == usize::MAX {
                island_of[root] = islands.len();
                islands.push(Vec::new());
            }
            islands[island_of[root]].push(i);
        }
        islands.sort_by_key(|island| std::cmp::Reverse(island.len()));
        islands
    }

    /// The simulation's bodies and springs for `island`, indexed within it. A held node is
    /// pinned where it was.
    fn bodies(&self, island: &[usize]) -> (Vec<Body>, Vec<Spring>) {
        let mut within = vec![usize::MAX; self.order.len()];
        for (j, &i) in island.iter().enumerate() {
            within[i] = j;
        }
        let bodies = island
            .iter()
            .map(|&i| {
                let held = self.mobility[i] == Mobility::Held;
                Body {
                    size: self.size[i],
                    weight: self
                        .store
                        .node(self.order[i])
                        .map_or(1.0, |node| f64::from(node.spec.weight)),
                    degree: self.degree[i],
                    ideal: self.ideal[i],
                    pin: self.pin[i].or(self.previous[i].filter(|_| held)),
                    previous: self.previous[i],
                    tether: match self.mobility[i] {
                        Mobility::Tethered(tether) => tether,
                        Mobility::Held => 0.0,
                    },
                }
            })
            .collect();
        let springs = self
            .springs
            .iter()
            .filter(|&&(a, _, _)| within[a] != usize::MAX)
            .map(|&(a, b, weight)| Spring {
                ends: (within[a], within[b]),
                weight,
            })
            .collect();
        (bodies, springs)
    }

    /// The box around `members`' boxes at `at`; `None` for no members.
    fn bounds(&self, members: &[usize], at: &[(f64, f64)]) -> Option<Bounds> {
        members.iter().fold(None, |acc, &i| {
            let ((cx, cy), (width, height)) = (at[i], self.size[i]);
            let own = Bounds {
                x0: cx - width / 2.0,
                y0: cy - height / 2.0,
                x1: cx + width / 2.0,
                y1: cy + height / 2.0,
            };
            Bounds::union(acc, Some(own))
        })
    }

    /// Places every node outside the core: at its pin, where it was, or, new or after a
    /// resize, on the ring around the core, turned toward its neighbors there.
    fn ring(&self, islands: &[Vec<usize>], at: &mut [(f64, f64)], resize: bool) {
        let outside: Vec<usize> = (0..self.order.len())
            .filter(|&i| self.degree[i] == 0)
            .collect();
        let core: Vec<usize> = islands.iter().flatten().copied().collect();
        let anchored: Vec<(f64, f64)> = if core.is_empty() {
            outside
                .iter()
                .filter_map(|&i| self.pin[i].or(self.previous[i]))
                .collect()
        } else {
            core.iter().map(|&i| at[i]).collect()
        };
        let center = centroid(&anchored);

        let mut fresh = Vec::new();
        for &i in &outside {
            match self.pin[i].or(self.previous[i].filter(|_| !resize)) {
                Some(p) => at[i] = p,
                None => fresh.push(i),
            }
        }
        let nodes: Vec<RingNode> = fresh
            .iter()
            .map(|&i| RingNode {
                size: self.size[i],
                ideal: self.ideal_angle(i, center, at),
            })
            .collect();
        let core_boxes: Vec<Placed> = core.iter().map(|&i| (at[i], self.size[i])).collect();
        let radius = ring::radius(center, &core_boxes, &nodes);
        for (&i, p) in fresh.iter().zip(ring::place(&nodes, center, radius)) {
            at[i] = p;
        }
    }

    /// The direction from `center` to the mean of node `i`'s neighbors in the core, along any
    /// edge; `None` with no such neighbor.
    fn ideal_angle(&self, i: usize, center: (f64, f64), at: &[(f64, f64)]) -> Option<f64> {
        let ix = self.order[i];
        let (mut sx, mut sy, mut count) = (0.0, 0.0, 0u32);
        let edges = self
            .store
            .out_edges(ix)
            .iter()
            .chain(self.store.in_edges(ix));
        for &e in edges {
            let Some(edge) = self.store.edge(e) else {
                continue;
            };
            let other = if edge.from == ix { edge.to } else { edge.from };
            let j = self.local[other.slot()];
            if self.degree[j] > 0 {
                (sx, sy, count) = (sx + at[j].0, sy + at[j].1, count + 1);
            }
        }
        (count > 0).then(|| {
            let m = f64::from(count);
            fmath::atan2(sy / m - center.1, sx / m - center.0)
        })
    }
}

/// An axis-aligned box in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Bounds {
    fn union(a: Option<Bounds>, b: Option<Bounds>) -> Option<Bounds> {
        match (a, b) {
            (Some(a), Some(b)) => Some(Bounds {
                x0: a.x0.min(b.x0),
                y0: a.y0.min(b.y0),
                x1: a.x1.max(b.x1),
                y1: a.y1.max(b.y1),
            }),
            (a, b) => a.or(b),
        }
    }

    fn middle_y(&self) -> f64 {
        f64::midpoint(self.y0, self.y1)
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "stability_tests.rs"]
mod stability_tests;

#[cfg(test)]
#[path = "relayout_tests.rs"]
mod relayout_tests;
