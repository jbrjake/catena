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
//!
//! A canvas clips its walks (plan §6, ledger row 14): each segment's walk starts where it enters
//! the canvas grown by [`CLIP_SLACK`] and stops where it leaves, jumping there through the
//! walk's closed form instead of stepping, so a segment across the whole `i32` range costs what
//! its visible part does, and every pixel inside is the one the whole walk would light
//! (invariant K).

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

/// How far past the canvas, in pixels, a clipped walk still steps through every pixel. Beyond
/// it the walk skips straight to where a segment comes back, which bounds what a huge segment
/// costs; within it a dash keeps its exact phase (after a skip, [`dash`] restarts on the nominal
/// phase of the pattern).
pub(crate) const CLIP_SLACK: i64 = 256;

/// The pixels a clipped walk may emit, bounds inclusive.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Clip {
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
}

impl Clip {
    /// A `width × height`-pixel canvas grown by [`CLIP_SLACK`] on every side.
    pub(crate) fn canvas(width: i64, height: i64) -> Clip {
        Clip {
            x0: -CLIP_SLACK,
            y0: -CLIP_SLACK,
            x1: width - 1 + CLIP_SLACK,
            y1: height - 1 + CLIP_SLACK,
        }
    }
}

/// The Bresenham walk from one pixel to another, both inclusive, in `i64` so no `i32` endpoints
/// overflow. Every step moves one pixel along the major axis, so the state after `k` steps has a
/// closed form, and a clipped walk starts where it enters the clip without stepping there.
struct Line {
    x: i64,
    y: i64,
    dx: i64,
    dy: i64,
    sx: i64,
    sy: i64,
    err: i64,
    /// Pixels still to emit, this one included.
    remaining: u64,
    /// Whether the clip cut pixels off the start, or the end.
    cut: (bool, bool),
}

impl Line {
    fn new(from: (i64, i64), to: (i64, i64)) -> Self {
        Line::at(from, to, 0)
    }

    /// The walk from `from` to `to` after `k` steps, `k` at most the major axis's extent: after
    /// `k` major steps the minor axis has stepped `⌊(2·minor·k + major) / (2·major)⌋` times,
    /// which is exactly what stepping gives, ties included.
    fn at((x0, y0): (i64, i64), (x1, y1): (i64, i64), k: u64) -> Self {
        let (span_x, span_y) = (x0.abs_diff(x1), y0.abs_diff(y1));
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let major = span_x.max(span_y);
        let minor = minor_steps(major, span_x.min(span_y), k);
        let wide = |v: u64| i128::from(v);
        let (along, across, run, rise) = (wide(k), wide(minor), wide(span_x), wide(span_y));
        let (x, y, err) = if run >= rise {
            (
                i128::from(x0) + i128::from(sx) * along,
                i128::from(y0) + i128::from(sy) * across,
                run - rise - along * rise + across * run,
            )
        } else {
            (
                i128::from(x0) + i128::from(sx) * across,
                i128::from(y0) + i128::from(sy) * along,
                run - rise + along * run - across * rise,
            )
        };
        let fit =
            |v: i128| i64::try_from(v).expect("a walk's state lies within its i32-range segment");
        Line {
            x: fit(x),
            y: fit(y),
            dx: fit(run),
            dy: -fit(rise),
            sx,
            sy,
            err: fit(err),
            remaining: major - k + 1,
            cut: (false, false),
        }
    }

    /// The part of the walk inside `clip`, or `None` when no pixel of it is. Along the major axis
    /// the in-clip steps follow from the bounds; along the minor axis the step count is monotone
    /// in `k`, so two binary searches find the rest.
    fn clipped(from: (i64, i64), to: (i64, i64), clip: Clip) -> Option<Line> {
        let (a, b) = (from.0.abs_diff(to.0), from.1.abs_diff(to.1));
        let sign = |p: i64, q: i64| if p < q { 1 } else { -1 };
        let (x_steps, y_steps) = (
            (from.0, sign(from.0, to.0), clip.x0, clip.x1, a),
            (from.1, sign(from.1, to.1), clip.y0, clip.y1, b),
        );
        let ((k_lo, k_hi), (j_lo, j_hi), major, minor) = if a >= b {
            (in_bounds(x_steps)?, in_bounds(y_steps)?, a, b)
        } else {
            (in_bounds(y_steps)?, in_bounds(x_steps)?, b, a)
        };
        let j = |k: u64| minor_steps(major, minor, k);
        let first = first_where(k_lo, k_hi, |k| j(k) >= j_lo)?;
        if j(first) > j_hi {
            return None;
        }
        let last = first_where(first, k_hi, |k| j(k) > j_hi).map_or(k_hi, |k| k - 1);
        let mut line = Line::at(from, to, first);
        line.remaining = last - first + 1;
        line.cut = (first > 0, last < major);
        Some(line)
    }
}

/// Minor-axis steps after `k` major steps (see [`Line::at`]).
fn minor_steps(major: u64, minor: u64, k: u64) -> u64 {
    if major == 0 {
        return 0;
    }
    let steps =
        (2 * u128::from(minor) * u128::from(k) + u128::from(major)) / (2 * u128::from(major));
    u64::try_from(steps).expect("at most `minor` steps")
}

/// The steps `t` in `0..=extent` at which `start + sign·t` lies in `lo..=hi`, or `None`.
fn in_bounds((start, sign, lo, hi, extent): (i64, i64, i64, i64, u64)) -> Option<(u64, u64)> {
    let (start, lo, hi) = (i128::from(start), i128::from(lo), i128::from(hi));
    let (from, to) = if sign > 0 {
        (lo - start, hi - start)
    } else {
        (start - hi, start - lo)
    };
    let (from, to) = (from.max(0), to.min(i128::from(extent)));
    (from <= to).then(|| {
        (
            u64::try_from(from).unwrap_or(0),
            u64::try_from(to).unwrap_or(0),
        )
    })
}

