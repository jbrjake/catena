// `allow`, not `expect`: clippy 1.99's `float_cmp` no longer fires here and earlier releases do.
#![allow(
    clippy::float_cmp,
    reason = "exact zeros, exact body counts and exact centres are the claims under test"
)]

use super::*;

/// The oracle: the plain all-pairs sum of the same FR kernel (`k² / d`, `d` floored at 0.01),
/// skipping points within `EPSILON` of the query (itself, or coincident with it).
fn brute_force(points: &[(f64, f64)], x: f64, y: f64, k: f64) -> (f64, f64) {
    let (mut fx, mut fy) = (0.0, 0.0);
    for &(px, py) in points {
        let (dx, dy) = (x - px, y - py);
        let dist_sq = dx * dx + dy * dy;
        if dist_sq < EPSILON * EPSILON {
            continue;
        }
        let dist = dist_sq.sqrt().max(0.01);
        let force = k * k / dist;
        fx += dx / dist * force;
        fy += dy / dist * force;
    }
    (fx, fy)
}

fn tree_of(points: &[(f64, f64)]) -> QuadTree {
    let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
    for &(x, y) in points {
        tree.insert(x, y);
    }
    tree
}

fn deepest_leaf(tree: &QuadTree, depth: u32) -> u32 {
    match &tree.children {
        None => depth,
        Some(children) => children
            .iter()
            .map(|c| deepest_leaf(c, depth + 1))
            .max()
            .unwrap_or(depth),
    }
}

/// Whether every body's routing point lies inside the leaf that holds it.
fn routes_inside_their_leaves(tree: &QuadTree) -> bool {
    match &tree.children {
        Some(children) => children.iter().all(routes_inside_their_leaves),
        None => tree.body.iter().chain(&tree.bucket).all(|b| {
            let (x, y) = b.route;
            let bounds = &tree.bounds;
            (bounds.x_min..=bounds.x_max).contains(&x) && (bounds.y_min..=bounds.y_max).contains(&y)
        }),
    }
}

fn assert_close(got: (f64, f64), want: (f64, f64), what: &str) {
    let tolerance = 1e-9 * want.0.abs().max(want.1.abs()).max(1.0);
    assert!(
        (got.0 - want.0).abs() <= tolerance && (got.1 - want.1).abs() <= tolerance,
        "{what}: tree {got:?}, brute force {want:?}"
    );
}

#[test]
fn center_of_mass_single_body() {
    let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
    tree.insert(25.0, 75.0);
    assert_eq!(tree.total_mass, 1.0);
    assert!((tree.cx - 25.0).abs() < 1e-10);
    assert!((tree.cy - 75.0).abs() < 1e-10);
}

#[test]
fn center_of_mass_two_bodies() {
    let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
    tree.insert(20.0, 40.0);
    tree.insert(80.0, 60.0);
    assert_eq!(tree.total_mass, 2.0);
    assert!((tree.cx - 50.0).abs() < 1e-10);
    assert!((tree.cy - 50.0).abs() < 1e-10);
}

#[test]
fn center_of_mass_three_bodies() {
    let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
    tree.insert(10.0, 10.0);
    tree.insert(20.0, 20.0);
    tree.insert(30.0, 30.0);
    assert_eq!(tree.total_mass, 3.0);
    assert!((tree.cx - 20.0).abs() < 1e-10);
    assert!((tree.cy - 20.0).abs() < 1e-10);
}

#[test]
fn force_pushes_bodies_apart() {
    let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
    tree.insert(40.0, 50.0);
    tree.insert(60.0, 50.0);

    let k = 10.0;
    let theta = 0.0; // exact calculation

    // Force on a body at (40, 50) should push it leftward (away from 60, 50).
    let (fx, fy) = tree.compute_force(40.0, 50.0, theta, k);
    assert!(fx < 0.0, "body at x=40 should be pushed left, got fx={fx}");
    assert!(fy.abs() < 1e-6, "no vertical force expected, got fy={fy}");

    // Force on a body at (60, 50) should push it rightward.
    let (fx, fy) = tree.compute_force(60.0, 50.0, theta, k);
    assert!(fx > 0.0, "body at x=60 should be pushed right, got fx={fx}");
    assert!(fy.abs() < 1e-6, "no vertical force expected, got fy={fy}");
}

#[test]
fn force_magnitude_decreases_with_distance() {
    let mut tree = QuadTree::new(0.0, 0.0, 200.0, 200.0);
    tree.insert(100.0, 100.0);

    let k = 10.0;
    let theta = 0.0;

    let (fx_near, _) = tree.compute_force(110.0, 100.0, theta, k);
    let (fx_far, _) = tree.compute_force(150.0, 100.0, theta, k);
    assert!(
        fx_near.abs() > fx_far.abs(),
        "near force ({}) should exceed far force ({})",
        fx_near.abs(),
        fx_far.abs()
    );
}

#[test]
fn identical_positions_terminate_and_feel_no_force() {
    let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
    // Insert multiple bodies at the exact same position.
    for _ in 0..10 {
        tree.insert(50.0, 50.0);
    }
    assert_eq!(tree.total_mass, 10.0);
    // Coincident bodies have no direction to push each other in, exactly as brute force says;
    // the seed reported a finite but nonzero push from their jittered copies.
    assert_eq!(tree.compute_force(50.0, 50.0, 0.8, 10.0), (0.0, 0.0));
    assert_eq!(tree.cx, 50.0, "the centre of mass ignores routing nudges");
    assert_eq!(tree.cy, 50.0);
}

