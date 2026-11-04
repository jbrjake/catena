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
