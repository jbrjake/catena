use super::*;

// ── Arc allocation tests ───────────────────────────────────────

#[test]
fn arcs_proportional_to_entity_count() {
    let sizes = vec![(0, 10), (1, 5), (2, 15)];
    let arcs = compute_arcs(&sizes);

    assert_eq!(arcs.len(), 3);

    let arc_span = |cid: i32| -> f64 {
        let a = arcs.iter().find(|a| a.community_id == cid).unwrap();
        a.end_angle - a.start_angle
    };

    assert!(
        arc_span(2) > arc_span(0),
        "Community 2 (15) should have larger arc than community 0 (10)"
    );
    assert!(
        arc_span(0) > arc_span(1),
        "Community 0 (10) should have larger arc than community 1 (5)"
    );
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
            arcs[i].community_id,
            arcs[i + 1].community_id,
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
            arc.community_id,
            arc.end_angle
        );
    }
}

#[test]
fn single_community_arc_covers_full_circle() {
    let sizes = vec![(42, 10)];
    let arcs = compute_arcs(&sizes);

    assert_eq!(arcs.len(), 1);
    let span = arcs[0].end_angle - arcs[0].start_angle;
    assert!(
        (span - 2.0 * PI).abs() < 0.01,
        "Single community should span ~2pi, got {:.4}",
        span
    );
}

#[test]
fn empty_input_returns_empty_arcs() {
    let arcs = compute_arcs(&[]);
    assert!(arcs.is_empty());
}

#[test]
fn zero_entity_communities_return_empty() {
    let sizes = vec![(0, 0), (1, 0)];
    let arcs = compute_arcs(&sizes);
    assert!(arcs.is_empty());
}

// ── Community positioning tests ───────────────────────────────

#[test]
fn community_nodes_on_inner_ring() {
    let sizes = vec![(0, 5), (1, 3), (2, 7)];
    let arcs = compute_arcs(&sizes);
    let cx = 100.0;
    let cy = 100.0;
    let r_inner = 40.0;

    let comm_positions = place_communities(&arcs, cx, cy, r_inner);

    assert_eq!(comm_positions.len(), 3);
    for cp in &comm_positions {
        let dist = ((cp.px - cx).powi(2) + (cp.py - cy).powi(2)).sqrt();
        assert!(
            (dist - r_inner).abs() < 0.1,
            "Community {} at ({:.1}, {:.1}) is {:.1} from center, expected {:.1}",
            cp.community_id,
            cp.px,
            cp.py,
            dist,
            r_inner
        );
    }
}

#[test]
fn community_angle_at_arc_midpoint() {
    let sizes = vec![(0, 10), (1, 5)];
    let arcs = compute_arcs(&sizes);
    let comm_positions = place_communities(&arcs, 100.0, 100.0, 40.0);

    for cp in &comm_positions {
        let arc = arcs
            .iter()
            .find(|a| a.community_id == cp.community_id)
            .unwrap();
        let expected = (arc.start_angle + arc.end_angle) / 2.0;
        assert!(
            (cp.angle - expected).abs() < 0.001,
            "Community {} angle {:.4} should be arc midpoint {:.4}",
            cp.community_id,
            cp.angle,
            expected
        );
    }
}

// ── Entity positioning tests ───────────────────────────────────

#[test]
fn entities_placed_on_outer_ring() {
    let sizes = vec![(0, 3), (1, 2)];
    let arcs = compute_arcs(&sizes);

    let entity_communities = vec![
        (0, 0),
        (1, 0),
        (2, 0), // community 0
        (3, 1),
        (4, 1), // community 1
    ];

    let cx = 100.0;
    let cy = 100.0;
    let r = 80.0;

    let positions = place_entities(&arcs, &entity_communities, cx, cy, r);
    assert_eq!(positions.len(), 5);

    for pos in &positions {
        let dist = ((pos.px - cx).powi(2) + (pos.py - cy).powi(2)).sqrt();
        assert!(
            (dist - r).abs() < 0.1,
            "Entity {} at ({:.1}, {:.1}) is {:.1} from center, expected {:.1}",
            pos.entity_index,
            pos.px,
            pos.py,
            dist,
            r
        );
    }
}

