// `allow`, not `expect`: clippy 1.99's `float_cmp` no longer fires here and earlier releases do.
#![allow(
    clippy::float_cmp,
    reason = "positions that must not move, and runs that must repeat, are compared exactly"
)]

use super::*;

/// A body `width` columns wide and one row (two world units) tall.
fn body(width: f64) -> Body {
    Body {
        size: (width, 2.0),
        weight: 1.0,
        degree: 0,
        pin: None,
        previous: None,
    }
}

fn spring(a: usize, b: usize) -> Spring {
    Spring {
        ends: (a, b),
        weight: 1.0,
    }
}

const AREA: (f64, f64) = (80.0, 48.0);

fn run(params: &ForceParams, bodies: &[Body], springs: &[Spring]) -> Vec<(f64, f64)> {
    let widths: Vec<f64> = bodies.iter().map(|b| b.size.0).collect();
    let k = ideal_distance(params, AREA, &widths);
    simulate(params, bodies, springs, k, AREA)
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    (dx * dx + dy * dy).sqrt()
}

/// Twelve bodies in two triangles joined by a path, plus a self-loop and a parallel edge.
fn sample() -> (Vec<Body>, Vec<Spring>) {
    let bodies = (0..12).map(|i| body(4.0 + f64::from(i % 5))).collect();
    let springs = [
        (0, 1),
        (1, 2),
        (2, 0),
        (2, 3),
        (3, 4),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 5),
        (8, 9),
        (9, 10),
        (10, 11),
        (11, 11),
        (0, 1),
    ]
    .map(|(a, b)| spring(a, b))
    .to_vec();
    (bodies, springs)
}

#[test]
fn an_empty_run_places_nothing() {
    assert_eq!(run(&ForceParams::default(), &[], &[]), []);
}

#[test]
fn a_lone_body_rests_where_it_starts() {
    let at = run(&ForceParams::default(), &[body(6.0)], &[]);
    assert_eq!(at, [(6.0, 0.0)], "the one-body circle: radius √1 × width");
}

#[test]
fn connected_bodies_settle_closer_than_unconnected_ones() {
    // The seed's version could only check that positions existed: its layout was not
    // reproducible enough to assert this.
    let at = run(
        &ForceParams::default(),
        &[body(3.0), body(3.0), body(3.0)],
        &[spring(0, 1)],
    );
    assert!(
        distance(at[0], at[1]) < distance(at[0], at[2]),
        "joined {} apart, unjoined {}",
        distance(at[0], at[1]),
        distance(at[0], at[2])
    );
}

#[test]
fn iterations_are_honored_as_given() {
    // Ledger row 11: the seed floored every cold run to 100 steps.
    let (bodies, springs) = sample();
    let steps = |iterations| {
        let params = ForceParams {
            iterations,
            converge_eps: 0.0,
            ..ForceParams::default()
        };
        run(&params, &bodies, &springs)
    };
    let start = steps(0);
    assert_eq!(
        start,
        start_positions(&bodies, &springs, 0.0),
        "no step at all"
    );
    assert_ne!(steps(1), start);
    assert_ne!(steps(1), steps(2));
    assert_ne!(steps(30), steps(100));
}

#[test]
fn the_starting_circle_is_round() {
    // Plan §6: no placement-time aspect squash, which the seed applied as `0.5 * sin`.
    let bodies = [body(5.0); 4];
    let at = start_positions(&bodies, &[], 1.0);
    let radius = 2.0 * 5.0;
    let expected = [(radius, 0.0), (0.0, radius), (-radius, 0.0), (0.0, -radius)];
    for (got, want) in at.iter().zip(expected) {
        assert!(distance(*got, want) < 1e-9, "{got:?} vs {want:?}");
    }
}

#[test]
fn a_pinned_body_never_moves_but_still_pushes() {
    let mut pinned = body(4.0);
    pinned.pin = Some((3.0, -7.0));
    let free = run(&ForceParams::default(), &[body(4.0), body(4.0)], &[]);
    let at = run(&ForceParams::default(), &[pinned, body(4.0)], &[]);
    assert_eq!(at[0], (3.0, -7.0));
    assert_ne!(at[1], free[1], "the pinned body still repels");
}

#[test]
fn degree_scaled_repulsion_weights_each_pair_by_both_masses() {
    let mut hub = body(4.0);
    hub.degree = 3;
    hub.previous = Some((10.0, 0.0));
    let mut leaf = body(4.0);
    leaf.previous = Some((0.0, 0.0));
    leaf.weight = 2.0;
    let bodies = [leaf, hub];
    let push = |repulsion| {
        let params = ForceParams {
            repulsion,
            gravity: 0.0,
            ..ForceParams::default()
        };
        let mut run = Run::new(&params, &bodies, &[], 5.0, AREA);
        run.forces(0);
        run.displacements.clone()
    };
    let unit = 5.0 * 5.0 / 10.0;
    assert_eq!(
        push(Repulsion::Uniform),
        [(-unit * 2.0, 0.0), (unit * 2.0, 0.0)],
        "weights multiply: 2 × 1"
    );
    assert_eq!(
        push(Repulsion::DegreeScaled),
        [(-unit * 2.0 * 4.0, 0.0), (unit * 2.0 * 4.0, 0.0)],
        "and degree scaling makes the hub's mass 1 × (3 + 1)"
    );
}

