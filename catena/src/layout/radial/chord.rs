//! Radial hierarchy layout with Holten's hierarchical edge bundling.
//!
//! Produces a graph-tool–style `draw_hierarchy()` visualization:
//! - Root node at center
//! - Group nodes on an inner ring, angular sectors proportional to node count
//! - Member nodes on an outer ring within their group's sector
//! - Edges drawn as bundled Bezier curves following hierarchy tree paths
//!
//! The hierarchy tree is two levels (root → group → node) because groups are a flat
//! partition: plain `u32` ids supplied by the host (plan §9.3).

use std::collections::HashMap;
use std::f64::consts::PI;

use crate::fmath;
use crate::geometry::curve::Bezier;
use crate::layout::count;

/// A group's angular sector on the radial layout.
#[derive(Debug, Clone)]
pub(crate) struct GroupArc {
    pub group: u32,
    /// Start angle in radians (0 = right, counter-clockwise).
    pub start_angle: f64,
    /// End angle in radians.
    pub end_angle: f64,
    /// Number of nodes in this group.
    pub node_count: usize,
}

/// Position of a single node on the outer ring.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NodePosition {
    /// Index into the original node slice.
    pub node_index: usize,
    /// Angle on the outer ring in radians.
    pub angle: f64,
    /// Pixel x coordinate on the outer ring.
    pub px: f64,
    /// Pixel y coordinate on the outer ring.
    pub py: f64,
}

/// Position of a group node on the inner ring.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GroupPosition {
    pub group: u32,
    /// Center angle of this group's sector.
    pub angle: f64,
    /// Pixel x coordinate on the inner ring.
    pub px: f64,
    /// Pixel y coordinate on the inner ring.
    pub py: f64,
}

/// A bundled edge rendered as a chain of Bézier segments.
///
/// Intra-group edges use one quadratic (node → group → node).
/// Inter-group edges use two cubics chained through the root with matched tangents (G1).
#[derive(Debug, Clone)]
pub(crate) struct BundledEdge {
    /// The chain, each segment starting where the previous one ends.
    pub segments: Vec<Bezier>,
    /// Whether the edge crosses group boundaries.
    pub inter_group: bool,
}

/// Gap between group arcs in radians (~3 degrees).
const ARC_GAP_RAD: f64 = 3.0 * PI / 180.0;

/// Default bundling strength — interpolates between tree path (1.0) and
/// straight line (0.0). 0.85 produces tight bundles with visible spread
/// at endpoints, matching graph-tool's typical output.
const BUNDLING_BETA: f64 = 0.85;

/// Default waist: the control-arm length at the bundle root, as a fraction of the edge's chord
/// (owner ruling A2). A short arm pinches the bundle's waist at the root; a long one loosens it
/// until the curve overshoots. Tuned to the fairest chain: at the default β the mean bending
/// energy `∫ κ² ds` of the test wheels' inter-group edges bottoms out near 0.245
/// (`default_waist_minimizes_the_bending_energy`). At most 0.25, so the β = 0 chain never
/// doubles back on its straight line.
const BUNDLING_WAIST: f64 = 0.24;

/// Inner ring radius as a fraction of canvas radius.
const INNER_RING_RATIO: f64 = 0.40;

/// Outer ring radius as a fraction of canvas radius.
const OUTER_RING_RATIO: f64 = 0.85;

