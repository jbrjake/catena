//! Fruchterman-Reingold force-directed layout on an integer grid.
//! See PRD-021-TUI.md §3.1.1 for the grid snapping algorithm.
//!
//! The layout runs in continuous f64 space, then snaps to discrete terminal cells.
//! The simulation scales optimal distance (k) by average label width so that
//! connected nodes settle far enough apart for their labels to be readable.
//! A weak gravity term keeps the layout centered and prevents outliers from
//! dominating the bounding box during grid normalization.

use std::collections::HashMap;
use uuid::Uuid;

/// A positioned node in continuous layout space.
#[derive(Debug, Clone)]
struct LayoutNode {
    id: Uuid,
    x: f64,
    y: f64,
    /// Total degree (in + out edges), used for collision priority.
    degree: u32,
    /// Display label length — determines horizontal cell reservation.
    label_len: u16,
    /// Display height in rows (1 for regular entities, >1 for expanded overlay nodes).
    label_height: u16,
}

/// An edge between two node indices.
#[derive(Debug, Clone)]
struct LayoutEdge {
    source_idx: usize,
    target_idx: usize,
}

/// Input entity for the layout engine.
pub struct LayoutEntity {
    pub id: Uuid,
    pub name: String,
    pub degree: u32,
    /// Height in rows (default 1, >1 for expanded overlay nodes).
    pub label_height: u16,
}

/// Input edge for the layout engine.
pub struct LayoutRelation {
    pub source_id: Uuid,
    pub target_id: Uuid,
}

/// Grid-snapped position for a node.
///
/// Coordinates are signed so nodes can be positioned outside the viewport
/// (e.g. negative values for off-screen left/top). The renderer skips nodes
/// that don't fit, but still draws edge lines toward them so users see that
/// connections extend beyond the visible area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridPosition {
    pub x: i16,
    pub y: i16,
    /// Width reserved for the label (including brackets).
    pub label_width: u16,
    /// Height reserved in rows (1 for regular entities, >1 for expanded overlays).
    pub label_height: u16,
}

/// Result of running the layout engine.
pub struct LayoutResult {
    pub positions: HashMap<Uuid, GridPosition>,
}