/// The smallest `k` in `lo..=hi` where the monotone `holds` is true, or `None`.
fn first_where(lo: u64, hi: u64, holds: impl Fn(u64) -> bool) -> Option<u64> {
    if lo > hi || !holds(hi) {
        return None;
    }
    let (mut lo, mut hi) = (lo, hi);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if holds(mid) { hi = mid } else { lo = mid + 1 }
    }
    Some(lo)
}

impl Iterator for Line {
    type Item = (i64, i64);

    fn next(&mut self) -> Option<(i64, i64)> {
        if self.remaining == 0 {
            return None;
        }
        let here = (self.x, self.y);
        self.remaining -= 1;
        if self.remaining > 0 {
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

/// One pixel of a polyline's walk, the arc length it sits at (0 when not measured), the
/// segment, from `points[segment]` to the next vertex, whose walk lit it, and whether a clip cut
/// the walk's pixels just before it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PathPixel {
    pub x: i64,
    pub y: i64,
    pub s: f64,
    pub segment: usize,
    pub resumed: bool,
}

/// The pixel walk of a polyline, in path order.
pub(crate) struct Walk<'a> {
    points: &'a [(f64, f64)],
    /// The segment being walked: from `points[segment]` to the next vertex.
    segment: usize,
    line: Option<Line>,
    recent: [Option<(i64, i64)>; 2],
    arc: Option<ArcLength>,
    clip: Option<Clip>,
    /// Whether the clip has cut pixels since the last one emitted.
    skipped: bool,
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
            clip: None,
            skipped: false,
        }
    }

    /// The walk emitting only the pixels in `clip`, which it skips to without stepping through
    /// the rest. Every pixel it emits is one the whole walk emits; a pixel after a cut is
    /// marked [`PathPixel::resumed`].
    pub(crate) fn clipped_to(self, clip: Clip) -> Self {
        Walk {
            clip: Some(clip),
            ..self
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

    /// The walk of the current segment (or the first after it with a pixel in the clip), or
    /// `None` when the polyline is done. A one-vertex polyline is one zero-length segment.
    fn start_segment(&mut self) -> Option<Line> {
        let last = self.points.len().checked_sub(1)?;
        loop {
            if self.segment > last.saturating_sub(1) {
                return None;
            }
            let from = snap(self.points[self.segment]);
            let to = snap(self.points[(self.segment + 1).min(last)]);
            let Some(clip) = self.clip else {
                return Some(Line::new(from, to));
            };
            if let Some(line) = Line::clipped(from, to, clip) {
                self.skipped |= line.cut.0;
                return Some(line);
            }
            self.skipped = true;
            self.segment += 1;
        }
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
                self.skipped |= line.cut.1;
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
            let resumed = std::mem::take(&mut self.skipped);
            return Some(PathPixel {
                x: pixel.0,
                y: pixel.1,
                s,
                segment: self.segment,
                resumed,
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
/// calling `plot` with each pixel of each dash. A run holds at least one pixel, so a positive
/// length shorter than a pixel step still shows. `on <= 0` draws nothing, `off <= 0` draws
/// solid, and so does a NaN `off`.
///
/// A pixel the walk [resumed](PathPixel::resumed) at, after a clip cut the pixels before it,
/// restarts the runs on the pattern's nominal phase at its arc length, as if every run so far
/// had been exactly its pattern length; the runs before a cut are not walked to find their drift.
pub(crate) fn dash(walk: &mut Walk<'_>, on: f64, off: f64, mut plot: impl FnMut(PathPixel)) {
    if on.is_nan() || on <= 0.0 {
        return;
    }
    if off.is_nan() || off <= 0.0 {
        walk.for_each(plot);
        return;
    }
    let Some(mut here) = walk.next() else {
        return;
    };
    let mut next = walk.next();
    let mut after = walk.next();
    // The current run spans from `start` (a pixel boundary) to the boundary after `here`.
    let (mut start, mut drawing) = match next {
        _ if here.resumed => nominal_run(here.s, on, off),
        Some(n) if !n.resumed => (here.s - (n.s - here.s) / 2.0, true),
        _ => (here.s - 0.5, true),
    };
    loop {
        if drawing {
            plot(here);
        }
        let Some(n) = next else {
            return;
        };
        if n.resumed {
            (start, drawing) = nominal_run(n.s, on, off);
        } else {
            let target = if drawing { on } else { off };
            let end_here = f64::midpoint(here.s, n.s);
            let end_next = match after {
                Some(a) if !a.resumed => f64::midpoint(n.s, a.s),
                _ => n.s + (n.s - here.s) / 2.0,
            };
            // End the run after `here` unless one more pixel brings its span strictly closer to
            // the target: `|end_here − start − target| <= |end_next − start − target|`, written
            // so a target far larger than the span cannot round both sides to a tie.
            if target <= f64::midpoint(end_here, end_next) - start {
                start = end_here;
                drawing = !drawing;
            }
        }
        here = n;
        next = after;
        after = if next.is_some() { walk.next() } else { None };
    }
}

/// The run a pixel at arc length `s` falls in when every run is exactly its pattern length,
/// runs starting half a pixel before arc length 0 as the walk's first run does: its start and
/// whether it is a dash.
fn nominal_run(s: f64, on: f64, off: f64) -> (f64, bool) {
    let phase = (s + 0.5).rem_euclid(on + off);
    if phase < on {
        (s - phase, true)
    } else {
        (s - (phase - on), false)
    }
}

#[cfg(test)]
#[path = "polyline_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "polyline_clip_tests.rs"]
mod clip_tests;
