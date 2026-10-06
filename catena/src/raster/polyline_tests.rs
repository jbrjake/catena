use super::*;

fn pixels(points: &[(f64, f64)]) -> Vec<(i64, i64)> {
    polyline_pixels(points).collect()
}

fn dashed(points: &[(f64, f64)], on: f64, off: f64) -> Vec<(i64, i64)> {
    let mut lit = Vec::new();
    let mut walk = Walk::measured(points, Vec::new());
    dash(&mut walk, on, off, |p| lit.push((p.x, p.y)));
    lit
}

#[test]
fn an_empty_polyline_lights_nothing_and_one_vertex_lights_one_pixel() {
    assert_eq!(pixels(&[]), []);
    assert_eq!(pixels(&[(2.4, -3.6)]), [(2, -4)]);
}

#[test]
fn a_two_vertex_polyline_is_the_bresenham_line() {
    assert_eq!(
        pixels(&[(0.0, 0.0), (5.0, 2.0)]),
        [(0, 0), (1, 0), (2, 1), (3, 1), (4, 2), (5, 2)]
    );
}

#[test]
fn joints_and_zero_length_segments_appear_once() {
    let walked = pixels(&[(0.0, 0.0), (3.0, 0.0), (3.2, 0.1), (3.0, 3.0)]);
    assert_eq!(walked.len(), 7);
    assert_eq!(
        walked,
        [(0, 0), (1, 0), (2, 0), (3, 0), (3, 1), (3, 2), (3, 3)]
    );
}

#[test]
fn a_back_step_is_not_emitted_twice() {
    // Rounding near an extremum can step back onto the pixel just drawn.
    let walked = pixels(&[(0.0, 0.0), (3.0, 0.0), (2.0, 0.0), (5.0, 0.0)]);
    assert_eq!(walked.len(), 6);
    assert_eq!(walked, [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)]);
}

#[test]
fn vertices_round_to_pixels_and_clamp_to_the_i32_range() {
    assert_eq!(snap((2.5, -2.5)), (3, -3), "half rounds away from zero");
    assert_eq!(
        snap((1e300, -1e300)),
        (i64::from(i32::MAX), i64::from(i32::MIN))
    );
    assert_eq!(snap((f64::NAN, f64::INFINITY)), (0, i64::from(i32::MAX)));
}

#[test]
fn arc_length_is_measured_along_the_polyline() {
    let walk: Vec<PathPixel> =
        Walk::measured(&[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)], Vec::new()).collect();
    assert_eq!(walk.len(), 8);
    let s: Vec<f64> = walk.iter().map(|p| p.s).collect();
    assert_eq!(s, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);

    let diagonal: Vec<f64> = Walk::measured(&[(0.0, 0.0), (3.0, 3.0)], Vec::new())
        .map(|p| p.s)
        .collect();
    assert_eq!(diagonal.len(), 4);
    for (k, s) in diagonal.iter().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "k < 4")]
        let expected = k as f64 * std::f64::consts::SQRT_2;
        assert!((s - expected).abs() < 1e-12, "pixel {k}: {s} vs {expected}");
    }
}

#[test]
fn arc_length_never_steps_backwards() {
    // A tight hairpin: pixels near the bend project onto both arms.
    let mut points = Vec::new();
    crate::geometry::curve::tessellate(
        &[crate::geometry::curve::Bezier::Quadratic {
            from: (0.0, 0.0),
            ctrl: (30.0, 1.5),
            to: (0.0, 3.0),
        }],
        &mut points,
    );
    let s: Vec<f64> = Walk::measured(&points, Vec::new()).map(|p| p.s).collect();
    assert!(s.len() > 30, "{} pixels", s.len());
    for pair in s.windows(2) {
        assert!(pair[1] >= pair[0], "{pair:?}");
    }
}

#[test]
fn axis_aligned_dashes_count_whole_pixels_like_the_seed() {
    let lit = dashed(&[(0.0, 0.0), (11.0, 0.0)], 3.0, 2.0);
    let xs: Vec<i64> = lit.iter().map(|p| p.0).collect();
    assert_eq!(xs, [0, 1, 2, 5, 6, 7, 10, 11]);
}

#[test]
fn diagonal_dashes_keep_their_arc_length_not_their_step_count() {
    // Each 45° step is √2 long, so a 3-long dash is two pixels (2√2 ≈ 2.83) and a 2-long gap
    // one (√2 ≈ 1.41): the closest whole-pixel spans. Step counting would draw 3 and skip 2,
    // spans of 4.24 and 2.83.
    let lit = dashed(&[(0.0, 0.0), (11.0, 11.0)], 3.0, 2.0);
    let xs: Vec<i64> = lit.iter().map(|p| p.0).collect();
    assert_eq!(xs, [0, 1, 3, 4, 6, 7, 9, 10]);
}

#[test]
fn degenerate_patterns_are_total() {
    let line = [(0.0, 0.0), (9.0, 0.0)];
    assert_eq!(dashed(&line, 0.0, 3.0), []);
    assert_eq!(dashed(&line, 0.0, 0.0), []);
    assert_eq!(dashed(&line, f64::NAN, 1.0), []);
    assert_eq!(dashed(&line, 2.0, 0.0), pixels(&line));
    assert_eq!(dashed(&line, f64::MAX, 1.0), pixels(&line));
    assert_eq!(dashed(&[], 2.0, 2.0), []);
    assert_eq!(dashed(&[(4.0, 4.0)], 2.0, 2.0), [(4, 4)]);
}

#[test]
fn a_dash_shorter_than_a_pixel_step_still_lights_a_pixel() {
    let lit = dashed(&[(0.0, 0.0), (9.0, 0.0)], 0.25, 0.25);
    let xs: Vec<i64> = lit.iter().map(|p| p.0).collect();
    assert_eq!(xs, [0, 2, 4, 6, 8]);
}
