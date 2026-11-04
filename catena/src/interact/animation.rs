//! Animations over injected time (plan §10.4), ported from the seed's `PanAnimation` and
//! `ZoomAnimation`. They read no clock: each advances by the `dt` the host passes to `tick`,
//! so a test steps them exactly, which retires the seed's `force_complete`, which rewound a
//! wall-clock start time.
//!
//! - [`PanGlide`] moves the view by a fixed offset over [`PAN_GLIDE`] (300 ms), easing out by
//!   a cubic: fast start, smooth stop, as the seed's did. It applies only the increments, in
//!   `f64` cells, so it composes with any pan made meanwhile and accumulates no rounding (the
//!   seed rounded the pan to whole cells every tick).
//! - [`ZoomGlide`] smooths the zoom toward a target that input may move at any time:
//!   exponential smoothing, `α = 1 − exp(−dt / τ)` with τ = [`ZOOM_TAU`] (40 ms). The seed
//!   moved 35% of the way per tick whatever the tick's length; at 60 frames a second that is
//!   τ ≈ 39 ms. Its doc comment's 65% after one frame described a factor of 0.65, not the 0.35
//!   its code used, and is not carried. It snaps within 0.001 of the target, as the seed did.

use std::time::Duration;

use crate::fmath;
use crate::geometry::cell::SubPt;

/// How long a pan glide takes (the seed's).
pub(crate) const PAN_GLIDE: Duration = Duration::from_millis(300);
/// The zoom smoothing's time constant (plan §6).
pub(crate) const ZOOM_TAU: Duration = Duration::from_millis(40);
/// How close to its target the zoom snaps there (the seed's).
const ZOOM_SNAP: f64 = 0.001;

/// Ease-out cubic over `t` in `[0, 1]`: `1 − (1 − t)³`, clamped.
pub(crate) fn ease_out_cubic(t: f64) -> f64 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

/// A pan of `total` cells, in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PanGlide {
    total: (f64, f64),
    elapsed: Duration,
    /// The eased fraction of `total` applied so far.
    applied: f64,
}

impl PanGlide {
    /// A glide that will move the view by `total` cells.
    pub(crate) fn new(total: (f64, f64)) -> Self {
        PanGlide {
            total,
            elapsed: Duration::ZERO,
            applied: 0.0,
        }
    }

    /// Advances by `dt`: the pan to apply now, and whether the glide goes on. Its increments
    /// sum to exactly `total` when it ends.
    pub(crate) fn advance(&mut self, dt: Duration) -> ((f64, f64), bool) {
        self.elapsed = self.elapsed.saturating_add(dt);
        let t = self.elapsed.as_secs_f64() / PAN_GLIDE.as_secs_f64();
        let eased = ease_out_cubic(t);
        let step = eased - self.applied;
        self.applied = eased;
        ((self.total.0 * step, self.total.1 * step), t < 1.0)
    }
}

/// A zoom moving toward `target`, keeping the view point `about` where it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ZoomGlide {
    pub(crate) target: f64,
    pub(crate) about: SubPt,
}

impl ZoomGlide {
    /// The zoom `dt` after `zoom`, and whether it has reached the target.
    pub(crate) fn step(&self, zoom: f64, dt: Duration) -> (f64, bool) {
        let diff = self.target - zoom;
        if diff.abs() < ZOOM_SNAP {
            return (self.target, true);
        }
        let alpha = 1.0 - fmath::exp(-dt.as_secs_f64() / ZOOM_TAU.as_secs_f64());
        let next = zoom + diff * alpha;
        if (self.target - next).abs() < ZOOM_SNAP {
            (self.target, true)
        } else {
            (next, false)
        }
    }
}

#[cfg(test)]
#[path = "animation_tests.rs"]
mod tests;
