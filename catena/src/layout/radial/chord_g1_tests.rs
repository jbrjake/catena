//! Owner ruling A2: an inter-group edge is two cubics with matched tangents at the bundled root,
//! where the seed chained two quadratics that kinked there.

use super::test_wheel::{Wheel, angle, controls, cross, dist, dot, eight_group_wheel, norm};
use super::*;

/// Replaces the seed's `segments_chain_continuously` (C⁰: segment N ends where N+1 starts) with
/// the G1 claim: the chain also leaves the junction in the direction it arrived.
#[test]
fn inter_group_chain_is_g1_at_the_junction() {
    let wheel = eight_group_wheel();
    let pairs = wheel.inter_group_pairs();
    assert_eq!(
        pairs.len(),
        48 * 48 - (144 + 81 + 49 + 36 + 25 + 16 + 9 + 4)
    );
    let mut checked = 0;
    for beta in [0.0, 0.3, default_beta(), 1.0] {
        for waist in [0.01, default_waist(), 0.25] {
            for edge in wheel.bundle_with(&pairs, beta, waist) {
                let [first, second] = edge.segments[..] else {
                    panic!("two segments: {edge:?}");
                };
                assert_eq!(first.end(), second.start(), "C⁰ at the junction");
                let (arrive, leave) = (first.derivative(1.0), second.derivative(0.0));
                assert!(norm(arrive) > 0.0 && norm(leave) > 0.0, "{edge:?}");
                let turn = angle(arrive, leave);
                assert!(
                    turn < 1e-9,
                    "the tangent turns {turn:e} rad at the root: {edge:?}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 12 * pairs.len());
}

#[test]
fn the_junction_tangent_runs_from_one_group_control_to_the_other() {
    let wheel = eight_group_wheel();
    for edge in wheel.bundle(&wheel.inter_group_pairs(), default_beta()) {
        let [first, second] = edge.segments[..] else {
            panic!("two segments: {edge:?}");
        };
        let (b_src, b_tgt) = (controls(&first)[1], controls(&second)[2]);
        let d = (b_tgt.0 - b_src.0, b_tgt.1 - b_src.1);
        assert!(angle(second.derivative(0.0), d) < 1e-9, "{edge:?}");
    }
}

#[test]
fn the_waist_is_the_arm_length_at_the_root_as_a_fraction_of_the_chord() {
    let wheel = Wheel::two_by_two();
    let (src, tgt) = (&wheel.nodes[&0], &wheel.nodes[&2]);
    let chord = dist(src.px, src.py, tgt.px, tgt.py);
    for waist in [0.05, 0.1, 0.2] {
        let edges = wheel.bundle_with(&[(0, 2)], default_beta(), waist);
        let [first, second] = edges[0].segments[..] else {
            panic!("two segments");
        };
        let (root, arm_in, arm_out) = (first.end(), controls(&first)[2], controls(&second)[1]);
        for arm in [arm_in, arm_out] {
            let length = dist(arm.0, arm.1, root.0, root.1);
            assert!(
                (length - waist * chord).abs() < 1e-9,
                "{length} vs {}",
                waist * chord
            );
        }
    }
}

#[test]
fn coincident_group_controls_fall_back_to_the_chord_direction() {
    // Two groups at the same point: at β = 1 both group controls are that point, so the
    // direction between them is degenerate and the junction tangent follows src → tgt.
    let shared = GroupPosition {
        group: 0,
        angle: 0.0,
        px: 120.0,
        py: 90.0,
    };
    let node = |node_index, px, py| NodePosition {
        node_index,
        angle: 0.0,
        px,
        py,
    };
    let wheel = Wheel {
        nodes: [(0, node(0, 10.0, 20.0)), (1, node(1, 190.0, 160.0))]
            .into_iter()
            .collect(),
        node_groups: [(0, 0), (1, 1)].into_iter().collect(),
        groups: [(0, shared), (1, GroupPosition { group: 1, ..shared })]
            .into_iter()
            .collect(),
    };
    let edges = wheel.bundle(&[(0, 1)], 1.0);
    let [first, second] = edges[0].segments[..] else {
        panic!("two segments");
    };
    assert_eq!(
        controls(&first)[1],
        controls(&second)[2],
        "degenerate on purpose"
    );
    let chord = (180.0, 140.0);
    assert!(angle(first.derivative(1.0), chord) < 1e-9);
    assert!(angle(second.derivative(0.0), chord) < 1e-9);
}

#[test]
fn beta_zero_chain_is_collinear() {
    // At β = 0 every bundled point is on the straight line from src to tgt, and so is each arm
    // along it: the whole chain is that line, advancing monotonically along it.
    let wheel = eight_group_wheel();
    let pairs = wheel.inter_group_pairs();
    let edges = wheel.bundle(&pairs, 0.0);
    assert_eq!(edges.len(), pairs.len());
    for edge in &edges {
        let points: Vec<(f64, f64)> = edge.segments.iter().flat_map(controls).collect();
        assert_eq!(points.len(), 8);
        let (src, tgt) = (points[0], points[7]);
        let chord = (tgt.0 - src.0, tgt.1 - src.1);
        let mut along_before = f64::NEG_INFINITY;
        for &p in &points {
            let offset = (p.0 - src.0, p.1 - src.1);
            let off_line = cross(chord, offset).abs() / norm(chord);
            assert!(
                off_line < 1e-9,
                "{p:?} is {off_line:e} off the line: {edge:?}"
            );
            let along = dot(chord, offset) / norm(chord);
            assert!(
                along >= along_before - 1e-9,
                "the chain doubles back: {edge:?}"
            );
            along_before = along;
        }
    }
}

// ── Waist tuning ───────────────────────────────────────────────

/// The test wheels' inter-group edges: the eight-group wheel's first members pairwise, and a
/// three-groups-of-ten wheel (the community fixture's shape) from one node to every other group.
fn tuning_edges(beta: f64, waist: f64) -> Vec<BundledEdge> {
    let eight = eight_group_wheel();
    let firsts = [0usize, 12, 21, 28, 34, 39, 43, 46];
    let mut pairs = Vec::new();
    for (i, &a) in firsts.iter().enumerate() {
        for &b in &firsts[i + 1..] {
            pairs.push((a, b));
        }
    }
    let mut edges = eight.bundle_with(&pairs, beta, waist);
    let node_groups: Vec<(usize, u32)> = (0..30)
        .map(|n| (n, u32::try_from(n / 10).unwrap()))
        .collect();
    let three = Wheel::new(&[(0, 10), (1, 10), (2, 10)], &node_groups);
    let spokes: Vec<(usize, usize)> = (10..30).map(|b| (3, b)).collect();
    edges.extend(three.bundle_with(&spokes, beta, waist));
    assert_eq!(edges.len(), 28 + 20);
    edges
}

/// A chain's bending energy `∫ κ² ds`, times its chord so it does not depend on scale: what an
/// elastic rod through the same constraints would minimize, and so a measure of fairness. With
/// `κ² ds = (B′ × B″)² / |B′|⁵ dt`, it is Simpson's rule on 400 intervals per piece, `B″` by
/// central differences of `B′`.
fn bending_energy(edge: &BundledEdge) -> f64 {
    let intervals = 400;
    let h = 1e-6;
    let mut energy = 0.0;
    for piece in &edge.segments {
        let integrand = |t: f64| {
            let d1 = piece.derivative(t);
            let (lo, hi) = (piece.derivative(t - h), piece.derivative(t + h));
            let d2 = ((hi.0 - lo.0) / (2.0 * h), (hi.1 - lo.1) / (2.0 * h));
            let (bend, speed) = (cross(d1, d2), norm(d1));
            bend * bend / (speed * speed * speed * speed * speed)
        };
        let step = 1.0 / f64::from(intervals);
        let mut sum = integrand(0.0) + integrand(1.0);
        for i in 1..intervals {
            let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
            sum += weight * integrand(f64::from(i) * step);
        }
        energy += sum * step / 3.0;
    }
    let (start, end) = (
        edge.segments[0].start(),
        edge.segments[edge.segments.len() - 1].end(),
    );
    energy * dist(start.0, start.1, end.0, end.1)
}

fn mean_energy(waist: f64) -> f64 {
    let edges = tuning_edges(default_beta(), waist);
    #[expect(clippy::cast_precision_loss, reason = "48 edges")]
    let n = edges.len() as f64;
    edges.iter().map(bending_energy).sum::<f64>() / n
}

/// The waist is tuned, not picked: at the default β the default arm gives the fairest chains
/// on the test wheels, within 0.1% of the best on a 0.01 grid from 0.10 to 0.40, and halving or
/// doubling it (pinching or loosening the waist) costs at least a tenth more bending.
#[test]
fn default_waist_minimizes_the_bending_energy() {
    let at_default = mean_energy(default_waist());
    let best = (10..=40)
        .map(|i| mean_energy(f64::from(i) / 100.0))
        .fold(f64::INFINITY, f64::min);
    assert!(
        at_default <= best * 1.001,
        "default {at_default} vs best {best}"
    );
    assert!(mean_energy(default_waist() / 2.0) > at_default * 1.1);
    assert!(mean_energy(default_waist() * 2.0) > at_default * 1.1);
    assert!(
        default_waist() <= 0.25,
        "a β = 0 chain would double back past its quarter points"
    );
}
