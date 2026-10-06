// `allow`, not `expect`: clippy 1.99's `float_cmp` no longer fires here and earlier releases do.
#![allow(
    clippy::float_cmp,
    reason = "positions that must not move, and runs that must repeat, are compared exactly"
)]

use super::*;

/// A body `width` columns wide and one row (two world units) tall, with the ideal distance
/// a layout in `AREA` of 12 such bodies would give it.
fn body(width: f64) -> Body {
    Body {
        size: (width, 2.0),
        weight: 1.0,
        degree: 0,
        ideal: ideal_distance(&ForceParams::default(), base_distance(AREA, 12), width),
        pin: None,
        previous: None,
        tether: 0.0,
    }
}

/// A body with ideal distance `ideal`, placed at `at` by a previous layout.
fn placed(ideal: f64, at: (f64, f64)) -> Body {
    Body {
        ideal,
        previous: Some(at),
        ..body(4.0)
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
    simulate(params, bodies, springs, AREA, Schedule::Cold)
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
    assert_eq!(start, start_positions(&bodies, &springs), "no step at all");
    assert_ne!(steps(1), start);
    assert_ne!(steps(1), steps(2));
    assert_ne!(steps(30), steps(100));
}

#[test]
fn the_starting_circle_is_round() {
    // Plan §6: no placement-time aspect squash, which the seed applied as `0.5 * sin`.
    let bodies = [body(5.0); 4];
    let at = start_positions(&bodies, &[]);
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
    let mut hub = placed(5.0, (10.0, 0.0));
    hub.degree = 3;
    let mut leaf = placed(5.0, (0.0, 0.0));
    leaf.weight = 2.0;
    let bodies = [leaf, hub];
    let push = |repulsion| {
        let params = ForceParams {
            repulsion,
            gravity: 0.0,
            ..ForceParams::default()
        };
        let mut run = Run::new(&params, &bodies, &[], AREA, Schedule::Warm);
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
    let k = 8.0;
    let bodies = [
        placed(k, (0.0, 0.0)),
        placed(k, (20.0, 0.0)),
        Body {
            ideal: k,
            ..body(4.0)
        },
    ];
    let springs = [spring(0, 2), spring(2, 1)];
    let at = start_positions(&bodies, &springs);
    let turn = 2.0 * GOLDEN_ANGLE;
    let want = (10.0 + 4.0 * fmath::cos(turn), 4.0 * fmath::sin(turn));
    assert!(distance(at[2], want) < 1e-9, "{:?} vs {want:?}", at[2]);
    assert_eq!((at[0], at[1]), ((0.0, 0.0), (20.0, 0.0)));

    let params = ForceParams {
        gravity: 0.0,
        ..ForceParams::default()
    };
    let mut run = Run::new(&params, &bodies, &[], AREA, Schedule::Warm);
    let pushed = |run: &mut Run<'_>, step| {
        run.forces(step);
        run.displacements[0].0
    };
    let first = pushed(&mut run, 0);
    let full = pushed(&mut run, 9);
    let on_a_from_b = push(-20.0, 0.0, k).force.0;
    let from_new = full - on_a_from_b;
    assert!(
        (first - (on_a_from_b + from_new / 10.0)).abs() < 1e-9,
        "at step 0 the new body pushes with a tenth of its force"
    );
    assert_eq!(pushed(&mut run, 50), full, "and all of it from step 9");
}

#[test]
fn a_step_is_the_force_over_the_stiffness_capped_by_the_temperature() {
    // One free body on a spring to a pinned one, 12 apart with ideal distance 4: the spring
    // pulls 12² / 4 = 36 and stiffens 2 · 12 / 4 = 6; repulsion pushes 4² / 12 and stiffens
    // 4² / 12². The seed would step the whole 36 (capped at the temperature) and overshoot
    // the anchor by far; here the step is the force over twice the stiffness, the bound that
    // holds when the other end moves too.
    let params = ForceParams {
        gravity: 0.0,
        ..ForceParams::default()
    };
    let mut anchor = placed(4.0, (0.0, 0.0));
    anchor.pin = Some((0.0, 0.0));
    let bodies = [anchor, placed(4.0, (12.0, 0.0))];
    let springs = [spring(0, 1)];
    let mut run = Run::new(&params, &bodies, &springs, AREA, Schedule::Cold);
    run.step(0);
    let force = 36.0 - 16.0 / 12.0;
    let stiffness = 2.0 * (6.0 + 16.0 / 144.0);
    let want = 12.0 - force / stiffness;
    assert!(
        (run.positions[1].0 - want).abs() < 1e-9,
        "{:?}, want x = {want}",
        run.positions[1]
    );

    let hot = ForceParams {
        gravity: 0.0,
        ..ForceParams::default()
    };
    let cramped = (0.5, 0.5);
    let mut run = Run::new(&hot, &bodies, &springs, cramped, Schedule::Cold);
    run.step(0);
    assert!(
        (run.positions[1].0 - (12.0 - 0.25)).abs() < 1e-9,
        "a 0.5 frame starts at temperature 0.25, which caps the step: {:?}",
        run.positions[1]
    );
}

#[test]
fn a_body_whose_forces_balance_stays_put_however_hot_the_run() {
    // The middle of three bodies on a line, springs both ways: every force on it cancels,
    // and a run at a temperature of half the frame leaves it exactly where it was.
    let bodies = [
        placed(4.0, (-6.0, 0.0)),
        placed(4.0, (0.0, 0.0)),
        placed(4.0, (6.0, 0.0)),
    ];
    let springs = [spring(0, 1), spring(1, 2)];
    let at = simulate(
        &ForceParams::default(),
        &bodies,
        &springs,
        AREA,
        Schedule::Cold,
    );
    assert_eq!(at[1], (0.0, 0.0));
    assert!(
        (at[0].0 + at[2].0).abs() < 1e-9,
        "and the ends stay mirrored"
    );
}

#[test]
fn the_temperature_cools_after_a_rise_and_warms_after_five_falls() {
    let params = ForceParams::default();
    let bodies = [body(4.0)];
    let mut run = Run::new(&params, &bodies, &[], AREA, Schedule::Cold);
    let t0 = run.temperature;
    for energy in [50.0, 40.0, 30.0, 20.0] {
        run.adapt(energy);
        assert_eq!(run.temperature, t0, "four falls change nothing");
    }
    run.adapt(10.0);
    assert!(
        (run.temperature - t0 / params.cooling).abs() < 1e-9,
        "the fifth warms"
    );
    run.adapt(10.0);
    assert!((run.temperature - t0).abs() < 1e-9, "no fall cools");
    for energy in [9.0, 8.0, 7.0, 6.0] {
        run.adapt(energy);
    }
    assert!(
        (run.temperature - t0).abs() < 1e-9,
        "a rise resets the count: four falls since"
    );
}

#[test]
fn a_tether_pulls_a_body_back_toward_where_it_was() {
    let params = ForceParams {
        gravity: 0.0,
        ..ForceParams::default()
    };
    let mut tethered = placed(4.0, (3.0, -4.0));
    tethered.tether = 2.0;
    let free = placed(4.0, (3.0, -4.0));
    let lone = |b: Body| {
        let bodies = [b];
        let mut run = Run::new(&params, &bodies, &[], AREA, Schedule::Warm);
        run.positions[0] = (6.0, 0.0);
        run.forces(0);
        (run.displacements[0], run.stiffness[0])
    };
    assert_eq!(lone(tethered), ((-6.0, -8.0), 2.0));
    assert_eq!(lone(free), ((0.0, 0.0), 0.0));
}

#[test]
fn a_pair_rests_at_the_geometric_mean_of_its_ideal_distances() {
    // Plan §8.1 scales one k by the average label width; per body, a pair's attraction and
    // repulsion balance at √(kᵢ kⱼ), so a wide box keeps its neighbors further off.
    let params = ForceParams {
        gravity: 0.0,
        converge_eps: 1e-6,
        iterations: 400,
        ..ForceParams::default()
    };
    let bodies = [placed(4.0, (0.0, 0.0)), placed(16.0, (3.0, 0.0))];
    let at = simulate(&params, &bodies, &[spring(0, 1)], AREA, Schedule::Cold);
    let rest = distance(at[0], at[1]);
    assert!((rest - 8.0).abs() < 1e-3, "rests {rest} apart, not 8");
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
        tether_near: f64::NAN,
        tether_far: f64::INFINITY,
        ..ForceParams::default()
    };
    for at in run(&params, &bodies, &springs) {
        assert!(at.0.is_finite() && at.1.is_finite(), "{at:?}");
    }
}
