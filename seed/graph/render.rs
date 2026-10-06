//! Graph -> terminal cell rendering.
//! Converts positioned nodes and edges into styled terminal characters.
//! See PRD-021-TUI.md §3.1.2 for the edge rendering algorithm.

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::braille::BrailleCanvas;
use super::layout_fr::GridPosition;
use super::render_overlay::{render_canvas_to_buf, render_canvas_to_buf_overwrite, render_node};
use crate::app::ColorPalette;
use crate::ui::theme::Theme;

/// Entity type → display color mapping (delegates to theme).
pub(crate) fn type_color(entity_type: &str, theme: &Theme) -> Color {
    theme.entity_types.type_color(entity_type)
}

/// Map a 0.0–1.0 convergence score to a red→blue gradient.
/// 0.0 (divergent) = red, 1.0 (convergent) = blue.
pub(crate) fn convergence_color(value: f64) -> Color {
    let v = value.clamp(0.0, 1.0);
    // Red (0.0) → Purple (0.5) → Blue (1.0)
    let r = ((1.0 - v) * 255.0) as u8;
    let b = (v * 255.0) as u8;
    let g = ((0.5 - (v - 0.5).abs()) * 128.0) as u8; // slight purple at midpoint
    Color::Rgb(r, g, b)
}

/// Map a 0.0–1.0 confidence/score value to a red→yellow→green gradient.
pub(crate) fn confidence_color(value: f64) -> Color {
    let v = value.clamp(0.0, 1.0);
    if v < 0.5 {
        // Red → Yellow (0.0 → 0.5)
        let t = (v * 2.0 * 255.0) as u8;
        Color::Rgb(255, t, 0)
    } else {
        // Yellow → Green (0.5 → 1.0)
        let t = ((v - 0.5) * 2.0 * 255.0) as u8;
        Color::Rgb(255 - t, 255, 0)
    }
}

/// Map degree centrality to a blue→cyan→yellow→red gradient.
/// Low connectivity nodes are cool blue, high-hub nodes are hot red.
/// Normalizes against a soft cap of 20 connections.
pub(crate) fn centrality_color(degree: i32) -> Color {
    let v = (degree.max(0) as f64 / 20.0).clamp(0.0, 1.0);
    if v < 0.33 {
        // Blue → Cyan
        let t = (v / 0.33 * 255.0) as u8;
        Color::Rgb(0, t, 255)
    } else if v < 0.66 {
        // Cyan → Yellow
        let t = (v - 0.33) / 0.33;
        let r = (t * 255.0) as u8;
        let g = 255;
        let b = (255.0 * (1.0 - t)) as u8;
        Color::Rgb(r, g, b)
    } else {
        // Yellow → Red
        let t = (v - 0.66) / 0.34;
        let g = (255.0 * (1.0 - t)) as u8;
        Color::Rgb(255, g, 0)
    }
}

/// Map PageRank (typically 0.0–1.0) to a dim blue → bright gold gradient.
/// Low importance = cool dim, high importance = bright warm.
pub(crate) fn pagerank_color(rank: f64) -> Color {
    let v = rank.clamp(0.0, 1.0);
    // Blue/purple (low) → Amber/gold (high)
    let r = (v * 255.0) as u8;
    let g = (v * 200.0) as u8;
    let b = ((1.0 - v) * 200.0) as u8;
    Color::Rgb(r, g, b)
}

/// Resolve display color based on the active palette (convenience wrapper).
#[cfg(test)]
pub(crate) fn entity_color(
    entity_type: &str,
    community_id: Option<i32>,
    confidence: f64,
    palette: ColorPalette,
    theme: &Theme,
) -> Color {
    entity_color_full(
        entity_type,
        community_id,
        confidence,
        None,
        None,
        None,
        None,
        None,
        palette,
        theme,
    )
}

