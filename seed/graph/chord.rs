//! Radial hierarchy layout with Holten's hierarchical edge bundling.
//!
//! Produces a graph-tool–style `draw_hierarchy()` visualization:
//! - Root node at center
//! - Community nodes on an inner ring, angular sectors proportional to entity count
//! - Entity nodes on an outer ring within their community's sector
//! - Edges drawn as bundled Bezier curves following hierarchy tree paths
//!
//! The hierarchy tree is two levels (root → community → entity) because
//! Cylvia uses single-level Louvain communities.

use std::collections::HashMap;
use std::f64::consts::PI;

/// A community's angular sector on the radial layout.
#[derive(Debug, Clone)]
pub(crate) struct CommunityArc {
    pub community_id: i32,
    /// Start angle in radians (0 = right, counter-clockwise).
    pub start_angle: f64,
    /// End angle in radians.
    pub end_angle: f64,
    /// Number of entities in this community.
    #[allow(dead_code)] // Used by tests; available for layout introspection.
    pub entity_count: usize,
}

/// Position of a single entity on the outer ring.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EntityPosition {
    /// Index into the original entity slice.
    pub entity_index: usize,
    /// Angle on the outer ring in radians.
    #[allow(dead_code)] // Used by tests; available for layout introspection.
    pub angle: f64,
    /// Pixel x coordinate on the outer ring.
    pub px: f64,
    /// Pixel y coordinate on the outer ring.
    pub py: f64,
}

/// Position of a community node on the inner ring.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CommunityPosition {
    pub community_id: i32,
    /// Center angle of this community's sector.
    #[allow(dead_code)] // Used by tests; available for layout introspection.
    pub angle: f64,
    /// Pixel x coordinate on the inner ring.
    pub px: f64,
    /// Pixel y coordinate on the inner ring.
    pub py: f64,
}

/// A bundled edge rendered as a sequence of quadratic Bezier segments.
///
/// Intra-community edges use one segment (entity → community → entity).
/// Inter-community edges use two segments chained through the root.
#[derive(Debug, Clone)]
pub(crate) struct BundledEdge {
    /// Quadratic Bezier segments: each is (start, control, end).
    pub segments: Vec<BezierSegment>,
    /// Whether the edge crosses community boundaries.
    pub inter_community: bool,
}

/// A single quadratic Bezier segment: B(t) = (1-t)^2 P0 + 2(1-t)t C + t^2 P1.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BezierSegment {
    pub x0: f64,
    pub y0: f64,
    pub ctrl_x: f64,
    pub ctrl_y: f64,
    pub x1: f64,
    pub y1: f64,
}

/// Gap between community arcs in radians (~3 degrees).
const ARC_GAP_RAD: f64 = 3.0 * PI / 180.0;

/// Default bundling strength — interpolates between tree path (1.0) and
/// straight line (0.0). 0.85 produces tight bundles with visible spread
/// at endpoints, matching graph-tool's typical output.
const BUNDLING_BETA: f64 = 0.85;

/// Inner ring radius as a fraction of canvas radius.
const INNER_RING_RATIO: f64 = 0.40;

/// Outer ring radius as a fraction of canvas radius.
const OUTER_RING_RATIO: f64 = 0.85;

/// Compute arc allocations for communities on the circumference.
///
/// Each community gets an arc proportional to its entity count.
/// Small gaps separate adjacent arcs. Communities are sorted by
/// descending size for visual balance (largest arcs first).
pub(crate) fn compute_arcs(community_sizes: &[(i32, usize)]) -> Vec<CommunityArc> {
    if community_sizes.is_empty() {
        return Vec::new();
    }

    // Sort descending by entity count for visual balance.
    let mut sorted: Vec<(i32, usize)> = community_sizes.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let total_entities: usize = sorted.iter().map(|&(_, n)| n).sum();
    if total_entities == 0 {
        return Vec::new();
    }

    let num_communities = sorted.len();
    // No gaps needed when only one community.
    let (usable_angle, gap) = if num_communities <= 1 {
        (2.0 * PI, 0.0)
    } else {
        let total_gap = ARC_GAP_RAD * num_communities as f64;
        // Clamp total gap so arcs never go negative even with many tiny communities.
        let usable = (2.0 * PI - total_gap).max(PI);
        let g = (2.0 * PI - usable) / num_communities as f64;
        (usable, g)
    };

    let mut arcs = Vec::with_capacity(num_communities);
    let mut cursor = 0.0_f64;

    for &(cid, count) in &sorted {
        let arc_span = usable_angle * (count as f64 / total_entities as f64);
        arcs.push(CommunityArc {
            community_id: cid,
            start_angle: cursor,
            end_angle: cursor + arc_span,
            entity_count: count,
        });
        cursor += arc_span + gap;
    }

    arcs
}

