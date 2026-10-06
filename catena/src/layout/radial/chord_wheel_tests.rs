//! Test support shared by the chord tests: a laid-out wheel of groups and nodes, and the vector
//! arithmetic the curve assertions need.

use super::*;

/// Distance from `(cx, cy)` to `(x, y)`.
pub(super) fn dist(x: f64, y: f64, cx: f64, cy: f64) -> f64 {
    let (dx, dy) = (x - cx, y - cy);
    (dx * dx + dy * dy).sqrt()
}

/// Groups and nodes placed on a 100-radius canvas centred at (100, 100), as the three lookup
/// maps `generate_bundled_edges` takes.
pub(super) struct Wheel {
    pub(super) nodes: HashMap<usize, NodePosition>,
    pub(super) node_groups: HashMap<usize, u32>,
    pub(super) groups: HashMap<u32, GroupPosition>,
}

impl Wheel {
    pub(super) fn new(group_sizes: &[(u32, usize)], node_groups: &[(usize, u32)]) -> Self {
        let arcs = compute_arcs(group_sizes);
        let (r_inner, r_outer) = ring_radii(100.0);
        let nodes = place_nodes(&arcs, node_groups, 100.0, 100.0, r_outer);
        let groups = place_groups(&arcs, 100.0, 100.0, r_inner);
        Wheel {
            nodes: nodes.iter().map(|p| (p.node_index, *p)).collect(),
            node_groups: node_groups.iter().copied().collect(),
            groups: groups.iter().map(|p| (p.group, *p)).collect(),
        }
    }

    /// Two groups of two: nodes 0 and 1 in group 0, nodes 2 and 3 in group 1.
    pub(super) fn two_by_two() -> Self {
        Wheel::new(&[(0, 2), (1, 2)], &[(0, 0), (1, 0), (2, 1), (3, 1)])
    }

    pub(super) fn bundle(&self, relations: &[(usize, usize)], beta: f64) -> Vec<BundledEdge> {
        self.bundle_with(relations, beta, default_waist())
    }

    pub(super) fn bundle_with(
        &self,
        relations: &[(usize, usize)],
        beta: f64,
        waist: f64,
    ) -> Vec<BundledEdge> {
        generate_bundled_edges(
            relations,
            &self.nodes,
            &self.node_groups,
            &self.groups,
            100.0,
            100.0,
            beta,
            waist,
        )
    }

    /// Every pair of nodes in different groups, once each way round.
    pub(super) fn inter_group_pairs(&self) -> Vec<(usize, usize)> {
        let mut nodes: Vec<usize> = self.nodes.keys().copied().collect();
        nodes.sort_unstable();
        let mut pairs = Vec::new();
        for &a in &nodes {
            for &b in &nodes {
                if self.node_groups[&a] != self.node_groups[&b] {
                    pairs.push((a, b));
                }
            }
        }
        pairs
    }
}

/// Eight groups of mixed sizes, 48 nodes: the canonical chord fixture's shape (plan §20).
pub(super) fn eight_group_wheel() -> Wheel {
    let sizes = [12usize, 9, 7, 6, 5, 4, 3, 2];
    let mut group_sizes = Vec::new();
    let mut node_groups = Vec::new();
    for (group, &size) in (0u32..).zip(&sizes) {
        group_sizes.push((group, size));
        for _ in 0..size {
            node_groups.push((node_groups.len(), group));
        }
    }
    Wheel::new(&group_sizes, &node_groups)
}

pub(super) fn controls(piece: &Bezier) -> Vec<(f64, f64)> {
    match *piece {
        Bezier::Quadratic { from, ctrl, to } => vec![from, ctrl, to],
        Bezier::Cubic {
            from,
            ctrl0,
            ctrl1,
            to,
        } => vec![from, ctrl0, ctrl1, to],
    }
}

pub(super) fn quadratic_ctrl(piece: &Bezier) -> (f64, f64) {
    match *piece {
        Bezier::Quadratic { ctrl, .. } => ctrl,
        Bezier::Cubic { .. } => panic!("expected a quadratic, got {piece:?}"),
    }
}

pub(super) fn cross(u: (f64, f64), v: (f64, f64)) -> f64 {
    u.0 * v.1 - u.1 * v.0
}

pub(super) fn dot(u: (f64, f64), v: (f64, f64)) -> f64 {
    u.0 * v.0 + u.1 * v.1
}

pub(super) fn norm(u: (f64, f64)) -> f64 {
    dot(u, u).sqrt()
}

/// The angle between `u` and `v` in radians, through `fmath::atan2` (plan §11).
pub(super) fn angle(u: (f64, f64), v: (f64, f64)) -> f64 {
    fmath::atan2(cross(u, v).abs(), dot(u, v))
}