/// Compute arc allocations for groups on the circumference.
///
/// Each group gets an arc proportional to its node count.
/// Small gaps separate adjacent arcs. Groups are sorted by
/// descending size for visual balance (largest arcs first).
pub(crate) fn compute_arcs(group_sizes: &[(u32, usize)]) -> Vec<GroupArc> {
    if group_sizes.is_empty() {
        return Vec::new();
    }

    // Sort descending by node count for visual balance.
    let mut sorted: Vec<(u32, usize)> = group_sizes.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let total_nodes: usize = sorted.iter().map(|&(_, n)| n).sum();
    if total_nodes == 0 {
        return Vec::new();
    }

    let num_groups = sorted.len();
    // No gaps needed when only one group.
    let (usable_angle, gap) = if num_groups <= 1 {
        (2.0 * PI, 0.0)
    } else {
        let total_gap = ARC_GAP_RAD * count(num_groups);
        // Clamp total gap so arcs never go negative even with many tiny groups.
        let usable = (2.0 * PI - total_gap).max(PI);
        let g = (2.0 * PI - usable) / count(num_groups);
        (usable, g)
    };

    let mut arcs = Vec::with_capacity(num_groups);
    let mut cursor = 0.0_f64;

    for &(group, node_count) in &sorted {
        let arc_span = usable_angle * (count(node_count) / count(total_nodes));
        arcs.push(GroupArc {
            group,
            start_angle: cursor,
            end_angle: cursor + arc_span,
            node_count,
        });
        cursor += arc_span + gap;
    }

    arcs
}

/// Compute group node positions on the inner ring.
pub(crate) fn place_groups(
    arcs: &[GroupArc],
    center_x: f64,
    center_y: f64,
    inner_radius: f64,
) -> Vec<GroupPosition> {
    arcs.iter()
        .map(|arc| {
            let angle = f64::midpoint(arc.start_angle, arc.end_angle);
            GroupPosition {
                group: arc.group,
                angle,
                px: center_x + inner_radius * fmath::cos(angle),
                py: center_y + inner_radius * fmath::sin(angle),
            }
        })
        .collect()
}

/// Place nodes on the outer ring within their group arcs.
///
/// Returns a Vec of `NodePosition` in the same order as the input
/// node-to-group mapping. Nodes without a group arc are silently skipped.
/// A node listed twice in one group takes the slot of its first listing,
/// and both listings count toward the group's size.
pub(crate) fn place_nodes(
    arcs: &[GroupArc],
    node_groups: &[(usize, u32)], // (node_index, group)
    center_x: f64,
    center_y: f64,
    radius: f64,
) -> Vec<NodePosition> {
    // Build a lookup from group to arc.
    let arc_map: HashMap<u32, &GroupArc> = arcs.iter().map(|a| (a.group, a)).collect();

    // Each node's slot within its group (its first listing wins), and each group's size: one
    // pass, where the seed searched the member list once per node, O(n²) per group.
    let mut group_sizes: HashMap<u32, usize> = HashMap::new();
    let mut slots: HashMap<(u32, usize), usize> = HashMap::new();
    for &(node, group) in node_groups {
        let size = group_sizes.entry(group).or_default();
        slots.entry((group, node)).or_insert(*size);
        *size += 1;
    }

    let mut positions = Vec::with_capacity(node_groups.len());

    for &(node, group) in node_groups {
        let Some(arc) = arc_map.get(&group) else {
            continue;
        };
        let (Some(&n), Some(&local_idx)) = (group_sizes.get(&group), slots.get(&(group, node)))
        else {
            continue;
        };

        // Evenly space nodes within the arc, with small insets from arc edges.
        let angle = if n == 1 {
            f64::midpoint(arc.start_angle, arc.end_angle)
        } else {
            // Inset by half-step from arc boundaries for visual padding.
            let step = (arc.end_angle - arc.start_angle) / count(n);
            arc.start_angle + step * (count(local_idx) + 0.5)
        };

        let px = center_x + radius * fmath::cos(angle);
        let py = center_y + radius * fmath::sin(angle);

        positions.push(NodePosition {
            node_index: node,
            angle,
            px,
            py,
        });
    }

    positions
}

/// Apply Holten's bundling: interpolate a point between the tree path
/// position and the straight-line position.
///
/// `bundled = beta * tree_point + (1 - beta) * straight_point`
fn bundle_point(
    tree_x: f64,
    tree_y: f64,
    straight_x: f64,
    straight_y: f64,
    beta: f64,
) -> (f64, f64) {
    (
        beta * tree_x + (1.0 - beta) * straight_x,
        beta * tree_y + (1.0 - beta) * straight_y,
    )
}

