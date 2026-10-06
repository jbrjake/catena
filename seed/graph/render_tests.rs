use super::*;

#[test]
fn type_color_known_types() {
    let theme = Theme::cyberpunk();
    assert_eq!(type_color("person", &theme), theme.colors.info);
    assert_eq!(type_color("organization", &theme), theme.colors.primary);
    assert_eq!(type_color("unknown_type", &theme), theme.colors.dim_text);
}

#[test]
fn type_symbol_known_types() {
    let theme = Theme::cyberpunk();
    assert_eq!(type_symbol("person", &theme), '\u{25CF}');
    assert_eq!(type_symbol("organization", &theme), '\u{25A0}');
    assert_eq!(type_symbol("location", &theme), '\u{25B2}');
}

#[test]
fn entity_color_community_palette() {
    let theme = Theme::cyberpunk();
    let color = entity_color("person", Some(0), 0.9, ColorPalette::Community, &theme);
    assert_eq!(color, theme.colors.community_palette[0]);

    let color = entity_color("person", Some(3), 0.9, ColorPalette::Community, &theme);
    assert_eq!(color, theme.colors.community_palette[3]);
}

#[test]
fn entity_color_community_falls_back_to_type() {
    let theme = Theme::cyberpunk();
    let color = entity_color("person", None, 0.9, ColorPalette::Community, &theme);
    assert_eq!(color, theme.colors.info);
}

#[test]
fn community_color_wraps_around() {
    let theme = Theme::cyberpunk();
    let color = entity_color("person", Some(8), 0.9, ColorPalette::Community, &theme);
    assert_eq!(color, theme.colors.community_palette[0]);

    let color = entity_color("person", Some(10), 0.9, ColorPalette::Community, &theme);
    assert_eq!(color, theme.colors.community_palette[2]);
}

#[test]
fn entity_color_type_palette_ignores_community() {
    let theme = Theme::cyberpunk();
    let color = entity_color("person", Some(0), 0.9, ColorPalette::EntityType, &theme);
    assert_eq!(color, theme.colors.info);
}

#[test]
fn entity_color_confidence_palette() {
    let theme = Theme::cyberpunk();
    // Low confidence → reddish
    let low = entity_color("person", Some(0), 0.0, ColorPalette::Confidence, &theme);
    assert_eq!(low, Color::Rgb(255, 0, 0));

    // Mid confidence → yellowish
    let mid = entity_color("person", None, 0.5, ColorPalette::Confidence, &theme);
    assert_eq!(mid, Color::Rgb(255, 255, 0));

    // High confidence → greenish
    let high = entity_color("person", None, 1.0, ColorPalette::Confidence, &theme);
    assert_eq!(high, Color::Rgb(0, 255, 0));
}

#[test]
fn confidence_color_gradient() {
    assert_eq!(confidence_color(0.0), Color::Rgb(255, 0, 0));
    assert_eq!(confidence_color(0.5), Color::Rgb(255, 255, 0));
    assert_eq!(confidence_color(1.0), Color::Rgb(0, 255, 0));
}

#[test]
fn convergence_color_gradient() {
    // 0.0 (divergent) → red, 1.0 (convergent) → blue
    let c0 = convergence_color(0.0);
    let c1 = convergence_color(1.0);
    assert_eq!(c0, Color::Rgb(255, 0, 0));
    assert_eq!(c1, Color::Rgb(0, 0, 255));
}

#[test]
fn entity_color_convergence_palette() {
    let theme = Theme::cyberpunk();
    let color = entity_color_full(
        "person",
        None,
        0.5,
        Some(0.8),
        None,
        None,
        None,
        None,
        ColorPalette::Convergence,
        &theme,
    );
    // Should use the convergence_score (0.8), not confidence (0.5).
    assert_eq!(color, convergence_color(0.8));
}

#[test]
fn convergence_none_falls_back_to_type_color() {
    let theme = Theme::cyberpunk();
    // When convergence_score is None, should use type_color fallback
    // instead of the old behavior that defaulted to 0.5 (making all
    // entities appear as 50% convergence).
    let color = entity_color_full(
        "person",
        None,
        0.5,
        None, // no convergence score
        None,
        None,
        None,
        None,
        ColorPalette::Convergence,
        &theme,
    );
    assert_eq!(color, type_color("person", &theme));
}

#[test]
fn pagerank_none_falls_back_to_type_color() {
    let theme = Theme::cyberpunk();
    // When pagerank is None, should use type_color fallback.
    let color = entity_color_full(
        "organization",
        None,
        0.5,
        None,
        None,
        None,
        None,
        None, // no pagerank
        ColorPalette::PageRank,
        &theme,
    );
    assert_eq!(color, type_color("organization", &theme));
}

