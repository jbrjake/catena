use super::*;
use crate::ui::dag_layout::builder::DagBuilder;
use crate::ui::dag_layout::render::render_to_string;

#[test]
fn visual_example_a_two_entities_shared_snippet() {
    let dag = DagBuilder::new()
        .entity_with_labels("Bob", &["MANAGES"])
        .entity_with_labels("ACME Corp", &["WORKS_FOR"])
        .snippet("Alice manages Bob at ACME Corp HQ", &["Bob", "ACME Corp"])
        .document(
            "quarterly_report.pdf",
            &["Alice manages Bob at ACME Corp HQ"],
        )
        .build();

    let layout = layout_dag(&dag, 60, 40);
    let text = render_to_string(&layout);

    eprintln!("=== Example A ===\n{}\n=================", text);

    assert!(text.contains("Bob"), "Should show Bob");
    assert!(text.contains("ACME"), "Should show ACME Corp");
    assert!(text.contains("quarterly_report"), "Should show document");

    assert!(
        text.contains('\u{2514}') || text.contains('\u{2518}'),
        "Should have merge bar corners"
    );
    assert!(
        text.contains('\u{252C}'),
        "Should have TeeDown at snippet center"
    );

    let lines: Vec<&str> = text.lines().collect();
    let bob_row = lines.iter().position(|l| l.contains("Bob")).unwrap();
    let snippet_row = lines
        .iter()
        .position(|l| l.contains("Alice manages"))
        .unwrap();
    assert!(bob_row < snippet_row, "Bob should be above snippet");

    let doc_count = lines
        .iter()
        .filter(|l| l.contains("quarterly_report"))
        .count();
    assert!(
        doc_count <= 2,
        "Document should appear at most in 2 lines (top+bottom border or content)"
    );
}

#[test]
fn visual_example_b_complex_dag() {
    let dag = DagBuilder::new()
        .entity_with_labels("Bob", &["MANAGES", "MENTORS"])
        .entity_with_labels("ACME Corp", &["WORKS_FOR"])
        .entity_with_labels("Carol", &["ADVISED_BY"])
        .snippet("Alice mentors Bob on leadership", &["Bob"])
        .snippet(
            "Alice manages Bob's team and works with ACME Corp",
            &["Bob", "ACME Corp"],
        )
        .snippet("Carol advises Alice on compliance", &["Carol"])
        .document(
            "quarterly_report.pdf",
            &[
                "Alice mentors Bob on leadership",
                "Alice manages Bob's team and works with ACME Corp",
            ],
        )
        .document("hr_notes.txt", &["Carol advises Alice on compliance"])
        .build();

    let layout = layout_dag(&dag, 70, 40);
    let text = render_to_string(&layout);

    eprintln!("=== Example B ===\n{}\n=================", text);

    assert!(text.contains("Bob"), "Missing Bob");
    assert!(text.contains("ACME"), "Missing ACME Corp");
    assert!(text.contains("Carol"), "Missing Carol");
    assert!(
        text.contains("quarterly_report"),
        "Missing quarterly_report.pdf"
    );
    assert!(text.contains("hr_notes"), "Missing hr_notes.txt");
    assert!(
        text.contains('\u{2514}') || text.contains('\u{2518}') || text.contains('\u{251C}'),
        "Should have merge/fork routing characters"
    );
}

#[test]
fn visual_responsive_narrow() {
    let dag = DagBuilder::new()
        .entity_with_labels("Bob", &["MANAGES"])
        .entity_with_labels("ACME Corp", &["WORKS_FOR"])
        .snippet("Alice manages Bob at ACME Corp", &["Bob", "ACME Corp"])
        .document("report.pdf", &["Alice manages Bob at ACME Corp"])
        .build();

    let wide = layout_dag(&dag, 60, 40);
    let wide_text = render_to_string(&wide);
    eprintln!(
        "=== Responsive wide ===\n{}\n=======================",
        wide_text
    );
    assert!(wide_text.contains("Bob"));
    assert!(
        wide_text.contains("report"),
        "Should show document in wide layout"
    );

    let short = layout_dag(&dag, 60, 20);
    let short_text = render_to_string(&short);
    eprintln!(
        "=== Responsive short ===\n{}\n========================",
        short_text
    );
    assert!(!short_text.is_empty());
    let has_entities = short_text.contains("Bob");
    let has_docs = short_text.contains("report");
    assert!(
        has_entities || !has_docs,
        "If entities show, docs should not at short height (progressive collapse)"
    );
}

#[test]
fn layout_dag_empty_dag() {
    let dag = Dag {
        nodes: vec![],
        edges: vec![],
    };
    let layout = layout_dag(&dag, 60, 30);
    assert!(layout.boxes.is_empty());
    assert!(layout.edges.is_empty());
}

