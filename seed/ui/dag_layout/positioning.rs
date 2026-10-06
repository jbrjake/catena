//! Phase 3: X and Y coordinate assignment.
//!
//! assign_x_entities(), assign_x_children(), plan_tiers(), assign_y().

#![allow(dead_code)]

use super::models::*;
use crate::ui::box_layout::word_wrap;
use crate::ui::flex_layout::{flex_center_with_gaps, flex_distribute, FlexItem};

// ── Phase 3a: X coordinate assignment ────────────────────────────────────────

/// Assign X positions for entity boxes.
/// Returns Vec<(col, width)> in the same order as `ordered_ids`.
pub(super) fn assign_x_entities(
    dag: &Dag,
    ordered_ids: &[usize],
    available_width: usize,
) -> Vec<(usize, usize)> {
    if ordered_ids.is_empty() {
        return Vec::new();
    }

    let n = ordered_ids.len();
    let max_box_width = if n > 0 {
        available_width / n
    } else {
        available_width
    };

    let box_widths: Vec<usize> = ordered_ids
        .iter()
        .map(|&id| {
            let node = dag
                .nodes
                .iter()
                .find(|n| n.id == id)
                .expect("ordered id must exist in dag");
            let label_w = node.label.chars().count() + 4; // +2 border +2 padding
            let secondary_w = node
                .secondary_labels
                .iter()
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(0);
            let content_w = label_w.max(secondary_w + 4);
            let min_w = node.min_width.unwrap_or(8);
            content_w.max(min_w).min(max_box_width)
        })
        .collect();

    let widths_u16: Vec<u16> = box_widths.iter().map(|&w| w as u16).collect();
    let offsets = flex_center_with_gaps(available_width as u16, &widths_u16);

    offsets
        .iter()
        .zip(box_widths.iter())
        .map(|(&off, &w)| (off as usize, w))
        .collect()
}

// ── Phase 3b: X coordinate assignment for children ──────────────────────────

/// Assign X positions for snippet or document boxes.
/// Each child is centered at the centroid of its parents' centers.
/// Single-parent children are constrained to parent width.
///
/// Returns Vec<(node_id, col, width)>.
pub(super) fn assign_x_children(
    dag: &Dag,
    child_ids: &[usize],
    parent_positions: &[(usize, usize, usize)], // (node_id, col, width)
    available_width: usize,
    wrap_width_for_content: bool,
) -> Vec<(usize, usize, usize)> {
    if child_ids.is_empty() {
        return Vec::new();
    }

    let parent_center = |pid: usize| -> usize {
        parent_positions
            .iter()
            .find(|(id, _, _)| *id == pid)
            .map(|(_, col, w)| col + w / 2)
            .unwrap_or(available_width / 2)
    };

    let parent_width = |pid: usize| -> usize {
        parent_positions
            .iter()
            .find(|(id, _, _)| *id == pid)
            .map(|(_, _, w)| *w)
            .unwrap_or(20)
    };

    let mut result: Vec<(usize, usize, usize)> = Vec::new();

    for &cid in child_ids {
        let node = dag
            .nodes
            .iter()
            .find(|n| n.id == cid)
            .expect("child id must exist in dag");
        let parents: Vec<usize> = dag
            .edges
            .iter()
            .filter(|e| e.to == cid)
            .map(|e| e.from)
            .collect();

        let (center, max_width) = if parents.len() == 1 {
            let pc = parent_center(parents[0]);
            let pw = parent_width(parents[0]);
            // Snippet boxes need enough width to avoid breaking common words.
            // Documents (non-wrapping) grow to fit their label, capped at 40% of total width.
            let min_w = if wrap_width_for_content {
                pw.max(20)
            } else {
                let label_w = dag
                    .nodes
                    .iter()
                    .find(|n| n.id == cid)
                    .map(|n| n.label.chars().count() + 4)
                    .unwrap_or(pw);
                pw.max(label_w).min(available_width * 40 / 100)
            };
            (pc, min_w)
        } else if parents.len() > 1 {
            let centers: Vec<usize> = parents.iter().map(|&pid| parent_center(pid)).collect();
            let sum: usize = centers.iter().sum();
            let len = centers.len();
            let centroid = (sum + len / 2) / len; // Rounded division
            let leftmost = *centers.iter().min().expect("centers is non-empty");
            let rightmost = *centers.iter().max().expect("centers is non-empty");
            let span = rightmost - leftmost + 4;
            let max_w = span.max(20).min(available_width);
            (centroid, max_w)
        } else {
            (available_width / 2, available_width / 2)
        };

        let content_w = if wrap_width_for_content {
            let wrap_target = max_width.saturating_sub(2).max(8);
            let wrapped = word_wrap(&node.label, wrap_target);
            let actual_content_w = wrapped.iter().map(|l| l.chars().count()).max().unwrap_or(8);
            actual_content_w + 2
        } else {
            let label_w = node.label.chars().count() + 4;
            label_w.max(node.min_width.unwrap_or(8))
        };

        let width = content_w.min(max_width).max(8);
        let col = center.saturating_sub(width / 2);
        let col = col.min(available_width.saturating_sub(width));

        result.push((cid, col, width));
    }

    // Overlap resolution: left-to-right sweep.
    let min_gap = 2;
    result.sort_by_key(|&(_, col, _)| col);
    for i in 1..result.len() {
        let prev_right = result[i - 1].1 + result[i - 1].2;
        if result[i].1 < prev_right + min_gap {
            result[i].1 = prev_right + min_gap;
        }
        if result[i].1 + result[i].2 > available_width {
            result[i].1 = available_width.saturating_sub(result[i].2);
        }
    }

    result
}

