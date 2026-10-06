//! Bézier curves and the one function that turns them into polylines (owner ruling A1).
//!
//! Every curve `catena` draws is a quadratic or a cubic Bézier, or a chain of them, and every
//! raster primitive draws polylines: [`tessellate`] is the bridge between the two. Coordinates
//! are sub-cell pixels (plan §6), as `f64` so that arc length along the polyline stays exact
//! enough to dash by.

/// A quadratic or cubic Bézier piece in sub-cell pixel coordinates. A chain of pieces, each
/// starting where the previous one ends, is a `&[Bezier]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bezier {
    /// `B(t) = (1−t)² from + 2(1−t)t ctrl + t² to`.
    Quadratic {
        /// The start point, `B(0)`.
        from: (f64, f64),
        /// The control point.
        ctrl: (f64, f64),
        /// The end point, `B(1)`.
        to: (f64, f64),
    },
    /// `B(t) = (1−t)³ from + 3(1−t)²t ctrl0 + 3(1−t)t² ctrl1 + t³ to`.
    Cubic {
        /// The start point, `B(0)`.
        from: (f64, f64),
        /// The control point that sets the start tangent.
        ctrl0: (f64, f64),
        /// The control point that sets the end tangent.
        ctrl1: (f64, f64),
        /// The end point, `B(1)`.
        to: (f64, f64),
    },
}

impl Bezier {
    /// The start point, `B(0)`.
    #[must_use]
    pub fn start(&self) -> (f64, f64) {
        match *self {
            Bezier::Quadratic { from, .. } | Bezier::Cubic { from, .. } => from,
        }
    }

    /// The end point, `B(1)`.
    #[must_use]
    pub fn end(&self) -> (f64, f64) {
        match *self {
            Bezier::Quadratic { to, .. } | Bezier::Cubic { to, .. } => to,
        }
    }

    /// The point at parameter `t`. `t = 0` and `t = 1` give the end points exactly.
    #[must_use]
    pub fn point(&self, t: f64) -> (f64, f64) {
        let u = 1.0 - t;
        match *self {
            // The seed's expression, term for term, so tessellated vertices land on the pixels
            // its sampler plotted.
            Bezier::Quadratic { from, ctrl, to } => (
                u * u * from.0 + 2.0 * u * t * ctrl.0 + t * t * to.0,
                u * u * from.1 + 2.0 * u * t * ctrl.1 + t * t * to.1,
            ),
            Bezier::Cubic {
                from,
                ctrl0,
                ctrl1,
                to,
            } => {
                // The four Bernstein weights.
                let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                (
                    w[0] * from.0 + w[1] * ctrl0.0 + w[2] * ctrl1.0 + w[3] * to.0,
                    w[0] * from.1 + w[1] * ctrl0.1 + w[2] * ctrl1.1 + w[3] * to.1,
                )
            }
        }
    }

    /// The derivative `B′(t)`: the tangent direction, scaled by the parameter speed.
    #[must_use]
    pub fn derivative(&self, t: f64) -> (f64, f64) {
        let u = 1.0 - t;
        match *self {
            Bezier::Quadratic { from, ctrl, to } => (
                2.0 * u * (ctrl.0 - from.0) + 2.0 * t * (to.0 - ctrl.0),
                2.0 * u * (ctrl.1 - from.1) + 2.0 * t * (to.1 - ctrl.1),
            ),
            Bezier::Cubic {
                from,
                ctrl0,
                ctrl1,
                to,
            } => {
                // The quadratic Bernstein weights of the three control-polygon legs, times 3.
                let w = [3.0 * u * u, 6.0 * u * t, 3.0 * t * t];
                (
                    w[0] * (ctrl0.0 - from.0)
                        + w[1] * (ctrl1.0 - ctrl0.0)
                        + w[2] * (to.0 - ctrl1.0),
                    w[0] * (ctrl0.1 - from.1)
                        + w[1] * (ctrl1.1 - ctrl0.1)
                        + w[2] * (to.1 - ctrl1.1),
                )
            }
        }
    }

    /// The straight-line distance from start to end, which sets the sample count.
    fn chord(&self) -> f64 {
        let ((x0, y0), (x1, y1)) = (self.start(), self.end());
        let (dx, dy) = (x1 - x0, y1 - y0);
        (dx * dx + dy * dy).sqrt()
    }
}

/// Replaces `out` with the polyline through `chain`: each piece sampled at `steps + 1` evenly
/// spaced parameters, where `steps = (chord / 2).clamp(10, 200)` is the seed's density of one
/// sample per 2 sub-pixels of chord. The first and last vertices are the chain's end points
/// exactly. Where a piece starts at the point the previous one ended, that joint appears once;
/// where it starts elsewhere, the polyline bridges the gap with a straight segment. An empty
/// chain gives an empty polyline.
pub fn tessellate(chain: &[Bezier], out: &mut Vec<(f64, f64)>) {
    out.clear();
    for piece in chain {
        let steps = sample_count(piece.chord());
        let first = u32::from(out.last() == Some(&piece.start()));
        for i in first..=steps {
            out.push(piece.point(f64::from(i) / f64::from(steps)));
        }
    }
}

/// `(len / 2).clamp(10, 200)` for a chord of `len` sub-pixels, truncating like the seed did. A
/// NaN or infinite chord saturates into the clamp rather than failing.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the float-to-int cast saturates (NaN to 0), and the result is clamped anyway"
)]
fn sample_count(len: f64) -> u32 {
    let half = (len as u64) / 2;
    u32::try_from(half.clamp(10, 200)).expect("clamped to at most 200")
}

#[cfg(test)]
#[path = "curve_tests.rs"]
mod tests;