/// `v` scaled to unit length, or `None` when it is too short (relative to `scale`) to have a
/// direction worth trusting.
fn unit((x, y): (f64, f64), scale: f64) -> Option<(f64, f64)> {
    let len = (x * x + y * y).sqrt();
    (len > 1e-9 * scale.max(1.0)).then(|| (x / len, y / len))
}

/// The two G1 cubics of an inter-group edge (owner ruling A2): `(src, b_src, J − a·d, J)` and
/// `(J, J + a·d, b_tgt, tgt)`, where `J` is the bundled root, `d` the unit direction from the
/// bundled source-group control to the bundled target-group one (from `src` to `tgt` when those
/// coincide), and `a = waist · |tgt − src|`. Both cubics leave `J` along `d` at speed `3a`, so
/// the chain is C¹ there and the tangent turns not at all.
fn g1_chain(
    src: (f64, f64),
    b_src: (f64, f64),
    root: (f64, f64),
    b_tgt: (f64, f64),
    tgt: (f64, f64),
    waist: f64,
) -> [Bezier; 2] {
    let chord = (tgt.0 - src.0, tgt.1 - src.1);
    let chord_len = (chord.0 * chord.0 + chord.1 * chord.1).sqrt();
    let d = unit((b_tgt.0 - b_src.0, b_tgt.1 - b_src.1), chord_len)
        .or_else(|| unit(chord, chord_len))
        .unwrap_or((0.0, 0.0));
    let arm = waist * chord_len;
    let (ax, ay) = (arm * d.0, arm * d.1);
    [
        Bezier::Cubic {
            from: src,
            ctrl0: b_src,
            ctrl1: (root.0 - ax, root.1 - ay),
            to: root,
        },
        Bezier::Cubic {
            from: root,
            ctrl0: (root.0 + ax, root.1 + ay),
            ctrl1: b_tgt,
            to: tgt,
        },
    ]
}