#[test]
fn entities_within_same_community_share_arc() {
    let sizes = vec![(0, 3)];
    let arcs = compute_arcs(&sizes);

    let entity_communities = vec![(0, 0), (1, 0), (2, 0)];
    let positions = place_entities(&arcs, &entity_communities, 100.0, 100.0, 80.0);

    let arc = &arcs[0];
    for pos in &positions {
        assert!(
            pos.angle >= arc.start_angle - 0.001 && pos.angle <= arc.end_angle + 0.001,
            "Entity {} angle {:.4} outside arc [{:.4}, {:.4}]",
            pos.entity_index,
            pos.angle,
            arc.start_angle,
            arc.end_angle
        );
    }
}

#[test]
fn single_entity_community_places_at_arc_midpoint() {
    let sizes = vec![(0, 1), (1, 5)];
    let arcs = compute_arcs(&sizes);

    let entity_communities = vec![(0, 0)];
    let positions = place_entities(&arcs, &entity_communities, 100.0, 100.0, 80.0);

    assert_eq!(positions.len(), 1);
    let arc = arcs.iter().find(|a| a.community_id == 0).unwrap();
    let expected_angle = (arc.start_angle + arc.end_angle) / 2.0;
    assert!(
        (positions[0].angle - expected_angle).abs() < 0.001,
        "Single entity should be at arc midpoint"
    );
}

#[test]
fn empty_entity_list_returns_empty_positions() {
    let sizes = vec![(0, 5)];
    let arcs = compute_arcs(&sizes);
    let positions = place_entities(&arcs, &[], 100.0, 100.0, 80.0);
    assert!(positions.is_empty());
}

// ── Bundled edge tests ─────────────────────────────────────────

#[test]
fn intra_community_edge_has_one_segment() {
    let sizes = vec![(0, 3)];
    let arcs = compute_arcs(&sizes);
    let cx = 100.0;
    let cy = 100.0;
    let (r_inner, r_outer) = ring_radii(100.0);

    let entity_communities_vec = vec![(0, 0), (1, 0), (2, 0)];
    let positions = place_entities(&arcs, &entity_communities_vec, cx, cy, r_outer);
    let comm_positions = place_communities(&arcs, cx, cy, r_inner);

    let pos_map: HashMap<usize, EntityPosition> =
        positions.iter().map(|p| (p.entity_index, *p)).collect();
    let comm_map: HashMap<usize, i32> = entity_communities_vec
        .iter()
        .map(|&(e, c)| (e, c))
        .collect();
    let cpos_map: HashMap<i32, CommunityPosition> = comm_positions
        .iter()
        .map(|p| (p.community_id, *p))
        .collect();

    let relations = vec![(0, 1)];
    let edges = generate_bundled_edges(&relations, &pos_map, &comm_map, &cpos_map, cx, cy, 0.85);

    assert_eq!(edges.len(), 1);
    assert!(!edges[0].inter_community);
    assert_eq!(
        edges[0].segments.len(),
        1,
        "Intra-community edges use one quadratic Bezier segment"
    );
}

#[test]
fn inter_community_edge_has_two_segments() {
    let sizes = vec![(0, 2), (1, 2)];
    let arcs = compute_arcs(&sizes);
    let cx = 100.0;
    let cy = 100.0;
    let (r_inner, r_outer) = ring_radii(100.0);

    let entity_communities_vec = vec![(0, 0), (1, 0), (2, 1), (3, 1)];
    let positions = place_entities(&arcs, &entity_communities_vec, cx, cy, r_outer);
    let comm_positions = place_communities(&arcs, cx, cy, r_inner);

    let pos_map: HashMap<usize, EntityPosition> =
        positions.iter().map(|p| (p.entity_index, *p)).collect();
    let comm_map: HashMap<usize, i32> = entity_communities_vec
        .iter()
        .map(|&(e, c)| (e, c))
        .collect();
    let cpos_map: HashMap<i32, CommunityPosition> = comm_positions
        .iter()
        .map(|p| (p.community_id, *p))
        .collect();

    let relations = vec![(0, 2)]; // cross-community
    let edges = generate_bundled_edges(&relations, &pos_map, &comm_map, &cpos_map, cx, cy, 0.85);

    assert_eq!(edges.len(), 1);
    assert!(edges[0].inter_community);
    assert_eq!(
        edges[0].segments.len(),
        2,
        "Inter-community edges use two quadratic Bezier segments chained through root"
    );
}

