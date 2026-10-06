//! Curves through the tessellator (owner ruling A1): invariant E (plan §16.2-E) on tessellated
//! polylines, and dash and gap lengths measured along the polyline.
//!
//! The generated curves are the shapes catena draws: bowed quadratics, cubics and two-cubic G1
//! chains like the radial view's bundled edges, short (chord under 20 sub-pixels) and long. Every
//! family advances monotonically along its chord, so none crosses itself; a curve that does
//! crosses its own pixels by geometry.

use std::collections::HashSet;

use catena::geometry::curve::{Bezier, tessellate};
use catena::raster::{BrailleCanvas, polyline_pixels};
use catena_testkit::braille_asserts::{
    assert_8_connected, assert_endpoints_exact, assert_no_duplicates, lit_pixels,
};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x6375_7276),
        failure_persistence: None,
        ..Config::default()
    }
}

type Pt = (f64, f64);

fn add(a: Pt, b: Pt) -> Pt {
    (a.0 + b.0, a.1 + b.1)
}

fn sub(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}

fn scale(a: Pt, k: f64) -> Pt {
    (a.0 * k, a.1 * k)
}

fn length(a: Pt) -> f64 {
    (a.0 * a.0 + a.1 * a.1).sqrt()
}

/// A point `along` chord lengths down the chord and `across` chord lengths to its left.
fn frame(from: Pt, to: Pt, along: f64, across: f64) -> Pt {
    let d = sub(to, from);
    let normal = (-d.1, d.0);
    add(from, add(scale(d, along), scale(normal, across)))
}

/// Integer end points `from` and `from + offset`, with the chord's length in `lengths`.
fn chord(reach: i32, lengths: std::ops::Range<f64>) -> impl Strategy<Value = (Pt, Pt)> {
    (-reach..=reach, -reach..=reach)
        .prop_filter("chord length in range", move |&(dx, dy)| {
            lengths.contains(&length((f64::from(dx), f64::from(dy))))
        })
        .prop_map(|(dx, dy)| ((0.0, 0.0), (f64::from(dx), f64::from(dy))))
}

/// Short chords (under 20 sub-pixels, where the seed's dashes coarsened) or long ones.
fn any_chord() -> impl Strategy<Value = (Pt, Pt)> {
    prop_oneof![chord(19, 6.0..20.0), chord(90, 20.0..120.0)]
}

/// A quadratic bowed `bow` chord lengths off its chord's midpoint.
fn bowed(max_bow: f64) -> impl Strategy<Value = Vec<Bezier>> {
    (any_chord(), -max_bow..=max_bow).prop_map(|((from, to), bow)| {
        vec![Bezier::Quadratic {
            from,
            ctrl: frame(from, to, 0.5, bow),
            to,
        }]
    })
}

/// A cubic whose control arms point forward along the chord, possibly to opposite sides.
fn cubic() -> impl Strategy<Value = Vec<Bezier>> {
    (any_chord(), 0.1..0.45, 0.1..0.45, -0.4..0.4, -0.4..0.4).prop_map(
        |((from, to), u0, u1, v0, v1)| {
            vec![Bezier::Cubic {
                from,
                ctrl0: frame(from, to, u0, v0),
                ctrl1: frame(from, to, 1.0 - u1, v1),
                to,
            }]
        },
    )
}

/// The radial view's inter-group edge (owner ruling A2): two cubics meeting at `J` with
/// matched tangents along `d`, the direction between the two group controls.
fn g1_chain() -> impl Strategy<Value = Vec<Bezier>> {
    (
        any_chord(),
        (0.15..0.3, -0.25..0.25),
        (0.4..0.6, -0.15..0.15),
        (0.7..0.85, -0.25..0.25),
        0.05..0.3,
    )
        .prop_map(|((src, tgt), b_src, j, b_tgt, waist)| {
            let b_src = frame(src, tgt, b_src.0, b_src.1);
            let j = frame(src, tgt, j.0, j.1);
            let b_tgt = frame(src, tgt, b_tgt.0, b_tgt.1);
            let span = sub(b_tgt, b_src);
            let d = scale(span, 1.0 / length(span));
            let arm = scale(d, waist * length(span));
            vec![
                Bezier::Cubic {
                    from: src,
                    ctrl0: b_src,
                    ctrl1: sub(j, arm),
                    to: j,
                },
                Bezier::Cubic {
                    from: j,
                    ctrl0: add(j, arm),
                    ctrl1: b_tgt,
                    to: tgt,
                },
            ]
        })
}

