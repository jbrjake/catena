//! Pan, zoom, animation, layout invalidation, data refresh,
//! palette cycling, and entity navigation.

use std::time::{Duration, Instant};
use uuid::Uuid;

use super::*;

impl App {
    pub(super) fn nudge_focused_window(&mut self, dx: i16, dy: i16) {
        let wid = match self.window_manager.focused_window {
            Some(id) => id,
            None => return,
        };
        let area = self.graph_area.unwrap_or(Rect::new(0, 1, 80, 23));
        if let Some(win) = self.window_manager.windows.get(&wid) {
            let new_x = (win.rect.x as i16 + dx).max(0) as u16;
            let new_y = (win.rect.y as i16 + dy).max(0) as u16;
            self.window_manager.move_window(wid, new_x, new_y, area);
        }
    }

    /// Resize the focused window by (dw, dh) cells. Keyboard equivalent of edge drag.
    pub(super) fn grow_focused_window(&mut self, dw: i16, dh: i16) {
        let wid = match self.window_manager.focused_window {
            Some(id) => id,
            None => return,
        };
        let area = self.graph_area.unwrap_or(Rect::new(0, 1, 80, 23));
        if let Some(win) = self.window_manager.windows.get(&wid) {
            let new_w = (win.rect.width as i16 + dw).max(1) as u16;
            let new_h = (win.rect.height as i16 + dh).max(1) as u16;
            self.window_manager.resize_window(wid, new_w, new_h, area);
        }
    }

    /// Bump the layout generation counter so the next render recomputes positions.
    pub fn invalidate_layout(&mut self) {
        self.layout_generation = self.layout_generation.wrapping_add(1);
        self.embedding_fr_cache = None;
    }

