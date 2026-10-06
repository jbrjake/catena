// `allow`, not `expect`: clippy 1.99's `float_cmp` no longer fires here and earlier releases do.
#![allow(
    clippy::float_cmp,
    reason = "a fit that must be exact is compared exactly"
)]

//! The two maps a snap makes: [`fit`], world space to canonical cells, and [`derive`],
//! canonical cells to the cells a frame draws.

use proptest::prelude::*;

use super::tests::{GRID, ix, labels, metrics, store, world};
use super::*;

#[test]
fn derive_shifts_by_the_pan_exactly() {
    // The seed's `pan_shifts_positions` checked only that a panned node stayed in bounds; the
    // pan now lives in the derive step, and shifts every node by exactly itself.
    let canonical = CellPt::new(17, 9);
    assert_eq!(derive(canonical, 1.0, (0.0, 0.0)), canonical);
    assert_eq!(derive(canonical, 1.0, (5.0, 3.0)), CellPt::new(22, 12));
    assert_eq!(derive(canonical, 1.0, (-2.4, 0.6)), CellPt::new(15, 10));
}

#[test]
fn a_large_pan_pushes_nodes_off_screen_without_clamping() {
    let canonical = CellPt::new(17, 9);
    assert_eq!(
        derive(canonical, 1.0, (-100.0, -50.0)),
        CellPt::new(-83, -41),
        "signed, not clamped to the viewport"
    );
}

#[test]
fn derive_rounds_half_up_so_whole_cells_of_pan_commute() {
    // Half away from zero would send 1.5 to 2 but -2.5 to -3: a pan of -4 cells would then
    // move a node by -5.
    assert_eq!(
        derive(CellPt::new(1, 1), 1.0, (0.5, -0.5)),
        CellPt::new(2, 1)
    );
    assert_eq!(
        derive(CellPt::new(1, 1), 1.0, (-3.5, -4.5)),
        CellPt::new(-2, -3)
    );
    for whole in [-7.0, -1.0, 3.0] {
        let moved = derive(CellPt::new(4, 9), 1.3, (0.5 + whole, 0.5 + whole));
        let base = derive(CellPt::new(4, 9), 1.3, (0.5, 0.5));
        assert_eq!(
            moved,
            CellPt::new(base.x + to_cells(whole), base.y + to_cells(whole))
        );
    }
}

#[test]
fn derive_scales_about_the_margin() {
    // (c − margin) × zoom / ref_zoom + margin + pan, rounded once (plan §6).
    assert_eq!(
        derive(CellPt::new(11, 5), 2.0, (0.0, 0.0)),
        CellPt::new(21, 9)
    );
    assert_eq!(
        derive(CellPt::new(11, 5), 0.5, (0.0, 0.0)),
        CellPt::new(6, 3)
    );
    assert_eq!(
        derive(CellPt::new(MARGIN, MARGIN), 3.7, (0.0, 0.0)),
        CellPt::new(MARGIN, MARGIN)
    );
}

#[test]
fn contain_scales_both_axes_alike() {
    // Ledger row 9: the seed normalized x and y independently. A world square of side 10
    // spans 10s columns and 10s · cell_aspect rows.
    let store = store(&labels(&["a", "b", "c"]), &[]);
    let metrics = metrics(&store);
    let wanted = fit(
        &world(&store, &[(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)]),
        &metrics,
        GRID,
        Fit::Contain,
        1.0,
    );
    let p = |i: usize| wanted[ix(&store, i).slot()].expect("fitted");
    let across = p(1).x - p(0).x;
    let down = p(2).y - p(0).y;
    assert!(across > 0.0);
    assert!(
        (down - across * GRID.cell_aspect).abs() < 1e-9,
        "{across} columns across, {down} rows down"
    );
}

#[test]
fn contain_fills_the_tighter_axis_and_centers_the_other() {
    // Three-column boxes reserve one column each side, one-row boxes no row: the usable area
    // is 100 − 2 − 3 = 95 by 30 − 2 − 1 = 27 cells, from column 2.5 and row 1.5.
    let store = store(&labels(&["a", "b"]), &[]);
    let metrics = metrics(&store);
    let wanted = fit(
        &world(&store, &[(0.0, 0.0), (10.0, 10.0)]),
        &metrics,
        GRID,
        Fit::Contain,
        1.0,
    );
    let (a, b) = (
        wanted[ix(&store, 0).slot()].expect("fitted"),
        wanted[ix(&store, 1).slot()].expect("fitted"),
    );
    let s = 27.0 / (10.0 * 0.5);
    assert_eq!((b.y - a.y, a.y), (27.0, 1.5), "rows fill");
    assert_eq!(b.x - a.x, 10.0 * s);
    let left = 2.5 + (95.0 - 10.0 * s) / 2.0;
    assert_eq!(a.x, left, "columns letterbox, centered");
}

#[test]
fn stretch_fills_both_axes() {
    let store = store(&labels(&["a", "b"]), &[]);
    let metrics = metrics(&store);
    let wanted = fit(
        &world(&store, &[(0.0, 0.0), (10.0, 10.0)]),
        &metrics,
        GRID,
        Fit::Stretch,
        1.0,
    );
    let (a, b) = (
        wanted[ix(&store, 0).slot()].expect("fitted"),
        wanted[ix(&store, 1).slot()].expect("fitted"),
    );
    assert_eq!((a.x, a.y), (2.5, 1.5));
    assert_eq!((b.x, b.y), (97.5, 28.5));
}

#[test]
fn one_node_fits_to_the_middle() {
    let store = store(&labels(&["a"]), &[]);
    let metrics = metrics(&store);
    let wanted = fit(
        &world(&store, &[(123.0, -4.0)]),
        &metrics,
        GRID,
        Fit::Contain,
        1.0,
    );
    assert_eq!(wanted[ix(&store, 0).slot()], Some(SubPt::new(50.0, 15.0)));
}

#[test]
fn zoom_scales_the_fit_about_the_margin() {
    let store = store(&labels(&["a", "b"]), &[]);
    let metrics = metrics(&store);
    let points = world(&store, &[(0.0, 0.0), (10.0, 10.0)]);
    let at_one = fit(&points, &metrics, GRID, Fit::Contain, 1.0);
    let at_three = fit(&points, &metrics, GRID, Fit::Contain, 3.0);
    let m = f64::from(MARGIN);
    let pairs: Vec<(SubPt, SubPt)> = at_one
        .iter()
        .zip(&at_three)
        .filter_map(|(one, three)| Some(((*one)?, (*three)?)))
        .collect();
    assert_eq!(pairs.len(), 2);
    for (one, three) in pairs {
        assert!((three.x - (m + (one.x - m) * 3.0)).abs() < 1e-9);
        assert!((three.y - (m + (one.y - m) * 3.0)).abs() < 1e-9);
    }
}

#[expect(clippy::cast_possible_truncation, reason = "small whole numbers")]
fn to_cells(v: f64) -> i32 {
    v as i32
}

proptest! {
    #[test]
    fn invariant_i_derived_at_the_snap_zoom_with_no_pan_is_canonical(
        x in -100_000i32..100_000,
        y in -100_000i32..100_000,
    ) {
        // Plan §16.2-I.
        let c = CellPt::new(x, y);
        prop_assert_eq!(derive(c, 1.0, (0.0, 0.0)), c);
    }
}
