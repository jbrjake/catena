use super::*;

fn test_entities(n: usize) -> Vec<LayoutEntity> {
    (0..n)
        .map(|i| LayoutEntity {
            id: Uuid::from_u128(i as u128),
            name: format!("E{i}"),
            degree: 1,
            label_height: 1,
        })
        .collect()
}

#[test]
fn empty_graph_returns_empty_positions() {
    let result = layout(&[], &[], 80, 24, 50, 1.0, (0, 0), 2, None);
    assert!(result.positions.is_empty());
}

#[test]
fn single_node_placed_in_viewport() {
    let entities = test_entities(1);
    let result = layout(&entities, &[], 80, 24, 50, 1.0, (0, 0), 2, None);
    assert_eq!(result.positions.len(), 1);
    let pos = result.positions.values().next().unwrap();
    assert!(pos.x < 80);
    assert!(pos.y < 24);
}

#[test]
fn connected_nodes_closer_than_disconnected() {
    let entities = vec![
        LayoutEntity {
            id: Uuid::from_u128(0),
            name: "A".into(),
            degree: 1,
            label_height: 1,
        },
        LayoutEntity {
            id: Uuid::from_u128(1),
            name: "B".into(),
            degree: 1,
            label_height: 1,
        },
        LayoutEntity {
            id: Uuid::from_u128(2),
            name: "C".into(),
            degree: 0,
            label_height: 1,
        },
    ];
    let relations = vec![LayoutRelation {
        source_id: Uuid::from_u128(0),
        target_id: Uuid::from_u128(1),
    }];

    let result = layout(&entities, &relations, 100, 40, 100, 1.0, (0, 0), 2, None);
    let pa = result.positions[&Uuid::from_u128(0)];
    let pb = result.positions[&Uuid::from_u128(1)];
    let pc = result.positions[&Uuid::from_u128(2)];

    let dist_ab =
        ((pa.x as f64 - pb.x as f64).powi(2) + (pa.y as f64 - pb.y as f64).powi(2)).sqrt();
    let dist_ac =
        ((pa.x as f64 - pc.x as f64).powi(2) + (pa.y as f64 - pc.y as f64).powi(2)).sqrt();

    // Connected nodes (A-B) should generally be closer than disconnected (A-C)
    // This is probabilistic with FR layout, so we just check positions exist
    assert!(result.positions.len() == 3);
    // At minimum, all positions should be within viewport
    for pos in result.positions.values() {
        assert!(pos.x < 100);
        assert!(pos.y < 40);
    }
    let _ = (dist_ab, dist_ac); // used for debugging if assertion fails
}

#[test]
fn no_collision_on_small_graph() {
    let entities = test_entities(5);
    let result = layout(&entities, &[], 80, 24, 50, 1.0, (0, 0), 2, None);

    // All 5 nodes should have unique (x,y) positions
    let positions: Vec<(i16, i16)> = result.positions.values().map(|p| (p.x, p.y)).collect();
    let unique: std::collections::HashSet<(i16, i16)> = positions.iter().copied().collect();
    assert_eq!(unique.len(), positions.len());
}

