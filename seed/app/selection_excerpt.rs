//! Entity and edge selection, hover, focus, and pin logic.

use uuid::Uuid;

use super::*;

impl App {
    pub(super) fn select_entity_at(&mut self, mx: u16, my: u16) {
        let area = match self.graph_area {
            Some(a) => a,
            None => return,
        };

        // Ignore clicks outside the graph pane.
        if mx < area.x || mx >= area.x + area.width || my < area.y || my >= area.y + area.height {
            return;
        }

        // Convert screen coordinates to graph-pane-relative coordinates.
        let rel_x = (mx - area.x) as i16;
        let rel_y = (my - area.y) as i16;

        // Check overlay nodes first (they render on top of entities).
        let overlay_ids: Vec<Uuid> = self.graph_overlay_nodes.iter().map(|n| n.id).collect();
        let mut best_overlay: Option<(Uuid, f64)> = None;
        for ov_id in &overlay_ids {
            if let Some(pos) = self.graph_positions.get(ov_id) {
                let label_center_x = pos.x + pos.label_width as i16 / 2;
                let center_y = pos.y + pos.label_height as i16 / 2;
                let dx = (rel_x - label_center_x) as f64;
                let dy = (rel_y - center_y) as f64;
                let dist = (dx * dx + dy * dy).sqrt();
                match best_overlay {
                    Some((_, best_dist)) if dist < best_dist => {
                        best_overlay = Some((*ov_id, dist));
                    }
                    None => best_overlay = Some((*ov_id, dist)),
                    _ => {}
                }
            }
        }
        if let Some((ov_id, dist)) = best_overlay {
            if dist < 5.0 {
                self.focused_overlay = Some(ov_id);
                let label = self
                    .graph_overlay_nodes
                    .iter()
                    .find(|n| n.id == ov_id)
                    .map(|n| n.label.as_str())
                    .unwrap_or("?");
                self.status_message = Some(format!("Overlay: {label}"));
                return;
            }
        }

        // Find the closest entity label to the click point.
        let mut best: Option<(Uuid, f64)> = None;
        for (id, pos) in &self.graph_positions {
            // Skip overlay node positions
            if overlay_ids.contains(id) {
                continue;
            }
            let label_center_x = pos.x + pos.label_width as i16 / 2;
            let dx = (rel_x - label_center_x) as f64;
            let dy = (rel_y - pos.y) as f64;
            let dist = (dx * dx + dy * dy).sqrt();

            match best {
                Some((_, best_dist)) if dist < best_dist => {
                    best = Some((*id, dist));
                }
                None => {
                    best = Some((*id, dist));
                }
                _ => {}
            }
        }

        // Select if within reasonable click distance (5 cells).
        // Click pins/unpins: clicking an already-pinned entity unpins it.
        if let Some((id, dist)) = best {
            if dist < 5.0 {
                // Clicking on an entity clears overlay focus.
                self.focused_overlay = None;
                let name = self
                    .entities
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| e.name.as_str())
                    .unwrap_or("?");
                if self.pinned_entity == Some(id) {
                    self.pinned_entity = None;
                    self.selected_entity = Some(id);
                    self.status_message = Some(format!("Unpinned: {name}"));
                } else {
                    self.pinned_entity = Some(id);
                    self.pinned_edge = None; // entity pin clears edge pin
                    self.selected_entity = Some(id);
                    self.status_message = Some(format!("Pinned: {name}"));
                }
                self.pan_to_entity(id);
                return;
            }
        }