    /// Update entities/relations from a fresh data fetch, distinguishing
    /// topology changes (entity/relation set changed) from property-only
    /// changes (confidence, community_id, embedding_2d).
    ///
    /// Returns `(topology_changed, any_changed)`:
    /// - `topology_changed` = entity IDs or relation IDs differ → full
    ///   layout recomputation (graph_positions cleared, layout invalidated).
    /// - `any_changed` = topology OR property values differ → entities/relations
    ///   swapped in, but layout positions stay stable for property-only updates.
    ///
    /// This prevents the graph from flickering when the engine returns updated
    /// confidence/community/embedding values during enrichment — the layout
    /// remains stable and only colors/labels update.
    pub fn apply_data_refresh(
        &mut self,
        entities: Vec<TuiEntity>,
        relations: Vec<TuiRelation>,
    ) -> (bool, bool) {
        use std::collections::HashSet;

        // Topology: does the set of entity IDs or relation IDs differ?
        // Comparing relation IDs (not just count) catches replacements where
        // the count stays the same but the actual edges changed.
        let old_ids: HashSet<Uuid> = self.entities.iter().map(|e| e.id).collect();
        let new_ids: HashSet<Uuid> = entities.iter().map(|e| e.id).collect();
        let old_rel_ids: HashSet<Uuid> = self.relations.iter().map(|r| r.id).collect();
        let new_rel_ids: HashSet<Uuid> = relations.iter().map(|r| r.id).collect();
        let topology_changed = old_ids != new_ids || old_rel_ids != new_rel_ids;

        // Property-only: same entity set, but values differ.
        // Order-independent — looks up by ID, not by position.
        let properties_changed = if !topology_changed {
            entities.iter().any(|new_e| {
                self.entities
                    .iter()
                    .find(|old_e| old_e.id == new_e.id)
                    .is_some_and(|old_e| {
                        old_e.confidence != new_e.confidence
                            || old_e.community_id != new_e.community_id
                            || old_e.embedding_2d != new_e.embedding_2d
                            || old_e.structural_2d != new_e.structural_2d
                            || old_e.convergence_score != new_e.convergence_score
                            || old_e.pagerank != new_e.pagerank
                            || old_e.degree_centrality != new_e.degree_centrality
                            || old_e.user_cluster_id != new_e.user_cluster_id
                    })
            })
        } else {
            false
        };

        let any_changed = topology_changed || properties_changed;

        if any_changed {
            self.entities = entities;
            self.relations = relations;
            self.dirty = true;
        }

        if topology_changed {
            // Only remove positions for entities that were deleted — keep
            // positions for surviving entities so they can be used as seed
            // positions in the next layout pass (warm start). This prevents
            // the graph from jumping when new entities arrive during loading.
            let removed_ids: Vec<Uuid> = self
                .canonical_positions
                .keys()
                .filter(|id| !new_ids.contains(id))
                .copied()
                .collect();
            for id in &removed_ids {
                self.canonical_positions.remove(id);
                self.graph_positions.remove(id);
            }
            self.edge_positions.clear();
            self.provenance_cache.clear();
            self.invalidate_layout();

            // Clear selection/pin/hover if they point at a now-deleted entity.
            // Leaving a stale UUID causes the inspector to show "?" and can
            // cause pending_entity_delete to target an entity that no longer
            // exists (APP-002).
            if let Some(id) = self.selected_entity {
                if !new_ids.contains(&id) {
                    self.selected_entity = self.entities.first().map(|e| e.id);
                }
            }
            if let Some(id) = self.pinned_entity {
                if !new_ids.contains(&id) {
                    self.pinned_entity = None;
                }
            }
            if let Some(id) = self.hovered_entity {
                if !new_ids.contains(&id) {
                    self.hovered_entity = None;
                }
            }
            if let Some(id) = self.pending_entity_delete {
                if !new_ids.contains(&id) {
                    self.pending_entity_delete = None;
                }
            }
        }

        // Notify the user the first time text embeddings arrive.
        // We now default to Text space (not Minimap), so no space switch is needed —
        // just a one-time status message so the user knows the pane became meaningful.
        //
        // DEFERRED DURING LOADING: the notification is skipped while a demo is
        // loading (demo_loading is true) to avoid premature messages. The demo
        // completion handler emits its own status message.
        if !self.embeddings_auto_switched
            && !self.demo_loading
            && self.entities.iter().any(|e| e.embedding_2d.is_some())
        {
            self.embeddings_auto_switched = true;
            self.embedding_fr_cache = None;
            self.status_message = Some("Text embeddings ready — Embeddings pane updated".into());
        }

        // Auto-switch from EntityType to Community when communities first arrive.
        // Starts with EntityType (always available); flips to Community once the
        // enricher has run. Only triggers once — user overrides are respected.
        // Guarded on `any_changed` to avoid false triggers on no-op refreshes
        // (e.g., entity reordering with no actual data change).
        //
        // DEFERRED DURING LOADING: the palette switch causes EVERY entity label
        // to change color simultaneously (500+ cell diffs), which produces a
        // visible blink. Deferring to the DemoProgress::Finished handler groups
        // the palette switch with the "Demo loaded" status message into one
        // atomic visual transition.
        if any_changed
            && !self.communities_auto_switched
            && !self.demo_loading
            && self.color_palette == ColorPalette::EntityType
            && self.entities.iter().any(|e| e.community_id.is_some())
        {
            self.color_palette = ColorPalette::Community;
            self.communities_auto_switched = true;
            self.pending_community_names_fetch = true;
            self.status_message =
                Some("Communities detected — switched to community colors".into());
        }

        (topology_changed, any_changed)
    }

    pub(super) fn pan(&mut self, dx: i32, dy: i32) {
        self.graph_offset.0 += dx;
        self.graph_offset.1 += dy;
        // Manual pan cancels any running animation.
        self.pan_animation = None;
        // Pan does NOT invalidate layout — positions are derived from
        // canonical_positions + graph_offset in layout_and_render.
    }