/// Full entity color resolution with all embedding fields.
///
/// `pagerank` is expected to be **pre-normalized** to [0, 1] relative to the
/// visible entity set (see `normalize_pagerank` in `graph/mod.rs`). Raw
/// PageRank values sum to 1.0 across the entire graph, so without
/// normalization they cluster near zero and produce indistinguishable colors.
#[allow(clippy::too_many_arguments)]
pub(crate) fn entity_color_full(
    entity_type: &str,
    community_id: Option<i32>,
    confidence: f64,
    convergence_score: Option<f64>,
    embedding_community_id: Option<i32>,
    user_cluster_id: Option<i32>,
    degree_centrality: Option<i32>,
    pagerank: Option<f64>,
    palette: ColorPalette,
    theme: &Theme,
) -> Color {
    let community_palette = &theme.colors.community_palette;
    match palette {
        ColorPalette::Community => match community_id {
            // -1 is the "Others" sentinel: sub-threshold communities (singletons/pairs).
            // Render dim so they visually recede behind named communities.
            Some(-1) => Color::DarkGray,
            Some(cid) => community_palette[(cid as usize) % community_palette.len()],
            None => type_color(entity_type, theme),
        },
        ColorPalette::EntityType => type_color(entity_type, theme),
        ColorPalette::Confidence => confidence_color(confidence),
        ColorPalette::EmbeddingCommunity => match embedding_community_id {
            Some(cid) => {
                // Offset by 3 to visually distinguish from Louvain communities.
                community_palette[((cid.unsigned_abs() as usize) + 3) % community_palette.len()]
            }
            None => type_color(entity_type, theme),
        },
        ColorPalette::Convergence => match convergence_score {
            Some(v) => convergence_color(v),
            None => type_color(entity_type, theme),
        },
        ColorPalette::CustomCluster => match user_cluster_id {
            Some(cid) => {
                // Offset by 5 to visually distinguish from both community palettes.
                community_palette[((cid.unsigned_abs() as usize) + 5) % community_palette.len()]
            }
            None => type_color(entity_type, theme),
        },
        ColorPalette::DegreeCentrality => centrality_color(degree_centrality.unwrap_or(0)),
        ColorPalette::PageRank => match pagerank {
            Some(v) => pagerank_color(v),
            None => type_color(entity_type, theme),
        },
        ColorPalette::Overlay => {
            // Overlay color resolution happens in the graph pane render loop,
            // which has access to the overlay data. This fallback returns the
            // entity type color when no overlay data is available at this layer.
            type_color(entity_type, theme)
        }
    }
}

/// Dim a color to ~25% intensity for glow halo backgrounds.
pub(super) fn dim_color(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(r / 4, g / 4, b / 4),
        _ => Color::Rgb(20, 18, 30),
    }
}

/// Entity type → single-character symbol (used at zoom level 0 and legend).
pub(crate) fn type_symbol(entity_type: &str, theme: &Theme) -> char {
    theme.entity_types.type_symbol(entity_type)
}

/// Information passed per entity for rendering.
#[allow(dead_code)] // Fields used once render pipeline reads all entity properties.
pub struct RenderEntity {
    pub id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub confidence: f64,
    pub selected: bool,
    pub community_id: Option<i32>,
    pub pinned: bool,
    /// When true, render in `theme.colors.dim_text` (non-matching entity during active search).
    pub dimmed: bool,
    pub convergence_score: Option<f64>,
    pub embedding_community_id: Option<i32>,
    pub user_cluster_id: Option<i32>,
    pub degree_centrality: Option<i32>,
    pub pagerank: Option<f64>,
}

/// Information passed per relation for rendering.
pub struct RenderRelation {
    pub source_id: Uuid,
    pub target_id: Uuid,
    #[allow(dead_code)] // Reserved for weight-based edge styling in future zoom levels.
    pub weight: f64,
    /// Draw as Bézier curve instead of straight line.
    pub draw_curved: bool,
    /// Overlay edges use a distinct color (purple) vs layout edges (DarkGray).
    pub is_overlay: bool,
    /// True for structural edges (hierarchy, containment), false for behavioral.
    /// Both render as solid lines; color alone differentiates the categories.
    pub is_structural: bool,
    /// Edge should glow (pinned edge, or connected to pinned/selected node).
    pub highlighted: bool,
}

/// Information passed per overlay node (chunk/document pinned into graph).
pub struct RenderOverlayNode {
    pub id: Uuid,
    pub label: String,
    pub content: String,
    pub expanded: bool,
    pub focused: bool,
    pub scroll_offset: usize,
    pub is_document: bool,
    pub document_source: String,
    pub connected_entity_ids: Vec<Uuid>,
}

/// Render the graph (nodes + edges) onto a ratatui Buffer.
///
/// `zoom_level` controls semantic zoom:
/// - 0: single character by type
/// - 1: \[3-char\] abbreviated label
/// - 2: \[FullName\] (default)
/// - 3+: \[FullName\] + extras (future)
#[allow(clippy::too_many_arguments)]
pub fn render_graph(
    buf: &mut Buffer,
    area: Rect,
    entities: &[RenderEntity],
    relations: &[RenderRelation],
    positions: &HashMap<Uuid, GridPosition>,
    zoom_level: u8,
    palette: ColorPalette,
    theme: &Theme,
) {
    // 1. Draw edges first (underneath nodes)
    render_edges(buf, area, relations, positions, zoom_level, theme);

    // 2. Draw nodes on top
    for entity in entities {
        if let Some(pos) = positions.get(&entity.id) {
            render_node(buf, area, entity, pos, zoom_level, palette, theme);
        }
    }
}