#[test]
fn segments_chain_continuously() {
    // The end of segment N must equal the start of segment N+1.
    let sizes = vec![(0, 2), (1, 2)];
    let arcs = compute_arcs(&sizes);
    let cx = 100.0;
    let cy = 100.0;
    let (r_inner, r_outer) = ring_radii(100.0);

    let entity_communities_vec = vec![(0, 0), (1, 0), (2, 1), (3, 1)];
    let positions = place_entities(&arcs, &entity_communities_vec, cx, cy, r_outer);
    let comm_positions = place_communities(&arcs, cx, cy, r_inner);

    let pos_map: HashMap<usize, EntityPosition> =
        positions.iter().map(|p| (p.entity_index, *p)).collect();
    let comm_map: HashMap<usize, i32> = entity_communities_vec
        .iter()
        .map(|&(e, c)| (e, c))
        .collect();
    let cpos_map: HashMap<i32, CommunityPosition> = comm_positions
        .iter()
        .map(|p| (p.community_id, *p))
        .collect();

    let relations = vec![(0, 2)];
    let edges = generate_bundled_edges(&relations, &pos_map, &comm_map, &cpos_map, cx, cy, 0.85);

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
    let sizes = vec![(0, 2)];
    let arcs = compute_arcs(&sizes);
    let cx = 100.0;
    let cy = 100.0;
    let (r_inner, r_outer) = ring_radii(100.0);

    let entity_communities_vec = vec![(0, 0), (1, 0)];
    let positions = place_entities(&arcs, &entity_communities_vec, cx, cy, r_outer);
    let comm_positions = place_communities(&arcs, cx, cy, r_inner);

    let pos_map: HashMap<usize, EntityPosition> =
        positions.iter().map(|p| (p.entity_index, *p)).collect();
    let comm_map: HashMap<usize, i32> = entity_communities_vec
        .iter()
        .map(|&(e, c)| (e, c))
        .collect();
    let cpos_map: HashMap<i32, CommunityPosition> = comm_positions
        .iter()
        .map(|p| (p.community_id, *p))
        .collect();

    let relations = vec![(0, 1)];
    let edges = generate_bundled_edges(&relations, &pos_map, &comm_map, &cpos_map, cx, cy, 0.0);

    assert_eq!(edges.len(), 1);
    let seg = &edges[0].segments[0];
    let src = &pos_map[&0];
    let tgt = &pos_map[&1];
    let expected_ctrl_x = (src.px + tgt.px) / 2.0;
    let expected_ctrl_y = (src.py + tgt.py) / 2.0;
    assert!(
        (seg.ctrl_x - expected_ctrl_x).abs() < 0.001
            && (seg.ctrl_y - expected_ctrl_y).abs() < 0.001,
        "Beta=0 control should be straight-line midpoint, got ({:.1}, {:.1}) expected ({:.1}, {:.1})",
        seg.ctrl_x, seg.ctrl_y, expected_ctrl_x, expected_ctrl_y
    );
}

#[test]
fn beta_one_gives_tree_path_control_points() {
    // With beta=1, the control point for intra-community edges
    // should exactly equal the community position.
    let sizes = vec![(0, 2)];
    let arcs = compute_arcs(&sizes);
    let cx = 100.0;
    let cy = 100.0;
    let (r_inner, r_outer) = ring_radii(100.0);

    let entity_communities_vec = vec![(0, 0), (1, 0)];
    let positions = place_entities(&arcs, &entity_communities_vec, cx, cy, r_outer);
    let comm_positions = place_communities(&arcs, cx, cy, r_inner);

    let pos_map: HashMap<usize, EntityPosition> =
        positions.iter().map(|p| (p.entity_index, *p)).collect();
    let comm_map: HashMap<usize, i32> = entity_communities_vec
        .iter()
        .map(|&(e, c)| (e, c))
        .collect();
    let cpos_map: HashMap<i32, CommunityPosition> = comm_positions
        .iter()
        .map(|p| (p.community_id, *p))
        .collect();

    let relations = vec![(0, 1)];
    let edges = generate_bundled_edges(&relations, &pos_map, &comm_map, &cpos_map, cx, cy, 1.0);

    assert_eq!(edges.len(), 1);
    let seg = &edges[0].segments[0];
    let comm = &cpos_map[&0];
    assert!(
        (seg.ctrl_x - comm.px).abs() < 0.001 && (seg.ctrl_y - comm.py).abs() < 0.001,
        "Beta=1 control should be community position, got ({:.1}, {:.1}) expected ({:.1}, {:.1})",
        seg.ctrl_x,
        seg.ctrl_y,
        comm.px,
        comm.py
    );
}

