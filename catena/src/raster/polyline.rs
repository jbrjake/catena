//! Polyline rasterization: the one pixel walk under every line and curve primitive (owner ruling
//! A1). A straight line is a two-vertex polyline; a curve is the polyline `tessellate` makes of it.
//!
//! The walk runs Bresenham between consecutive vertices rounded to pixels, so its pixels are
//! 8-connected and include both end points. A pixel equal to either of the two last emitted is
//! skipped: that drops each joint's repeat and the one-pixel back-steps rounding makes near a
//! curve's extremum, so a polyline that does not cross itself emits every pixel once.
//!
//! Dashes are measured by arc length along the polyline. Each pixel sits at the arc length of
//! its centre's nearest point on the polyline, a run of pixels spans from the midpoint before its
//! first pixel to the midpoint after its last, and each dash or gap ends at the pixel boundary
//! that brings its span closest to the pattern's length. A run's span is therefore within half a
//! pixel step (at most √2/2 sub-pixels) of its pattern length on any curve, short or long.

/// Vertices round to pixels and then clamp to the `i32` range, so a polyline is no more costly
/// to walk than the integer primitives' widest line.
pub(crate) fn snap((x, y): (f64, f64)) -> (i64, i64) {
    (snap_coord(x), snap_coord(y))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the float-to-int cast saturates (NaN to 0) and the result is clamped to i32"
)]
fn snap_coord(v: f64) -> i64 {
    (v.round() as i64).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
}

/// The Bresenham walk from one pixel to another, both inclusive, in `i64` so no `i32` endpoints
/// overflow.
struct Line {
    x: i64,
    y: i64,
    x1: i64,
    y1: i64,
    dx: i64,
    dy: i64,
    sx: i64,
    sy: i64,
    err: i64,
    done: bool,
}

impl Line {
    fn new((x0, y0): (i64, i64), (x1, y1): (i64, i64)) -> Self {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        Line {
            x: x0,
            y: y0,
            x1,
            y1,
            dx,
            dy,
            sx: if x0 < x1 { 1 } else { -1 },
            sy: if y0 < y1 { 1 } else { -1 },
            err: dx + dy,
            done: false,
        }
    }
}

impl Iterator for Line {
    type Item = (i64, i64);

    fn next(&mut self) -> Option<(i64, i64)> {
        if self.done {
            return None;
        }
        let here = (self.x, self.y);
        if here == (self.x1, self.y1) {
            self.done = true;
        } else {
            let e2 = 2 * self.err;
            if e2 >= self.dy {
                self.err += self.dy;
                self.x += self.sx;
            }
            if e2 <= self.dx {
                self.err += self.dx;
                self.y += self.sy;
            }
        }
        Some(here)
    }
}

/// Places pixels at arc lengths along a polyline: the nearest point among the segments around
/// the walk's position, never behind the previous pixel's.
struct ArcLength {
    /// `cumulative[k]` is the arc length from the first vertex to vertex `k`.
    cumulative: Vec<f64>,
    /// The segment the previous pixel's nearest point lay on.
    segment: usize,
    last: f64,
}

impl ArcLength {
    fn new(points: &[(f64, f64)], mut cumulative: Vec<f64>) -> Self {
        cumulative.clear();
        let mut total = 0.0;
        cumulative.push(total);
        for pair in points.windows(2) {
            let (dx, dy) = (pair[1].0 - pair[0].0, pair[1].1 - pair[0].1);
            total += (dx * dx + dy * dy).sqrt();
            cumulative.push(total);
        }
        ArcLength {
            cumulative,
            segment: 0,
            last: 0.0,
        }
    }

