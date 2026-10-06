//! Layout orchestrator: layout_dag() entry point + route_between_layers() + helpers.

#![allow(dead_code)]

use super::models::*;
use super::ordering::order_entities;
use super::positioning::{assign_x_children, assign_x_entities, assign_y, plan_tiers};
use super::routing::{build_merge_bar_with_continuation, detect_crossings};
use crate::ui::box_layout::word_wrap;

/// Count how many routing bars are needed between two layers.
fn count_routing_bars(dag: &Dag, parent_ids: &[usize], child_ids: &[usize]) -> usize {
    let multi_parent_children = child_ids
        .iter()
        .filter(|&&cid| {
            dag.edges
                .iter()
                .filter(|e| e.to == cid && parent_ids.contains(&e.from))
                .count()
                > 1
        })
        .count();
    let forking_parents = parent_ids
        .iter()
        .filter(|&&pid| {
            dag.edges
                .iter()
                .filter(|e| e.from == pid && child_ids.contains(&e.to))
                .count()
                > 1
        })
        .count();
    multi_parent_children.max(forking_parents).max(1)
}

pub(super) fn pad_center_str(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.chars().take(width).collect();
    }
    let left = (width - len) / 2;
    let right = width - len - left;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(right))
}

pub(super) fn pad_right_str(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.chars().take(width).collect();
    }
    format!("{}{}", s, " ".repeat(width - len))
}

/// Route all edges between two adjacent layers.
///
/// Groups edges by child (merge) and by parent (fork), builds bars,
/// adds vertical pipes, detects crossings.
fn route_between_layers(
    dag: &Dag,
    boxes: &[LayoutBox],
    parent_ids: &[usize],
    child_ids: &[usize],
    pipe_start_row: usize,
    route_row: usize,
    route_height: usize,
) -> Vec<EdgeSegment> {
    let mut segments: Vec<EdgeSegment> = Vec::new();

    let box_center = |id: usize| -> usize {
        boxes
            .iter()
            .find(|b| b.id == id)
            .map(|b| b.col + b.width / 2)
            .unwrap_or(0)
    };

    let box_bottom = |id: usize| -> usize {
        boxes
            .iter()
            .find(|b| b.id == id)
            .map(|b| b.row + b.height)
            .unwrap_or(pipe_start_row)
    };

    // Group edges by child (merge groups).
    let mut merge_groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for &cid in child_ids {
        let parents: Vec<usize> = dag
            .edges
            .iter()
            .filter(|e| e.to == cid && parent_ids.contains(&e.from))
            .map(|e| e.from)
            .collect();
        if !parents.is_empty() {
            merge_groups.push((cid, parents));
        }
    }

    // Group edges by parent (fork groups) — for continuation detection.
    let mut fork_groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for &pid in parent_ids {
        let children: Vec<usize> = dag
            .edges
            .iter()
            .filter(|e| e.from == pid && child_ids.contains(&e.to))
            .map(|e| e.to)
            .collect();
        if children.len() > 1 {
            fork_groups.push((pid, children));
        }
    }

    let forking_parents: Vec<usize> = fork_groups.iter().map(|(pid, _)| *pid).collect();

    // Build merge bars and vertical pipes.
    // Place the bar midway between the actual parent bottoms and child tops,
    // not at the zone midpoint (which may be much larger than needed).
    let child_top = child_ids
        .iter()
        .filter_map(|&cid| boxes.iter().find(|b| b.id == cid))
        .map(|b| b.row)
        .min()
        .unwrap_or(route_row + route_height);
    // Use the max individual parent bottom (not the passed-in pipe_start_row)
    // so bar_row sits midway between the tallest parent's bottom and child tops.
    let max_parent_bottom = parent_ids
        .iter()
        .map(|&pid| box_bottom(pid))
        .max()
        .unwrap_or(pipe_start_row);
    let bar_row = max_parent_bottom + (child_top.saturating_sub(max_parent_bottom)) / 2;
    for (cid, parents) in &merge_groups {
        let child_box = boxes.iter().find(|b| b.id == *cid);

        if parents.len() > 1 {
            // Multi-parent: merge bar + pipes above and below.
            let parent_centers: Vec<(usize, usize)> =
                parents.iter().map(|&pid| (pid, box_center(pid))).collect();
            let child_center = box_center(*cid);
            let continues = parents
                .iter()
                .find(|&&pid| forking_parents.contains(&pid))
                .copied();
            let bar = build_merge_bar_with_continuation(
                &parent_centers,
                child_center,
                bar_row,
                EdgeColor::Neutral,
                continues,
            );
            segments.extend(bar);

            // Vertical pipes from parents to merge bar.
            for &pid in parents {
                let pc = box_center(pid);
                let pbot = box_bottom(pid);
                if pbot < bar_row {
                    segments.push(EdgeSegment::VPipe {
                        col: pc,
                        row_start: pbot,
                        row_end: bar_row.saturating_sub(1),
                        color: EdgeColor::Neutral,
                    });
                }
            }

            // Vertical pipe from merge bar to child.
            let cc = box_center(*cid);
            if let Some(cb) = child_box {
                if bar_row < cb.row {
                    segments.push(EdgeSegment::VPipe {
                        col: cc,
                        row_start: bar_row + 1,
                        row_end: cb.row.saturating_sub(1),
                        color: EdgeColor::Neutral,
                    });
                }
            }
        } else {
            // Single-parent: continuous pipe, no merge bar needed.
            let pc = box_center(parents[0]);
            let pbot = box_bottom(parents[0]);
            let cc = box_center(*cid);
            if let Some(cb) = child_box {
                // Pipe from parent center to child top.
                if pc == cc {
                    // Straight pipe — one continuous segment.
                    if pbot < cb.row {
                        segments.push(EdgeSegment::VPipe {
                            col: pc,
                            row_start: pbot,
                            row_end: cb.row.saturating_sub(1),
                            color: EdgeColor::Neutral,
                        });
                    }
                } else {
                    // Different columns: pipe down, bar, pipe down.
                    if pbot < bar_row {
                        segments.push(EdgeSegment::VPipe {
                            col: pc,
                            row_start: pbot,
                            row_end: bar_row.saturating_sub(1),
                            color: EdgeColor::Neutral,
                        });
                    }
                    // Horizontal connection at bar_row.
                    let (left, right) = if pc < cc { (pc, cc) } else { (cc, pc) };
                    segments.push(EdgeSegment::HBar {
                        row: bar_row,
                        col_start: left,
                        col_end: right,
                        color: EdgeColor::Neutral,
                    });
                    if bar_row < cb.row {
                        segments.push(EdgeSegment::VPipe {
                            col: cc,
                            row_start: bar_row + 1,
                            row_end: cb.row.saturating_sub(1),
                            color: EdgeColor::Neutral,
                        });
                    }
                }
            }
        }
    }

    // Detect crossings.
    let hbars: Vec<EdgeSegment> = segments
        .iter()
        .filter(|s| matches!(s, EdgeSegment::HBar { .. }))
        .cloned()
        .collect();
    let vpipes: Vec<EdgeSegment> = segments
        .iter()
        .filter(|s| matches!(s, EdgeSegment::VPipe { .. }))
        .cloned()
        .collect();
    let crossings = detect_crossings(&hbars, &vpipes);
    segments.extend(crossings);

    segments
}

