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

/// Run Fruchterman-Reingold layout and snap to a terminal grid.
///
/// - `viewport_width` / `viewport_height`: available terminal cells.
/// - `iterations`: number of simulation steps (default: 50).
/// - `zoom`: zoom multiplier for spacing.
/// - `pan`: (dx, dy) offset in grid cells.
/// - `semantic_zoom`: discrete zoom level (0–4) controlling visual label width
///   for collision detection during grid snapping.
/// - `seed_positions`: optional map of entity UUIDs to their previous grid
///   positions. Nodes with seeds start at those positions (warm start);
///   new nodes without seeds start on the circular perimeter. This keeps
///   existing nodes stable when the topology grows incrementally.
#[allow(clippy::too_many_arguments)]
pub fn layout(
    entities: &[LayoutEntity],
    relations: &[LayoutRelation],
    viewport_width: u16,
    viewport_height: u16,
    iterations: u32,
    zoom: f32,
    pan: (i32, i32),
    semantic_zoom: u8,
    seed_positions: Option<&HashMap<Uuid, GridPosition>>,
) -> LayoutResult {
    if entities.is_empty() {
        return LayoutResult {
            positions: HashMap::new(),
        };
    }

    // Sort by (name, id) so the circular initialization is deterministic regardless
    // of the order entities arrive from the engine. Sorting by name (rather than
    // UUID) is critical for reproducibility: entity UUIDs are generated with
    // new_v4() and differ between process runs, but names are stable fixture data.
    let mut sorted_entities: Vec<&LayoutEntity> = entities.iter().collect();
    sorted_entities.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));

    // Build index map: Uuid → node index (based on sorted order)
    let id_to_idx: HashMap<Uuid, usize> = sorted_entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id, i))
        .collect();

    // Initialize nodes. Entities with seed positions start at their previous
    // location (warm start); new entities start on a circular perimeter.
    // This prevents the graph from jumping when entities arrive incrementally
    // during loading — existing nodes stay put while new ones settle in.
    let n = sorted_entities.len();
    let has_seeds = seed_positions
        .map(|s| sorted_entities.iter().any(|e| s.contains_key(&e.id)))
        .unwrap_or(false);
    let mut nodes: Vec<LayoutNode> = sorted_entities
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let label_len = (e.name.len().min(20) + 2) as u16; // [Name] brackets

            // Try to warm-start from previous position.
            if let Some(seeds) = seed_positions {
                if let Some(prev) = seeds.get(&e.id) {
                    return LayoutNode {
                        id: e.id,
                        x: prev.x as f64,
                        y: prev.y as f64,
                        degree: e.degree,
                        label_len,
                        label_height: e.label_height.max(1),
                    };
                }
            }

            let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            LayoutNode {
                id: e.id,
                // Spread initial circle wide enough that labels don't start overlapping.
                // Radius scales with sqrt(n) * average label width for breathing room.
                x: (n as f64).sqrt() * (label_len as f64) * angle.cos(),
                y: (n as f64).sqrt() * (label_len as f64) * 0.5 * angle.sin(),
                degree: e.degree,
                label_len,
                label_height: e.label_height.max(1),
            }
        })
        .collect();

    // Build edge list
    let edges: Vec<LayoutEdge> = relations
        .iter()
        .filter_map(|r| {
            let s = id_to_idx.get(&r.source_id)?;
            let t = id_to_idx.get(&r.target_id)?;
            Some(LayoutEdge {
                source_idx: *s,
                target_idx: *t,
            })
        })
        .collect();

    // Fruchterman-Reingold parameters.
    // k (optimal distance) is scaled up by average label width so the simulation
    // keeps nodes far enough apart that labels won't pile up after grid snap.
    let avg_label = nodes.iter().map(|n| n.label_len as f64).sum::<f64>() / n.max(1) as f64;
    let area = (viewport_width as f64) * (viewport_height as f64);
    let k = (area / n as f64).sqrt() * (avg_label / 4.0).max(1.0);
    let mut temperature = if has_seeds {
        // Lower temperature for warm start: existing nodes barely move,
        // new nodes settle near their circular starting positions.
        (viewport_width.max(viewport_height) as f64) / 8.0
    } else {
        (viewport_width.max(viewport_height) as f64) / 2.0
    };

    // Centroid for gravity — prevents outliers from dominating the bounding box.
    let gravity_strength = 0.1;

    // Simulation loop with early termination on convergence.
    // When warm-starting from seed positions, most nodes are already near
    // equilibrium — use fewer iterations and lower initial temperature so
    // existing nodes barely move while new ones settle quickly.
    let effective_iters = if has_seeds {
        iterations.max(30)
    } else {
        iterations.max(100)
    };
    for _ in 0..effective_iters {
        // Calculate repulsive forces (all pairs)
        let mut displacements = vec![(0.0f64, 0.0f64); n];

        // Compute centroid
        let cx = nodes.iter().map(|n| n.x).sum::<f64>() / n as f64;
        let cy = nodes.iter().map(|n| n.y).sum::<f64>() / n as f64;

        // Build Barnes-Hut quadtree for O(n log n) repulsive force calculation.
        let mut tree = {
            let mut x0 = f64::INFINITY;
            let mut y0 = f64::INFINITY;
            let mut x1 = f64::NEG_INFINITY;
            let mut y1 = f64::NEG_INFINITY;
            for nd in nodes.iter() {
                x0 = x0.min(nd.x);
                y0 = y0.min(nd.y);
                x1 = x1.max(nd.x);
                y1 = y1.max(nd.y);
            }
            // Expand slightly so no point sits exactly on the boundary.
            super::quadtree::QuadTree::new(x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0)
        };
        for nd in nodes.iter() {
            tree.insert(nd.x, nd.y);
        }

        let theta = 0.8;
        for i in 0..n {
            let (fx, fy) = tree.compute_force(nodes[i].x, nodes[i].y, theta, k);
            displacements[i].0 += fx;
            displacements[i].1 += fy;

            // Gravity: gently pull toward centroid to keep layout compact.
            displacements[i].0 -= gravity_strength * (nodes[i].x - cx);
            displacements[i].1 -= gravity_strength * (nodes[i].y - cy);
        }

        // Calculate attractive forces (edges only)
        for edge in &edges {
            let si = edge.source_idx;
            let ti = edge.target_idx;
            let dx = nodes[si].x - nodes[ti].x;
            let dy = nodes[si].y - nodes[ti].y;
            let dist = (dx * dx + dy * dy).sqrt().max(0.01);
            let force = (dist * dist) / k;
            let fx = (dx / dist) * force;
            let fy = (dy / dist) * force;
            displacements[si].0 -= fx;
            displacements[si].1 -= fy;
            displacements[ti].0 += fx;
            displacements[ti].1 += fy;
        }

        // Apply displacements, clamped by temperature
        let mut max_disp = 0.0f64;
        for i in 0..n {
            let (dx, dy) = displacements[i];
            let mag = (dx * dx + dy * dy).sqrt().max(0.01);
            let clamped = mag.min(temperature);
            nodes[i].x += (dx / mag) * clamped;
            nodes[i].y += (dy / mag) * clamped;
            max_disp = max_disp.max(clamped);
        }

        // Early termination: layout has converged.
        if max_disp < 0.5 {
            break;
        }

        // Cool temperature
        temperature *= 0.95;
    }

    // Snap to grid: ContinuousToGrid algorithm (PRD-021 §3.1.1)
    let positions = snap_to_grid(
        &nodes,
        viewport_width,
        viewport_height,
        zoom,
        pan,
        semantic_zoom,
    );

    LayoutResult { positions }
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

#[cfg(test)]
#[path = "layout_fr_tests.rs"]
mod tests;