    /// Compute the center of the visible (unoccluded) region of the graph area.
    ///
    /// Floating windows (Inspector, Embeddings, Legend) occlude portions of the
    /// graph. This method returns the centroid of the *free* area — the graph area
    /// minus visible, non-minimized window rects. When all windows are hidden, this
    /// is just the raw center of `graph_area`.
    ///
    /// Returns `(cx, cy)` relative to `graph_area` origin.
    pub fn effective_visible_center(&self) -> (f64, f64) {
        let area = match self.graph_area {
            Some(a) => a,
            None => return (40.0, 12.0),
        };

        let base_cx = area.width as f64 / 2.0;
        let base_cy = area.height as f64 / 2.0;
        let total_area = area.width as f64 * area.height as f64;
        if total_area < 1.0 {
            return (base_cx, base_cy);
        }

        // Accumulate the area-weighted centroid of all occluded regions.
        let mut occluded_wx = 0.0_f64;
        let mut occluded_wy = 0.0_f64;
        let mut total_occluded = 0.0_f64;

        let area_right = area.x + area.width;
        let area_bottom = area.y + area.height;

        for id in crate::ui::window_manager::WindowId::ALL {
            let win = &self.window_manager.windows[&id];
            if !win.visible || win.is_minimized() {
                continue;
            }

            // Intersection of window rect with graph area.
            let ix = win.rect.x.max(area.x);
            let iy = win.rect.y.max(area.y);
            let ix2 = (win.rect.x + win.rect.width).min(area_right);
            let iy2 = (win.rect.y + win.rect.height).min(area_bottom);

            if ix >= ix2 || iy >= iy2 {
                continue; // No overlap
            }

            let ow = (ix2 - ix) as f64;
            let oh = (iy2 - iy) as f64;
            let occluded_area = ow * oh;

            // Center of this occluded rect, relative to graph_area origin.
            let ocx = (ix as f64 + ix2 as f64) / 2.0 - area.x as f64;
            let ocy = (iy as f64 + iy2 as f64) / 2.0 - area.y as f64;

            occluded_wx += occluded_area * ocx;
            occluded_wy += occluded_area * ocy;
            total_occluded += occluded_area;
        }

        if total_occluded < 1.0 {
            return (base_cx, base_cy);
        }

        let free_area = total_area - total_occluded;
        if free_area < 1.0 {
            return (base_cx, base_cy);
        }

        // Free centroid = (total_centroid * total_area - occluded_centroid * occluded_area) / free_area
        let occluded_cx = occluded_wx / total_occluded;
        let occluded_cy = occluded_wy / total_occluded;
        let free_cx = (base_cx * total_area - occluded_cx * total_occluded) / free_area;
        let free_cy = (base_cy * total_area - occluded_cy * total_occluded) / free_area;

        (free_cx, free_cy)
    }

    /// Start a smooth animated pan to center the given entity in the viewport.
    ///
    /// Uses the entity's canonical position (pan-independent) and the current
    /// zoom level to calculate the target offset. The animation runs over 300ms
    /// with an ease-out cubic curve for a snappy, decelerating feel.
    ///
    /// The target center accounts for floating window occlusion — entities are
    /// centered in the visible (unoccluded) region, not the raw graph area.
    pub fn pan_to_entity(&mut self, entity_id: Uuid) {
        let pos = match self.canonical_positions.get(&entity_id) {
            Some(p) => *p,
            None => return, // entity not laid out yet
        };

        let zoom_ratio = if self.layout_reference_zoom > 0.0 {
            (self.zoom_level / self.layout_reference_zoom) as f64
        } else {
            1.0
        };

        // Use the effective visible center instead of the raw area center.
        let (center_x, center_y) = self.effective_visible_center();
        let entity_x = (pos.x as f64 - 1.0) * zoom_ratio + 1.0;
        let entity_y = (pos.y as f64 - 1.0) * zoom_ratio + 1.0;
        let target_x = (center_x - entity_x) as f32;
        let target_y = (center_y - entity_y) as f32;

        // Skip animation if the entity is already near center (< 3 cells).
        let dx = target_x - self.graph_offset.0 as f32;
        let dy = target_y - self.graph_offset.1 as f32;
        if dx.abs() < 3.0 && dy.abs() < 3.0 {
            return;
        }

        self.pan_animation = Some(PanAnimation {
            start_offset: (self.graph_offset.0 as f32, self.graph_offset.1 as f32),
            target_offset: (target_x, target_y),
            start_time: Instant::now(),
            duration: Duration::from_millis(300),
        });
    }

