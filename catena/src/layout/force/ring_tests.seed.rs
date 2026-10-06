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