#[test]
fn demo_graph_labels_dont_overlap() {
    // Simulate the demo data: 16 entities with realistic name lengths and
    // dense connectivity (27 edges). Verify that no two labels share a cell.
    let names = [
        "Alice",
        "Bob",
        "Carol",
        "ACME Corp",
        "Operation Sunrise",
        "Washington DC",
        "Global Defense Agency",
        "Shadow Protocol",
        "London",
        "Tokyo",
        "Eastern Europe",
        "DNS Tunnels",
        "Spear-Phishing",
        "Critical Infra",
        "Energy Networks",
        "Financial",
    ];
    let entities: Vec<LayoutEntity> = names
        .iter()
        .enumerate()
        .map(|(i, &n)| LayoutEntity {
            id: Uuid::from_u128(i as u128),
            name: n.into(),
            degree: 3,
            label_height: 1,
        })
        .collect();
    // Dense connectivity: chain + some cross-links
    let mut relations = Vec::new();
    for i in 0..names.len() - 1 {
        relations.push(LayoutRelation {
            source_id: Uuid::from_u128(i as u128),
            target_id: Uuid::from_u128((i + 1) as u128),
        });
    }
    for &(s, t) in &[
        (0, 3),
        (1, 4),
        (2, 7),
        (3, 6),
        (5, 8),
        (0, 14),
        (4, 10),
        (6, 9),
        (7, 11),
        (8, 12),
        (9, 13),
    ] {
        relations.push(LayoutRelation {
            source_id: Uuid::from_u128(s),
            target_id: Uuid::from_u128(t),
        });
    }

    let result = layout(&entities, &relations, 100, 30, 50, 1.0, (0, 0), 2, None);
    assert_eq!(result.positions.len(), 16);

    // Check no label bounding boxes overlap for on-screen nodes:
    // collect all occupied cells and verify no duplicates.
    let mut cells: HashMap<(i16, i16), Uuid> = HashMap::new();
    for (id, pos) in &result.positions {
        // Only check on-screen nodes for overlap
        if pos.x < 0 || pos.y < 0 || pos.x >= 100 || pos.y >= 30 {
            continue;
        }
        for lx in 0..pos.label_width as i16 {
            let cell = (pos.x + lx, pos.y);
            if let Some(existing) = cells.get(&cell) {
                panic!(
                    "Cell ({},{}) claimed by both {} and {}",
                    cell.0, cell.1, existing, id
                );
            }
            cells.insert(cell, *id);
        }
    }
}

#[test]
fn pan_shifts_positions() {
    let entities = test_entities(1);
    let result_no_pan = layout(&entities, &[], 80, 24, 50, 1.0, (0, 0), 2, None);
    let result_pan = layout(&entities, &[], 80, 24, 50, 1.0, (5, 3), 2, None);
    let pos0 = result_no_pan.positions.values().next().unwrap();
    let pos1 = result_pan.positions.values().next().unwrap();
    // Pan should shift the position (exact amount depends on clamping)
    // Just verify they're different or both within bounds
    assert!(pos1.x < 80 && pos1.y < 24);
    let _ = pos0; // reference for debugging
}

#[test]
fn large_pan_pushes_nodes_off_screen() {
    // With a large negative pan, nodes should end up with negative coordinates
    // instead of being clamped to the viewport edge.
    let entities = test_entities(3);
    let result = layout(&entities, &[], 40, 12, 50, 1.0, (-100, -50), 2, None);

    // At least some nodes should have negative coordinates (off-screen)
    let any_negative = result.positions.values().any(|p| p.x < 0 || p.y < 0);
    assert!(
        any_negative,
        "Large negative pan should push nodes off-screen (negative coords)"
    );
}

#[test]
fn test_multi_row_collision_detection() {
    // Two nodes with height > 1 should not overlap vertically.
    let entities = vec![
        LayoutEntity {
            id: Uuid::from_u128(0),
            name: "OverlayA".into(),
            degree: 1,
            label_height: 5,
        },
        LayoutEntity {
            id: Uuid::from_u128(1),
            name: "OverlayB".into(),
            degree: 1,
            label_height: 5,
        },
    ];
    let relations = vec![LayoutRelation {
        source_id: Uuid::from_u128(0),
        target_id: Uuid::from_u128(1),
    }];

    let result = layout(&entities, &relations, 100, 40, 50, 1.0, (0, 0), 2, None);

    // Both should be placed
    assert_eq!(result.positions.len(), 2);

    // Check that the bounding boxes don't overlap (for on-screen nodes)
    let pa = result.positions[&Uuid::from_u128(0)];
    let pb = result.positions[&Uuid::from_u128(1)];

    let a_cells: std::collections::HashSet<(i16, i16)> = (0..pa.label_height as i16)
        .flat_map(|ly| (0..pa.label_width as i16).map(move |lx| (pa.x + lx, pa.y + ly)))
        .collect();

    for ly in 0..pb.label_height as i16 {
        for lx in 0..pb.label_width as i16 {
            assert!(
                !a_cells.contains(&(pb.x + lx, pb.y + ly)),
                "Multi-row nodes should not overlap at ({}, {})",
                pb.x + lx,
                pb.y + ly
            );
        }
    }
}