/// Compute community node positions on the inner ring.
pub(crate) fn place_communities(
    arcs: &[CommunityArc],
    center_x: f64,
    center_y: f64,
    inner_radius: f64,
) -> Vec<CommunityPosition> {
    arcs.iter()
        .map(|arc| {
            let angle = (arc.start_angle + arc.end_angle) / 2.0;
            CommunityPosition {
                community_id: arc.community_id,
                angle,
                px: center_x + inner_radius * angle.cos(),
                py: center_y + inner_radius * angle.sin(),
            }
        })
        .collect()
}

/// Place entities on the outer ring within their community arcs.
///
/// Returns a Vec of EntityPosition in the same order as the input
/// entity-to-community mapping. Entities without a community arc
/// are silently skipped.
pub(crate) fn place_entities(
    arcs: &[CommunityArc],
    entity_communities: &[(usize, i32)], // (entity_index, community_id)
    center_x: f64,
    center_y: f64,
    radius: f64,
) -> Vec<EntityPosition> {
    // Build a lookup from community_id to arc.
    let arc_map: HashMap<i32, &CommunityArc> = arcs.iter().map(|a| (a.community_id, a)).collect();

    // Group entity indices by community, preserving order within each group.
    let mut community_entities: HashMap<i32, Vec<usize>> = HashMap::new();
    for &(eidx, cid) in entity_communities {
        community_entities.entry(cid).or_default().push(eidx);
    }

    let mut positions = Vec::with_capacity(entity_communities.len());

    for &(eidx, cid) in entity_communities {
        let arc = match arc_map.get(&cid) {
            Some(a) => a,
            None => continue,
        };
        let members = match community_entities.get(&cid) {
            Some(m) => m,
            None => continue,
        };

        // Find this entity's position within its community group.
        let local_idx = members.iter().position(|&e| e == eidx).unwrap_or(0);
        let n = members.len();

        // Evenly space entities within the arc, with small insets from arc edges.
        let angle = if n == 1 {
            (arc.start_angle + arc.end_angle) / 2.0
        } else {
            // Inset by half-step from arc boundaries for visual padding.
            let step = (arc.end_angle - arc.start_angle) / n as f64;
            arc.start_angle + step * (local_idx as f64 + 0.5)
        };

        let px = center_x + radius * angle.cos();
        let py = center_y + radius * angle.sin();

        positions.push(EntityPosition {
            entity_index: eidx,
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
/// bundled = beta * tree_point + (1 - beta) * straight_point
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

/// Generate bundled edges using Holten's hierarchical edge bundling.
///
/// For each relation, computes Bezier control points along the hierarchy
/// tree path. The bundling strength (beta) controls how tightly edges
/// follow the tree vs. taking a straight line.
///
/// Hierarchy path for intra-community: entity_u → community → entity_v
/// Hierarchy path for inter-community: entity_u → comm_a → root → comm_b → entity_v
pub(crate) fn generate_bundled_edges(
    relations: &[(usize, usize)],
    entity_positions: &HashMap<usize, EntityPosition>,
    entity_communities: &HashMap<usize, i32>,
    community_positions: &HashMap<i32, CommunityPosition>,
    center_x: f64,
    center_y: f64,
    beta: f64,
) -> Vec<BundledEdge> {
    let mut edges = Vec::with_capacity(relations.len());

    for &(src_idx, tgt_idx) in relations {
        let src = match entity_positions.get(&src_idx) {
            Some(p) => p,
            None => continue,
        };
        let tgt = match entity_positions.get(&tgt_idx) {
            Some(p) => p,
            None => continue,
        };

        let src_cid = match entity_communities.get(&src_idx) {
            Some(&c) => c,
            None => continue,
        };
        let tgt_cid = match entity_communities.get(&tgt_idx) {
            Some(&c) => c,
            None => continue,
        };

        let inter_community = src_cid != tgt_cid;

        if inter_community {
            // Inter-community: path is src → src_comm → root → tgt_comm → tgt
            let src_comm = match community_positions.get(&src_cid) {
                Some(p) => p,
                None => continue,
            };
            let tgt_comm = match community_positions.get(&tgt_cid) {
                Some(p) => p,
                None => continue,
            };

            // Straight-line midpoints for bundling interpolation.
            // The straight line from src to tgt is divided into 4 equal segments
            // corresponding to the 4 tree hops.
            let s_quarter_x = src.px + (tgt.px - src.px) * 0.25;
            let s_quarter_y = src.py + (tgt.py - src.py) * 0.25;
            let s_mid_x = (src.px + tgt.px) / 2.0;
            let s_mid_y = (src.py + tgt.py) / 2.0;
            let s_three_quarter_x = src.px + (tgt.px - src.px) * 0.75;
            let s_three_quarter_y = src.py + (tgt.py - src.py) * 0.75;

            // Bundle tree points with straight-line points.
            let (b_src_comm_x, b_src_comm_y) =
                bundle_point(src_comm.px, src_comm.py, s_quarter_x, s_quarter_y, beta);
            let (b_root_x, b_root_y) = bundle_point(center_x, center_y, s_mid_x, s_mid_y, beta);
            let (b_tgt_comm_x, b_tgt_comm_y) = bundle_point(
                tgt_comm.px,
                tgt_comm.py,
                s_three_quarter_x,
                s_three_quarter_y,
                beta,
            );

            // Split into two quadratic Bezier segments:
            // Segment 1: src → bundled_src_comm (control) → bundled_root
            // Segment 2: bundled_root → bundled_tgt_comm (control) → tgt
            edges.push(BundledEdge {
                segments: vec![
                    BezierSegment {
                        x0: src.px,
                        y0: src.py,
                        ctrl_x: b_src_comm_x,
                        ctrl_y: b_src_comm_y,
                        x1: b_root_x,
                        y1: b_root_y,
                    },
                    BezierSegment {
                        x0: b_root_x,
                        y0: b_root_y,
                        ctrl_x: b_tgt_comm_x,
                        ctrl_y: b_tgt_comm_y,
                        x1: tgt.px,
                        y1: tgt.py,
                    },
                ],
                inter_community: true,
            });
        } else {
            // Intra-community: path is src → community → tgt (single Bezier).
            let comm = match community_positions.get(&src_cid) {
                Some(p) => p,
                None => continue,
            };

            // Straight-line midpoint for bundling interpolation.
            let s_mid_x = (src.px + tgt.px) / 2.0;
            let s_mid_y = (src.py + tgt.py) / 2.0;

            let (ctrl_x, ctrl_y) = bundle_point(comm.px, comm.py, s_mid_x, s_mid_y, beta);

            edges.push(BundledEdge {
                segments: vec![BezierSegment {
                    x0: src.px,
                    y0: src.py,
                    ctrl_x,
                    ctrl_y,
                    x1: tgt.px,
                    y1: tgt.py,
                }],
                inter_community: false,
            });
        }
    }

    edges
}

/// Compute the label position for a community arc.
/// Returns (pixel_x, pixel_y) placed outside the outer ring near the arc's midpoint.
pub(crate) fn arc_label_position(
    arc: &CommunityArc,
    center_x: f64,
    center_y: f64,
    radius: f64,
    label_offset: f64,
) -> (f64, f64) {
    let mid_angle = (arc.start_angle + arc.end_angle) / 2.0;
    let lx = center_x + (radius + label_offset) * mid_angle.cos();
    let ly = center_y + (radius + label_offset) * mid_angle.sin();
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

#[cfg(test)]
#[path = "chord_tests.rs"]
mod tests;
