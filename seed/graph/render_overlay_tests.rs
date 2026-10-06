use super::*;

#[test]
fn test_render_overlay_collapsed() {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    let overlays = vec![RenderOverlayNode {
        id: Uuid::from_u128(500),
        label: "Chunk 0: Alice met Bob...".into(),
        content: "Alice met Bob at the headquarters.".into(),
        expanded: false,
        focused: false,
        scroll_offset: 0,
        is_document: false,
        document_source: "report.md".into(),
        connected_entity_ids: vec![],
    }];

    let mut positions = HashMap::new();
    positions.insert(
        Uuid::from_u128(500),
        GridPosition {
            x: 5,
            y: 3,
            label_width: 30,
            label_height: 1,
        },
    );

    let theme = Theme::cyberpunk();
    render_overlay_nodes(&mut buf, area, &overlays, &positions, 2, &theme);

    // The collapsed overlay should render a single row at (5, 3)
    let cell = &buf[(5, 3)];
    assert_eq!(cell.symbol(), "[", "collapsed overlay starts with [");
}

#[test]
fn test_render_overlay_expanded() {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    let overlays = vec![RenderOverlayNode {
        id: Uuid::from_u128(501),
        label: "Chunk 0: test content".into(),
        content: "This is the full text content of the chunk.".into(),
        expanded: true,
        focused: true,
        scroll_offset: 0,
        is_document: false,
        document_source: "report.md".into(),
        connected_entity_ids: vec![],
    }];

    let mut positions = HashMap::new();
    positions.insert(
        Uuid::from_u128(501),
        GridPosition {
            x: 2,
            y: 1,
            label_width: 40,
            label_height: 12,
        },
    );

    let theme = Theme::cyberpunk();
    render_overlay_nodes(&mut buf, area, &overlays, &positions, 2, &theme);

    // The expanded overlay should render a multi-row bordered box.
    // Top-left corner should be the rounded corner character.
    let cell = &buf[(2, 1)];
    assert_eq!(
        cell.symbol(),
        "\u{256d}",
        "expanded overlay starts with rounded top-left corner"
    );

    // Bottom-left corner
    let bottom_cell = &buf[(2, 12)]; // y=1 + height=12 - 1 = 12
    assert_eq!(
        bottom_cell.symbol(),
        "\u{2570}",
        "expanded overlay has rounded bottom-left corner"
    );
}

/// Verify that the glow halo applies its background color to cells that
/// already contain content (e.g. edge line characters), not just empty cells.
/// This is the fix for TODO line 23 — edge lines passing through the highlight
/// border now get the glow background.
#[test]
fn glow_halo_applies_to_non_empty_cells() {
    let area = Rect::new(0, 0, 30, 10);
    let mut buf = Buffer::empty(area);

    // Pre-fill a cell in the glow border area with a Braille edge character.
    // This simulates an edge line drawn before the node glow is rendered.
    let edge_fg = Color::Rgb(50, 50, 50);
    buf[(4, 2)].set_char('\u{2801}');
    buf[(4, 2)].set_style(Style::default().fg(edge_fg));

    // Render a selected entity at position (5, 3) — the glow border extends
    // one cell in each direction, so (4, 2) is in the top-left corner of the halo.
    let entity = RenderEntity {
        id: Uuid::from_u128(1),
        name: "Test".into(),
        entity_type: "person".into(),
        confidence: 0.9,
        selected: true,
        community_id: Some(0),
        pinned: false,
        dimmed: false,
        convergence_score: None,
        embedding_community_id: None,
        user_cluster_id: None,
        degree_centrality: None,
        pagerank: None,
    };

    let pos = GridPosition {
        x: 5,
        y: 3,
        label_width: 4,
        label_height: 1,
    };

    let theme = Theme::cyberpunk();
    render_node(
        &mut buf,
        area,
        &entity,
        &pos,
        2,
        ColorPalette::Community,
        &theme,
    );

    // The cell at (4, 2) should now have:
    // - The original Braille character preserved
    // - A glow background color applied (non-default)
    let cell = &buf[(4, 2)];
    assert_eq!(
        cell.symbol(),
        "\u{2801}",
        "edge character should be preserved"
    );
    // The glow background should be the dimmed version of the entity color.
    let expected_bg = dim_color(theme.colors.community_palette[0]);
    assert_eq!(
        cell.bg, expected_bg,
        "glow background should be applied to non-empty halo cells"
    );
    // The original foreground should be preserved.
    assert_eq!(
        cell.fg, edge_fg,
        "original foreground should be preserved in glow halo"
    );
}

