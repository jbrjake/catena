use super::*;

/// De Casteljau's construction: repeated linear interpolation of the control polygon. It shares
/// no arithmetic with `Bezier::point`'s Bernstein form, which makes it the oracle for it.
fn de_casteljau(control: &[(f64, f64)], t: f64) -> (f64, f64) {
    let mut pts = control.to_vec();
    while pts.len() > 1 {
        pts = pts
            .windows(2)
            .map(|w| {
                (
                    w[0].0 + (w[1].0 - w[0].0) * t,
                    w[0].1 + (w[1].1 - w[0].1) * t,
                )
            })
            .collect();
    }
    pts[0]
}

fn control_points(piece: &Bezier) -> Vec<(f64, f64)> {
    match *piece {
        Bezier::Quadratic { from, ctrl, to } => vec![from, ctrl, to],
        Bezier::Cubic {
            from,
            ctrl0,
            ctrl1,
            to,
        } => vec![from, ctrl0, ctrl1, to],
    }
}

fn near(a: (f64, f64), b: (f64, f64), tol: f64) -> bool {
    (a.0 - b.0).abs() <= tol && (a.1 - b.1).abs() <= tol
}

fn quad(from: (f64, f64), ctrl: (f64, f64), to: (f64, f64)) -> Bezier {
    Bezier::Quadratic { from, ctrl, to }
}

fn cubic(from: (f64, f64), ctrl0: (f64, f64), ctrl1: (f64, f64), to: (f64, f64)) -> Bezier {
    Bezier::Cubic {
        from,
        ctrl0,
        ctrl1,
        to,
    }
}

fn polyline(chain: &[Bezier]) -> Vec<(f64, f64)> {
    let mut out = vec![(-1.0, -1.0)]; // stale content `tessellate` must replace
    tessellate(chain, &mut out);
    out
}

#[test]
fn density_is_one_sample_per_two_sub_pixels_of_chord_clamped_to_10_and_200() {
    // (chord length, expected vertex count = steps + 1)
    let cases = [
        (0.0, 11),
        (5.0, 11),
        (21.9, 11),
        (22.0, 12),
        (25.0, 13),
        (399.0, 200),
        (400.0, 201),
        (1.0e6, 201),
    ];
    for (len, vertices) in cases {
        let pts = polyline(&[quad((0.0, 0.0), (len / 2.0, 7.0), (len, 0.0))]);
        assert_eq!(pts.len(), vertices, "chord {len}");
        let pts = polyline(&[cubic((0.0, 0.0), (3.0, 9.0), (-4.0, 2.0), (0.0, len))]);
        assert_eq!(pts.len(), vertices, "cubic chord {len}");
    }
}

#[test]
fn vertices_are_the_curve_at_evenly_spaced_parameters() {
    let pieces = [
        quad((0.0, 0.0), (0.0, 23.0), (23.0, 23.0)),
        quad((-6.5, 10.25), (12.0, -8.0), (30.0, 10.0)),
        cubic((0.0, 0.0), (40.0, 0.0), (0.0, 40.0), (40.0, 40.0)),
        cubic((3.0, 90.0), (-50.0, 10.0), (80.0, -20.0), (100.0, 100.0)),
    ];
    for piece in pieces {
        let pts = polyline(&[piece]);
        let steps = pts.len() - 1;
        assert!(steps >= 10, "{piece:?}");
        for (i, &p) in pts.iter().enumerate() {
            #[expect(clippy::cast_precision_loss, reason = "at most 201 vertices")]
            let t = i as f64 / steps as f64;
            let expected = de_casteljau(&control_points(&piece), t);
            assert!(
                near(p, expected, 1e-9),
                "{piece:?} at t={t}: {p:?} vs {expected:?}"
            );
        }
    }
}

#[test]
fn end_points_are_exact() {
    let pieces = [
        quad((0.1, -7.3), (55.5, 1e3), (-12.25, 19.0)),
        cubic((1e5, -1e5), (0.3, 0.7), (-9.0, 4.4), (17.125, 3.0)),
    ];
    for piece in pieces {
        let pts = polyline(&[piece]);
        assert_eq!(pts.first(), Some(&piece.start()));
        assert_eq!(pts.last(), Some(&piece.end()));
        assert_eq!(piece.point(0.0), piece.start());
        assert_eq!(piece.point(1.0), piece.end());
    }
}

#[test]
fn a_chain_shares_its_joints_and_bridges_its_gaps() {
    let a = quad((0.0, 0.0), (10.0, 20.0), (30.0, 0.0));
    let b = cubic((30.0, 0.0), (40.0, -10.0), (50.0, 10.0), (60.0, 0.0));
    let (na, nb) = (polyline(&[a]).len(), polyline(&[b]).len());
    let joined = polyline(&[a, b]);
    assert_eq!(joined.len(), na + nb - 1, "the joint appears once");
    assert_eq!(joined[na - 1], (30.0, 0.0));
    assert_eq!(
        joined[na],
        b.point(1.0 / f64::from(u32::try_from(nb - 1).unwrap()))
    );

    let c = quad((70.0, 5.0), (75.0, 0.0), (80.0, 5.0));
    let nc = polyline(&[c]).len();
    let gapped = polyline(&[a, c]);
    assert_eq!(gapped.len(), na + nc, "a gap keeps both end points");
    assert_eq!(gapped[na - 1], (30.0, 0.0));
    assert_eq!(gapped[na], (70.0, 5.0));
}

#[test]
fn an_empty_chain_gives_an_empty_polyline() {
    assert_eq!(polyline(&[]), [] as [(f64, f64); 0]);
}

#[test]
fn derivatives_match_central_differences_and_the_control_arms_at_the_ends() {
    let pieces = [
        quad((0.0, 0.0), (10.0, 30.0), (40.0, 5.0)),
        cubic((0.0, 0.0), (40.0, 0.0), (0.0, 40.0), (40.0, 40.0)),
    ];
    let h = 1e-6;
    for piece in pieces {
        for i in 1..10 {
            let t = f64::from(i) / 10.0;
            let (a, b) = (piece.point(t - h), piece.point(t + h));
            let numeric = ((b.0 - a.0) / (2.0 * h), (b.1 - a.1) / (2.0 * h));
            assert!(near(piece.derivative(t), numeric, 1e-4), "{piece:?} at {t}");
        }
    }
    // B′(0) and B′(1) are the end arms times the degree.
    assert_eq!(pieces[0].derivative(0.0), (20.0, 60.0));
    assert_eq!(pieces[0].derivative(1.0), (60.0, -50.0));
    assert_eq!(pieces[1].derivative(0.0), (120.0, 0.0));
    assert_eq!(pieces[1].derivative(1.0), (120.0, 0.0));
}

#[test]
fn non_finite_input_tessellates_without_panicking() {
    // A NaN chord saturates to the 10-step floor, an infinite one to the 200-step ceiling.
    for (bad, vertices) in [
        (f64::NAN, 11),
        (f64::INFINITY, 201),
        (f64::NEG_INFINITY, 201),
    ] {
        let pts = polyline(&[quad((0.0, 0.0), (bad, 1.0), (bad, 2.0))]);
        assert_eq!(pts.len(), vertices, "{bad}");
        let pts = polyline(&[cubic((bad, 0.0), (0.0, 0.0), (1.0, 1.0), (f64::MAX, 0.0))]);
        assert_eq!(pts.len(), vertices, "{bad}");
    }
    let pts = polyline(&[quad((-f64::MAX, 0.0), (0.0, 0.0), (f64::MAX, 0.0))]);
    assert_eq!(pts.len(), 201, "a chord that overflows to infinity");
}