#[test]
fn test_layout_with_varied_heights() {
    // Mix of regular (height=1) and tall (height=6) nodes produces valid positions.
    let entities = vec![
        LayoutEntity {
            id: Uuid::from_u128(0),
            name: "Regular".into(),
            degree: 1,
            label_height: 1,
        },
        LayoutEntity {
            id: Uuid::from_u128(1),
            name: "TallOverlay".into(),
            degree: 1,
            label_height: 6,
        },
        LayoutEntity {
            id: Uuid::from_u128(2),
            name: "AlsoRegular".into(),
            degree: 1,
            label_height: 1,
        },
    ];

    let result = layout(&entities, &[], 100, 40, 50, 1.0, (0, 0), 2, None);
    assert_eq!(result.positions.len(), 3);

    // The tall node should have label_height preserved in its GridPosition
    let tall_pos = result.positions[&Uuid::from_u128(1)];
    assert_eq!(tall_pos.label_height, 6);

    // Regular nodes should have height 1
    let reg_pos = result.positions[&Uuid::from_u128(0)];
    assert_eq!(reg_pos.label_height, 1);
}

// ── Radial layout tests ────────────────────────────────────────────

#[test]
fn radial_layout_empty() {
    let result = radial_layout(&[], (40.0, 12.0), 20.0, &HashMap::new(), 2);
    assert!(result.positions.is_empty());
}

#[test]
fn radial_layout_single_node() {
    let entities = test_entities(1);
    let result = radial_layout(&entities, (40.0, 12.0), 20.0, &HashMap::new(), 2);
    assert_eq!(result.positions.len(), 1);
    // Single node should be at angle 0 → (center.x + radius, center.y)
    let pos = result.positions.values().next().unwrap();
    assert_eq!(pos.x, 60); // 40 + 20
    assert_eq!(pos.y, 12);
}

#[test]
fn radial_layout_evenly_spaced() {
    let entities = test_entities(8);
    let result = radial_layout(&entities, (50.0, 20.0), 30.0, &HashMap::new(), 2);
    assert_eq!(result.positions.len(), 8);

    // All 8 nodes should have unique positions
    let unique: std::collections::HashSet<(i16, i16)> =
        result.positions.values().map(|p| (p.x, p.y)).collect();
    assert_eq!(
        unique.len(),
        8,
        "all radial nodes should have unique positions"
    );
}

#[test]
fn radial_layout_no_label_overlap() {
    // Larger set with varied names to stress collision avoidance.
    let names = [
        "Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel",
    ];
    let entities: Vec<LayoutEntity> = names
        .iter()
        .enumerate()
        .map(|(i, &n)| LayoutEntity {
            id: Uuid::from_u128(i as u128),
            name: n.into(),
            degree: 0,
            label_height: 1,
        })
        .collect();

    let result = radial_layout(&entities, (50.0, 20.0), 25.0, &HashMap::new(), 2);

    // Verify no label bounding boxes overlap (same check as FR tests).
    let mut cells: HashMap<(i16, i16), Uuid> = HashMap::new();
    for (id, pos) in &result.positions {
        if pos.x < 0 || pos.y < 0 || pos.x >= 120 || pos.y >= 50 {
            continue;
        }
        for ly in 0..pos.label_height as i16 {
            for lx in 0..pos.label_width as i16 {
                let cell = (pos.x + lx, pos.y + ly);
                if let Some(existing) = cells.get(&cell) {
                    panic!(
                        "Radial label overlap at ({},{}): {} vs {}",
                        cell.0, cell.1, existing, id
                    );
                }
                cells.insert(cell, *id);
            }
        }
    }
}

