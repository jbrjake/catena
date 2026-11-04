//! Navigation as free functions over the view (plan §6, §10), ported from the seed's
//! `navigation.rs`, which ran them as `impl App` methods: the zoom steps and their clamp, the
//! pan that centers a node, and occlusion-aware centering.
//!
//! Kept: the keyboard zoom step of 0.25 and the scroll step of 0.08 (touchpads send many
//! events per gesture), each added to the zoom's current target so that steps accumulate
//! while a zoom is still moving; the clamp to `[0.1, 4.0]`; and a node already within three
//! cells of the center is not panned to. The anchor-keeping zoom itself is the viewport's
//! (`Viewport::zoom_about`), and the smoothing and glide are in `animation`.
//!
//! Fixed: the seed's visible center subtracted each occluding window's area and moment on its
//! own, so two overlapping windows were subtracted twice and the center could land outside
//! the free region, and it indexed its window map in a way that could panic. Here the free
//! region is counted cell by cell, every occluded cell once. Dropped: everything about the
//! seed's own windows, data refresh, palettes and inspector.

use crate::geometry::cell::{CellBox, CellPt, SubPt};
use crate::geometry::{MAX_ZOOM, MIN_ZOOM};

/// The zoom a key press adds or takes away (the seed's).
pub(crate) const KEY_ZOOM_STEP: f64 = 0.25;
/// The zoom one wheel notch adds or takes away (the seed's).
pub(crate) const SCROLL_ZOOM_STEP: f64 = 0.08;
/// How near the center, in cells on both axes, a node needs no pan (the seed's).
const CENTER_SLACK: f64 = 3.0;

/// The zoom target after a step of `step` from `target`, clamped to the zoom's range.
pub(crate) fn step_zoom(target: f64, step: f64) -> f64 {
    (target + step).clamp(MIN_ZOOM, MAX_ZOOM)
}

/// The pan, in cells, that brings `anchor` to `center`; `None` when it is already within
/// three cells of it on both axes.
pub(crate) fn pan_to(anchor: CellPt, center: SubPt) -> Option<(f64, f64)> {
    let (dx, dy) = (
        center.x - f64::from(anchor.x),
        center.y - f64::from(anchor.y),
    );
    (dx.abs() >= CENTER_SLACK || dy.abs() >= CENTER_SLACK).then_some((dx, dy))
}

/// The middle of the part of a `width × height` frame that `occlusions` (windows the host
/// draws over the view) leave visible: the mean of its free cells. The frame's own middle when
/// nothing is occluded, or everything is.
///
/// Counted without a pass over the cells, which at 65535 × 65535 would be 4.3 billion: the
/// windows' edges cut the frame into rectangles that are each wholly free or wholly occluded,
/// and a free rectangle's cells sum in closed form. Every sum stays below 2⁵³, so the mean is
/// exactly the cell-by-cell one.
#[expect(
    clippy::cast_precision_loss,
    reason = "counts and sums stay below 2^53, where an f64 holds an integer exactly"
)]
pub(crate) fn visible_center((width, height): (u16, u16), occlusions: &[CellBox]) -> SubPt {
    let whole = SubPt::new(
        (f64::from(width) - 1.0).max(0.0) / 2.0,
        (f64::from(height) - 1.0).max(0.0) / 2.0,
    );
    if occlusions.is_empty() || width == 0 || height == 0 {
        return whole;
    }
    let (w, h) = (i64::from(width), i64::from(height));
    // Each window's part on the frame, as half-open spans `[x0, x1) × [y0, y1)`.
    let parts: Vec<[i64; 4]> = occlusions
        .iter()
        .filter_map(|o| {
            let (x0, y0) = (i64::from(o.x).clamp(0, w), i64::from(o.y).clamp(0, h));
            let (x1, y1) = (o.right().clamp(0, w), o.bottom().clamp(0, h));
            (x0 < x1 && y0 < y1).then_some([x0, y0, x1, y1])
        })
        .collect();
    let cuts = |lo: usize, hi: usize, end: i64| {
        let mut cuts: Vec<i64> = parts.iter().flat_map(|p| [p[lo], p[hi]]).collect();
        cuts.extend([0, end]);
        cuts.sort_unstable();
        cuts.dedup();
        cuts
    };
    let (xs, ys) = (cuts(0, 2, w), cuts(1, 3, h));
    // The sum of the integers in `[a, b)`, exact: a run of an odd length has an even
    // first-plus-last.
    let run = |a: i64, b: i64| (a + b - 1) * (b - a) / 2;
    let (mut free, mut sx, mut sy) = (0i64, 0i64, 0i64);
    for y in ys.windows(2) {
        for x in xs.windows(2) {
            let covered = parts
                .iter()
                .any(|p| p[0] <= x[0] && x[0] < p[2] && p[1] <= y[0] && y[0] < p[3]);
            if !covered {
                let (dx, dy) = (x[1] - x[0], y[1] - y[0]);
                free += dx * dy;
                sx += dy * run(x[0], x[1]);
                sy += dx * run(y[0], y[1]);
            }
        }
    }
    if free == 0 {
        return whole;
    }
    SubPt::new(sx as f64 / free as f64, sy as f64 / free as f64)
}

/// Half the spatial-navigation cone, as its cosine: ±60° (plan §10.3).
const CONE_COS: f64 = 0.5;

/// The index in `candidates` of the node spatial navigation moves to from `from` in `dir`
/// (plan §10.3), or `None` at the graph's edge in that direction.
///
/// Offsets are aspect-corrected, a row counting `1 / cell_aspect` columns, so a cell is as far
/// across as down. A candidate in the ±60° cone around `dir` scores its distance times
/// `1 + 2(1 − cos θ)`, and the lowest wins. With the cone empty, the nearest candidate ahead
/// in the half-plane wins. Ties go to the earlier candidate; one at `from` is never taken.
pub(crate) fn nearest_toward(
    from: CellPt,
    dir: super::Dir,
    candidates: &[CellPt],
    cell_aspect: f64,
) -> Option<usize> {
    let aspect = if cell_aspect.is_finite() && cell_aspect > 0.0 {
        cell_aspect
    } else {
        0.5
    };
    let (ux, uy) = match dir {
        super::Dir::Up => (0.0, -1.0),
        super::Dir::Down => (0.0, 1.0),
        super::Dir::Left => (-1.0, 0.0),
        super::Dir::Right => (1.0, 0.0),
    };
    // The best (score, index) in the cone, and the nearest (distance, index) in the half-plane.
    let mut cone: Option<(f64, usize)> = None;
    let mut plane: Option<(f64, usize)> = None;
    for (i, c) in candidates.iter().enumerate() {
        let (dx, dy) = (f64::from(c.x - from.x), f64::from(c.y - from.y) / aspect);
        let ahead = dx * ux + dy * uy;
        if ahead <= 0.0 {
            continue;
        }
        let distance = (dx * dx + dy * dy).sqrt();
        let cos = ahead / distance;
        let better = |best: Option<(f64, usize)>, score: f64| best.is_none_or(|(b, _)| score < b);
        if cos >= CONE_COS {
            let score = distance * (1.0 + 2.0 * (1.0 - cos));
            if better(cone, score) {
                cone = Some((score, i));
            }
        } else if better(plane, distance) {
            plane = Some((distance, i));
        }
    }
    cone.or(plane).map(|(_, i)| i)
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;
