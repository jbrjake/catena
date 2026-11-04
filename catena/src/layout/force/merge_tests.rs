//! The pass that pulls joined islands together, on bodies alone.

use super::*;

fn body(ideal: f64, previous: Option<(f64, f64)>) -> Body {
    Body {
        size: (3.0, 2.0),
        weight: 1.0,
        degree: 1,
        ideal,
        pin: None,
        previous,
        tether: 0.0,
    }
}

fn spring(a: usize, b: usize) -> Spring {
    Spring {
        ends: (a, b),
        weight: 1.0,
    }
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    (dx * dx + dy * dy).sqrt()
}

/// Island 0: a 3-body chain at x = 0..20; island 1: a 2-body chain far to the right, joined
/// through its far end (body 4) by a new body 5 to island 0's right end (body 2).
fn far_join() -> (Vec<Body>, Vec<Spring>, Vec<Option<usize>>) {
    let bodies = vec![
        body(10.0, Some((0.0, 0.0))),
        body(10.0, Some((10.0, 0.0))),
        body(10.0, Some((20.0, 0.0))),
        body(10.0, Some((80.0, 0.0))),
        body(10.0, Some((90.0, 0.0))),
        body(10.0, None),
    ];
    let springs = vec![
        spring(0, 1),
        spring(1, 2),
        spring(3, 4),
        spring(5, 2),
        spring(5, 4),
    ];
    let islands = vec![Some(0), Some(0), Some(0), Some(1), Some(1), None];
    (bodies, springs, islands)
}

const AREA: (f64, f64) = (80.0, 48.0);

#[test]
fn the_smaller_island_turns_and_slides_until_its_joining_edge_rests() {
    let (bodies, springs, islands) = far_join();
    let pinned = vec![false; bodies.len()];
    let moved = pull_together(
        &ForceParams::default(),
        &bodies,
        &springs,
        &islands,
        &pinned,
        AREA,
    );
    assert_eq!(moved.len(), 6);
    for i in 0..3 {
        assert_eq!(
            Some(moved[i]),
            bodies[i].previous,
            "body {i} of the larger island moved"
        );
    }
    let shape = distance(moved[3], moved[4]);
    assert!(
        (shape - 10.0).abs() < 1e-9,
        "the smaller island's edge is now {shape}"
    );
    for (a, b) in [(5, 2), (5, 4)] {
        let length = distance(moved[a], moved[b]);
        assert!(
            length <= 1.5 * 10.0,
            "the joining edge {a}–{b} ends {length:.2} long, its rest length 10"
        );
    }
    // Body 4, joined, now faces island 0; body 3, unjoined, turned away.
    assert!(
        distance(moved[4], moved[2]) < distance(moved[3], moved[2]),
        "the joined end did not turn toward the larger island"
    );
}

#[test]
fn an_island_with_a_pinned_body_stays_put_even_when_smaller() {
    let (bodies, springs, islands) = far_join();
    let mut pinned = vec![false; bodies.len()];
    pinned[3] = true;
    let moved = pull_together(
        &ForceParams::default(),
        &bodies,
        &springs,
        &islands,
        &pinned,
        AREA,
    );
    for i in 0..5 {
        assert_eq!(Some(moved[i]), bodies[i].previous, "body {i} moved");
    }
}

#[test]
fn one_island_and_new_bodies_move_nothing_placed() {
    let (bodies, springs, mut islands) = far_join();
    for island in islands.iter_mut().flatten() {
        *island = 0;
    }
    let pinned = vec![false; bodies.len()];
    let moved = pull_together(
        &ForceParams::default(),
        &bodies,
        &springs,
        &islands,
        &pinned,
        AREA,
    );
    assert_eq!(moved, start_positions(&bodies, &springs));
}

#[test]
fn islands_that_meet_push_only_within_contact() {
    // Two single-body islands joined by one spring, starting far apart: the spring pulls them
    // in, and only within `CONTACT` ideal distances does the push resist.
    let bodies = vec![body(10.0, Some((0.0, 0.0))), body(10.0, Some((300.0, 0.0)))];
    let springs = vec![spring(0, 1)];
    let islands = vec![Some(0), Some(1)];
    let moved = pull_together(
        &ForceParams::default(),
        &bodies,
        &springs,
        &islands,
        &[false, false],
        AREA,
    );
    assert_eq!(
        moved[0],
        (0.0, 0.0),
        "the first-listed of equal islands stays"
    );
    let gap = distance(moved[0], moved[1]);
    assert!(
        gap > 0.5 * 10.0 && gap <= CONTACT * 10.0,
        "the joined bodies end {gap:.2} apart"
    );
}