    /// Advance the pan animation by one tick. Returns `true` if the animation
    /// is still in progress (caller should set `dirty = true` to keep rendering).
    pub fn tick_pan_animation(&mut self) -> bool {
        let anim = match &self.pan_animation {
            Some(a) => a,
            None => return false,
        };
        match anim.tick() {
            Some(offset) => {
                self.graph_offset = offset;
                true
            }
            None => {
                // Animation complete — snap to final target.
                let target = self
                    .pan_animation
                    .take()
                    .expect("animation was Some when tick() returned None")
                    .target_offset;
                self.graph_offset = (target.0.round() as i32, target.1.round() as i32);
                false
            }
        }
    }

    /// Zoom in with smooth animation, keeping `anchor` visually fixed.
    /// Pass `None` for keyboard zoom (centers on viewport, larger step).
    pub(super) fn zoom_in(&mut self, anchor: Option<(u16, u16)>) {
        // Keyboard: 0.25 per press (intentional discrete step).
        // Mouse scroll: 0.08 per event (touchpads fire many events per gesture).
        let step = if anchor.is_some() { 0.08 } else { 0.25 };
        let current_target = self
            .zoom_animation
            .as_ref()
            .map_or(self.zoom_level, |a| a.target_zoom);
        let new_target = (current_target + step).min(4.0);
        self.start_zoom_animation(new_target, anchor);
        self.status_message = Some(format!("Zoom: {:.0}%", new_target * 100.0));
    }

    /// Zoom out with smooth animation, keeping `anchor` visually fixed.
    /// Pass `None` for keyboard zoom (centers on viewport, larger step).
    pub(super) fn zoom_out(&mut self, anchor: Option<(u16, u16)>) {
        let step = if anchor.is_some() { 0.08 } else { 0.25 };
        let current_target = self
            .zoom_animation
            .as_ref()
            .map_or(self.zoom_level, |a| a.target_zoom);
        let new_target = (current_target - step).max(0.1);
        self.start_zoom_animation(new_target, anchor);
        self.status_message = Some(format!("Zoom: {:.0}%", new_target * 100.0));
    }

    /// Start or retarget a zoom animation toward `target_zoom`.
    ///
    /// With exponential smoothing, retargeting just updates the target —
    /// no animation restart, no jerk. Rapid scroll events accumulate
    /// naturally into smooth continuous motion.
    pub(super) fn start_zoom_animation(&mut self, target_zoom: f32, anchor: Option<(u16, u16)>) {
        let area = self.graph_area.unwrap_or(Rect::new(0, 1, 80, 23));
        let anchor_rel = anchor.map(|(mx, my)| {
            (
                mx.saturating_sub(area.x) as f32,
                my.saturating_sub(area.y) as f32,
            )
        });

        if let Some(ref mut anim) = self.zoom_animation {
            // Retarget existing animation — keep smoothing, no restart.
            anim.target_zoom = target_zoom;
            if anchor_rel.is_some() {
                anim.anchor = anchor_rel;
            }
        } else {
            self.zoom_animation = Some(ZoomAnimation {
                target_zoom,
                anchor: anchor_rel,
                snap: false,
            });
        }
    }