fn gentle_curve() -> impl Strategy<Value = Vec<Bezier>> {
    prop_oneof![bowed(0.6), cubic(), g1_chain()]
}

/// Every point of a piece, so a chain can be moved and bounded.
fn control_points(piece: &Bezier) -> Vec<Pt> {
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

fn shift(piece: &Bezier, by: Pt) -> Bezier {
    match *piece {
        Bezier::Quadratic { from, ctrl, to } => Bezier::Quadratic {
            from: add(from, by),
            ctrl: add(ctrl, by),
            to: add(to, by),
        },
        Bezier::Cubic {
            from,
            ctrl0,
            ctrl1,
            to,
        } => Bezier::Cubic {
            from: add(from, by),
            ctrl0: add(ctrl0, by),
            ctrl1: add(ctrl1, by),
            to: add(to, by),
        },
    }
}

/// The chain moved by whole pixels so its control hull (and so the curve) sits at least 2
/// sub-pixels inside a canvas just big enough to hold it: nothing clips.
fn on_canvas(chain: &[Bezier]) -> (Vec<Bezier>, BrailleCanvas, BrailleCanvas) {
    let all: Vec<Pt> = chain.iter().flat_map(control_points).collect();
    let min_x = all.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let min_y = all.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let by = (2.0 - min_x.floor(), 2.0 - min_y.floor());
    let moved: Vec<Bezier> = chain.iter().map(|p| shift(p, by)).collect();
    let moved_pts: Vec<Pt> = moved.iter().flat_map(control_points).collect();
    let max_x = moved_pts.iter().map(|p| p.0).fold(0.0, f64::max);
    let max_y = moved_pts.iter().map(|p| p.1).fold(0.0, f64::max);
    let cells = |extent: f64, per_cell: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a small positive extent"
        )]
        let n = ((extent + 3.0) / per_cell).ceil() as usize;
        n
    };
    let (w, h) = (cells(max_x, 2.0), cells(max_y, 4.0));
    (moved, BrailleCanvas::new(w, h), BrailleCanvas::new(w, h))
}

fn polyline(chain: &[Bezier]) -> Vec<Pt> {
    let mut points = Vec::new();
    tessellate(chain, &mut points);
    points
}

fn as_usize(pixels: &[(i64, i64)]) -> Vec<(usize, usize)> {
    pixels
        .iter()
        .map(|&(x, y)| (usize::try_from(x).unwrap(), usize::try_from(y).unwrap()))
        .collect()
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "integer end points by construction"
)]
fn int(p: Pt) -> (i32, i32) {
    (p.0 as i32, p.1 as i32)
}

