/// Screen-space endpoints of a rendered edge for line-segment hit-testing.
/// Coordinates are absolute screen positions (signed to handle off-screen edges).
#[derive(Debug, Clone, Copy)]
pub struct EdgeScreenPos {
    pub x1: i16,
    pub y1: i16,
    pub x2: i16,
    pub y2: i16,
}

/// Animated pan state for smooth graph transitions.
///
/// When an entity is selected, the graph smoothly pans to center it in the
/// viewport using an ease-out cubic curve — fast start, smooth deceleration.
#[derive(Debug, Clone)]
pub struct PanAnimation {
    pub(super) start_offset: (f32, f32),
    pub(super) target_offset: (f32, f32),
    pub(super) start_time: Instant,
    pub(super) duration: Duration,
}

impl PanAnimation {
    /// Force the animation to complete on the next tick.
    pub fn force_complete(&mut self) {
        self.start_time -= self.duration + Duration::from_secs(1);
    }

    /// Advance the animation and return the interpolated offset.
    /// Returns `None` when the animation is complete.
    pub(super) fn tick(&self) -> Option<(i32, i32)> {
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let total = self.duration.as_secs_f32();
        if elapsed >= total {
            return None;
        }
        let t = elapsed / total;
        // Ease-out cubic: fast start, smooth deceleration.
        let eased = 1.0 - (1.0 - t).powi(3);
        let x = self.start_offset.0 + (self.target_offset.0 - self.start_offset.0) * eased;
        let y = self.start_offset.1 + (self.target_offset.1 - self.start_offset.1) * eased;
        Some((x.round() as i32, y.round() as i32))
    }
}

/// Exponential-smoothing zoom for responsive, natural-feeling scroll zoom.
///
/// Instead of a timed animation, each frame moves a fixed fraction of the
/// remaining distance toward the target. This naturally decelerates without
/// needing explicit duration or easing curves, and handles rapid touchpad
/// scroll events gracefully (targets accumulate, motion stays smooth).
///
/// At ~20fps (50ms poll timeout), the 0.35 lerp factor gives:
/// - 65% of distance covered after 1 frame (50ms)
/// - 88% after 2 frames (100ms)
/// - 96% after 3 frames (150ms)
/// - Snap to target when within 0.1% → clean stop, no oscillation.
pub struct ZoomAnimation {
    pub target_zoom: f32,
    /// Anchor point in graph-area-relative coordinates for zoom-toward-cursor.
    pub(super) anchor: Option<(f32, f32)>,
    /// When true, the next tick snaps directly to target (for tests).
    pub(super) snap: bool,
}

impl ZoomAnimation {
    /// Force the animation to complete on the next tick (for tests).
    pub fn force_complete(&mut self) {
        self.snap = true;
    }
}

/// Grid-based spatial index for O(1) edge hit-testing.
///
/// Divides the viewport into cells (16 cols × 8 rows each) and maps each cell
/// to the edges whose bounding boxes overlap it. On hover, only edges in the
/// mouse's cell need distance checks, replacing the O(n) linear scan.
#[derive(Debug, Clone)]
pub struct EdgeSpatialGrid {
    cell_w: u16,
    cell_h: u16,
    cols: u16,
    origin_x: u16,
    origin_y: u16,
    /// Flattened grid: cells[row * cols + col] → edge IDs overlapping that cell.
    cells: Vec<Vec<Uuid>>,
}

impl Default for EdgeSpatialGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl EdgeSpatialGrid {
    /// Empty grid (no edges indexed).
    pub fn new() -> Self {
        Self {
            cell_w: 16,
            cell_h: 8,
            cols: 0,
            origin_x: 0,
            origin_y: 0,
            cells: Vec::new(),
        }
    }

    /// Build from edge screen positions and viewport bounds.
    pub fn build(edges: &HashMap<Uuid, EdgeScreenPos>, viewport: Rect) -> Self {
        let cell_w: u16 = 16;
        let cell_h: u16 = 8;
        let cols = viewport.width.saturating_add(cell_w - 1) / cell_w.max(1);
        let rows = viewport.height.saturating_add(cell_h - 1) / cell_h.max(1);
        if cols == 0 || rows == 0 {
            return Self::new();
        }
        let grid_len = (cols as usize) * (rows as usize);
        let mut cells = vec![Vec::new(); grid_len];
        let ox = viewport.x as i32;
        let oy = viewport.y as i32;
        let max_col = cols.saturating_sub(1);
        let max_row = rows.saturating_sub(1);

        for (id, pos) in edges {
            // Expand bounding box by 2 pixels (the hit-test threshold) so
            // edges near cell boundaries are registered in adjacent cells.
            let hit_margin = 2i32;
            let min_x = pos.x1.min(pos.x2) as i32 - hit_margin;
            let max_x = pos.x1.max(pos.x2) as i32 + hit_margin;
            let min_y = pos.y1.min(pos.y2) as i32 - hit_margin;
            let max_y = pos.y1.max(pos.y2) as i32 + hit_margin;

            let c0 = ((min_x - ox).max(0) as u16 / cell_w).min(max_col);
            let c1 = ((max_x - ox).max(0) as u16 / cell_w).min(max_col);
            let r0 = ((min_y - oy).max(0) as u16 / cell_h).min(max_row);
            let r1 = ((max_y - oy).max(0) as u16 / cell_h).min(max_row);

            for r in r0..=r1 {
                for c in c0..=c1 {
                    let idx = r as usize * cols as usize + c as usize;
                    if idx < grid_len {
                        cells[idx].push(*id);
                    }
                }
            }
        }

        Self {
            cell_w,
            cell_h,
            cols,
            origin_x: viewport.x,
            origin_y: viewport.y,
            cells,
        }
    }

    /// Return edge IDs that might be near the given screen position.
    pub fn query(&self, x: u16, y: u16) -> &[Uuid] {
        if self.cols == 0 || self.cells.is_empty() {
            return &[];
        }
        let c = x.saturating_sub(self.origin_x) / self.cell_w.max(1);
        let r = y.saturating_sub(self.origin_y) / self.cell_h.max(1);
        let rows = self.cells.len() / self.cols.max(1) as usize;
        if c >= self.cols || r as usize >= rows {
            return &[];
        }
        let idx = r as usize * self.cols as usize + c as usize;
        if idx < self.cells.len() {
            &self.cells[idx]
        } else {
            &[]
        }
    }
}
