//! The peripheral ring (plan §8.2), ported from the seed's `radial_layout`, `centroid` and
//! `max_distance_from`: nodes with no layout edge circle the force core instead of drifting
//! off under repulsion. They are ordered by the direction of their neighbors in the core, so
//! their edges into it do not cross, but spaced evenly, so their labels do not pile up.
//!
//! The ring lives in world space, round, with no aspect factor (the seed drew it in cells and
//! squashed its height by half); the grid snapper fits ring and core to the viewport together.

use crate::fmath;
use crate::layout::count;

/// A node to place on the ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RingNode {
    /// Its box's width and height in world units.
    pub(crate) size: (f64, f64),
    /// The direction, in radians, from the ring's center toward the node's neighbors in the
    /// core; `None` for a node with none.
    pub(crate) ideal: Option<f64>,
}

/// A box's center and its width and height.
pub(crate) type Placed = ((f64, f64), (f64, f64));

/// The ring's radius around `center`: clear of every core box by 10% of the farthest corner's
/// distance plus 3 units, and long enough around for every ring box with 2 units between
/// each. The seed halved the first term because its core already filled the viewport when
/// the ring was placed; here the snapper fits both afterwards, so the ring circles the core
/// instead of cutting through it.
pub(crate) fn radius(center: (f64, f64), core: &[Placed], ring: &[RingNode]) -> f64 {
    let farthest = core
        .iter()
        .map(|&((x, y), (w, h))| {
            let (dx, dy) = (x - center.0, y - center.1);
            (dx * dx + dy * dy).sqrt() + (w * w + h * h).sqrt() / 2.0
        })
        .fold(0.0f64, f64::max);
    let around: f64 = ring.iter().map(|node| node.size.0 + 2.0).sum();
    ((farthest + 3.0) * 1.1).max(around / std::f64::consts::TAU)
}

/// Places `nodes` evenly around `center` at `radius` and returns their centers in input order.
///
/// They go round in order of ideal angle, those without one last, ties in input order (which
/// the caller makes canonical); the first sits at its own ideal angle, or at angle 0.
pub(crate) fn place(nodes: &[RingNode], center: (f64, f64), radius: f64) -> Vec<(f64, f64)> {
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| {
        let angle = |i: usize| nodes[i].ideal.unwrap_or(f64::INFINITY);
        angle(a).total_cmp(&angle(b)).then(a.cmp(&b))
    });
    let start = order
        .first()
        .and_then(|&first| nodes[first].ideal)
        .unwrap_or(0.0);
    let step = std::f64::consts::TAU / count(nodes.len().max(1));
    let mut placed = vec![(0.0, 0.0); nodes.len()];
    for (slot, &i) in order.iter().enumerate() {
        let angle = start + step * count(slot);
        placed[i] = (
            center.0 + radius * fmath::cos(angle),
            center.1 + radius * fmath::sin(angle),
        );
    }
    placed
}

#[cfg(test)]
#[path = "ring_tests.rs"]
mod tests;