#[test]
fn theta_approximation_reasonable() {
    // Compare exact (theta=0) vs approximate (theta=0.8) for a distant cluster.
    let mut tree = QuadTree::new(0.0, 0.0, 1000.0, 1000.0);
    for i in 0..50 {
        let offset = f64::from(i) * 2.0;
        tree.insert(100.0 + offset, 100.0 + offset);
    }

    let k = 20.0;
    let query = (800.0, 800.0);

    let (ex, ey) = tree.compute_force(query.0, query.1, 0.0, k);
    let (ax, ay) = tree.compute_force(query.0, query.1, 0.8, k);

    // Approximation should have the same sign and similar magnitude.
    assert!(
        ex.signum() == ax.signum(),
        "force x signs differ: exact={ex}, approx={ax}"
    );
    assert!(
        ey.signum() == ay.signum(),
        "force y signs differ: exact={ey}, approx={ay}"
    );

    let exact_mag = (ex * ex + ey * ey).sqrt();
    let approx_mag = (ax * ax + ay * ay).sqrt();
    let ratio = approx_mag / exact_mag;
    // Tightened from the seed's (0.5, 2) to plan §16.2-M's band; the seed's own ratio here was
    // 0.998.
    assert!(
        ratio > 0.7 && ratio < 1.4,
        "approximation ratio {ratio:.2} out of range (exact={exact_mag:.2}, approx={approx_mag:.2})"
    );
}

#[test]
fn empty_tree_zero_force() {
    let tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
    let (fx, fy) = tree.compute_force(50.0, 50.0, 0.8, 10.0);
    assert_eq!(fx, 0.0);
    assert_eq!(fy, 0.0);
}

// ── Port regressions (plan §8.1, §14 row 12) ───────────────────────────────

#[test]
fn a_max_depth_bucket_sums_its_bodies_exactly() {
    // Two points past the root's east edge route to the same child at every level, so they
    // never separate and land in one MAX_DEPTH leaf. The seed treated that leaf as one mass at
    // its centroid, which includes the query point itself: -40 here, against brute force's -10.
    let points = [(150.0, 50.0), (160.0, 50.0)];
    let tree = tree_of(&points);
    assert_eq!(deepest_leaf(&tree, 0), MAX_DEPTH);
    let got = tree.compute_force(150.0, 50.0, 0.0, 10.0);
    assert_close(got, brute_force(&points, 150.0, 50.0, 10.0), "bucket");
    assert_eq!(got, (-10.0, 0.0));
}

#[test]
fn a_coincident_pair_exerts_no_force_on_itself() {
    // The seed's jitter moved the stored copy √2·1e-4 away, outside the 1e-4 self-skip radius,
    // so each point of the pair was repelled by its own copy with force k².
    let points = [(50.0, 50.0), (50.0, 50.0)];
    let tree = tree_of(&points);
    assert_eq!(tree.compute_force(50.0, 50.0, 0.0, 10.0), (0.0, 0.0));
    assert_eq!(brute_force(&points, 50.0, 50.0, 10.0), (0.0, 0.0));
}

#[test]
fn a_coincident_pair_on_the_max_edge_stays_in_its_leaf_and_separates_promptly() {
    // The seed nudged by +1e-4 unconditionally, out of the cell, and the pair nested 54 levels
    // deep. Nudging inward splits it once a cell is about 2e-4 wide: depth 19 or 20 here.
    let tree = tree_of(&[(100.0, 100.0), (100.0, 100.0)]);
    assert!(routes_inside_their_leaves(&tree));
    let depth = deepest_leaf(&tree, 0);
    assert!(
        depth <= 24,
        "a coincident corner pair nested {depth} levels deep"
    );
}

/// Forty points of a 2-D Weyl sequence in the root's 100×100 bounds, plus exact duplicates,
/// near-duplicates inside the coincidence radius, a pair past the east edge, and a pair past
/// both edges (which lands in a `MAX_DEPTH` bucket). No RNG: the core has none (plan §8.1).
fn awkward_points() -> Vec<(f64, f64)> {
    let mut points: Vec<(f64, f64)> = (1..=40)
        .map(|i| {
            let i = f64::from(i);
            (
                (i * 0.618_033_988_7).fract() * 100.0,
                (i * 0.754_877_666_2).fract() * 100.0,
            )
        })
        .collect();
    for i in [3, 17, 29] {
        points.push(points[i]);
    }
    for i in [5, 22] {
        let (x, y) = points[i];
        points.push((x + 5e-5, y));
    }
    points.extend([(130.0, 50.0), (135.0, 52.0), (150.0, 200.0), (160.0, 200.0)]);
    points
}

#[test]
fn theta_zero_matches_brute_force_on_awkward_inputs() {
    let points = awkward_points();
    assert_eq!(points.len(), 49);
    let tree = tree_of(&points);
    assert_eq!(tree.total_mass, 49.0);
    assert_eq!(
        deepest_leaf(&tree, 0),
        MAX_DEPTH,
        "the far pair exercises the bucket"
    );
    for (i, &(x, y)) in points.iter().enumerate() {
        let want = brute_force(&points, x, y, 7.0);
        let got = tree.compute_force(x, y, 0.0, 7.0);
        assert_close(got, want, &format!("point {i} at ({x}, {y})"));
    }
}