    /// The arc length of pixel `(x, y)`, found while the walk is on segment `walking`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "pixel coordinates are clamped to the i32 range, which f64 holds exactly"
    )]
    fn locate(&mut self, points: &[(f64, f64)], walking: usize, (x, y): (i64, i64)) -> f64 {
        let segments = points.len().saturating_sub(1);
        if segments == 0 {
            return 0.0;
        }
        let (px, py) = (x as f64, y as f64);
        let lo = self.segment.saturating_sub(1);
        let hi = (walking + 1).max(self.segment + 1).min(segments - 1);
        let mut best = (f64::INFINITY, self.last, self.segment);
        for k in lo..=hi {
            let ((ax, ay), (bx, by)) = (points[k], points[k + 1]);
            let (ux, uy) = (bx - ax, by - ay);
            let len2 = ux * ux + uy * uy;
            let t = if len2 > 0.0 {
                (((px - ax) * ux + (py - ay) * uy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (qx, qy) = (ax + t * ux - px, ay + t * uy - py);
            let dist2 = qx * qx + qy * qy;
            if dist2 < best.0 {
                let s = self.cumulative[k] + t * (self.cumulative[k + 1] - self.cumulative[k]);
                best = (dist2, s, k);
            }
        }
        self.segment = best.2;
        self.last = self.last.max(best.1);
        self.last
    }

    fn into_buffer(self) -> Vec<f64> {
        self.cumulative
    }
}

/// One pixel of a polyline's walk and the arc length it sits at (0 when not measured).
#[derive(Debug, Clone, Copy)]
pub(crate) struct PathPixel {
    pub x: i64,
    pub y: i64,
    pub s: f64,
}

/// The pixel walk of a polyline, in path order.
pub(crate) struct Walk<'a> {
    points: &'a [(f64, f64)],
    /// The segment being walked: from `points[segment]` to the next vertex.
    segment: usize,
    line: Option<Line>,
    recent: [Option<(i64, i64)>; 2],
    arc: Option<ArcLength>,
}

impl<'a> Walk<'a> {
    /// The walk without arc lengths.
    pub(crate) fn new(points: &'a [(f64, f64)]) -> Self {
        Walk {
            points,
            segment: 0,
            line: None,
            recent: [None; 2],
            arc: None,
        }
    }

    /// The walk with each pixel's arc length, using `buffer`'s allocation for the vertices'
    /// cumulative lengths (get it back with [`Walk::into_buffer`]).
    pub(crate) fn measured(points: &'a [(f64, f64)], buffer: Vec<f64>) -> Self {
        Walk {
            arc: Some(ArcLength::new(points, buffer)),
            ..Walk::new(points)
        }
    }

    pub(crate) fn into_buffer(self) -> Vec<f64> {
        self.arc.map(ArcLength::into_buffer).unwrap_or_default()
    }

    /// The segment after the current one, or `None` when the polyline is done. A one-vertex
    /// polyline is one zero-length segment.
    fn start_segment(&mut self) -> Option<Line> {
        let last = self.points.len().checked_sub(1)?;
        if self.segment > last.saturating_sub(1) {
            return None;
        }
        let from = self.points[self.segment];
        let to = self.points[(self.segment + 1).min(last)];
        Some(Line::new(snap(from), snap(to)))
    }
}

impl Iterator for Walk<'_> {
    type Item = PathPixel;

    fn next(&mut self) -> Option<PathPixel> {
        loop {
            if self.line.is_none() {
                self.line = Some(self.start_segment()?);
            }
            let line = self.line.as_mut().expect("set just above");
            let Some(pixel) = line.next() else {
                self.line = None;
                self.segment += 1;
                continue;
            };
            if self.recent.contains(&Some(pixel)) {
                continue;
            }
            self.recent = [self.recent[1], Some(pixel)];
            let s = match &mut self.arc {
                Some(arc) => arc.locate(self.points, self.segment, pixel),
                None => 0.0,
            };
            return Some(PathPixel {
                x: pixel.0,
                y: pixel.1,
                s,
            });
        }
    }
}

/// The pixels a polyline of sub-pixel vertices lights, in path order: Bresenham between
/// consecutive vertices rounded to pixels, each pixel once unless the polyline crosses itself.
/// The canvas primitives draw exactly these pixels (solid) or a subset (dashed, hop-gapped).
pub fn polyline_pixels(points: &[(f64, f64)]) -> impl Iterator<Item = (i64, i64)> + '_ {
    Walk::new(points).map(|p| (p.x, p.y))
}

/// Runs of `on` then `off` sub-pixels of arc length along a measured walk, starting with a dash,
/// calling `plot` for each pixel of each dash. A run holds at least one pixel, so a positive
/// length shorter than a pixel step still shows. `on <= 0` draws nothing, `off <= 0` draws
/// solid, and so does a NaN `off`.
pub(crate) fn dash(walk: &mut Walk<'_>, on: f64, off: f64, mut plot: impl FnMut(i64, i64)) {
    if on.is_nan() || on <= 0.0 {
        return;
    }
    if off.is_nan() || off <= 0.0 {
        walk.for_each(|p| plot(p.x, p.y));
        return;
    }
    let Some(mut here) = walk.next() else {
        return;
    };
    let mut next = walk.next();
    let mut after = walk.next();
    // The current run spans from `start` (a pixel boundary) to the boundary after `here`.
    let mut start = match next {
        Some(n) => here.s - (n.s - here.s) / 2.0,
        None => here.s - 0.5,
    };
    let mut drawing = true;
    loop {
        if drawing {
            plot(here.x, here.y);
        }
        let Some(n) = next else {
            return;
        };
        let target = if drawing { on } else { off };
        let end_here = f64::midpoint(here.s, n.s);
        let end_next = match after {
            Some(a) => f64::midpoint(n.s, a.s),
            None => n.s + (n.s - here.s) / 2.0,
        };
        // End the run after `here` unless one more pixel brings its span strictly closer to the
        // target: `|end_here − start − target| <= |end_next − start − target|`, written so a
        // target far larger than the span cannot round both sides to a tie.
        if target <= f64::midpoint(end_here, end_next) - start {
            start = end_here;
            drawing = !drawing;
        }
        here = n;
        next = after;
        after = if next.is_some() { walk.next() } else { None };
    }
}

#[cfg(test)]
#[path = "polyline_tests.rs"]
mod tests;
