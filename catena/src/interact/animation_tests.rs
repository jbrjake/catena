#![allow(
    clippy::float_cmp,
    reason = "the eased fractions at 0, ½ and 1 and a finished glide's sum are exact"
)]

use super::*;

#[test]
fn the_ease_runs_from_zero_to_one_and_slows_down() {
    assert_eq!(ease_out_cubic(0.0), 0.0);
    assert_eq!(ease_out_cubic(1.0), 1.0);
    assert_eq!(ease_out_cubic(0.5), 0.875);
    assert_eq!(ease_out_cubic(-1.0), 0.0);
    assert_eq!(ease_out_cubic(9.0), 1.0);
    let steps: Vec<f64> = (0..=10)
        .map(|i| ease_out_cubic(f64::from(i) / 10.0))
        .collect();
    for pair in steps.windows(3) {
        assert!(
            pair[1] - pair[0] > pair[2] - pair[1],
            "fast start, smooth stop"
        );
    }
}

#[test]
fn a_glide_applies_its_whole_offset_by_the_end_and_then_stops() {
    let mut glide = PanGlide::new((30.0, -12.0));
    let (mut x, mut y) = (0.0, 0.0);
    let mut ticks = 0;
    loop {
        let ((dx, dy), going) = glide.advance(Duration::from_millis(16));
        (x, y) = (x + dx, y + dy);
        ticks += 1;
        if !going {
            break;
        }
    }
    assert_eq!(ticks, 19, "300 ms in 16 ms ticks");
    assert!(
        (x - 30.0).abs() < 1e-9 && (y + 12.0).abs() < 1e-9,
        "({x}, {y})"
    );
}

#[test]
fn half_way_through_a_glide_it_has_covered_seven_eighths() {
    let mut glide = PanGlide::new((8.0, 0.0));
    let ((dx, _), going) = glide.advance(Duration::from_millis(150));
    assert!(going);
    assert_eq!(dx, 7.0);
    let ((rest, _), going) = glide.advance(Duration::from_secs(5));
    assert!(!going);
    assert_eq!(dx + rest, 8.0);
}

#[test]
fn a_zero_tick_moves_nothing() {
    let mut glide = PanGlide::new((8.0, 4.0));
    assert_eq!(glide.advance(Duration::ZERO), ((0.0, 0.0), true));
    let zoom = ZoomGlide {
        target: 2.0,
        about: SubPt::new(0.0, 0.0),
    };
    assert_eq!(zoom.step(1.0, Duration::ZERO), (1.0, false));
}

#[test]
fn the_zoom_covers_one_minus_one_over_e_in_one_time_constant() {
    let zoom = ZoomGlide {
        target: 2.0,
        about: SubPt::new(0.0, 0.0),
    };
    let (next, done) = zoom.step(1.0, ZOOM_TAU);
    assert!(!done);
    let expected = 1.0 + (1.0 - 1.0 / std::f64::consts::E);
    assert!((next - expected).abs() < 1e-12, "{next} against {expected}");
    // Two half-steps reach the same zoom as one whole step: the rate does not depend on dt.
    let (half, _) = zoom.step(1.0, ZOOM_TAU / 2);
    let (twice, _) = zoom.step(half, ZOOM_TAU / 2);
    assert!((twice - next).abs() < 1e-12, "{twice} against {next}");
}

#[test]
fn the_zoom_snaps_to_its_target_once_it_is_close() {
    let zoom = ZoomGlide {
        target: 1.25,
        about: SubPt::new(0.0, 0.0),
    };
    assert_eq!(zoom.step(1.2496, Duration::from_millis(1)), (1.25, true));
    let (mut z, mut ticks) = (1.0, 0);
    loop {
        let (next, done) = zoom.step(z, Duration::from_millis(16));
        z = next;
        ticks += 1;
        if done {
            break;
        }
    }
    assert_eq!(z, 1.25);
    assert!((10..=20).contains(&ticks), "{ticks} ticks");
}