// ── Phase 3b: Y coordinate assignment ────────────────────────────────────────

pub(super) struct TierPlan {
    pub show_entities: bool,
    pub show_snippets: bool,
    pub show_docs: bool,
}

/// Determine which tiers to show based on available height.
/// Tiers collapse bottom-up: docs → snippets → entities.
pub(super) fn plan_tiers(height: usize, has_connections: bool) -> TierPlan {
    if !has_connections {
        return TierPlan {
            show_entities: false,
            show_snippets: false,
            show_docs: false,
        };
    }
    // Minimum space: card(5) + bus(4) + entities(4) + padding(2) = 15.
    let base = 15;
    let snippet_tier_min = 10; // route(2) + snippets(5) + padding(3)
    let doc_tier_min = 10; // route(2) + docs(5) + padding(3)

    TierPlan {
        show_entities: height >= base,
        show_snippets: height >= base + snippet_tier_min,
        show_docs: height >= base + snippet_tier_min + doc_tier_min,
    }
}

/// Assign Y coordinates to all layout tiers.
/// Returns a Vec of (tier_name, row_offset, row_count) for each visible zone.
///
/// Zones: Card, Bus, Entities, Route1→2, Snippets, Route2→3, Documents.
pub(super) fn assign_y(
    height: usize,
    plan: &TierPlan,
    n_label_rows: usize,
    n_entity_snippet_bars: usize,
    snippet_content_height: usize,
    n_snippet_doc_bars: usize,
) -> Vec<(&'static str, usize, usize)> {
    let mut items: Vec<(&str, FlexItem)> = Vec::new();

    // Card (always present).
    items.push((
        "card",
        FlexItem {
            min: 4,
            preferred: 6,
            max: 12,
        },
    ));

    if plan.show_entities {
        let label_rows = n_label_rows as u16;
        items.push((
            "bus",
            FlexItem {
                min: 2 + label_rows,
                preferred: 3 + label_rows,
                max: 6 + label_rows,
            },
        ));
        items.push((
            "entities",
            FlexItem {
                min: 3,
                preferred: 3,
                max: 7,
            },
        ));

        if plan.show_snippets {
            let bars = n_entity_snippet_bars.max(1) as u16;
            items.push((
                "route_e2s",
                FlexItem {
                    min: 1,
                    preferred: bars + 1,
                    max: bars + 4,
                },
            ));
            let snip_pref = (snippet_content_height + 2) as u16;
            items.push((
                "snippets",
                FlexItem {
                    min: 3,
                    preferred: snip_pref.max(3),
                    max: snip_pref.saturating_add(20).max(10),
                },
            ));

            if plan.show_docs {
                let doc_bars = n_snippet_doc_bars.max(1) as u16;
                items.push((
                    "route_s2d",
                    FlexItem {
                        min: 1,
                        preferred: doc_bars + 1,
                        max: doc_bars + 4,
                    },
                ));
                items.push((
                    "docs",
                    FlexItem {
                        min: 3,
                        preferred: 3,
                        max: 7,
                    },
                ));
            }
        }
    }

    let flex_items: Vec<FlexItem> = items.iter().map(|(_, fi)| *fi).collect();
    let results = flex_distribute(height as u16, &flex_items);

    items
        .iter()
        .zip(results.iter())
        .map(|((name, _), fr)| (*name, fr.offset as usize, fr.size as usize))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::dag_layout::builder::DagBuilder;

    #[test]
    fn assign_x_entities_evenly_spaced() {
        let dag = DagBuilder::new()
            .entity("Bob")
            .entity("ACME Corp")
            .entity("Carol")
            .build();
        let ordered = vec![0, 1, 2]; // Bob, ACME, Carol
        let positions = assign_x_entities(&dag, &ordered, 60);

        assert_eq!(positions.len(), 3);
        for (col, width) in &positions {
            assert!(
                *col + *width <= 60,
                "Box at col {} width {} exceeds 60",
                col,
                width
            );
        }
        for i in 1..positions.len() {
            assert!(
                positions[i].0 >= positions[i - 1].0 + positions[i - 1].1,
                "Boxes overlap: {:?}",
                positions
            );
        }
    }

    #[test]
    fn assign_x_snippets_centroid_between_parents() {
        let dag = DagBuilder::new()
            .entity("Bob")
            .entity("ACME Corp")
            .snippet("Shared snippet", &["Bob", "ACME Corp"])
            .build();

        let parent_positions = vec![
            (0, 5, 12),  // Bob at col 5, width 12, center 11
            (1, 35, 14), // ACME at col 35, width 14, center 42
        ];
        let result = assign_x_children(&dag, &[2], &parent_positions, 60, true);

        assert_eq!(result.len(), 1);
        let (_, col, width) = result[0];
        let center = col + width / 2;
        assert!(
            (center as i32 - 26).abs() <= 3,
            "Snippet center {} should be near 26",
            center
        );
    }

    #[test]
    fn assign_x_single_parent_child_not_wider() {
        let dag = DagBuilder::new()
            .entity("Bob")
            .snippet("Short", &["Bob"])
            .build();

        let parent_positions = vec![(0, 10, 14)]; // Bob: col 10, width 14
        let result = assign_x_children(&dag, &[1], &parent_positions, 60, true);

        let (_, _, width) = result[0];
        assert!(
            width <= 14,
            "Single-parent child width {} should be <= parent width 14",
            width
        );
    }

    #[test]
    fn tier_plan_collapses_at_short_heights() {
        let full = plan_tiers(50, true);
        assert!(full.show_entities && full.show_snippets && full.show_docs);

        let no_docs = plan_tiers(32, true);
        assert!(no_docs.show_entities && no_docs.show_snippets && !no_docs.show_docs);

        let no_snippets = plan_tiers(22, true);
        assert!(no_snippets.show_entities && !no_snippets.show_snippets && !no_snippets.show_docs);
    }

    #[test]
    fn assign_y_produces_valid_zones() {
        let plan = TierPlan {
            show_entities: true,
            show_snippets: true,
            show_docs: true,
        };
        let zones = assign_y(50, &plan, 1, 2, 3, 1);

        let names: Vec<&str> = zones.iter().map(|(n, _, _)| *n).collect();
        assert!(names.contains(&"card"));
        assert!(names.contains(&"entities"));
        assert!(names.contains(&"snippets"));
        assert!(names.contains(&"docs"));

        for i in 1..zones.len() {
            let (_, prev_off, prev_size) = zones[i - 1];
            let (_, cur_off, _) = zones[i];
            assert!(
                cur_off >= prev_off + prev_size,
                "Zone {:?} overlaps {:?}",
                zones[i],
                zones[i - 1]
            );
        }

        if let Some(&(_, off, size)) = zones.last() {
            assert!(
                off + size <= 50,
                "Zones exceed height: last zone ends at {}",
                off + size
            );
        }
    }

    #[test]
    fn plan_tiers_no_connections() {
        let plan = plan_tiers(50, false);
        assert!(!plan.show_entities);
        assert!(!plan.show_snippets);
        assert!(!plan.show_docs);
    }
}
