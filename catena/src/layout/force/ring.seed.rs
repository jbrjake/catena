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

/// Place entities on a radial ring directly in grid coordinates.
///
/// Unlike `layout()` which works in continuous space and normalizes via
/// `snap_to_grid`, this function places nodes directly in the grid coordinate
/// system that force-layout positions already live in. This preserves the
/// spatial relationship between the radial ring and the force-directed core.
///
/// `center` and `radius` are in grid cells (the same coordinate space as
/// `GridPosition.x` / `GridPosition.y`).
///
/// `ideal_angles` maps each entity's UUID to its preferred angle (radians)
/// on the ring — typically the direction from center toward the entity's
/// connections in the force core. Entities are sorted by this angle so that
/// adjacent ring nodes connect to nearby force nodes, minimizing edge crossings.
/// Entities not in the map get a fallback angle that fills gaps.
pub fn radial_layout(
    entities: &[LayoutEntity],
    center: (f64, f64),
    radius: f64,
    ideal_angles: &HashMap<Uuid, f64>,
    semantic_zoom: u8,
) -> LayoutResult {
    if entities.is_empty() {
        return LayoutResult {
            positions: HashMap::new(),
        };
    }

    // Sort by ideal angle, with (name, id) as tiebreaker for full determinism.
    // Entities without an ideal angle get f64::MAX so they fill the end.
    // Using name (not UUID) as the primary tiebreaker is critical: entity UUIDs
    // are generated with new_v4() and differ between process runs, but names are
    // stable fixture data. When all entities lack ideal angles (e.g. pure radial
    // mode with no structural edges), this ensures the ring order is identical
    // across runs and thus snapshot-reproducible.
    let mut sorted: Vec<&LayoutEntity> = entities.iter().collect();
    sorted.sort_by(|a, b| {
        let angle_a = ideal_angles.get(&a.id).copied().unwrap_or(f64::MAX);
        let angle_b = ideal_angles.get(&b.id).copied().unwrap_or(f64::MAX);
        angle_a
            .partial_cmp(&angle_b)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.name.cmp(&b.name))
            .then(a.id.cmp(&b.id))
    });

    let n = sorted.len();
    let mut result = HashMap::new();
    let mut occupied: HashMap<(i16, i16), Uuid> = HashMap::new();

    // Anchor the ring so the first entity starts near its ideal direction.
    let start_angle = ideal_angles.get(&sorted[0].id).copied().unwrap_or(0.0);
    let step = 2.0 * std::f64::consts::PI / (n as f64);

    for (i, e) in sorted.iter().enumerate() {
        // Evenly space in the angular-sorted order, anchored to the first
        // entity's ideal direction. This preserves circular ordering from
        // ideal_angles (minimizing crossings) while guaranteeing uniform
        // spacing (preventing label pile-ups).
        let angle = start_angle + step * (i as f64);
        let full_lw = (e.name.len().min(20) + 2) as u16; // [Name] brackets
        let lw = super::visual_label_width(full_lw, semantic_zoom) as i16;
        let lh = e.label_height.max(1) as i16;

        // Terminal characters are roughly 2:1 height-to-width, so scale Y by
        // 0.5 to make the ring appear as a circle rather than a tall oval.
        let mut gx = (center.0 + radius * angle.cos()).round() as i16;
        let mut gy = (center.1 + radius * 0.5 * angle.sin()).round() as i16;

        // Collision resolution: spiral outward from the ideal position.
        let mut attempts: i16 = 0;
        while rect_overlaps(&occupied, gx, gy, lw, lh) && attempts < 50 {
            attempts += 1;
            let ring = (attempts + 1) / 2;
            match attempts % 4 {
                0 => gx += lw * ring,
                1 => gy += (2 * ring).max(lh * ring),
                2 => gx -= lw * ring,
                _ => gy -= (2 * ring).max(lh * ring),
            }
        }

        // Reserve cells for the visual bounding box.
        for ly in 0..lh {
            for lx in 0..lw {
                occupied.insert((gx + lx, gy + ly), e.id);
            }
        }

        result.insert(
            e.id,
            GridPosition {
                x: gx,
                y: gy,
                label_width: full_lw,
                label_height: lh as u16,
            },
        );
    }

    LayoutResult { positions: result }
}

/// Compute the maximum distance from a center point to any grid position.
///
/// Uses the label center (not top-left corner) for each position.
/// Returns 0.0 for empty input.
pub fn max_distance_from(center: (f64, f64), positions: &HashMap<Uuid, GridPosition>) -> f64 {
    positions
        .values()
        .map(|p| {
            let px = p.x as f64 + p.label_width as f64 / 2.0;
            let py = p.y as f64 + p.label_height as f64 / 2.0;
            ((px - center.0).powi(2) + (py - center.1).powi(2)).sqrt()
        })
        .fold(0.0f64, f64::max)
}

/// Compute the centroid (mean position) of a set of grid positions.
///
/// More robust than bounding-box midpoint when FR pushes small clusters
/// to the periphery — the centroid stays near the visual "center of mass."
/// Returns `(0.0, 0.0)` for empty input.
pub fn centroid(positions: &HashMap<Uuid, GridPosition>) -> (f64, f64) {
    if positions.is_empty() {
        return (0.0, 0.0);
    }
    let n = positions.len() as f64;
    let sum_x: f64 = positions
        .values()
        .map(|p| p.x as f64 + p.label_width as f64 / 2.0)
        .sum();
    let sum_y: f64 = positions
        .values()
        .map(|p| p.y as f64 + p.label_height as f64 / 2.0)
        .sum();
    (sum_x / n, sum_y / n)
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