        // No entity nearby — check for edge midpoints.
        self.select_edge_at(mx, my);
    }

    /// Select the edge nearest to the given screen coordinates.
    /// Called as a fallback when no entity is within click distance.
    pub(super) fn select_edge_at(&mut self, mx: u16, my: u16) {
        let mut best: Option<(Uuid, f64)> = None;
        let px = mx as f64;
        let py = my as f64;
        for (id, pos) in &self.edge_positions {
            let (x1, y1, x2, y2) = (pos.x1, pos.y1, pos.x2, pos.y2);
            let dist = point_to_segment_dist(px, py, x1 as f64, y1 as f64, x2 as f64, y2 as f64);
            match best {
                Some((_, best_dist)) if dist < best_dist => best = Some((*id, dist)),
                None => best = Some((*id, dist)),
                _ => {}
            }
        }

        if let Some((id, dist)) = best {
            if dist < 2.0 {
                let rel_type = self
                    .relations
                    .iter()
                    .find(|r| r.id == id)
                    .map(|r| r.relation_type.as_str())
                    .unwrap_or("?");
                if self.pinned_edge == Some(id) {
                    self.pinned_edge = None;
                    self.status_message = Some(format!("Unpinned edge: {rel_type}"));
                } else {
                    self.pinned_edge = Some(id);
                    self.pinned_entity = None; // edge pin clears entity pin
                    self.status_message = Some(format!("Pinned edge: {rel_type}"));
                }
            }
        }
    }

    /// Resolve which entity is under the mouse cursor (hover, not select).
    pub(super) fn hover_entity_at(&mut self, mx: u16, my: u16) {
        let area = match self.graph_area {
            Some(a) => a,
            None => return,
        };

        if mx < area.x || mx >= area.x + area.width || my < area.y || my >= area.y + area.height {
            self.hovered_entity = None;
            return;
        }

        let rel_x = (mx - area.x) as i16;
        let rel_y = (my - area.y) as i16;

        let mut best: Option<(Uuid, f64)> = None;
        for (id, pos) in &self.graph_positions {
            let label_center_x = pos.x + pos.label_width as i16 / 2;
            let dx = (rel_x - label_center_x) as f64;
            let dy = (rel_y - pos.y) as f64;
            let dist = (dx * dx + dy * dy).sqrt();
            match best {
                Some((_, best_dist)) if dist < best_dist => best = Some((*id, dist)),
                None => best = Some((*id, dist)),
                _ => {}
            }
        }

        self.hovered_entity = best.and_then(|(id, dist)| if dist < 5.0 { Some(id) } else { None });
    }

    /// Returns the entity that should be shown in the inspector.
    /// Priority: pinned > hovered (in Normal mode) > selected.
    pub fn effective_focus_entity(&self) -> Option<Uuid> {
        if let Some(id) = self.pinned_entity {
            return Some(id);
        }
        if self.mode == AppMode::Normal {
            if let Some(id) = self.hovered_entity {
                return Some(id);
            }
        }
        self.selected_entity
    }

    /// Resolve which edge is under the mouse cursor (hover, not select).
    /// Uses the spatial grid to check only edges in the mouse's grid cell,
    /// then computes distance from cursor to their line segments.
    pub(super) fn hover_edge_at(&mut self, mx: u16, my: u16) {
        let area = match self.graph_area {
            Some(a) => a,
            None => return,
        };

        if mx < area.x || mx >= area.x + area.width || my < area.y || my >= area.y + area.height {
            self.hovered_edge = None;
            return;
        }

        let candidates = self.edge_spatial_grid.query(mx, my);
        let mut best: Option<(Uuid, f64)> = None;
        let px = mx as f64;
        let py = my as f64;
        for id in candidates {
            if let Some(pos) = self.edge_positions.get(id) {
                let dist = point_to_segment_dist(
                    px,
                    py,
                    pos.x1 as f64,
                    pos.y1 as f64,
                    pos.x2 as f64,
                    pos.y2 as f64,
                );
                match best {
                    Some((_, best_dist)) if dist < best_dist => best = Some((*id, dist)),
                    None => best = Some((*id, dist)),
                    _ => {}
                }
            }
        }

        self.hovered_edge = best.and_then(|(id, dist)| if dist < 2.0 { Some(id) } else { None });
    }

    /// Returns the edge that should be shown in the inspector.
    /// Priority: pinned_edge > hovered_edge (in Normal mode).
    pub fn effective_focus_edge(&self) -> Option<Uuid> {
        if let Some(id) = self.pinned_edge {
            return Some(id);
        }
        if self.mode == AppMode::Normal {
            if let Some(id) = self.hovered_edge {
                return Some(id);
            }
        }
        None
    }
}