/// Compute the intersection point of two line segments (if any).
///
/// Returns `Some((x, y))` at the crossing point.  Excludes near-endpoint
/// intersections (`t`/`u` within 2% of ends) to avoid false positives at
/// shared nodes where two edges meet.
#[allow(clippy::too_many_arguments)]
fn segment_intersection(
    ax0: f64,
    ay0: f64,
    ax1: f64,
    ay1: f64,
    bx0: f64,
    by0: f64,
    bx1: f64,
    by1: f64,
) -> Option<(f64, f64)> {
    let d1x = ax1 - ax0;
    let d1y = ay1 - ay0;
    let d2x = bx1 - bx0;
    let d2y = by1 - by0;

    let denom = d1x * d2y - d1y * d2x;
    if denom.abs() < 1e-10 {
        return None; // parallel or collinear
    }

    let t = ((bx0 - ax0) * d2y - (by0 - ay0) * d2x) / denom;
    let u = ((bx0 - ax0) * d1y - (by0 - ay0) * d1x) / denom;

    // Exclude near-endpoints to avoid false crossings at shared nodes.
    if t > 0.02 && t < 0.98 && u > 0.02 && u < 0.98 {
        Some((ax0 + t * d1x, ay0 + t * d1y))
    } else {
        None
    }
}

/// Hop radius in Braille pixels.  A radius of 3 creates a gap about
/// 1.5 terminal cells wide — clearly visible without being obtrusive.
const HOP_RADIUS: i32 = 3;