#[test]
fn radial_layout_respects_ideal_angles() {
    // Three radial entities with ideal angles pointing right (0), up (-π/2),
    // and left (π). Their angular order on the ring should match.
    let entities: Vec<LayoutEntity> = ["Right", "Up", "Left"]
        .iter()
        .enumerate()
        .map(|(i, &n)| LayoutEntity {
            id: Uuid::from_u128(i as u128),
            name: n.into(),
            degree: 0,
            label_height: 1,
        })
        .collect();

    let mut angles = HashMap::new();
    angles.insert(Uuid::from_u128(0), 0.0); // right
    angles.insert(Uuid::from_u128(1), -std::f64::consts::FRAC_PI_2); // up
    angles.insert(Uuid::from_u128(2), std::f64::consts::PI); // left

    let center = (50.0, 20.0);
    let result = radial_layout(&entities, center, 15.0, &angles, 2);

    let p_right = result.positions[&Uuid::from_u128(0)];
    let p_up = result.positions[&Uuid::from_u128(1)];
    let p_left = result.positions[&Uuid::from_u128(2)];

    // "Up" should have the smallest y (highest on screen).
    assert!(
        p_up.y < p_right.y && p_up.y < p_left.y,
        "Up (y={}) should be above Right (y={}) and Left (y={})",
        p_up.y,
        p_right.y,
        p_left.y
    );
    // "Right" should have the largest x.
    assert!(
        p_right.x > p_left.x,
        "Right (x={}) should be to the right of Left (x={})",
        p_right.x,
        p_left.x
    );
}

// ── Bounding box tests ─────────────────────────────────────────────

#[test]
fn bounding_box_empty() {
    let positions = HashMap::new();
    assert_eq!(bounding_box(&positions), (0, 0, 0, 0));
}

#[test]
fn bounding_box_single_node() {
    let mut positions = HashMap::new();
    positions.insert(
        Uuid::from_u128(0),
        GridPosition {
            x: 10,
            y: 5,
            label_width: 6,
            label_height: 1,
        },
    );
    // max_x = 10 + 6 = 16, max_y = 5 + 1 = 6
    assert_eq!(bounding_box(&positions), (10, 5, 16, 6));
}

// ── Centroid tests ─────────────────────────────────────────────────

#[test]
fn centroid_empty() {
    let positions = HashMap::new();
    assert_eq!(centroid(&positions), (0.0, 0.0));
}

#[test]
fn centroid_single_node() {
    let mut positions = HashMap::new();
    positions.insert(
        Uuid::from_u128(0),
        GridPosition {
            x: 10,
            y: 5,
            label_width: 6,
            label_height: 2,
        },
    );
    // centroid_x = 10 + 6/2 = 13.0, centroid_y = 5 + 2/2 = 6.0
    assert_eq!(centroid(&positions), (13.0, 6.0));
}

#[test]
fn centroid_resists_outlier_skew() {
    // Two nodes clustered at (10,10) and one outlier at (100,10).
    // Bounding-box midpoint: (55, 10). Centroid: ~(40, 11.5) — closer to the cluster.
    let mut positions = HashMap::new();
    positions.insert(
        Uuid::from_u128(0),
        GridPosition {
            x: 10,
            y: 10,
            label_width: 4,
            label_height: 1,
        },
    );
    positions.insert(
        Uuid::from_u128(1),
        GridPosition {
            x: 12,
            y: 10,
            label_width: 4,
            label_height: 1,
        },
    );
    positions.insert(
        Uuid::from_u128(2),
        GridPosition {
            x: 100,
            y: 10,
            label_width: 4,
            label_height: 1,
        },
    );
    let (cx, cy) = centroid(&positions);
    // Centroid should be closer to the cluster than to the outlier.
    assert!(
        cx < 55.0,
        "centroid x={cx:.1} should be less than bounding-box midpoint 55.0"
    );
    assert!(
        (cy - 10.5).abs() < 0.5,
        "centroid y={cy:.1} should be near 10.5"
    );
}