// ── Entry point ──────────────────────────────────────────────────────────────

/// Compute a complete DAG layout for the given width and height.
///
/// Pure function: no TUI dependency. Recompute on every frame for responsive behavior.
pub(crate) fn layout_dag(dag: &Dag, width: usize, height: usize) -> Layout {
    let mut boxes: Vec<LayoutBox> = Vec::new();
    let mut edges: Vec<EdgeSegment> = Vec::new();

    let entity_ids: Vec<usize> = dag
        .nodes
        .iter()
        .filter(|n| n.layer == Layer::Entity)
        .map(|n| n.id)
        .collect();
    let snippet_ids: Vec<usize> = dag
        .nodes
        .iter()
        .filter(|n| n.layer == Layer::Snippet)
        .map(|n| n.id)
        .collect();
    let doc_ids: Vec<usize> = dag
        .nodes
        .iter()
        .filter(|n| n.layer == Layer::Document)
        .map(|n| n.id)
        .collect();

    let has_connections = !entity_ids.is_empty();
    let plan = plan_tiers(height, has_connections);

    if !plan.show_entities {
        return Layout {
            boxes,
            edges,
            total_rows: height,
            total_cols: width,
        };
    }

    // Phase 2: Order entities.
    let ordered_entities = order_entities(dag);

    // Phase 3a: X coordinates.
    let entity_x = assign_x_entities(dag, &ordered_entities, width);

    let entity_positions: Vec<(usize, usize, usize)> = ordered_entities
        .iter()
        .zip(entity_x.iter())
        .map(|(&id, &(col, w))| (id, col, w))
        .collect();

    // Order snippets by centroid of parents.
    let mut ordered_snippets = snippet_ids.clone();
    ordered_snippets.sort_by_key(|&sid| {
        let parent_centers: Vec<usize> = dag
            .edges
            .iter()
            .filter(|e| e.to == sid)
            .filter_map(|e| entity_positions.iter().find(|(id, _, _)| *id == e.from))
            .map(|(_, col, w)| col + w / 2)
            .collect();
        if parent_centers.is_empty() {
            width / 2
        } else {
            parent_centers.iter().sum::<usize>() / parent_centers.len()
        }
    });

    let snippet_x = assign_x_children(dag, &ordered_snippets, &entity_positions, width, true);

    let snippet_positions: Vec<(usize, usize, usize)> = snippet_x.clone();
    let doc_x = if plan.show_docs {
        let mut ordered_docs = doc_ids.clone();
        ordered_docs.sort_by_key(|&did| {
            let parent_centers: Vec<usize> = dag
                .edges
                .iter()
                .filter(|e| e.to == did)
                .filter_map(|e| snippet_positions.iter().find(|(id, _, _)| *id == e.from))
                .map(|(_, col, w)| col + w / 2)
                .collect();
            if parent_centers.is_empty() {
                width / 2
            } else {
                parent_centers.iter().sum::<usize>() / parent_centers.len()
            }
        });
        assign_x_children(dag, &ordered_docs, &snippet_positions, width, false)
    } else {
        Vec::new()
    };

    // Phase 3b: Y coordinates.
    let n_label_rows = ordered_entities
        .iter()
        .filter_map(|&id| dag.nodes.iter().find(|n| n.id == id))
        .map(|n| n.secondary_labels.len())
        .max()
        .unwrap_or(0);

    let n_e2s_bars = count_routing_bars(dag, &ordered_entities, &ordered_snippets);
    let n_s2d_bars = if plan.show_docs {
        count_routing_bars(
            dag,
            &ordered_snippets,
            &doc_x.iter().map(|&(id, _, _)| id).collect::<Vec<_>>(),
        )
    } else {
        0
    };

    let snippet_content_height = snippet_x
        .iter()
        .filter_map(|&(id, _, w)| {
            let node = dag.nodes.iter().find(|n| n.id == id)?;
            let inner = w.saturating_sub(2).max(4);
            Some(word_wrap(&node.label, inner).len())
        })
        .max()
        .unwrap_or(1);

    let zones = assign_y(
        height,
        &plan,
        n_label_rows,
        n_e2s_bars,
        snippet_content_height,
        n_s2d_bars,
    );

    // ── Build LayoutBoxes ────────────────────────────────────────────────

    // Entity boxes.
    if let Some(&(_, entity_row, _entity_height)) =
        zones.iter().find(|(name, _, _)| *name == "entities")
    {
        for (&eid, &(col, w)) in ordered_entities.iter().zip(entity_x.iter()) {
            let node = dag
                .nodes
                .iter()
                .find(|n| n.id == eid)
                .expect("entity id must exist in dag");
            let inner = w.saturating_sub(2);
            let content = vec![pad_center_str(&node.label, inner)];
            boxes.push(LayoutBox {
                id: eid,
                col,
                width: w,
                row: entity_row,
                height: 3,
                content,
                border: BoxBorder::Single,
                layer: Layer::Entity,
            });
        }
    }

    // Snippet boxes.
    if let Some(&(_, snip_row, snip_height)) = zones.iter().find(|(name, _, _)| *name == "snippets")
    {
        for &(sid, col, w) in &snippet_x {
            let node = dag
                .nodes
                .iter()
                .find(|n| n.id == sid)
                .expect("snippet id must exist in dag");
            let inner = w.saturating_sub(2).max(4);
            let wrapped = word_wrap(&node.label, inner);
            let max_content = snip_height.saturating_sub(2);
            let content: Vec<String> = wrapped
                .into_iter()
                .take(max_content)
                .map(|line| pad_right_str(&line, inner))
                .collect();
            let h = content.len() + 2;
            boxes.push(LayoutBox {
                id: sid,
                col,
                width: w,
                row: snip_row,
                height: h,
                content,
                border: BoxBorder::Single,
                layer: Layer::Snippet,
            });
        }
    }

    // Document boxes.
    if plan.show_docs {
        if let Some(&(_, doc_row, _)) = zones.iter().find(|(name, _, _)| *name == "docs") {
            for &(did, col, w) in &doc_x {
                let node = dag
                    .nodes
                    .iter()
                    .find(|n| n.id == did)
                    .expect("doc id must exist in dag");
                let inner = w.saturating_sub(2);
                let content = vec![pad_center_str(&node.label, inner)];
                boxes.push(LayoutBox {
                    id: did,
                    col,
                    width: w,
                    row: doc_row,
                    height: 3,
                    content,
                    border: BoxBorder::Single,
                    layer: Layer::Document,
                });
            }
        }
    }

    // ── Phase 4: Route edges ─────────────────────────────────────────────

    // Route entity → snippet edges.
    if let Some(&(_, route_row, route_h)) = zones.iter().find(|(n, _, _)| *n == "route_e2s") {
        let entity_box_bottom = boxes
            .iter()
            .filter(|b| ordered_entities.contains(&b.id))
            .map(|b| b.row + b.height)
            .max()
            .unwrap_or(route_row);
        let e2s_edges = route_between_layers(
            dag,
            &boxes,
            &ordered_entities,
            &ordered_snippets,
            entity_box_bottom,
            route_row,
            route_h,
        );
        edges.extend(e2s_edges);
    }

    // Route snippet → document edges.
    if plan.show_docs {
        if let Some(&(_, route_row, route_h)) = zones.iter().find(|(n, _, _)| *n == "route_s2d") {
            let snippet_box_bottom = boxes
                .iter()
                .filter(|b| ordered_snippets.contains(&b.id))
                .map(|b| b.row + b.height)
                .max()
                .unwrap_or(route_row);
            let s2d_edges = route_between_layers(
                dag,
                &boxes,
                &ordered_snippets,
                &doc_x.iter().map(|&(id, _, _)| id).collect::<Vec<_>>(),
                snippet_box_bottom,
                route_row,
                route_h,
            );
            edges.extend(s2d_edges);
        }
    }

    Layout {
        boxes,
        edges,
        total_rows: height,
        total_cols: width,
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