#[test]
fn barnes_hut_at_theta_zero_matches_the_exact_sum() {
    let (bodies, springs) = sample();
    let five = |bh_threshold, theta| ForceParams {
        iterations: 5,
        bh_threshold,
        theta,
        ..ForceParams::default()
    };
    let exact = run(&five(bodies.len(), 0.8), &bodies, &springs);
    let tree = run(&five(0, 0.0), &bodies, &springs);
    assert_eq!(exact.len(), tree.len());
    for (e, t) in exact.iter().zip(&tree) {
        assert!(distance(*e, *t) < 1e-6, "exact {e:?}, tree {t:?}");
    }
    let approximate = five(0, 0.8);
    assert_ne!(
        run(&approximate, &bodies, &springs),
        exact,
        "θ = 0.8 approximates"
    );
}

#[test]
fn a_run_repeats_exactly() {
    let (bodies, springs) = sample();
    let params = ForceParams::default();
    assert_eq!(
        run(&params, &bodies, &springs),
        run(&params, &bodies, &springs)
    );
}

#[test]
fn a_converged_run_stops_early() {
    let (bodies, springs) = sample();
    let once = ForceParams {
        iterations: 1,
        ..ForceParams::default()
    };
    let eager = ForceParams {
        converge_eps: f64::MAX,
        ..ForceParams::default()
    };
    assert_eq!(
        run(&eager, &bodies, &springs),
        run(&once, &bodies, &springs)
    );
}

#[test]
fn a_new_body_in_a_warm_run_starts_beside_its_placed_neighbors_and_ramps_in() {
    let mut a = body(4.0);
    a.previous = Some((0.0, 0.0));
    let mut b = body(4.0);
    b.previous = Some((20.0, 0.0));
    let bodies = [a, b, body(4.0)];
    let springs = [spring(0, 2), spring(2, 1)];
    let k = 8.0;
    let at = start_positions(&bodies, &springs, k);
    let turn = 2.0 * GOLDEN_ANGLE;
    let want = (10.0 + 4.0 * fmath::cos(turn), 4.0 * fmath::sin(turn));
    assert!(distance(at[2], want) < 1e-9, "{:?} vs {want:?}", at[2]);
    assert_eq!((at[0], at[1]), ((0.0, 0.0), (20.0, 0.0)));

    let params = ForceParams {
        gravity: 0.0,
        ..ForceParams::default()
    };
    let mut run = Run::new(&params, &bodies, &[], k, AREA);
    let pushed = |run: &mut Run<'_>, step| {
        run.forces(step);
        run.displacements[0].0
    };
    let first = pushed(&mut run, 0);
    let full = pushed(&mut run, 9);
    let (on_a_from_b, _) = repulsion(-20.0, 0.0, k);
    let from_new = full - on_a_from_b;
    assert!(
        (first - (on_a_from_b + from_new / 10.0)).abs() < 1e-9,
        "at step 0 the new body pushes with a tenth of its force"
    );
    assert_eq!(pushed(&mut run, 50), full, "and all of it from step 9");
}

#[test]
fn a_survivor_in_a_warm_run_anneals_from_converge_eps_while_a_newcomer_moves_freely() {
    let mut a = body(4.0);
    a.previous = Some((0.0, 0.0));
    let mut b = body(4.0);
    b.previous = Some((1.0, 0.0));
    let bodies = [a, b, body(4.0)];
    let params = ForceParams::default();
    let springs = [spring(0, 2)];
    let mut run = Run::new(&params, &bodies, &springs, 8.0, AREA);
    let start = run.positions.clone();
    run.step(0);
    let first: Vec<f64> = start
        .iter()
        .zip(&run.positions)
        .map(|(&s, &p)| distance(s, p))
        .collect();
    assert!(
        (first[0] - params.converge_eps).abs() < 1e-9
            && (first[1] - params.converge_eps).abs() < 1e-9,
        "two survivors a unit apart push hard but step converge_eps: {first:?}"
    );
    assert!(
        first[2] > 1.0,
        "the newcomer takes the warm temperature: {first:?}"
    );
    let before = run.positions.clone();
    run.step(1);
    let second = distance(before[0], run.positions[0]);
    assert!(
        (second - params.converge_eps * params.cooling).abs() < 1e-9,
        "and the survivors' cap cools with the run: {second}"
    );
}

#[test]
fn hostile_parameters_still_give_finite_positions() {
    let (bodies, springs) = sample();
    let params = ForceParams {
        cooling: f64::NAN,
        gravity: f64::INFINITY,
        theta: -1.0,
        converge_eps: f64::NAN,
        bh_threshold: 0,
        ..ForceParams::default()
    };
    for at in run(&params, &bodies, &springs) {
        assert!(at.0.is_finite() && at.1.is_finite(), "{at:?}");
    }
}