#[test]
fn pagerank_color_gradient_spans_full_range() {
    // Normalized PageRank (0.0 = lowest, 1.0 = highest) should produce
    // visually distinct colors at the extremes.
    let low = pagerank_color(0.0);
    let high = pagerank_color(1.0);
    // Low: RGB(0, 0, 200) — dim blue/purple
    assert_eq!(low, Color::Rgb(0, 0, 200));
    // High: RGB(255, 200, 0) — bright gold
    assert_eq!(high, Color::Rgb(255, 200, 0));
    // They must be visually distinct (not the same color)
    assert_ne!(low, high);
}

#[test]
fn entity_color_embedding_community_palette() {
    let theme = Theme::cyberpunk();
    let color = entity_color_full(
        "person",
        None,
        0.5,
        None,
        Some(2),
        None,
        None,
        None,
        ColorPalette::EmbeddingCommunity,
        &theme,
    );
    // Offset by 3: (2 + 3) % 8 = 5
    assert_eq!(color, theme.colors.community_palette[5]);
}

/// Q4: render_graph silently skips entities that have no entry in the
/// positions map. This is the correct off-screen behavior — entities
/// outside the viewport have no position assigned but still have edges
/// that may extend to the visible area.
/// Regression test: must not panic, and only positioned entities render.
#[test]
fn render_graph_skips_entity_without_position() {
    let area = Rect::new(0, 0, 40, 15);
    let mut buf = Buffer::empty(area);
    let theme = Theme::cyberpunk();

    // Entity 1 has a position; entity 2 ("Ghost") does NOT.
    let entities = vec![
        RenderEntity {
            id: Uuid::from_u128(1),
            name: "Alice".into(),
            entity_type: "person".into(),
            confidence: 0.9,
            selected: false,
            community_id: Some(0),
            pinned: false,
            dimmed: false,
            convergence_score: None,
            embedding_community_id: None,
            user_cluster_id: None,
            degree_centrality: None,
            pagerank: None,
        },
        RenderEntity {
            id: Uuid::from_u128(2),
            name: "Ghost".into(),
            entity_type: "organization".into(),
            confidence: 0.8,
            selected: false,
            community_id: Some(1),
            pinned: false,
            dimmed: false,
            convergence_score: None,
            embedding_community_id: None,
            user_cluster_id: None,
            degree_centrality: None,
            pagerank: None,
        },
    ];

    let mut positions = HashMap::new();
    // Only entity 1 has a position; entity 2 ("Ghost") is intentionally absent.
    positions.insert(
        Uuid::from_u128(1),
        GridPosition {
            x: 5,
            y: 3,
            label_width: 7,
            label_height: 1,
        },
    );

    // Must not panic. Entity 2 should be silently skipped.
    render_graph(
        &mut buf,
        area,
        &entities,
        &[],
        &positions,
        2,
        crate::app::ColorPalette::EntityType,
        &theme,
    );

    // Entity 1 ("Alice") should appear at its position (zoom 2 → "[Alice]").
    let cell = &buf[(5, 3)];
    assert_ne!(
        cell.symbol(),
        " ",
        "Entity 1 at (5,3) should have rendered content"
    );

    // Entity 2 ("Ghost") has no position — the entire buffer row 0 should
    // be blank (no fallback render at origin or any unintended location).
    for x in 0..40u16 {
        let c = &buf[(x, 0)];
        assert_eq!(
            c.symbol(),
            " ",
            "Row 0 should be blank — no entity should render without a position; found {:?} at ({x},0)",
            c.symbol()
        );
    }
}

#[test]
fn segment_intersection_x_cross() {
    // Two lines forming an X: (0,0)→(10,10) and (0,10)→(10,0).
    let result = segment_intersection(0.0, 0.0, 10.0, 10.0, 0.0, 10.0, 10.0, 0.0);
    let (cx, cy) = result.expect("X-shaped lines should intersect");
    assert!(
        (cx - 5.0).abs() < 0.1,
        "crossing x should be near 5, got {cx}"
    );
    assert!(
        (cy - 5.0).abs() < 0.1,
        "crossing y should be near 5, got {cy}"
    );
}

#[test]
fn segment_intersection_parallel_returns_none() {
    // Two parallel horizontal lines.
    let result = segment_intersection(0.0, 0.0, 10.0, 0.0, 0.0, 5.0, 10.0, 5.0);
    assert!(result.is_none(), "parallel lines should not intersect");
}

#[test]
fn segment_intersection_non_overlapping_returns_none() {
    // Two segments that would cross if extended but don't actually overlap.
    let result = segment_intersection(0.0, 0.0, 2.0, 2.0, 8.0, 0.0, 10.0, 2.0);
    assert!(
        result.is_none(),
        "non-overlapping segments should not intersect"
    );
}

#[test]
fn segment_intersection_excludes_shared_endpoints() {
    // Two edges meeting at the same node (shared endpoint at (5,5)).
    let result = segment_intersection(0.0, 0.0, 5.0, 5.0, 5.0, 5.0, 10.0, 0.0);
    assert!(
        result.is_none(),
        "shared-endpoint edges should not register as crossings"
    );
}