    /// Advance the zoom animation via exponential smoothing.
    ///
    /// Each frame, zoom_level moves 35% of the remaining distance toward
    /// the target. This gives natural deceleration: fast initial response,
    /// smooth settling. Snaps to target at <0.1% distance to prevent
    /// infinite asymptotic crawl.
    pub fn tick_zoom_animation(&mut self) -> bool {
        let anim = match &self.zoom_animation {
            Some(a) => a,
            None => return false,
        };

        let target = anim.target_zoom;
        let diff = target - self.zoom_level;

        // Snap when close enough (or force_complete was called).
        if diff.abs() < 0.001 || anim.snap {
            let old_zoom = self.zoom_level;
            let anchor = anim.anchor;
            self.zoom_animation = None;
            self.zoom_level = target;
            let area = self.graph_area.unwrap_or(Rect::new(0, 1, 80, 23));
            let screen_anchor = anchor.map(|(ax, ay)| (ax as u16 + area.x, ay as u16 + area.y));
            self.adjust_pan_for_zoom(old_zoom, screen_anchor);
            let new_semantic = crate::graph::zoom_to_semantic(self.zoom_level);
            if new_semantic != self.last_semantic_zoom {
                self.invalidate_layout();
            }
            return true;
        }

        // Exponential smoothing: move 35% of remaining distance each frame.
        let lerp = 0.35;
        let old_zoom = self.zoom_level;
        self.zoom_level += diff * lerp;

        let area = self.graph_area.unwrap_or(Rect::new(0, 1, 80, 23));
        let anchor = self.zoom_animation.as_ref().and_then(|a| {
            a.anchor
                .map(|(ax, ay)| (ax as u16 + area.x, ay as u16 + area.y))
        });
        self.adjust_pan_for_zoom(old_zoom, anchor);
        let new_semantic = crate::graph::zoom_to_semantic(self.zoom_level);
        if new_semantic != self.last_semantic_zoom {
            self.invalidate_layout();
        }
        true
    }

    /// Advance the provenance reveal animation.
    /// Returns `true` while the animation is still active (needs redraws).
    pub fn tick_provenance_reveal(&mut self) -> bool {
        let anim = match &self.provenance_reveal {
            Some(a) => a,
            None => return false,
        };
        if anim.revealed_tiers().is_none() {
            // Animation complete — clear it.
            self.provenance_reveal = None;
            false
        } else {
            true
        }
    }

    /// Adjust `graph_offset` so the content under `anchor` stays at the same
    /// screen position after a zoom change. If no anchor is provided, the
    /// viewport center is used (standard keyboard-zoom behaviour).
    pub(super) fn adjust_pan_for_zoom(&mut self, old_zoom: f32, anchor: Option<(u16, u16)>) {
        let new_zoom = self.zoom_level;
        if (new_zoom - old_zoom).abs() < f32::EPSILON {
            return;
        }
        let area = self.graph_area.unwrap_or(Rect::new(0, 1, 80, 23));

        // Anchor in graph-area-relative coordinates.
        // When no anchor (keyboard zoom), use the geometric center of the
        // viewport. This keeps the visual fixed-point at the raw center during
        // zoom transitions, which is correct: the zoom anchor defines what stays
        // still on screen, not where the user wants to look.
        let (ax, ay) = match anchor {
            Some((mx, my)) => (
                mx.saturating_sub(area.x) as f32,
                my.saturating_sub(area.y) as f32,
            ),
            None => (area.width as f32 / 2.0, area.height as f32 / 2.0),
        };

        // The grid mapping is: gx = nx * base_w * zoom + margin + pan.
        // For the anchor to stay at the same screen position:
        //   new_pan = old_pan - (anchor_rel - margin - old_pan) * (new_zoom/old_zoom - 1)
        let margin = 1.0f32;
        let ratio = new_zoom / old_zoom;
        let pan_x = self.graph_offset.0 as f32;
        let pan_y = self.graph_offset.1 as f32;

        self.graph_offset.0 = (pan_x - (ax - margin - pan_x) * (ratio - 1.0)).round() as i32;
        self.graph_offset.1 = (pan_y - (ay - margin - pan_y) * (ratio - 1.0)).round() as i32;
    }

