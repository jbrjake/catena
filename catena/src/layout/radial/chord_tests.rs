use super::*;

/// Distance from `(cx, cy)` to `(x, y)`.
fn dist(x: f64, y: f64, cx: f64, cy: f64) -> f64 {
    let (dx, dy) = (x - cx, y - cy);
    (dx * dx + dy * dy).sqrt()
}

/// Groups and nodes placed on a 100-radius canvas centred at (100, 100), as the three lookup
/// maps `generate_bundled_edges` takes.
struct Wheel {
    nodes: HashMap<usize, NodePosition>,
    node_groups: HashMap<usize, u32>,
    groups: HashMap<u32, GroupPosition>,
}

impl Wheel {
    fn new(group_sizes: &[(u32, usize)], node_groups: &[(usize, u32)]) -> Self {
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
    fn two_by_two() -> Self {
        Wheel::new(&[(0, 2), (1, 2)], &[(0, 0), (1, 0), (2, 1), (3, 1)])
    }

    fn bundle(&self, relations: &[(usize, usize)], beta: f64) -> Vec<BundledEdge> {
        generate_bundled_edges(
            relations,
            &self.nodes,
            &self.node_groups,
            &self.groups,
            100.0,
            100.0,
            beta,
        )
    }
}

// ── Arc allocation tests ───────────────────────────────────────

#[test]
fn arcs_proportional_to_node_count() {
    let sizes = vec![(0, 10), (1, 5), (2, 15)];
    let arcs = compute_arcs(&sizes);

    assert_eq!(arcs.len(), 3);

    let arc_span = |group: u32| -> f64 {
        let a = arcs.iter().find(|a| a.group == group).unwrap();
        a.end_angle - a.start_angle
    };

    assert!(
        arc_span(2) > arc_span(0),
        "Group 2 (15) should have larger arc than group 0 (10)"
    );
    assert!(
        arc_span(0) > arc_span(1),
        "Group 0 (10) should have larger arc than group 1 (5)"
    );
    let counts: Vec<usize> = arcs.iter().map(|a| a.node_count).collect();
    assert_eq!(counts, [15, 10, 5], "largest arcs first");
}

#[test]
fn arcs_have_gaps_between_them() {
    let sizes = vec![(0, 5), (1, 5), (2, 5)];
    let arcs = compute_arcs(&sizes);

    for i in 0..arcs.len() - 1 {
        let gap = arcs[i + 1].start_angle - arcs[i].end_angle;
        assert!(
            gap > 0.0,
            "Gap between arc {} and {} should be positive, got {:.4}",
            arcs[i].group,
            arcs[i + 1].group,
            gap
        );
    }
}

#[test]
fn arcs_stay_within_two_pi() {
    let sizes = vec![(0, 10), (1, 5), (2, 8), (3, 3)];
    let arcs = compute_arcs(&sizes);

    for arc in &arcs {
        assert!(
            arc.end_angle <= 2.0 * PI + 0.01,
            "Arc {} end_angle {:.4} exceeds 2pi",
            arc.group,
            arc.end_angle
        );
    }
}

#[test]
fn single_group_arc_covers_full_circle() {
    let sizes = vec![(42, 10)];
    let arcs = compute_arcs(&sizes);

    assert_eq!(arcs.len(), 1);
    let span = arcs[0].end_angle - arcs[0].start_angle;
    assert!(
        (span - 2.0 * PI).abs() < 0.01,
        "Single group should span ~2pi, got {span:.4}"
    );
}

#[test]
fn empty_input_returns_empty_arcs() {
    let arcs = compute_arcs(&[]);
    assert!(arcs.is_empty());
}

#[test]
fn zero_node_groups_return_empty() {
    let sizes = vec![(0, 0), (1, 0)];
    let arcs = compute_arcs(&sizes);
    assert!(arcs.is_empty());
}

// ── Group positioning tests ───────────────────────────────

#[test]
fn group_nodes_on_inner_ring() {
    let sizes = vec![(0, 5), (1, 3), (2, 7)];
    let arcs = compute_arcs(&sizes);
    let cx = 100.0;
    let cy = 100.0;
    let r_inner = 40.0;

    let group_positions = place_groups(&arcs, cx, cy, r_inner);

    assert_eq!(group_positions.len(), 3);
    for gp in &group_positions {
        let d = dist(gp.px, gp.py, cx, cy);
        assert!(
            (d - r_inner).abs() < 0.1,
            "Group {} at ({:.1}, {:.1}) is {:.1} from center, expected {:.1}",
            gp.group,
            gp.px,
            gp.py,
            d,
            r_inner
        );
    }
}

#[test]
fn group_angle_at_arc_midpoint() {
    let sizes = vec![(0, 10), (1, 5)];
    let arcs = compute_arcs(&sizes);
    let group_positions = place_groups(&arcs, 100.0, 100.0, 40.0);

    for gp in &group_positions {
        let arc = arcs.iter().find(|a| a.group == gp.group).unwrap();
        let expected = f64::midpoint(arc.start_angle, arc.end_angle);
        assert!(
            (gp.angle - expected).abs() < 0.001,
            "Group {} angle {:.4} should be arc midpoint {:.4}",
            gp.group,
            gp.angle,
            expected
        );
    }
}

// ── Node positioning tests ───────────────────────────────────

#[test]
fn nodes_placed_on_outer_ring() {
    let sizes = vec![(0, 3), (1, 2)];
    let arcs = compute_arcs(&sizes);

    let node_groups = vec![
        (0, 0),
        (1, 0),
        (2, 0), // group 0
        (3, 1),
        (4, 1), // group 1
    ];

    let cx = 100.0;
    let cy = 100.0;
    let r = 80.0;

    let positions = place_nodes(&arcs, &node_groups, cx, cy, r);
    assert_eq!(positions.len(), 5);

    for pos in &positions {
        let d = dist(pos.px, pos.py, cx, cy);
        assert!(
            (d - r).abs() < 0.1,
            "Node {} at ({:.1}, {:.1}) is {:.1} from center, expected {:.1}",
            pos.node_index,
            pos.px,
            pos.py,
            d,
            r
        );
    }
}

#[test]
fn nodes_within_same_group_share_arc() {
    let sizes = vec![(0, 3)];
    let arcs = compute_arcs(&sizes);

    let node_groups = vec![(0, 0), (1, 0), (2, 0)];
    let positions = place_nodes(&arcs, &node_groups, 100.0, 100.0, 80.0);

    let arc = &arcs[0];
    for pos in &positions {
        assert!(
            pos.angle >= arc.start_angle - 0.001 && pos.angle <= arc.end_angle + 0.001,
            "Node {} angle {:.4} outside arc [{:.4}, {:.4}]",
            pos.node_index,
            pos.angle,
            arc.start_angle,
            arc.end_angle
        );
    }
}

#[test]
fn single_node_group_places_at_arc_midpoint() {
    let sizes = vec![(0, 1), (1, 5)];
    let arcs = compute_arcs(&sizes);

    let node_groups = vec![(0, 0)];
    let positions = place_nodes(&arcs, &node_groups, 100.0, 100.0, 80.0);

    assert_eq!(positions.len(), 1);
    let arc = arcs.iter().find(|a| a.group == 0).unwrap();
    let expected_angle = f64::midpoint(arc.start_angle, arc.end_angle);
    assert!(
        (positions[0].angle - expected_angle).abs() < 0.001,
        "Single node should be at arc midpoint"
    );
}

#[test]
fn empty_node_list_returns_empty_positions() {
    let sizes = vec![(0, 5)];
    let arcs = compute_arcs(&sizes);
    let positions = place_nodes(&arcs, &[], 100.0, 100.0, 80.0);
    assert!(positions.is_empty());
}

#[test]
fn slots_follow_listing_order_and_a_repeated_node_keeps_its_first_slot() {
    // The seed found each slot with a linear `position()` search, so a node listed twice took
    // its first slot both times, and both listings counted toward the group's size. The index
    // map must keep exactly that.
    let arcs = compute_arcs(&[(7, 4), (9, 1)]);
    let node_groups = [(10, 7), (11, 7), (10, 7), (12, 7), (30, 9), (99, 5)];
    let positions = place_nodes(&arcs, &node_groups, 0.0, 0.0, 1.0);
    assert_eq!(
        positions.len(),
        5,
        "node 99's group has no arc, so it is skipped"
    );
    let arc = arcs.iter().find(|a| a.group == 7).unwrap();
    let step = (arc.end_angle - arc.start_angle) / 4.0;
    let slot_angle = |slot: f64| arc.start_angle + step * (slot + 0.5);
    let angles: Vec<(usize, f64)> = positions.iter().map(|p| (p.node_index, p.angle)).collect();
    assert_eq!(
        angles[..4],
        [
            (10, slot_angle(0.0)),
            (11, slot_angle(1.0)),
            (10, slot_angle(0.0)),
            (12, slot_angle(3.0)),
        ]
    );
}

// ── Bundled edge tests ─────────────────────────────────────────

#[test]
fn intra_group_edge_has_one_segment() {
    let wheel = Wheel::new(&[(0, 3)], &[(0, 0), (1, 0), (2, 0)]);
    let edges = wheel.bundle(&[(0, 1)], 0.85);

    assert_eq!(edges.len(), 1);
    assert!(!edges[0].inter_group);
    assert_eq!(
        edges[0].segments.len(),
        1,
        "Intra-group edges use one quadratic Bezier segment"
    );
}

#[test]
fn inter_group_edge_has_two_segments() {
    let edges = Wheel::two_by_two().bundle(&[(0, 2)], 0.85);

    assert_eq!(edges.len(), 1);
    assert!(edges[0].inter_group);
    assert_eq!(
        edges[0].segments.len(),
        2,
        "Inter-group edges use two quadratic Bezier segments chained through root"
    );
}

#[test]
fn segments_chain_continuously() {
    // The end of segment N must equal the start of segment N+1.
    let edges = Wheel::two_by_two().bundle(&[(0, 2)], 0.85);

    assert_eq!(edges.len(), 1);
    for edge in &edges {
        for i in 0..edge.segments.len() - 1 {
            let end = &edge.segments[i];
            let start = &edge.segments[i + 1];
            assert!(
                (end.x1 - start.x0).abs() < 0.001 && (end.y1 - start.y0).abs() < 0.001,
                "Segment {} end ({:.1}, {:.1}) != segment {} start ({:.1}, {:.1})",
                i,
                end.x1,
                end.y1,
                i + 1,
                start.x0,
                start.y0
            );
        }
    }
}

#[test]
fn beta_zero_gives_straight_control_points() {
    // With beta=0, bundled points should equal straight-line midpoints.
    let wheel = Wheel::new(&[(0, 2)], &[(0, 0), (1, 0)]);
    let edges = wheel.bundle(&[(0, 1)], 0.0);

    assert_eq!(edges.len(), 1);
    let seg = &edges[0].segments[0];
    let (src, tgt) = (&wheel.nodes[&0], &wheel.nodes[&1]);
    let expected_ctrl_x = f64::midpoint(src.px, tgt.px);
    let expected_ctrl_y = f64::midpoint(src.py, tgt.py);
    assert!(
        (seg.ctrl_x - expected_ctrl_x).abs() < 0.001
            && (seg.ctrl_y - expected_ctrl_y).abs() < 0.001,
        "Beta=0 control should be straight-line midpoint, got ({:.1}, {:.1}) expected ({:.1}, {:.1})",
        seg.ctrl_x,
        seg.ctrl_y,
        expected_ctrl_x,
        expected_ctrl_y
    );
}

#[test]
fn beta_one_gives_tree_path_control_points() {
    // With beta=1, the control point for intra-group edges
    // should exactly equal the group position.
    let wheel = Wheel::new(&[(0, 2)], &[(0, 0), (1, 0)]);
    let edges = wheel.bundle(&[(0, 1)], 1.0);

    assert_eq!(edges.len(), 1);
    let seg = &edges[0].segments[0];
    let group = &wheel.groups[&0];
    assert!(
        (seg.ctrl_x - group.px).abs() < 0.001 && (seg.ctrl_y - group.py).abs() < 0.001,
        "Beta=1 control should be group position, got ({:.1}, {:.1}) expected ({:.1}, {:.1})",
        seg.ctrl_x,
        seg.ctrl_y,
        group.px,
        group.py
    );
}

#[test]
fn bundled_edge_control_points_inside_outer_ring() {
    // All control points should be closer to center than the outer ring.
    let node_groups = [(0, 0), (1, 0), (2, 0), (3, 1), (4, 1), (5, 1)];
    let wheel = Wheel::new(&[(0, 3), (1, 3)], &node_groups);
    let (_, r_outer) = ring_radii(100.0);

    let relations = [(0, 1), (0, 3), (1, 4), (2, 5)];
    let edges = wheel.bundle(&relations, default_beta());

    assert_eq!(edges.len(), relations.len());
    for edge in &edges {
        for seg in &edge.segments {
            let ctrl_dist = dist(seg.ctrl_x, seg.ctrl_y, 100.0, 100.0);
            assert!(
                ctrl_dist < r_outer + 1.0,
                "Control point ({:.1}, {:.1}) at dist {:.1} should be inside outer ring {:.1}",
                seg.ctrl_x,
                seg.ctrl_y,
                ctrl_dist,
                r_outer
            );
        }
    }
}

#[test]
fn missing_nodes_produce_no_bundled_edge() {
    let empty = Wheel {
        nodes: HashMap::new(),
        node_groups: HashMap::new(),
        groups: HashMap::new(),
    };
    let edges = empty.bundle(&[(0, 1)], 0.85);
    assert!(
        edges.is_empty(),
        "Relations with missing nodes should produce no edges"
    );
}

#[test]
fn edge_starts_at_source_ends_at_target() {
    let wheel = Wheel::two_by_two();

    // Test both intra and inter group edges.
    let relations = [(0, 1), (0, 2)];
    let edges = wheel.bundle(&relations, default_beta());

    assert_eq!(edges.len(), relations.len());
    for (i, &(src_idx, tgt_idx)) in relations.iter().enumerate() {
        let edge = &edges[i];
        let src = &wheel.nodes[&src_idx];
        let tgt = &wheel.nodes[&tgt_idx];

        let first_seg = &edge.segments[0];
        let last_seg = edge.segments.last().unwrap();

        assert!(
            (first_seg.x0 - src.px).abs() < 0.001 && (first_seg.y0 - src.py).abs() < 0.001,
            "Edge {i} first segment should start at source node"
        );
        assert!(
            (last_seg.x1 - tgt.px).abs() < 0.001 && (last_seg.y1 - tgt.py).abs() < 0.001,
            "Edge {i} last segment should end at target node"
        );
    }
}

// ── Ring radii tests ──────────────────────────────────────────

#[test]
fn ring_radii_proportions() {
    let (r_inner, r_outer) = ring_radii(100.0);
    assert!(
        r_inner < r_outer,
        "Inner ring ({r_inner:.1}) should be smaller than outer ring ({r_outer:.1})"
    );
    assert!(
        r_inner > 0.0 && r_outer > 0.0,
        "Both radii should be positive"
    );
    assert!(
        r_outer < 100.0,
        "Outer ring should be smaller than canvas radius for label space"
    );
}

// ── Label position tests ───────────────────────────────────────

#[test]
fn label_outside_outer_ring() {
    let sizes = vec![(0, 5), (1, 5)];
    let arcs = compute_arcs(&sizes);
    let cx = 100.0;
    let cy = 100.0;
    let (_, r_outer) = ring_radii(100.0);
    let offset = 6.0;

    for arc in &arcs {
        let (lx, ly) = arc_label_position(arc, cx, cy, r_outer, offset);
        let d = dist(lx, ly, cx, cy);
        assert!(
            d > r_outer,
            "Label for group {} should be outside outer ring, dist={:.1}",
            arc.group,
            d
        );
    }
}