/// Render all edges using Braille sub-cell rendering with two-pass support.
///
/// Layout edges (structural or all, depending on filter) are drawn as straight
/// lines in DarkGray. Overlay edges (those excluded from layout) are drawn as
/// Bezier curves in muted purple, visually distinguishing the two categories.
///
/// Each terminal cell maps to a 2x4 grid of Braille dots, giving lines that
fn render_edges(
    buf: &mut Buffer,
    area: Rect,
    relations: &[RenderRelation],
    positions: &HashMap<Uuid, GridPosition>,
    zoom_level: u8,
    theme: &Theme,
) {
    let w = area.width as usize;
    let h = area.height as usize;
    if w == 0 || h == 0 {
        return;
    }

    let mut layout_canvas = BrailleCanvas::new(w, h);
    let mut overlay_canvas = BrailleCanvas::new(w, h);
    let mut glow_canvas = BrailleCanvas::new(w, h);

    // Flat bool grid of terminal cells occupied by on-screen labels.
    // O(1) lookup vs HashSet hashing; indexed by (cy * w + cx).
    let mut label_grid = vec![false; w * h];
    for pos in positions.values() {
        if pos.x < 0 || pos.y < 0 || pos.y >= h as i16 {
            continue;
        }
        let vis_w = super::visual_label_width(pos.label_width, zoom_level);
        for ly in 0..pos.label_height {
            for lx in 0..vis_w {
                let cx = pos.x as u16 + lx;
                let cy = pos.y as u16 + ly;
                if cx < w as u16 && cy < h as u16 {
                    label_grid[cy as usize * w + cx as usize] = true;
                }
            }
        }
    }

    // ── Phase 1: collect visible edge segments ──
    //
    // Deduplicates, culls off-viewport edges, and resolves pixel endpoints
    // before any drawing happens.  This lets us detect crossings between
    // straight layout edges and insert hop gaps.

    struct EdgeSeg {
        px1: i32,
        py1: i32,
        px2: i32,
        py2: i32,
        is_overlay: bool,
        draw_curved: bool,
        highlighted: bool,
    }

    let mut seen_pairs: HashSet<(Uuid, Uuid)> = HashSet::with_capacity(relations.len());
    let px_w = (w * 2) as i32;
    let px_h = (h * 4) as i32;
    let mut segments: Vec<EdgeSeg> = Vec::with_capacity(relations.len());

    for rel in relations {
        let pair = if rel.source_id < rel.target_id {
            (rel.source_id, rel.target_id)
        } else {
            (rel.target_id, rel.source_id)
        };
        if !seen_pairs.insert(pair) {
            continue;
        }

        let (Some(src), Some(tgt)) = (positions.get(&rel.source_id), positions.get(&rel.target_id))
        else {
            continue;
        };

        let src_vw = super::visual_label_width(src.label_width, zoom_level);
        let tgt_vw = super::visual_label_width(tgt.label_width, zoom_level);
        let px1 = (src.x as i32 + src_vw as i32 / 2) * 2;
        let py1 = src.y as i32 * 4 + 2;
        let px2 = (tgt.x as i32 + tgt_vw as i32 / 2) * 2;
        let py2 = tgt.y as i32 * 4 + 2;

        // Viewport culling.
        let (min_x, max_x) = (px1.min(px2), px1.max(px2));
        let (min_y, max_y) = (py1.min(py2), py1.max(py2));
        if rel.draw_curved {
            let dx = (px2 - px1) as f64;
            let dy = (py2 - py1) as f64;
            let pad = ((dx * dx + dy * dy).sqrt() * 0.21) as i32 + 1;
            if max_x + pad < 0 || min_x - pad >= px_w || max_y + pad < 0 || min_y - pad >= px_h {
                continue;
            }
        } else if max_x < 0 || min_x >= px_w || max_y < 0 || min_y >= px_h {
            continue;
        }

        segments.push(EdgeSeg {
            px1,
            py1,
            px2,
            py2,
            is_overlay: rel.is_overlay,
            draw_curved: rel.draw_curved,
            highlighted: rel.highlighted,
        });
    }

    // ── Phase 2: detect crossings between straight layout edges ──
    //
    // For each pair of straight, non-overlay edges that cross, the more
    // horizontal one (smaller |slope|) gets a hop gap — the more vertical
    // one passes through uninterrupted, matching PCB/schematic convention.

    let mut hops: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); segments.len()];

    for i in 0..segments.len() {
        if segments[i].draw_curved || segments[i].is_overlay {
            continue;
        }
        for j in (i + 1)..segments.len() {
            if segments[j].draw_curved || segments[j].is_overlay {
                continue;
            }
            if let Some((cx, cy)) = segment_intersection(
                segments[i].px1 as f64,
                segments[i].py1 as f64,
                segments[i].px2 as f64,
                segments[i].py2 as f64,
                segments[j].px1 as f64,
                segments[j].py1 as f64,
                segments[j].px2 as f64,
                segments[j].py2 as f64,
            ) {
                let icx = cx.round() as i32;
                let icy = cy.round() as i32;
                // More horizontal → gets the hop gap; more vertical → passes over.
                let slope_i = ((segments[i].py2 - segments[i].py1) as f64).abs()
                    / ((segments[i].px2 - segments[i].px1) as f64)
                        .abs()
                        .max(0.001);
                let slope_j = ((segments[j].py2 - segments[j].py1) as f64).abs()
                    / ((segments[j].px2 - segments[j].px1) as f64)
                        .abs()
                        .max(0.001);
                if slope_i < slope_j {
                    hops[i].push((icx, icy, HOP_RADIUS));
                } else {
                    hops[j].push((icx, icy, HOP_RADIUS));
                }
            }
        }
    }

    // ── Phase 3: draw edges (with hop gaps where needed) ──

    for (si, seg) in segments.iter().enumerate() {
        let canvas = if seg.is_overlay {
            &mut overlay_canvas
        } else {
            &mut layout_canvas
        };

        if seg.draw_curved {
            canvas.draw_bezier(seg.px1, seg.py1, seg.px2, seg.py2);
        } else if !hops[si].is_empty() {
            canvas.draw_line_with_hops(seg.px1, seg.py1, seg.px2, seg.py2, &hops[si]);
        } else {
            canvas.draw_line(seg.px1, seg.py1, seg.px2, seg.py2);
        }

        if seg.highlighted {
            if seg.draw_curved {
                glow_canvas.draw_bezier(seg.px1, seg.py1, seg.px2, seg.py2);
            } else if !hops[si].is_empty() {
                glow_canvas.draw_line_with_hops(seg.px1, seg.py1, seg.px2, seg.py2, &hops[si]);
            } else {
                glow_canvas.draw_line(seg.px1, seg.py1, seg.px2, seg.py2);
            }
        }
    }

    // Render layout edges (dim)
    render_canvas_to_buf(
        buf,
        area,
        &layout_canvas,
        &label_grid,
        w,
        Style::default().fg(theme.colors.edge_layout),
    );

    // Render overlay edges (magenta)
    render_canvas_to_buf(
        buf,
        area,
        &overlay_canvas,
        &label_grid,
        w,
        Style::default().fg(theme.colors.edge_overlay),
    );

    // Render highlighted edges (bright glow on top of everything).
    render_canvas_to_buf_overwrite(
        buf,
        area,
        &glow_canvas,
        &label_grid,
        w,
        Style::default()
            .fg(theme.colors.edge_highlight)
            .add_modifier(Modifier::BOLD),
    );
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