    pub fn rotate_layout_category(&mut self) {
        // Anchor the focused entity so it stays visually stable across relayout.
        let anchor_id = self.pinned_entity.or(self.selected_entity);
        if let Some(id) = anchor_id {
            if let Some(pos) = self.canonical_positions.get(&id) {
                self.layout_anchor_entity = Some(id);
                let lw = pos.label_width as f64 / 2.0;
                self.layout_anchor_pos = Some((pos.x as f64 + lw, pos.y as f64));
            }
        }
        self.edge_display_mode = self.edge_display_mode.next();
        self.status_message = Some(format!("Edges: {}", self.edge_display_mode.label()));
        self.invalidate_layout();
    }

    pub(super) fn cycle_color_palette(&mut self) {
        self.color_palette = self.color_palette.next();
        // User explicitly chose a palette — don't auto-switch back.
        self.communities_auto_switched = true;
        // Fetch community names when switching to a community palette.
        if matches!(
            self.color_palette,
            ColorPalette::Community | ColorPalette::EmbeddingCommunity
        ) && !self.community_names_loaded
        {
            self.pending_community_names_fetch = true;
        }
        // Fetch user clusters when switching to CustomCluster palette.
        if self.color_palette == ColorPalette::CustomCluster {
            self.pending_user_clusters_fetch = true;
        }
        self.status_message = Some(format!("Colors: {}", self.color_palette.label()));
    }