/// Community labels render at cluster centroids when entities have community_ids
/// and names exist in the community_names map.
#[test]
fn community_labels_at_cluster_centers() {
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);

    // Two entities in community 1, positioned at (10, 5) and (20, 5).
    // Centroid should be at x=15, y=5.
    let entities = vec![
        RenderEntity {
            id: Uuid::from_u128(1),
            name: "Alice".into(),
            entity_type: "person".into(),
            confidence: 0.9,
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
        RenderEntity {
            id: Uuid::from_u128(2),
            name: "Bob".into(),
            entity_type: "person".into(),
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
    positions.insert(
        Uuid::from_u128(1),
        GridPosition {
            x: 10,
            y: 5,
            label_width: 7,
            label_height: 1,
        },
    );
    positions.insert(
        Uuid::from_u128(2),
        GridPosition {
            x: 20,
            y: 5,
            label_width: 5,
            label_height: 1,
        },
    );

    let mut community_names = HashMap::new();
    community_names.insert(1, "Research".into());

    let theme = Theme::cyberpunk();
    render_community_labels(
        &mut buf,
        area,
        &entities,
        &positions,
        &community_names,
        &theme,
    );

    // Centroid is at (15, 5). Label "Research" is 8 chars, centered: x = 15 - 4 = 11.
    // Read the rendered text from the buffer starting at (11, 5).
    let mut rendered = String::new();
    for col in 11..19 {
        let sym = buf[(col as u16, 5)].symbol().to_string();
        rendered.push_str(&sym);
    }
    assert_eq!(
        rendered, "Research",
        "community name should appear at cluster centroid"
    );
}

/// Communities without a name in community_names should not render a label.
#[test]
fn community_labels_skip_unnamed() {
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);

    let entities = vec![RenderEntity {
        id: Uuid::from_u128(1),
        name: "Alice".into(),
        entity_type: "person".into(),
        confidence: 0.9,
        selected: false,
        community_id: Some(99),
        pinned: false,
        dimmed: false,
        convergence_score: None,
        embedding_community_id: None,
        user_cluster_id: None,
        degree_centrality: None,
        pagerank: None,
    }];

    let mut positions = HashMap::new();
    positions.insert(
        Uuid::from_u128(1),
        GridPosition {
            x: 10,
            y: 5,
            label_width: 7,
            label_height: 1,
        },
    );

    // Empty community_names — no labels should render.
    let community_names = HashMap::new();
    let theme = Theme::cyberpunk();
    render_community_labels(
        &mut buf,
        area,
        &entities,
        &positions,
        &community_names,
        &theme,
    );

    // The cell at (10, 5) should remain blank.
    assert_eq!(
        buf[(10, 5)].symbol(),
        " ",
        "unnamed community should not render a label"
    );
}

/// Community labels longer than 8 characters should be truncated.
#[test]
fn community_labels_truncate_long_names() {
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);

    let entities = vec![RenderEntity {
        id: Uuid::from_u128(1),
        name: "Alice".into(),
        entity_type: "person".into(),
        confidence: 0.9,
        selected: false,
        community_id: Some(1),
        pinned: false,
        dimmed: false,
        convergence_score: None,
        embedding_community_id: None,
        user_cluster_id: None,
        degree_centrality: None,
        pagerank: None,
    }];

    let mut positions = HashMap::new();
    positions.insert(
        Uuid::from_u128(1),
        GridPosition {
            x: 20,
            y: 10,
            label_width: 7,
            label_height: 1,
        },
    );

    let mut community_names = HashMap::new();
    community_names.insert(1, "International Relations Committee".into());

    let theme = Theme::cyberpunk();
    render_community_labels(
        &mut buf,
        area,
        &entities,
        &positions,
        &community_names,
        &theme,
    );

    // Should truncate to 8 chars: "Internat"
    // Centroid is at (20, 10). Label "Internat" is 8 chars, centered: x = 20 - 4 = 16.
    let mut rendered = String::new();
    for col in 16..24 {
        let sym = buf[(col as u16, 10)].symbol().to_string();
        rendered.push_str(&sym);
    }
    assert_eq!(
        rendered, "Internat",
        "long community name should be truncated to 8 chars"
    );

    // Character beyond the truncation point should be blank.
    assert_eq!(
        buf[(24, 10)].symbol(),
        " ",
        "nothing beyond truncated label"
    );
}

/// §9.7: dimmed entities use the theme's dim_text color, not hardcoded DarkGray.
#[test]
fn dimmed_node_uses_theme_dim_text_color() {
    let area = Rect::new(0, 0, 30, 10);
    let mut buf = Buffer::empty(area);

    let entity = RenderEntity {
        id: Uuid::from_u128(1),
        name: "Alice".into(),
        entity_type: "person".into(),
        confidence: 0.9,
        selected: false,
        community_id: Some(0),
        pinned: false,
        dimmed: true,
        convergence_score: None,
        embedding_community_id: None,
        user_cluster_id: None,
        degree_centrality: None,
        pagerank: None,
    };

    let pos = GridPosition {
        x: 5,
        y: 3,
        label_width: 7,
        label_height: 1,
    };

    let theme = Theme::cyberpunk();
    render_node(
        &mut buf,
        area,
        &entity,
        &pos,
        2,
        ColorPalette::Community,
        &theme,
    );

    // The first label character should have the theme's dim_text foreground.
    let cell = &buf[(5, 3)];
    assert_eq!(
        cell.fg, theme.colors.dim_text,
        "dimmed node should use theme.colors.dim_text ({:?}), got {:?}",
        theme.colors.dim_text, cell.fg
    );
}