#[test]
fn bundled_edge_control_points_inside_outer_ring() {
    // All control points should be closer to center than the outer ring.
    let sizes = vec![(0, 3), (1, 3)];
    let arcs = compute_arcs(&sizes);
    let cx = 100.0;
    let cy = 100.0;
    let (r_inner, r_outer) = ring_radii(100.0);

    let entity_communities_vec = vec![(0, 0), (1, 0), (2, 0), (3, 1), (4, 1), (5, 1)];
    let positions = place_entities(&arcs, &entity_communities_vec, cx, cy, r_outer);
    let comm_positions = place_communities(&arcs, cx, cy, r_inner);

    let pos_map: HashMap<usize, EntityPosition> =
        positions.iter().map(|p| (p.entity_index, *p)).collect();
    let comm_map: HashMap<usize, i32> = entity_communities_vec
        .iter()
        .map(|&(e, c)| (e, c))
        .collect();
    let cpos_map: HashMap<i32, CommunityPosition> = comm_positions
        .iter()
        .map(|p| (p.community_id, *p))
        .collect();

    let relations = vec![(0, 1), (0, 3), (1, 4), (2, 5)];
    let edges = generate_bundled_edges(
        &relations,
        &pos_map,
        &comm_map,
        &cpos_map,
        cx,
        cy,
        default_beta(),
    );

    for edge in &edges {
        for seg in &edge.segments {
            let ctrl_dist = ((seg.ctrl_x - cx).powi(2) + (seg.ctrl_y - cy).powi(2)).sqrt();
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
fn missing_entities_produce_no_bundled_edge() {
    let pos_map: HashMap<usize, EntityPosition> = HashMap::new();
    let comm_map: HashMap<usize, i32> = HashMap::new();
    let cpos_map: HashMap<i32, CommunityPosition> = HashMap::new();
    let relations = vec![(0, 1)];
    let edges = generate_bundled_edges(
        &relations, &pos_map, &comm_map, &cpos_map, 100.0, 100.0, 0.85,
    );
    assert!(
        edges.is_empty(),
        "Relations with missing entities should produce no edges"
    );
}

#[test]
fn edge_starts_at_source_ends_at_target() {
    let sizes = vec![(0, 2), (1, 2)];
    let arcs = compute_arcs(&sizes);
    let cx = 100.0;
    let cy = 100.0;
    let (r_inner, r_outer) = ring_radii(100.0);

    let entity_communities_vec = vec![(0, 0), (1, 0), (2, 1), (3, 1)];
    let positions = place_entities(&arcs, &entity_communities_vec, cx, cy, r_outer);
    let comm_positions = place_communities(&arcs, cx, cy, r_inner);

    let pos_map: HashMap<usize, EntityPosition> =
        positions.iter().map(|p| (p.entity_index, *p)).collect();
    let comm_map: HashMap<usize, i32> = entity_communities_vec
        .iter()
        .map(|&(e, c)| (e, c))
        .collect();
    let cpos_map: HashMap<i32, CommunityPosition> = comm_positions
        .iter()
        .map(|p| (p.community_id, *p))
        .collect();

    // Test both intra and inter community edges.
    let relations = vec![(0, 1), (0, 2)];
    let edges = generate_bundled_edges(
        &relations,
        &pos_map,
        &comm_map,
        &cpos_map,
        cx,
        cy,
        default_beta(),
    );

    for (i, &(src_idx, tgt_idx)) in relations.iter().enumerate() {
        let edge = &edges[i];
        let src = &pos_map[&src_idx];
        let tgt = &pos_map[&tgt_idx];

        let first_seg = &edge.segments[0];
        let last_seg = edge.segments.last().unwrap();

        assert!(
            (first_seg.x0 - src.px).abs() < 0.001 && (first_seg.y0 - src.py).abs() < 0.001,
            "Edge {} first segment should start at source entity",
            i
        );
        assert!(
            (last_seg.x1 - tgt.px).abs() < 0.001 && (last_seg.y1 - tgt.py).abs() < 0.001,
            "Edge {} last segment should end at target entity",
            i
        );
    }
}

// ── Ring radii tests ──────────────────────────────────────────

#[test]
fn ring_radii_proportions() {
    let (r_inner, r_outer) = ring_radii(100.0);
    assert!(
        r_inner < r_outer,
        "Inner ring ({:.1}) should be smaller than outer ring ({:.1})",
        r_inner,
        r_outer
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
        let dist = ((lx - cx).powi(2) + (ly - cy).powi(2)).sqrt();
        assert!(
            dist > r_outer,
            "Label for community {} should be outside outer ring, dist={:.1}",
            arc.community_id,
            dist
        );
    }
}
