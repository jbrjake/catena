//! Clipping the walk to the canvas: exact skips, invariant K at the walk level, and the bound on
//! the work a huge segment costs.

use std::collections::BTreeSet;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use super::*;

/// A 40×30-pixel canvas at the origin.
const CANVAS: (i64, i64) = (40, 30);

fn canvas_clip() -> Clip {
    Clip::canvas(CANVAS.0, CANVAS.1)
}

fn on_canvas((x, y): (i64, i64)) -> bool {
    (0..CANVAS.0).contains(&x) && (0..CANVAS.1).contains(&y)
}

fn visible(pixels: impl Iterator<Item = (i64, i64)>) -> BTreeSet<(i64, i64)> {
    pixels.filter(|&p| on_canvas(p)).collect()
}

fn stepped(from: (i64, i64), to: (i64, i64), k: u64) -> (i64, i64, i64) {
    let mut line = Line::new(from, to);
    for _ in 0..k {
        line.next();
    }
    (line.x, line.y, line.err)
}

fn dashed_visible(walk: Walk<'_>, on: f64, off: f64) -> BTreeSet<(i64, i64)> {
    let mut walk = walk;
    let mut lit = BTreeSet::new();
    dash(&mut walk, on, off, |p| {
        if on_canvas((p.x, p.y)) {
            lit.insert((p.x, p.y));
        }
    });
    lit
}

#[test]
fn a_line_emits_one_pixel_per_major_step_plus_one() {
    assert_eq!(Line::new((0, 0), (7, -3)).count(), 8);
    assert_eq!(Line::new((2, 2), (2, 2)).count(), 1);
    assert_eq!(Line::new((0, 0), (-2, 9)).count(), 10);
}

#[test]
fn the_closed_form_state_is_the_stepped_state_at_any_scale() {
    let far = i64::from(i32::MAX);
    let (from, to) = ((-far - 1, -5), (far, 7));
    for k in [0u64, 1, 12_345, 1 << 31, (1u64 << 32) - 2] {
        let mut line = Line::at(from, to, k);
        line.next();
        let after = Line::at(from, to, k + 1);
        assert_eq!(
            (line.x, line.y, line.err),
            (after.x, after.y, after.err),
            "k = {k}"
        );
    }
    assert_eq!(
        Line::at(from, to, (1u64 << 32) - 1).remaining,
        1,
        "the last pixel"
    );
}

#[test]
fn a_segment_wholly_outside_the_clip_emits_nothing() {
    let clip = canvas_clip();
    let reach = CLIP_SLACK + 1;
    assert!(Line::clipped((-reach, -reach), (-reach, 500), clip).is_none());
    assert!(Line::clipped((-5000, -reach), (5000, -reach - 50), clip).is_none());
    assert!(Line::clipped((0, 0), (5, 5), clip).is_some());
}

#[test]
fn a_segment_across_the_whole_i32_range_walks_only_near_the_canvas() {
    let points = [(f64::from(i32::MIN), 0.0), (f64::from(i32::MAX), 3.0)];
    let walked = Walk::new(&points).clipped_to(Clip::canvas(16, 8)).count();
    let bound = usize::try_from(16 + 2 * CLIP_SLACK + 1).expect("small");
    assert!(walked <= bound, "{walked} pixels walked, over {bound}");
    assert!(
        walked >= 16,
        "{walked} pixels: the canvas row itself is crossed"
    );
    let mut bowed = Vec::new();
    crate::geometry::curve::tessellate(
        &[crate::geometry::curve::Bezier::Quadratic {
            from: points[0],
            ctrl: (0.0, 1e9),
            to: points[1],
        }],
        &mut bowed,
    );
    assert!(Walk::new(&bowed).clipped_to(Clip::canvas(16, 8)).count() <= bound);
}

#[test]
fn far_axis_aligned_dashes_keep_their_phase_through_a_skip() {
    let points = [(-10_000.0, 3.0), (100.0, 3.0)];
    let unclipped = dashed_visible(Walk::measured(&points, Vec::new()), 3.0, 2.0);
    let clipped = dashed_visible(
        Walk::measured(&points, Vec::new()).clipped_to(canvas_clip()),
        3.0,
        2.0,
    );
    assert_eq!(clipped.len(), unclipped.len());
    assert_eq!(clipped, unclipped);
    assert!(
        unclipped.contains(&(0, 3)),
        "x + 10000 = 10000 ≡ 0 (mod 5) is in a dash"
    );
    assert!(!unclipped.contains(&(3, 3)), "≡ 3 is in a gap");
}

// ── Properties ─────────────────────────────────────────────────────────────

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x636c_6970),
        failure_persistence: None,
        ..Config::default()
    }
}

fn endpoint(reach: i64) -> impl Strategy<Value = (i64, i64)> {
    (-reach..=reach, -reach..=reach)
}

/// Vertices near the canvas, beyond the slack and far away, mixed.
fn polyline() -> impl Strategy<Value = Vec<(f64, f64)>> {
    // Far enough to skip, near enough for the unclipped oracle to walk every pixel.
    let coord = prop_oneof![-60.0..100.0f64, -3000.0..3000.0f64, -20_000.0..20_000.0f64];
    prop::collection::vec((coord.clone(), coord), 1..6)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn the_closed_form_matches_the_stepped_walk(
        from in endpoint(300),
        to in endpoint(300),
        pick in 0.0..=1.0f64,
    ) {
        let steps = from.0.abs_diff(to.0).max(from.1.abs_diff(to.1));
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a fraction of at most 600 steps"
        )]
        let k = (pick * steps as f64) as u64;
        let at = Line::at(from, to, k);
        prop_assert_eq!((at.x, at.y, at.err), stepped(from, to, k));
        prop_assert_eq!(at.remaining, steps - k + 1);
    }

    #[test]
    fn clipping_keeps_every_pixel_on_the_canvas(points in polyline()) {
        let clipped: Vec<(i64, i64)> = Walk::new(&points)
            .clipped_to(canvas_clip())
            .map(|p| (p.x, p.y))
            .collect();
        let slack = CLIP_SLACK;
        for &(x, y) in &clipped {
            prop_assert!(
                (-slack..CANVAS.0 + slack).contains(&x) && (-slack..CANVAS.1 + slack).contains(&y),
                "({}, {}) is outside the clip", x, y
            );
        }
        let reference = visible(polyline_pixels(&points));
        let kept = visible(clipped.into_iter());
        prop_assert_eq!(kept.len(), reference.len());
        prop_assert_eq!(kept, reference);
    }

    #[test]
    fn dashes_inside_the_slack_are_unchanged_by_clipping(
        points in prop::collection::vec((-200.0..240.0f64, -200.0..230.0f64), 1..6),
        on in 1u8..6,
        off in 1u8..6,
    ) {
        let (on, off) = (f64::from(on), f64::from(off));
        let unclipped = dashed_visible(Walk::measured(&points, Vec::new()), on, off);
        let clipped = dashed_visible(
            Walk::measured(&points, Vec::new()).clipped_to(canvas_clip()),
            on,
            off,
        );
        prop_assert_eq!(clipped.len(), unclipped.len());
        prop_assert_eq!(clipped, unclipped);
    }
}