/// Generate bundled edges using Holten's hierarchical edge bundling.
///
/// For each relation, computes Bezier control points along the hierarchy
/// tree path. The bundling strength (beta) controls how tightly edges
/// follow the tree vs. taking a straight line; the waist sets an inter-group
/// edge's control-arm length at the root, as a fraction of its chord.
///
/// Hierarchy path for intra-group: `node_u → group → node_v`
/// Hierarchy path for inter-group: `node_u → group_a → root → group_b → node_v`
#[expect(
    clippy::too_many_arguments,
    reason = "the seed's signature plus the waist; the radial engine (M5) bundles these"
)]
pub(crate) fn generate_bundled_edges(
    relations: &[(usize, usize)],
    node_positions: &HashMap<usize, NodePosition>,
    node_groups: &HashMap<usize, u32>,
    group_positions: &HashMap<u32, GroupPosition>,
    center_x: f64,
    center_y: f64,
    beta: f64,
    waist: f64,
) -> Vec<BundledEdge> {
    let mut edges = Vec::with_capacity(relations.len());

    for &(src_idx, tgt_idx) in relations {
        let (Some(src), Some(tgt)) = (node_positions.get(&src_idx), node_positions.get(&tgt_idx))
        else {
            continue;
        };
        let (Some(&src_group), Some(&tgt_group)) =
            (node_groups.get(&src_idx), node_groups.get(&tgt_idx))
        else {
            continue;
        };

        if src_group == tgt_group {
            // Intra-group: path is src → group → tgt (single Bezier).
            let Some(group) = group_positions.get(&src_group) else {
                continue;
            };

            // Straight-line midpoint for bundling interpolation.
            let s_mid_x = f64::midpoint(src.px, tgt.px);
            let s_mid_y = f64::midpoint(src.py, tgt.py);

            let (ctrl_x, ctrl_y) = bundle_point(group.px, group.py, s_mid_x, s_mid_y, beta);

            edges.push(BundledEdge {
                segments: vec![Bezier::Quadratic {
                    from: (src.px, src.py),
                    ctrl: (ctrl_x, ctrl_y),
                    to: (tgt.px, tgt.py),
                }],
                inter_group: false,
            });
            continue;
        }

        // Inter-group: path is src → src_group → root → tgt_group → tgt
        let (Some(src_grp), Some(tgt_grp)) = (
            group_positions.get(&src_group),
            group_positions.get(&tgt_group),
        ) else {
            continue;
        };

        // Straight-line midpoints for bundling interpolation.
        // The straight line from src to tgt is divided into 4 equal segments
        // corresponding to the 4 tree hops.
        let s_quarter_x = src.px + (tgt.px - src.px) * 0.25;
        let s_quarter_y = src.py + (tgt.py - src.py) * 0.25;
        let s_mid_x = f64::midpoint(src.px, tgt.px);
        let s_mid_y = f64::midpoint(src.py, tgt.py);
        let s_three_quarter_x = src.px + (tgt.px - src.px) * 0.75;
        let s_three_quarter_y = src.py + (tgt.py - src.py) * 0.75;

        // Bundle tree points with straight-line points.
        let (b_src_grp_x, b_src_grp_y) =
            bundle_point(src_grp.px, src_grp.py, s_quarter_x, s_quarter_y, beta);
        let (b_root_x, b_root_y) = bundle_point(center_x, center_y, s_mid_x, s_mid_y, beta);
        let (b_tgt_grp_x, b_tgt_grp_y) = bundle_point(
            tgt_grp.px,
            tgt_grp.py,
            s_three_quarter_x,
            s_three_quarter_y,
            beta,
        );

        // Two cubics through the bundled root, tangent there (owner ruling A2). The seed's two
        // quadratics, (src, b_src_grp, b_root) and (b_root, b_tgt_grp, tgt), kinked at the root
        // whenever the group controls were not collinear with it.
        edges.push(BundledEdge {
            segments: g1_chain(
                (src.px, src.py),
                (b_src_grp_x, b_src_grp_y),
                (b_root_x, b_root_y),
                (b_tgt_grp_x, b_tgt_grp_y),
                (tgt.px, tgt.py),
                waist,
            )
            .to_vec(),
            inter_group: true,
        });
    }

    edges
}

/// Compute the label position for a group arc.
/// Returns `(pixel_x, pixel_y)` placed outside the outer ring near the arc's midpoint.
pub(crate) fn arc_label_position(
    arc: &GroupArc,
    center_x: f64,
    center_y: f64,
    radius: f64,
    label_offset: f64,
) -> (f64, f64) {
    let mid_angle = f64::midpoint(arc.start_angle, arc.end_angle);
    let lx = center_x + (radius + label_offset) * fmath::cos(mid_angle);
    let ly = center_y + (radius + label_offset) * fmath::sin(mid_angle);
    (lx, ly)
}

/// Compute the inner and outer ring radii from the canvas radius.
pub(crate) fn ring_radii(canvas_radius: f64) -> (f64, f64) {
    (
        canvas_radius * INNER_RING_RATIO,
        canvas_radius * OUTER_RING_RATIO,
    )
}

/// Return the default bundling strength.
pub(crate) fn default_beta() -> f64 {
    BUNDLING_BETA
}

/// Return the default waist: the control-arm length at the root, as a fraction of the chord.
pub(crate) fn default_waist() -> f64 {
    BUNDLING_WAIST
}

#[cfg(test)]
#[path = "chord_wheel_tests.rs"]
mod test_wheel;

#[cfg(test)]
#[path = "chord_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "chord_g1_tests.rs"]
mod g1_tests;
