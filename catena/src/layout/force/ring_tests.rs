// `allow`, not `expect`: clippy 1.99's `float_cmp` no longer fires here and earlier releases do.
#![allow(
    clippy::float_cmp,
    reason = "exact angles from fmath and exact centroids are the claims under test"
)]

use super::super::simulation::centroid;
use super::*;

fn node(width: f64, ideal: Option<f64>) -> RingNode {
    RingNode {
        size: (width, 2.0),
        ideal,
    }
}

fn plain(n: usize) -> Vec<RingNode> {
    (0..n).map(|_| node(4.0, None)).collect()
}

// ── Placement (the seed's `radial_layout` tests) ─────────────────────────────

#[test]
fn an_empty_ring_places_nothing() {
    assert_eq!(place(&[], (40.0, 12.0), 20.0), []);
}

#[test]
fn a_lone_node_sits_at_angle_zero() {
    assert_eq!(place(&plain(1), (40.0, 12.0), 20.0), [(60.0, 12.0)]);
}

#[test]
fn nodes_are_spaced_evenly() {
    let at = place(&plain(8), (50.0, 20.0), 30.0);
    assert_eq!(at.len(), 8);
    for (i, &(x, y)) in at.iter().enumerate() {
        let angle = std::f64::consts::TAU * count(i) / 8.0;
        assert_eq!(
            (x, y),
            (
                50.0 + 30.0 * fmath::cos(angle),
                20.0 + 30.0 * fmath::sin(angle)
            )
        );
    }
}

#[test]
fn a_ring_long_enough_for_its_labels_has_no_overlap() {
    let names = [
        "Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel",
    ];
    let nodes: Vec<RingNode> = names
        .iter()
        .map(|name| node(count(name.chars().count()) + 2.0, None))
        .collect();
    let r = radius((50.0, 20.0), &[], &nodes);
    let at = place(&nodes, (50.0, 20.0), r);
    for i in 0..nodes.len() {
        for j in i + 1..nodes.len() {
            let ((xi, yi), (wi, hi)) = (at[i], nodes[i].size);
            let ((xj, yj), (wj, hj)) = (at[j], nodes[j].size);
            let apart = (xi - xj).abs() >= f64::midpoint(wi, wj)
                || (yi - yj).abs() >= f64::midpoint(hi, hj);
            assert!(apart, "{} and {} overlap", names[i], names[j]);
        }
    }
}

#[test]
fn nodes_go_round_in_the_order_of_their_ideal_angles() {
    use std::f64::consts::{FRAC_PI_2, PI};
    // Right, up and left, given in that order; y grows downward, so "up" is −π/2.
    let nodes = [
        node(5.0, Some(0.0)),
        node(5.0, Some(-FRAC_PI_2)),
        node(5.0, Some(PI)),
    ];
    let [right, up, left] = place(&nodes, (50.0, 20.0), 15.0)[..] else {
        unreachable!("three nodes")
    };
    assert_eq!(
        up,
        (50.0 + 15.0 * fmath::cos(-FRAC_PI_2), 5.0),
        "the first sits at its angle"
    );
    assert!(
        up.1 < right.1 && up.1 < left.1,
        "up {up:?}, right {right:?}, left {left:?}"
    );
    assert!(right.0 > left.0, "right {right:?}, left {left:?}");
}

#[test]
fn nodes_without_an_ideal_angle_follow_the_rest_in_input_order() {
    let nodes = [node(3.0, None), node(3.0, Some(1.0)), node(3.0, None)];
    let at = place(&nodes, (0.0, 0.0), 10.0);
    let angle = |p: (f64, f64)| fmath::atan2(p.1, p.0);
    let step = std::f64::consts::TAU / 3.0;
    assert!((angle(at[1]) - 1.0).abs() < 1e-12);
    let wrapped = |a: f64| fmath::atan2(fmath::sin(a), fmath::cos(a));
    assert!((angle(at[0]) - wrapped(1.0 + step)).abs() < 1e-12);
    assert!((angle(at[2]) - wrapped(1.0 + 2.0 * step)).abs() < 1e-12);
}

// ── Radius ──────────────────────────────────────────────────────────────────

#[test]
fn the_ring_clears_the_core() {
    let core = [((10.0, 0.0), (8.0, 2.0)), ((-4.0, 3.0), (20.0, 6.0))];
    let center = (0.0, 0.0);
    let r = radius(center, &core, &plain(2));
    for &((x, y), (w, h)) in &core {
        let far = (x * x + y * y).sqrt() + (w * w + h * h).sqrt() / 2.0;
        assert!(r > far, "radius {r} inside the corner at {far}");
    }
    assert_eq!(
        r,
        (5.0 + 436.0f64.sqrt() / 2.0 + 3.0) * 1.1,
        "the second box's corner is the farther: 5 to its center, √436 / 2 on"
    );
}

#[test]
fn a_ring_with_no_core_is_long_enough_for_its_nodes() {
    let nodes: Vec<RingNode> = (0..40).map(|_| node(10.0, None)).collect();
    let r = radius((0.0, 0.0), &[], &nodes);
    assert_eq!(r, 40.0 * 12.0 / std::f64::consts::TAU);
    assert_eq!(
        radius((0.0, 0.0), &[], &[]),
        3.0 * 1.1,
        "and 3.3 with nothing at all"
    );
}

// ── Centroid (the seed's tests) ─────────────────────────────────────────────

#[test]
fn the_centroid_of_nothing_is_the_origin() {
    assert_eq!(centroid(&[]), (0.0, 0.0));
}

#[test]
fn the_centroid_of_one_point_is_the_point() {
    assert_eq!(centroid(&[(13.0, 6.0)]), (13.0, 6.0));
}

#[test]
fn the_centroid_resists_outlier_skew() {
    // Two points together and one far off: the box's middle is 55, the centroid nearer 40.
    let (cx, cy) = centroid(&[(12.0, 10.5), (14.0, 10.5), (102.0, 10.5)]);
    assert!(
        cx < 55.0,
        "centroid x={cx:.1} should be under the box middle, 55"
    );
    assert_eq!(cy, 10.5);
}