/// Build the ACME Corp scenario matching the user's threat-intel dataset.
fn build_acme_dag() -> Dag {
    DagBuilder::new()
        .entity_with_labels("Global Defense Agency", &["selected", "partnering_with"])
        .entity_with_labels("researchers", &["employs"])
        .entity_with_labels("engineers", &["employs"])
        .entity_with_labels("analysts", &["employs"])
        .entity_with_labels("Tokyo", &["maintains_office"])
        .entity_with_labels("London office", &["maintains_office"])
        .entity_with_labels("Washington DC", &["located_in"])
        .snippet(
            "ACME Corp is partnering with Global Defense Agency on a multi-year contract to monitor and counter advanced persistent threats. Global Defense Agency selected ACME Corp",
            &["Global Defense Agency"],
        )
        .snippet(
            "ACME Corp is headquartered in Washington DC and maintains regional offices in London and Tokyo. The firm employs over two thousand analysts, engineers, and researchers dedicated to cybersecurity and intelligence analysis.",
            &["researchers", "engineers", "analysts", "Tokyo", "London office"],
        )
        .snippet(
            "ACME Corp is headquartered in Washington DC",
            &["Washington DC"],
        )
        .document("briefings/strategic-overview-acme.md", &[
            "ACME Corp is partnering with Global Defense Agency on a multi-year contract to monitor and counter advanced persistent threats. Global Defense Agency selected ACME Corp",
            "ACME Corp is headquartered in Washington DC and maintains regional offices in London and Tokyo. The firm employs over two thousand analysts, engineers, and researchers dedicated to cybersecurity and intelligence analysis.",
        ])
        .document("reports/threat-intel-report.md", &[
            "ACME Corp is headquartered in Washington DC",
        ])
        .build()
}

#[test]
fn fix_3_document_boxes_fit_labels() {
    let dag = build_acme_dag();
    let layout = layout_dag(&dag, 200, 50);

    let doc_boxes: Vec<&LayoutBox> = layout
        .boxes
        .iter()
        .filter(|b| b.layer == Layer::Document)
        .collect();
    assert!(!doc_boxes.is_empty(), "Should have document boxes");

    for db in &doc_boxes {
        let node = dag.nodes.iter().find(|n| n.id == db.id).unwrap();
        let label_chars = node.label.chars().count();
        let inner = db.width.saturating_sub(2);
        assert!(
            inner >= label_chars,
            "Fix 3: Document '{}' (label {} chars) truncated in box width {} (inner={}).\n\
             Content: {:?}",
            node.label,
            label_chars,
            db.width,
            inner,
            db.content,
        );
    }
}

#[test]
fn bug_a_no_pipe_gap_between_entity_and_snippet() {
    let dag = build_acme_dag();
    let layout = layout_dag(&dag, 200, 50);
    let _rendered = render_to_string(&layout);

    let entity_boxes: Vec<&LayoutBox> = layout
        .boxes
        .iter()
        .filter(|b| b.layer == Layer::Entity)
        .collect();
    let snippet_boxes: Vec<&LayoutBox> = layout
        .boxes
        .iter()
        .filter(|b| b.layer == Layer::Snippet)
        .collect();
    assert!(!entity_boxes.is_empty(), "Should have entity boxes");
    assert!(!snippet_boxes.is_empty(), "Should have snippet boxes");

    let snippet_top = snippet_boxes.iter().map(|s| s.row).min().unwrap();

    for ebox in &entity_boxes {
        let center_col = ebox.col + ebox.width / 2;
        let entity_bottom = ebox.row + ebox.height;

        let vpipes_at_col: Vec<(usize, usize)> = layout
            .edges
            .iter()
            .filter_map(|seg| match seg {
                EdgeSegment::VPipe {
                    col,
                    row_start,
                    row_end,
                    ..
                } if *col == center_col
                    && *row_start >= entity_bottom
                    && *row_start < snippet_top =>
                {
                    Some((*row_start, *row_end))
                }
                _ => None,
            })
            .collect();

        let earliest_start = vpipes_at_col.iter().map(|(s, _)| *s).min();
        assert!(
            earliest_start.is_some(),
            "Bug A: No VPipe found at col {} (entity '{}') between entity bottom ({}) \
             and snippet top ({}).\nEdges: {:?}",
            center_col,
            ebox.content.first().unwrap_or(&String::new()),
            entity_bottom,
            snippet_top,
            layout
                .edges
                .iter()
                .filter(|seg| match seg {
                    EdgeSegment::VPipe { col, .. } => *col == center_col,
                    _ => false,
                })
                .collect::<Vec<_>>(),
        );
        let start = earliest_start.unwrap();
        let gap = start.saturating_sub(entity_bottom);
        assert!(
            gap <= 1,
            "Bug A: VPipe at col {} (entity '{}') starts {} rows below entity bottom ({}). \
             Pipe starts at row {}. Max allowed gap: 1.",
            center_col,
            ebox.content.first().unwrap_or(&String::new()),
            gap,
            entity_bottom,
            start,
        );
    }
}

