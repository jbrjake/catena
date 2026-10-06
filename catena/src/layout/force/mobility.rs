//! How far a relayout reaches (the owner's "Relayout" ruling, `docs/design/owner-rulings.md`):
//! where each node starts, and how freely it may move.
//!
//! A topology change or a semantic level change reaches `tether_reach` hops along layout edges
//! from what changed. Within reach a node moves as its forces say, tethered to where it was:
//! loosely (`tether_near`) up to one hop out, so it answers the change, then more firmly, up to
//! `tether_far` at the reach. Beyond reach a node holds its place, still pushing and pulling.
//! This is mobility fading with graph distance from a change, the online dynamic-graph
//! technique of Frishman and Tal ("Online dynamic graph drawing", 2008), with a horizon: a
//! settled layout far from any change has nothing to answer, and a free node there could only
//! slide along forces too weak to mean anything, which is what moved whole layouts on every
//! relayout before.
//!
//! What changed: after a topology change, a node new to the layout (hop 0; one that had no
//! layout edge and now has counts as new, since its ring place says nothing about where it
//! belongs) and every node that gained or lost a layout neighbor (hop 1, next to the change);
//! after a level change, the nodes whose boxes changed (hop 0). After a resize every node is
//! free and nothing is tethered, and the layout scales by the change in the ideal distance
//! before it relaxes, which is its equilibrium when every force scales with distance alike.

use std::collections::VecDeque;

use super::engine::{Change, ForceState};
use super::params::ForceParams;
use super::simulation::centroid;
use crate::graph::NodeIx;

/// How a node with a previous place moves in a relayout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Mobility {
    /// It moves, pulled back toward its previous place by a tether of this strength.
    Tethered(f64),
    /// It holds its previous place.
    Held,
}

/// What [`starts`] reads of the graph, every vector in canonical order.
pub(super) struct View<'g> {
    pub(super) order: &'g [NodeIx],
    /// Per node, its neighbors along layout edges, by place, sorted and distinct.
    pub(super) adjacent: &'g [Vec<usize>],
}

impl View<'_> {
    /// Node `i`'s neighbors along layout edges, sorted by index: what [`ForceState`] keeps.
    pub(super) fn neighbor_keys(&self, i: usize) -> Vec<NodeIx> {
        let mut keys: Vec<NodeIx> = self.adjacent[i].iter().map(|&j| self.order[j]).collect();
        keys.sort_unstable();
        keys
    }
}

/// Where each node starts a relayout, and how it may move.
pub(super) struct Starts {
    /// Per node, where it was (scaled to the new frame after a resize); `None` for a node that
    /// enters the layout afresh.
    pub(super) previous: Vec<Option<(f64, f64)>>,
    /// Per node, how it moves; meaningful only where `previous` is set.
    pub(super) mobility: Vec<Mobility>,
}

/// Where each node of `view` starts after `change` to the layout in `state`, and how it may
/// move, given the run's base ideal distance `base`.
pub(super) fn starts(
    params: &ForceParams,
    view: &View<'_>,
    change: Change<'_>,
    state: &ForceState,
    base: f64,
) -> Starts {
    let n = view.order.len();
    let old = |ix: NodeIx| state.positions.get(ix.slot()).copied().flatten();
    let mut previous: Vec<Option<(f64, f64)>> = view.order.iter().map(|&ix| old(ix)).collect();
    let mut sources = vec![None; n];
    match change {
        Change::Resize => {
            let ratio = state
                .base
                .filter(|&b| b > 0.0 && b.is_finite())
                .map_or(1.0, |b| base / b);
            let center = centroid(&previous.iter().flatten().copied().collect::<Vec<_>>());
            for p in previous.iter_mut().flatten() {
                *p = (
                    center.0 + (p.0 - center.0) * ratio,
                    center.1 + (p.1 - center.1) * ratio,
                );
            }
            return Starts {
                previous,
                mobility: vec![Mobility::Tethered(0.0); n],
            };
        }
        Change::Topology(added) => {
            for (i, &ix) in view.order.iter().enumerate() {
                let was = state
                    .neighbors
                    .get(ix.slot())
                    .map_or(&[][..], Vec::as_slice);
                let joined = !view.adjacent[i].is_empty();
                if added.contains(&ix) || (joined && was.is_empty()) {
                    previous[i] = None;
                }
                if previous[i].is_none() {
                    sources[i] = joined.then_some(0);
                } else {
                    let now = view.neighbor_keys(i);
                    if now != was || now.iter().any(|ix| added.contains(ix)) {
                        sources[i] = Some(1);
                    }
                }
            }
        }
        Change::Level(reshaped) => {
            for (i, ix) in view.order.iter().enumerate() {
                if reshaped.contains(ix) {
                    sources[i] = Some(0);
                }
            }
        }
    }
    let hops = hops(view.adjacent, &sources);
    let mobility = hops.iter().map(|&h| mobility_at(params, h)).collect();
    Starts { previous, mobility }
}

/// Each node's distance in hops along `adjacent` from the nearest source, a source counting
/// as its own hop; `u32::MAX` where no source reaches.
fn hops(adjacent: &[Vec<usize>], sources: &[Option<u32>]) -> Vec<u32> {
    let mut hops = vec![u32::MAX; adjacent.len()];
    let mut queue = VecDeque::new();
    // Sources at hop 0 queue before those at hop 1, so the queue stays in hop order.
    for level in [0, 1] {
        for (i, &source) in sources.iter().enumerate() {
            if source == Some(level) && hops[i] == u32::MAX {
                hops[i] = level;
                queue.push_back(i);
            }
        }
    }
    while let Some(i) = queue.pop_front() {
        for &j in &adjacent[i] {
            if hops[j] == u32::MAX {
                hops[j] = hops[i] + 1;
                queue.push_back(j);
            }
        }
    }
    hops
}

/// How a node `hops` from the nearest change moves: held beyond the reach, else tethered by
/// `tether_near` up to one hop and growing linearly to `tether_far` at the reach.
fn mobility_at(params: &ForceParams, hops: u32) -> Mobility {
    let reach = params.tether_reach;
    if hops > reach {
        Mobility::Held
    } else if hops <= 1 || reach <= 1 {
        Mobility::Tethered(params.tether_near)
    } else {
        let t = f64::from(hops - 1) / f64::from(reach - 1);
        Mobility::Tethered(params.tether_near + (params.tether_far - params.tether_near) * t)
    }
}

#[cfg(test)]
#[path = "mobility_tests.rs"]
mod tests;