    /// Open the category detail overlay for a legend category.
    ///
    /// Filters entities matching the category, ranks them by PageRank (fallback:
    /// degree centrality, then confidence), and populates the overlay list with
    /// the top entries.
    pub fn open_category_detail(&mut self, category: LegendCategory, display_title: String) {
        let max_entries = 20;

        let matching: Vec<&TuiEntity> = self
            .entities
            .iter()
            .filter(|e| match &category {
                LegendCategory::EntityType(t) => &e.entity_type == t,
                LegendCategory::Community(cid) => e.community_id == Some(*cid),
                LegendCategory::EmbeddingCommunity(cid) => e.embedding_community_id == Some(*cid),
                LegendCategory::CustomCluster(cid) => e.user_cluster_id == Some(*cid),
            })
            .collect();

        if matching.is_empty() {
            self.set_timed_status("No entities in this category", 3);
            return;
        }

        // Sort by PageRank desc, then degree centrality desc, then confidence desc.
        let mut ranked: Vec<&TuiEntity> = matching;
        ranked.sort_by(|a, b| {
            b.pagerank
                .unwrap_or(0.0)
                .partial_cmp(&a.pagerank.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.degree_centrality
                        .unwrap_or(0)
                        .cmp(&a.degree_centrality.unwrap_or(0))
                })
                .then_with(|| {
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        let entries: Vec<(uuid::Uuid, String)> = ranked
            .iter()
            .take(max_entries)
            .enumerate()
            .map(|(i, e)| {
                let rank_info = if let Some(pr) = e.pagerank {
                    format!("PR:{:.2}", pr)
                } else if let Some(dc) = e.degree_centrality {
                    format!("deg:{dc}")
                } else {
                    format!("conf:{:.0}%", e.confidence * 100.0)
                };
                let line = format!("{:>2}. {} [{}] {rank_info}", i + 1, e.name, e.entity_type,);
                (e.id, line)
            })
            .collect();

        let count = ranked.len();
        let shown = entries.len();
        self.category_detail_title = if count > shown {
            format!("{display_title} (top {shown} of {count})")
        } else {
            format!("{display_title} ({count})")
        };
        // Attach community summary if viewing a community category.
        self.category_detail_summary = match &category {
            LegendCategory::Community(cid) => self.community_summaries.get(cid).cloned(),
            _ => None,
        };
        self.category_detail_entries = entries;
        self.category_detail_cursor = 0;
        self.scroll_offset = 0;
        self.mode = AppMode::CategoryDetail;
        self.status_message = Some("j/k navigate, Enter select+pan, Esc close".into());
    }

    pub(super) fn toggle_embedding_space(&mut self) {
        self.embedding_space = self.embedding_space.next();
        self.status_message = Some(format!("Embeddings: {}", self.embedding_space.label()));
        // Invalidate FR cache since we're switching coordinate sources.
        self.embedding_fr_cache = None;
    }

    pub(super) fn cycle_focus(&mut self) {
        match self.focus {
            FocusPane::Graph => {
                // Try to focus the first visible window
                if let Some(id) = self.window_manager.cycle_focus() {
                    self.focus = FocusPane::Window(id);
                }
                // else stay on Graph (no visible windows)
            }
            FocusPane::Window(_) => {
                if let Some(id) = self.window_manager.cycle_focus() {
                    self.focus = FocusPane::Window(id);
                } else {
                    self.focus = FocusPane::Graph;
                }
            }
        }
    }

    pub(super) fn cycle_focus_reverse(&mut self) {
        match self.focus {
            FocusPane::Graph => {
                // Jump to the last visible window (reverse wraps to end)
                if let Some(id) = self.window_manager.cycle_focus_reverse() {
                    self.focus = FocusPane::Window(id);
                }
            }
            FocusPane::Window(_) => {
                if let Some(id) = self.window_manager.cycle_focus_reverse() {
                    self.focus = FocusPane::Window(id);
                } else {
                    self.focus = FocusPane::Graph;
                }
            }
        }
    }

    pub(super) fn select_at_cursor(&mut self) {
        self.scroll_offset = 0;
        let items: Vec<(Uuid, String)> = self
            .filtered_entities()
            .iter()
            .map(|e| (e.id, e.name.clone()))
            .collect();
        if items.is_empty() {
            self.status_message = Some("No entities to select".into());
            return;
        }
        // If nothing selected yet, pick the first; otherwise keep current
        if self.selected_entity.is_none() {
            self.selected_entity = Some(items[0].0);
            self.pan_to_entity(items[0].0);
            self.status_message = Some(format!("Selected: {} (n/N to cycle)", items[0].1));
        } else if let Some((id, name)) = items
            .iter()
            .find(|(id, _)| Some(*id) == self.selected_entity)
        {
            self.pan_to_entity(*id);
            self.status_message = Some(format!("Selected: {} (n/N to cycle)", name));
        }
    }

    /// Select the next entity in the filtered list (wraps around).
    pub(super) fn select_next_entity(&mut self) {
        self.scroll_offset = 0;
        let items: Vec<(Uuid, String)> = self
            .filtered_entities()
            .iter()
            .map(|e| (e.id, e.name.clone()))
            .collect();
        if items.is_empty() {
            return;
        }
        let current_idx = self
            .selected_entity
            .and_then(|sel| items.iter().position(|(id, _)| *id == sel))
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % items.len();
        self.selected_entity = Some(items[next_idx].0);
        self.pan_to_entity(items[next_idx].0);
        self.status_message = Some(format!(
            "[{}/{}] {}",
            next_idx + 1,
            items.len(),
            items[next_idx].1
        ));
    }

    /// Select the previous entity in the filtered list (wraps around).
    pub(super) fn select_prev_entity(&mut self) {
        self.scroll_offset = 0;
        let items: Vec<(Uuid, String)> = self
            .filtered_entities()
            .iter()
            .map(|e| (e.id, e.name.clone()))
            .collect();
        if items.is_empty() {
            return;
        }
        let current_idx = self
            .selected_entity
            .and_then(|sel| items.iter().position(|(id, _)| *id == sel))
            .unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            items.len() - 1
        } else {
            current_idx - 1
        };
        self.selected_entity = Some(items[prev_idx].0);
        self.pan_to_entity(items[prev_idx].0);
        self.status_message = Some(format!(
            "[{}/{}] {}",
            prev_idx + 1,
            items.len(),
            items[prev_idx].1
        ));
    }
}