#[test]
fn bug_b_pipes_below_snippets_start_promptly() {
    let dag = build_acme_dag();
    let layout = layout_dag(&dag, 200, 50);

    let snippet_boxes: Vec<&LayoutBox> = layout
        .boxes
        .iter()
        .filter(|b| b.layer == Layer::Snippet)
        .collect();
    let doc_boxes: Vec<&LayoutBox> = layout
        .boxes
        .iter()
        .filter(|b| b.layer == Layer::Document)
        .collect();

    if doc_boxes.is_empty() || snippet_boxes.is_empty() {
        return;
    }

    let tallest_snippet_bottom = snippet_boxes
        .iter()
        .map(|s| s.row + s.height)
        .max()
        .unwrap();

    let _snippet_top = snippet_boxes.iter().map(|s| s.row).min().unwrap();
    let doc_top = doc_boxes.iter().map(|d| d.row).min().unwrap();
    let s2d_pipes: Vec<&EdgeSegment> = layout
        .edges
        .iter()
        .filter(|seg| match seg {
            EdgeSegment::VPipe {
                row_start, row_end, ..
            } => *row_start >= tallest_snippet_bottom && *row_start < doc_top && *row_end < doc_top,
            _ => false,
        })
        .collect();

    assert!(
        !s2d_pipes.is_empty(),
        "Bug B: No VPipes found in snippet->doc zone (snippet_bottom={}, doc_top={}).\n\
         All edges: {:?}",
        tallest_snippet_bottom,
        doc_top,
        layout
            .edges
            .iter()
            .filter(|s| matches!(s, EdgeSegment::VPipe { .. }))
            .collect::<Vec<_>>(),
    );

    let earliest_start = s2d_pipes
        .iter()
        .filter_map(|seg| match seg {
            EdgeSegment::VPipe { row_start, .. } => Some(*row_start),
            _ => None,
        })
        .min()
        .unwrap();
    let gap = earliest_start.saturating_sub(tallest_snippet_bottom);
    assert!(
        gap <= 1,
        "Bug B: Earliest snippet->doc pipe starts {} rows below tallest snippet \
         bottom (row {}). Pipe row_start={}. Max allowed gap: 1 row.",
        gap,
        tallest_snippet_bottom,
        earliest_start,
    );

    for sbox in &snippet_boxes {
        let center_col = sbox.col + sbox.width / 2;
        let this_bottom = sbox.row + sbox.height;

        let children: Vec<usize> = dag
            .edges
            .iter()
            .filter(|e| e.from == sbox.id)
            .map(|e| e.to)
            .collect();
        if children.is_empty() {
            continue;
        }

        let pipes_at_col: Vec<usize> = layout
            .edges
            .iter()
            .filter_map(|seg| match seg {
                EdgeSegment::VPipe { col, row_start, .. }
                    if *col == center_col && *row_start >= this_bottom && *row_start < doc_top =>
                {
                    Some(*row_start)
                }
                _ => None,
            })
            .collect();

        if let Some(&start) = pipes_at_col.iter().min() {
            let per_snippet_gap = start.saturating_sub(this_bottom);
            assert!(
                per_snippet_gap <= 1,
                "Fix 1: VPipe at col {} (snippet '{}') starts {} rows below \
                 its own bottom (row {}). Pipe row_start={}. Max gap: 1.\n\
                 Snippet height={}, tallest_bottom={}",
                center_col,
                sbox.content.first().unwrap_or(&String::new()),
                per_snippet_gap,
                this_bottom,
                start,
                sbox.height,
                tallest_snippet_bottom,
            );
        }
    }
}

#[test]
fn bug_d_snippet_centered_under_parent_centroid() {
    let dag = build_acme_dag();
    let layout = layout_dag(&dag, 200, 50);

    let multi_parent_snippet = layout.boxes.iter().find(|b| {
        b.layer == Layer::Snippet
            && b.content.iter().any(|c| c.contains("headquartered"))
            && b.content.iter().any(|c| c.contains("maintains"))
    });

    if let Some(snippet_box) = multi_parent_snippet {
        let snippet_center = snippet_box.col + snippet_box.width / 2;

        let snippet_node = dag
            .nodes
            .iter()
            .find(|n| {
                n.layer == Layer::Snippet
                    && n.label.contains("headquartered")
                    && n.label.contains("maintains")
            })
            .expect("Should find snippet node");

        let parent_ids: Vec<usize> = dag
            .edges
            .iter()
            .filter(|e| e.to == snippet_node.id)
            .map(|e| e.from)
            .collect();

        let parent_boxes: Vec<&LayoutBox> = layout
            .boxes
            .iter()
            .filter(|b| parent_ids.contains(&b.id))
            .collect();

        assert!(!parent_boxes.is_empty(), "Should have parent boxes");

        let parent_centers: Vec<usize> = parent_boxes.iter().map(|b| b.col + b.width / 2).collect();
        let sum: usize = parent_centers.iter().sum();
        let len = parent_centers.len();
        let centroid = (sum + len / 2) / len;

        let offset = (snippet_center as isize - centroid as isize).unsigned_abs();
        assert!(
            offset <= 1,
            "Bug D: Snippet center ({}) should be within 1 col of parent centroid ({}).\n\
             Offset: {} cols. Parent centers: {:?}.\n\
             Snippet col={} width={}",
            snippet_center,
            centroid,
            offset,
            parent_centers,
            snippet_box.col,
            snippet_box.width,
        );
    }
}
