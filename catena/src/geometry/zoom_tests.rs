use super::*;

fn level(zoom: f64) -> u8 {
    SemanticZoomTable::default().level(zoom).index()
}

/// The seed's `visual_label_width`: a measured width held to its level's cap.
fn visual_width(table: &SemanticZoomTable, measured: u16, level: u8) -> u16 {
    table
        .cap(SemanticZoom(level))
        .map_or(measured, |cap| measured.min(cap))
}

fn visual_label_width(layout_width: u16, zoom_level: u8) -> u16 {
    visual_width(&SemanticZoomTable::default(), layout_width, zoom_level)
}

#[test]
fn semantic_zoom_levels() {
    assert_eq!(level(0.1), 0);
    assert_eq!(level(0.3), 0);
    // Abbreviated labels appear at zoom 0.31–0.35
    assert_eq!(level(0.35), 1);
    // Full labels (short) appear sooner: at zoom 0.36+
    assert_eq!(level(0.5), 2);
    assert_eq!(level(0.75), 2);
    assert_eq!(level(1.0), 2);
    // Progressive levels at high zoom
    assert_eq!(level(2.0), 3);
    assert_eq!(level(3.0), 4);
    assert_eq!(level(4.0), 5);
}

#[test]
fn visual_label_width_by_zoom() {
    // Zoom 0 (symbol): always 1 character regardless of name length
    assert_eq!(visual_label_width(22, 0), 1);
    assert_eq!(visual_label_width(3, 0), 1);

    // Zoom 1 (abbreviated [ABC]): capped at 5, but respects short names
    assert_eq!(visual_label_width(22, 1), 5);
    assert_eq!(visual_label_width(3, 1), 3);
    assert_eq!(visual_label_width(5, 1), 5);

    // Zoom 2 (short full name): capped at 14, ~12 char names + brackets
    assert_eq!(visual_label_width(22, 2), 14);
    assert_eq!(visual_label_width(3, 2), 3);
    assert_eq!(visual_label_width(14, 2), 14);

    // Zoom 3 (medium name): capped at 22
    assert_eq!(visual_label_width(30, 3), 22);
    assert_eq!(visual_label_width(10, 3), 10);

    // Zoom 4 (long name): capped at 34
    assert_eq!(visual_label_width(40, 4), 34);
    assert_eq!(visual_label_width(20, 4), 20);

    // Zoom 5+ (full name): uses layout width as-is
    assert_eq!(visual_label_width(50, 5), 50);
    assert_eq!(visual_label_width(3, 5), 3);
}

#[test]
fn every_boundary_is_inclusive_and_the_same_in_both_directions() {
    let table = SemanticZoomTable::default();
    for (i, &ceiling) in [0.30, 0.35, 1.5, 2.5, 3.5].iter().enumerate() {
        let at = table.level(ceiling).index();
        let above = table.level(ceiling.next_up()).index();
        let expected = u8::try_from(i).expect("five ceilings");
        assert_eq!((at, above), (expected, expected + 1), "ceiling {ceiling}");
    }
}

#[test]
fn levels_over_the_whole_range_are_ordered_and_reach_all_six() {
    let table = SemanticZoomTable::default();
    let mut seen = Vec::new();
    let mut previous = table.level(MIN_ZOOM);
    let steps = 390;
    for step in 0..=steps {
        let zoom = MIN_ZOOM + (MAX_ZOOM - MIN_ZOOM) * f64::from(step) / f64::from(steps);
        let current = table.level(zoom);
        assert!(
            current >= previous,
            "level fell from {previous:?} to {current:?} at {zoom}"
        );
        if seen.last() != Some(&current) {
            seen.push(current);
        }
        previous = current;
    }
    let indices: Vec<u8> = seen.iter().map(|l| l.index()).collect();
    assert_eq!(indices, [0, 1, 2, 3, 4, 5]);
}

#[test]
fn out_of_range_and_nan_zooms_have_defined_levels() {
    assert_eq!(level(f64::NAN), 0);
    assert_eq!(level(f64::NEG_INFINITY), 0);
    assert_eq!(level(-1.0), 0);
    assert_eq!(level(0.0), 0);
    assert_eq!(level(f64::INFINITY), 5);
    assert_eq!(level(f64::MAX), 5);
}

#[test]
fn the_clamp_is_the_seeds() {
    assert!((MIN_ZOOM - 0.1).abs() < f64::EPSILON);
    assert!((MAX_ZOOM - 4.0).abs() < f64::EPSILON);
    assert_eq!(level(MIN_ZOOM), 0);
    assert_eq!(level(MAX_ZOOM), 5);
}

#[test]
fn a_custom_table_replaces_the_defaults() {
    let table = SemanticZoomTable::new([0.2, 0.4, 0.8, 1.6, 3.2], [2, 6, 10, 20, 40])
        .expect("a valid table");
    assert_eq!(table.level(0.2).index(), 0);
    assert_eq!(table.level(0.5).index(), 2);
    assert_eq!(table.level(2.0).index(), 4);
    assert_eq!(table.level(3.3).index(), 5);
    assert_eq!(visual_width(&table, 30, 0), 2);
    assert_eq!(visual_width(&table, 30, 3), 20);
    assert_eq!(visual_width(&table, 300, 5), 300);
    assert_eq!(table.cap(SemanticZoom(5)), None, "level 5 has no cap");
}

#[test]
fn invalid_tables_are_refused() {
    let caps = [1, 5, 14, 22, 34];
    let upper = [0.30, 0.35, 1.5, 2.5, 3.5];
    assert!(SemanticZoomTable::new(upper, caps).is_some());
    assert!(SemanticZoomTable::new([0.30, 0.35, 0.35, 2.5, 3.5], caps).is_none());
    assert!(SemanticZoomTable::new([0.30, 0.25, 1.5, 2.5, 3.5], caps).is_none());
    assert!(SemanticZoomTable::new([0.30, 0.35, 1.5, 2.5, f64::INFINITY], caps).is_none());
    assert!(SemanticZoomTable::new([f64::NAN, 0.35, 1.5, 2.5, 3.5], caps).is_none());
    assert!(SemanticZoomTable::new(upper, [0, 5, 14, 22, 34]).is_none());
    assert!(SemanticZoomTable::new(upper, [1, 5, 4, 22, 34]).is_none());
}