/// Arc length along `points` of the nearest point to `p`, over every segment: the oracle the
/// dasher's windowed search must agree with.
#[expect(clippy::cast_precision_loss, reason = "small pixel coordinates")]
fn arc_position(points: &[Pt], (x, y): (i64, i64)) -> f64 {
    let pixel = (x as f64, y as f64);
    let mut walked = 0.0;
    let mut best = (f64::INFINITY, 0.0);
    for pair in points.windows(2) {
        let (start, leg) = (pair[0], sub(pair[1], pair[0]));
        let len = length(leg);
        let along = if len > 0.0 {
            let to_pixel = sub(pixel, start);
            ((to_pixel.0 * leg.0 + to_pixel.1 * leg.1) / (len * len)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dist = length(sub(add(start, scale(leg, along)), pixel));
        if dist < best.0 {
            best = (dist, walked + along * len);
        }
        walked += len;
    }
    best.1
}

/// The interior runs of a dashed walk as `(lit, span)`: consecutive path pixels alike in being
/// lit, each spanning from the midpoint before its first pixel to the midpoint after its last,
/// in arc length. The first and last runs are cut by the curve's ends and left out.
fn interior_runs(
    points: &[Pt],
    path: &[(i64, i64)],
    lit: &HashSet<(usize, usize)>,
) -> Vec<(bool, f64)> {
    let s: Vec<f64> = path.iter().map(|&p| arc_position(points, p)).collect();
    let on: Vec<bool> = as_usize(path).iter().map(|p| lit.contains(p)).collect();
    let mut runs = Vec::new();
    let mut start: Option<f64> = None; // None while in the first run
    for k in 1..path.len() {
        if on[k] != on[k - 1] {
            let boundary = f64::midpoint(s[k - 1], s[k]);
            if let Some(begin) = start {
                runs.push((on[k - 1], boundary - begin));
            }
            start = Some(boundary);
        }
    }
    runs
}

fn check_dashes(chain: &[Bezier], dash_on: u32, dash_off: u32) -> Result<usize, TestCaseError> {
    let (chain, mut solid, mut dashed) = on_canvas(chain);
    let points = polyline(&chain);
    let path: Vec<(i64, i64)> = polyline_pixels(&points).collect();
    solid.draw_polyline(&points);
    dashed.draw_dashed_curve(&chain, dash_on, dash_off);
    let solid_px: HashSet<(usize, usize)> = lit_pixels(&solid).into_iter().collect();
    let dashed_px: HashSet<(usize, usize)> = lit_pixels(&dashed).into_iter().collect();
    prop_assert!(
        dashed_px.is_subset(&solid_px),
        "a dash leaves the solid curve"
    );
    let runs = interior_runs(&points, &path, &dashed_px);
    for &(on, span) in &runs {
        let target = f64::from(if on { dash_on } else { dash_off });
        prop_assert!(
            (span - target).abs() <= 1.0,
            "{} spans {:.3} for a pattern of {}; runs {:?}",
            if on { "dash" } else { "gap" },
            span,
            target,
            runs
        );
    }
    Ok(runs.len())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn invariant_e_holds_on_tessellated_curves(
        chain in prop_oneof![bowed(1.0), cubic(), g1_chain()]
    ) {
        let (chain, mut canvas, _) = on_canvas(&chain);
        let points = polyline(&chain);
        let path: Vec<(i64, i64)> = polyline_pixels(&points).collect();
        let emitted = as_usize(&path);
        assert_no_duplicates(&emitted);

        canvas.draw_curve(&chain);
        let lit = lit_pixels(&canvas);
        prop_assert_eq!(lit.len(), emitted.len(), "the canvas lights exactly the walk");
        let lit_set: HashSet<_> = lit.iter().copied().collect();
        prop_assert_eq!(lit_set, emitted.iter().copied().collect::<HashSet<_>>());
        assert_8_connected(&lit);
        let ((x0, y0), (x1, y1)) = (int(chain[0].start()), int(chain[chain.len() - 1].end()));
        assert_endpoints_exact(&lit, x0, y0, x1, y1);
    }

    #[test]
    fn dash_and_gap_lengths_follow_the_pattern(
        chain in gentle_curve(), dash_on in 1u32..=6, dash_off in 1u32..=6
    ) {
        check_dashes(&chain, dash_on, dash_off)?;
    }
}

#[test]
fn dashes_survive_on_curves_under_20_px() {
    // The seed counted dash phase by sample index, and a short curve's 10 samples share pixels,
    // so its dashes coarsened or went solid. Every bow of a 6- to 19-sub-pixel chord must show
    // its 3-on, 2-off pattern along the curve.
    let mut checked = 0;
    for len in 6..20 {
        for bow in [-0.5, -0.2, 0.2, 0.5] {
            let to = (f64::from(len), f64::from(len / 3));
            let chain = [Bezier::Quadratic {
                from: (0.0, 0.0),
                ctrl: frame((0.0, 0.0), to, 0.5, bow),
                to,
            }];
            checked += check_dashes(&chain, 3, 2).unwrap();
        }
    }
    assert!(checked >= 56, "only {checked} interior runs measured");
}
