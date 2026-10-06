//! Graph layout and rendering module.
//! See PRD-021-TUI.md §3.1 for the full specification.
//!
//! Pipeline: entities + relations → layout (FR) → grid snap → render (Unicode + Braille)

pub mod braille;
pub(crate) mod chord;
pub mod layout_fr;
mod quadtree;
pub mod render;
mod render_overlay;
pub mod tree_layout;
mod zoom;
pub(crate) use zoom::*;

use std::collections::{HashMap, HashSet};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use uuid::Uuid;

use crate::app::{App, EdgeDisplayMode, EdgeScreenPos, EdgeSpatialGrid, OverlayNodeKind};
use layout_fr::{GridPosition, LayoutEntity, LayoutRelation};
use render::RenderOverlayNode;
use render::{RenderEntity, RenderRelation};

/// Compute layout and render the graph into a ratatui Buffer.
/// This is the top-level entry point called by graph_pane.
///
/// Respects active entity filters (type, confidence). Only filtered entities
/// and relations where both endpoints are visible are rendered.
pub fn layout_and_render(app: &mut App, buf: &mut Buffer, area: Rect) {
    // Collect filtered data into owned structures to avoid borrow conflicts
    // when we later mutate app.graph_positions.
    let visible_ids = app.filtered_entity_ids();
    let selected = app.selected_entity;
    let pinned = app.pinned_entity;
    let pinned_edge = app.pinned_edge;
    let search_active = !app.search_results.is_empty();
    let search_ids: std::collections::HashSet<Uuid> = app.search_results.iter().copied().collect();

    // Check if layout needs recomputation: generation mismatch, viewport resize,
    // or empty positions (first frame).
    let viewport = (area.width, area.height);
    let needs_layout = app.layout_generation != app.layout_computed_generation
        || app.layout_last_viewport != viewport
        || app.graph_positions.is_empty();

    // Compute semantic zoom level early — needed by both layout (collision
    // detection) and rendering (edge endpoints, label cell masking).
    let semantic_zoom = zoom_to_semantic(app.zoom_level);

    if needs_layout {
        let mut layout_entities: Vec<LayoutEntity> = app
            .entities
            .iter()
            .filter(|e| visible_ids.contains(&e.id))
            .map(|e| {
                let degree = app
                    .relations
                    .iter()
                    .filter(|r| {
                        (r.source_id == e.id || r.target_id == e.id)
                            && visible_ids.contains(&r.source_id)
                            && visible_ids.contains(&r.target_id)
                    })
                    .count() as u32;
                LayoutEntity {
                    id: e.id,
                    name: e.name.clone(),
                    degree,
                    label_height: 1,
                }
            })
            .collect();

        // Append overlay nodes (chunks/documents pinned into graph).
        for overlay in &app.graph_overlay_nodes {
            let (width, height) = if overlay.expanded {
                (40u16, 12u16) // expanded box: 40 wide × 12 tall
            } else {
                let collapsed_len = (overlay.label.len().min(38) + 4) as u16; // [| label |]
                (collapsed_len, 1u16)
            };
            let degree = overlay.connected_entity_ids.len() as u32;
            layout_entities.push(LayoutEntity {
                id: overlay.id,
                name: overlay.label.clone(),
                degree,
                label_height: height,
            });
            let _ = width; // width is captured via name length in the layout engine
        }

        let mode = app.edge_display_mode;
        let is_structural_fn =
            |r: &&crate::app::TuiRelation| r.relation_category.as_deref() == Some("structural");

        let layout_visible_rels: Vec<&_> = app
            .relations
            .iter()
            .filter(|r| visible_ids.contains(&r.source_id) && visible_ids.contains(&r.target_id))
            .collect();

        let mut layout_relations: Vec<LayoutRelation> = layout_visible_rels
            .iter()
            .filter(|r| match mode {
                EdgeDisplayMode::StructuralLayout => is_structural_fn(r),
                EdgeDisplayMode::BehavioralLayout => !is_structural_fn(r),
                _ => true,
            })
            .map(|r| LayoutRelation {
                source_id: r.source_id,
                target_id: r.target_id,
            })
            .collect();

        // Add synthetic edges from overlay nodes to their connected entities.
        for overlay in &app.graph_overlay_nodes {
            for entity_id in &overlay.connected_entity_ids {
                if visible_ids.contains(entity_id) {
                    layout_relations.push(LayoutRelation {
                        source_id: overlay.id,
                        target_id: *entity_id,
                    });
                }
            }
        }

        // Partition entities: those connected by layout edges go to FR,
        // the rest go to the radial ring around the force-directed core.
        let force_participant_ids: HashSet<uuid::Uuid> = layout_relations
            .iter()
            .flat_map(|r| [r.source_id, r.target_id])
            .collect();

        let (force_entities, radial_entities): (Vec<LayoutEntity>, Vec<LayoutEntity>) =
            layout_entities
                .into_iter()
                .partition(|e| force_participant_ids.contains(&e.id));

        // Run force-directed layout on connected entities.
        // Pass pan=(0,0) so canonical positions are pan-independent.
        // The actual graph_offset is applied in the derivation step below.
        // Pass canonical_positions as seeds so existing nodes stay stable
        // when new entities arrive during incremental loading.
        let seeds = if app.canonical_positions.is_empty() {
            None
        } else {
            Some(&app.canonical_positions)
        };
        let force_result = layout_fr::layout(
            &force_entities,
            &layout_relations,
            area.width,
            area.height,
            50, // iterations
            app.zoom_level,
            (0, 0),
            semantic_zoom,
            seeds,
        );

        // Place radial entities in a ring around the force-directed core.
        let radial_result = if radial_entities.is_empty() {
            layout_fr::LayoutResult {
                positions: std::collections::HashMap::new(),
            }
        } else {
            let (center, radius) = if force_result.positions.is_empty() {
                // No force core — center in the effective visible area
                // (accounting for floating window occlusion) with a reasonable radius.
                let (cx, cy) = app.effective_visible_center();
                (
                    (cx, cy),
                    // 1.1× for 10% extra ring distance.
                    (area.width.min(area.height) as f64 / 3.0).max(8.0) * 1.1,
                )
            } else {
                let center = layout_fr::centroid(&force_result.positions);
                let max_dist = layout_fr::max_distance_from(center, &force_result.positions);
                // 1.1× for 10% extra ring distance.
                (center, (max_dist + 3.0) / 2.0 * 1.1)
            };

            // Compute ideal angle for each radial entity: direction from
            // ring center toward the centroid of its connections in the
            // force core. This sorts radial nodes so their connection lines
            // don't cross each other.
            let radial_ids: HashSet<uuid::Uuid> = radial_entities.iter().map(|e| e.id).collect();
            let mut ideal_angles: std::collections::HashMap<uuid::Uuid, f64> =
                std::collections::HashMap::new();

            for re in &radial_entities {
                // Find all visible relations connecting this radial entity
                // to a force entity (these are the non-layout edges).
                let mut sum_x = 0.0;
                let mut sum_y = 0.0;
                let mut count = 0u32;
                for rel in &layout_visible_rels {
                    let other = if rel.source_id == re.id && !radial_ids.contains(&rel.target_id) {
                        Some(rel.target_id)
                    } else if rel.target_id == re.id && !radial_ids.contains(&rel.source_id) {
                        Some(rel.source_id)
                    } else {
                        None
                    };
                    if let Some(force_id) = other {
                        if let Some(pos) = force_result.positions.get(&force_id) {
                            sum_x += pos.x as f64 + pos.label_width as f64 / 2.0;
                            sum_y += pos.y as f64 + pos.label_height as f64 / 2.0;
                            count += 1;
                        }
                    }
                }
                if count > 0 {
                    let anchor_x = sum_x / count as f64;
                    let anchor_y = sum_y / count as f64;
                    let angle = (anchor_y - center.1).atan2(anchor_x - center.0);
                    ideal_angles.insert(re.id, angle);
                }
            }

            layout_fr::radial_layout(
                &radial_entities,
                center,
                radius,
                &ideal_angles,
                semantic_zoom,
            )
        };

        // Merge both position maps into canonical_positions (pan-independent).
        let mut positions = force_result.positions;
        positions.extend(radial_result.positions);
        app.canonical_positions = positions;
        app.layout_reference_zoom = app.zoom_level;
        app.last_semantic_zoom = semantic_zoom;
        app.layout_computed_generation = app.layout_generation;
        app.layout_last_viewport = viewport;

        // Anchor compensation: keep the focused entity visually stable when
        // layout is recomputed (e.g. 'r' key cycles edge display mode).
        // Adjust graph_offset by the delta between old and new canonical positions.
        if let (Some(anchor_id), Some((old_cx, old_cy))) = (
            app.layout_anchor_entity.take(),
            app.layout_anchor_pos.take(),
        ) {
            if let Some(new_pos) = app.canonical_positions.get(&anchor_id) {
                let new_cx = new_pos.x as f64 + new_pos.label_width as f64 / 2.0;
                let new_cy = new_pos.y as f64;
                let dx = old_cx - new_cx;
                let dy = old_cy - new_cy;
                app.graph_offset.0 += dx.round() as i32;
                app.graph_offset.1 += dy.round() as i32;
            }
        }
    }

    // Derive graph_positions from canonical_positions + current pan offset
    // and zoom scaling. This runs every dirty frame but is O(n) — much cheaper
    // than the O(n²) force-directed relayout it replaces for pan/zoom changes.
    {
        let zoom_ratio = if app.layout_reference_zoom.abs() > f32::EPSILON {
            app.zoom_level as f64 / app.layout_reference_zoom as f64
        } else {
            1.0
        };
        let pan_x = app.graph_offset.0 as i16;
        let pan_y = app.graph_offset.1 as i16;
        let need_scale = (zoom_ratio - 1.0).abs() > 1e-6;

        // The layout_fr grid snapping places nodes at:
        //   gx = round(nx * base_usable * zoom + margin)
        // where margin = 1. To rescale for a different zoom:
        //   gx_new = round((gx - 1) * (new_zoom / ref_zoom) + 1)
        let derived: HashMap<uuid::Uuid, GridPosition> = app
            .canonical_positions
            .iter()
            .map(|(id, pos)| {
                let (x, y) = if need_scale {
                    (
                        ((pos.x as f64 - 1.0) * zoom_ratio + 1.0).round() as i16 + pan_x,
                        ((pos.y as f64 - 1.0) * zoom_ratio + 1.0).round() as i16 + pan_y,
                    )
                } else {
                    (pos.x + pan_x, pos.y + pan_y)
                };
                (
                    *id,
                    GridPosition {
                        x,
                        y,
                        label_width: pos.label_width,
                        label_height: pos.label_height,
                    },
                )
            })
            .collect();
        app.graph_positions = derived;
    }

    // Normalize PageRank for color mapping. Raw PageRank values sum to 1.0
    // across ALL entities, so with N entities each value ≈ 1/N. Without
    // normalization, they all map to the same low-end color. We rescale to
    // [0, 1] relative to the visible set's min/max so the gradient spreads
    // across the full color range.
    let visible_entities: Vec<_> = app
        .entities
        .iter()
        .filter(|e| visible_ids.contains(&e.id))
        .collect();
    let (pr_min, pr_max) = {
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for e in &visible_entities {
            if let Some(pr) = e.pagerank {
                lo = lo.min(pr);
                hi = hi.max(pr);
            }
        }
        if lo > hi {
            (0.0, 1.0)
        } else {
            (lo, hi)
        }
    };
    let pr_range = pr_max - pr_min;

    let render_entities: Vec<RenderEntity> = visible_entities
        .iter()
        .map(|e| RenderEntity {
            id: e.id,
            name: e.name.clone(),
            entity_type: e.entity_type.clone(),
            confidence: e.confidence,
            selected: selected == Some(e.id),
            community_id: e.community_id,
            pinned: pinned == Some(e.id),
            dimmed: search_active && !search_ids.contains(&e.id),
            convergence_score: e.convergence_score,
            embedding_community_id: e.embedding_community_id,
            user_cluster_id: e.user_cluster_id,
            degree_centrality: e.degree_centrality,
            pagerank: e.pagerank.map(|pr| {
                if pr_range > 0.0 {
                    (pr - pr_min) / pr_range
                } else {
                    0.5 // all entities have equal rank
                }
            }),
        })
        .collect();

    // Partition visible relations based on edge display mode.
    // Each mode controls which edges drive the force layout vs overlay,
    // and whether edges are drawn straight or curved.
    let visible_rels: Vec<&_> = app
        .relations
        .iter()
        .filter(|r| visible_ids.contains(&r.source_id) && visible_ids.contains(&r.target_id))
        .collect();

    let mode = app.edge_display_mode;

    let is_structural =
        |r: &&crate::app::TuiRelation| r.relation_category.as_deref() == Some("structural");

    // Build render relations with per-edge drawing style.
    let render_relations: Vec<RenderRelation> = visible_rels
        .iter()
        .map(|r| {
            let structural = is_structural(r);
            let (draw_curved, is_overlay) = match mode {
                EdgeDisplayMode::StructuralLayout => {
                    if structural {
                        (false, false)
                    } else {
                        (true, true)
                    }
                }
                EdgeDisplayMode::BehavioralLayout => {
                    if structural {
                        (true, true)
                    } else {
                        (false, false)
                    }
                }
                EdgeDisplayMode::AllStraight => (false, false),
                EdgeDisplayMode::AllCurved => (true, false),
                EdgeDisplayMode::BothInLayout => {
                    if structural {
                        (false, false)
                    } else {
                        (true, true)
                    }
                }
            };
            // Highlight edges connected to pinned/selected entity, or the pinned edge.
            let highlighted = pinned_edge == Some(r.id)
                || pinned.is_some_and(|pid| r.source_id == pid || r.target_id == pid);
            RenderRelation {
                source_id: r.source_id,
                target_id: r.target_id,
                weight: r.weight,
                draw_curved,
                is_overlay,
                is_structural: structural,
                highlighted,
            }
        })
        .collect();

    // Recompute edge screen positions using visual label width at the current
    // zoom level. At low zoom, labels are truncated so edge endpoints must
    // target the visible center, not the full-width layout center.
    app.edge_positions.clear();
    for rel in &visible_rels {
        if let (Some(src), Some(tgt)) = (
            app.graph_positions.get(&rel.source_id),
            app.graph_positions.get(&rel.target_id),
        ) {
            let src_vw = visual_label_width(src.label_width, semantic_zoom);
            let tgt_vw = visual_label_width(tgt.label_width, semantic_zoom);
            let src_cx = area.x as i16 + src.x + src_vw as i16 / 2;
            let tgt_cx = area.x as i16 + tgt.x + tgt_vw as i16 / 2;
            let src_cy = area.y as i16 + src.y;
            let tgt_cy = area.y as i16 + tgt.y;
            app.edge_positions.insert(
                rel.id,
                EdgeScreenPos {
                    x1: src_cx,
                    y1: src_cy,
                    x2: tgt_cx,
                    y2: tgt_cy,
                },
            );
        }
    }

    // Build spatial index for edge hit-testing (O(e) where e = visible edges).
    app.edge_spatial_grid = EdgeSpatialGrid::build(&app.edge_positions, area);

    render::render_graph(
        buf,
        area,
        &render_entities,
        &render_relations,
        &app.graph_positions,
        semantic_zoom,
        app.color_palette,
        &app.theme,
    );

    // At zoom 0 (symbol-only view), overlay community names at cluster centers
    // so users can orient without zooming in. Only shown when named communities exist.
    if semantic_zoom == 0 && !app.community_names.is_empty() {
        render_overlay::render_community_labels(
            buf,
            area,
            &render_entities,
            &app.graph_positions,
            &app.community_names,
            &app.theme,
        );
    }

    // Render overlay nodes on top of entities.
    let render_overlays: Vec<RenderOverlayNode> = app
        .graph_overlay_nodes
        .iter()
        .map(|ov| RenderOverlayNode {
            id: ov.id,
            label: ov.label.clone(),
            content: ov.content.clone(),
            expanded: ov.expanded,
            focused: app.focused_overlay == Some(ov.id),
            scroll_offset: ov.overlay_scroll,
            is_document: ov.kind == OverlayNodeKind::Document,
            document_source: ov.document_source.clone(),
            connected_entity_ids: ov.connected_entity_ids.clone(),
        })
        .collect();

    render_overlay::render_overlay_nodes(
        buf,
        area,
        &render_overlays,
        &app.graph_positions,
        semantic_zoom,
        &app.theme,
    );
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