/// Snap continuous positions to discrete terminal cells.
///
/// The algorithm accounts for label widths when placing nodes, so long labels
/// like `[Operation Sunrise]` don't pile up. It also enforces a minimum 2-row
/// vertical gap so edges between adjacent nodes remain visible.
fn snap_to_grid(
    nodes: &[LayoutNode],
    viewport_width: u16,
    viewport_height: u16,
    zoom: f32,
    pan: (i32, i32),
    semantic_zoom: u8,
) -> HashMap<Uuid, GridPosition> {
    if nodes.is_empty() {
        return HashMap::new();
    }

    // 1. Compute bounding box of the continuous layout
    let min_x = nodes.iter().map(|n| n.x).fold(f64::INFINITY, f64::min);
    let max_x = nodes.iter().map(|n| n.x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = nodes.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
    let max_y = nodes.iter().map(|n| n.y).fold(f64::NEG_INFINITY, f64::max);

    let range_x = (max_x - min_x).max(1.0);
    let range_y = (max_y - min_y).max(1.0);

    // 2. Usable viewport, reserving space so the rightmost label can fit.
    //    Use visual label width — at low zoom, labels are truncated so we
    //    need less margin and smaller collision bounding boxes.
    let max_vis_label = nodes
        .iter()
        .map(|n| super::visual_label_width(n.label_len, semantic_zoom))
        .max()
        .unwrap_or(4);
    let margin_x = 1u16;
    let margin_y = 1u16;
    let usable_w = viewport_width.saturating_sub(margin_x * 2 + max_vis_label) as f64;
    let usable_w = usable_w.max(1.0) * zoom as f64;
    let usable_h = viewport_height.saturating_sub(margin_y * 2) as f64;
    let usable_h = usable_h.max(1.0) * zoom as f64;

    // 3. Map to grid coordinates with pan offset.
    //    Positions are NOT clamped to the viewport — nodes that land outside
    //    simply won't be rendered. Edge lines to off-screen nodes will extend
    //    to the viewport boundary, hinting at content beyond the visible area.
    let mut result = HashMap::new();
    let mut occupied: HashMap<(i16, i16), Uuid> = HashMap::new();
    let vw = viewport_width as i16;
    let vh = viewport_height as i16;

    // Sort by degree descending so high-degree (hub) nodes get priority placement
    let mut sorted: Vec<usize> = (0..nodes.len()).collect();
    sorted.sort_by(|&a, &b| nodes[b].degree.cmp(&nodes[a].degree));

    for &idx in &sorted {
        let node = &nodes[idx];

        // Normalized position [0, 1] → grid cells
        let nx = (node.x - min_x) / range_x;
        let ny = (node.y - min_y) / range_y;

        let cx = nx * usable_w + margin_x as f64 + pan.0 as f64;
        let cy = ny * usable_h + margin_y as f64 + pan.1 as f64;

        let mut gx = cx.round() as i16;
        let mut gy = cy.round() as i16;

        // 4. Collision resolution: only for nodes within the viewport.
        //    Off-screen nodes keep their natural position (no one sees them
        //    anyway, and their edges will clip at the viewport boundary).
        //    Use visual label width so collision boxes match what's rendered.
        let lw = super::visual_label_width(node.label_len, semantic_zoom) as i16;
        let lh = node.label_height.max(1) as i16;
        let on_screen = gx >= 0 && gx + lw <= vw && gy >= 0 && gy + lh <= vh;

        if on_screen {
            let mut attempts: i16 = 0;
            while rect_overlaps(&occupied, gx, gy, lw, lh) && attempts < 50 {
                attempts += 1;
                let ring = (attempts + 1) / 2;
                // Spiral: scale vertical displacement by node height
                match attempts % 4 {
                    0 => gx += lw * ring,
                    1 => gy += (2 * ring).max(lh * ring),
                    2 => gx -= lw * ring,
                    _ => gy -= (2 * ring).max(lh * ring),
                }
            }
        }

        // Reserve cells for the visual bounding box.
        for ly in 0..lh {
            for lx in 0..lw {
                occupied.insert((gx + lx, gy + ly), node.id);
            }
        }

        result.insert(
            node.id,
            GridPosition {
                x: gx,
                y: gy,
                label_width: node.label_len,
                label_height: node.label_height.max(1),
            },
        );
    }

    result
}

/// Compute the bounding box of a set of grid positions.
///
/// Returns `(min_x, min_y, max_x, max_y)` accounting for label widths and heights.
/// Returns `(0, 0, 0, 0)` for an empty input.
pub fn bounding_box(positions: &HashMap<Uuid, GridPosition>) -> (i16, i16, i16, i16) {
    if positions.is_empty() {
        return (0, 0, 0, 0);
    }
    let mut min_x = i16::MAX;
    let mut min_y = i16::MAX;
    let mut max_x = i16::MIN;
    let mut max_y = i16::MIN;
    for pos in positions.values() {
        min_x = min_x.min(pos.x);
        min_y = min_y.min(pos.y);
        max_x = max_x.max(pos.x + pos.label_width as i16);
        max_y = max_y.max(pos.y + pos.label_height as i16);
    }
    (min_x, min_y, max_x, max_y)
}

/// Check if placing a node at (gx, gy) with the given width and height
/// would overlap any already-placed node's bounding box.
fn rect_overlaps(
    occupied: &HashMap<(i16, i16), Uuid>,
    gx: i16,
    gy: i16,
    width: i16,
    height: i16,
) -> bool {
    for ly in 0..height {
        for lx in 0..width {
            if occupied.contains_key(&(gx + lx, gy + ly)) {
                return true;
            }
        }
    }
    false
}

