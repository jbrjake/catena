use std::collections::BTreeSet;

use proptest::prelude::*;

use super::*;
use crate::geometry::SemanticZoomTable;
use crate::geometry::snap::placement_order;
use crate::geometry::testing::at;
use crate::graph::{GraphStore, Limits, NodeSpec};

type Store = GraphStore<String>;

const GRID: Grid = Grid {
    cols: 80,
    rows: 24,
    cell_aspect: 0.5,
};

/// A store of nodes labelled `labels`, keyed `n0`, `n1`, … and measured at level 2.
fn store(labels: &[&str]) -> (Store, ResolvedMetrics) {
    let mut store = GraphStore::new(256, Limits::default());
    store
        .transact(|tx| {
            for (i, label) in labels.iter().enumerate() {
                tx.add_node(format!("n{i}"), NodeSpec::label(*label))?;
            }
            Ok(())
        })
        .expect("fresh keys");
    let mut metrics = ResolvedMetrics::new(SemanticZoomTable::default(), at(2));
    metrics.measure_all(&store);
    (store, metrics)
}

fn ix(store: &Store, i: usize) -> NodeIx {
    store.ix_of(&format!("n{i}")).expect("live")
}

fn world(store: &Store, points: &[(f64, f64)]) -> Vec<Option<(f64, f64)>> {
    let mut world = vec![None; store.nodes.len()];
    for (i, &p) in points.iter().enumerate() {
        world[ix(store, i).slot()] = Some(p);
    }
    world
}

/// A viewport over `points`, refitted.
fn fitted(store: &Store, metrics: &ResolvedMetrics, points: &[(f64, f64)]) -> Viewport {
    let mut view = Viewport::new(GRID, Fit::Contain);
    view.refit(
        &world(store, points),
        metrics,
        &placement_order(store),
        None,
    );
    view
}

/// Every node's drawn cell, by index of `n0`, `n1`, ….
fn cells(view: &Viewport, store: &Store, n: usize) -> Vec<CellPt> {
    (0..n)
        .map(|i| view.cell(ix(store, i)).expect("placed"))
        .collect()
}

fn intersect(a: CellBox, b: CellBox) -> bool {
    i64::from(a.x) < b.right()
        && i64::from(b.x) < a.right()
        && i64::from(a.y) < b.bottom()
        && i64::from(b.y) < a.bottom()
}

fn any_overlap(view: &Viewport, metrics: &ResolvedMetrics) -> bool {
    let boxes: Vec<CellBox> = view.boxes(metrics).map(|(_, b)| b).collect();
    boxes
        .iter()
        .enumerate()
        .any(|(i, &a)| boxes[i + 1..].iter().any(|&b| intersect(a, b)))
}

const SPREAD: [(f64, f64); 4] = [(0.0, 0.0), (40.0, 0.0), (0.0, 40.0), (40.0, 40.0)];

#[test]
fn a_fresh_snap_is_drawn_as_it_is() {
    // Invariant I at the viewport: at the zoom the snap ran at, with no pan, a node is drawn
    // on its canonical cell.
    let (store, metrics) = store(&["a", "b", "c", "d"]);
    let view = fitted(&store, &metrics, &SPREAD);
    for i in 0..4 {
        let ix = ix(&store, i);
        assert_eq!(view.cell(ix), view.canonical().anchors[ix.slot()]);
    }
    assert_eq!(view.ref_zoom(), view.zoom());
}

#[test]
fn pan_moves_every_node_by_the_pan_and_no_canonical_cell() {
    // Plan §11.2: pan never moves a node in the layout, only where it is drawn.
    let (store, metrics) = store(&["a", "b", "c", "d"]);
    let mut view = fitted(&store, &metrics, &SPREAD);
    let (before, canonical) = (cells(&view, &store, 4), view.canonical().clone());
    view.pan_by((3.4, -2.6));
    view.pan_by((0.2, 0.0));
    let after = cells(&view, &store, 4);
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(
            (a.x - b.x, a.y - b.y),
            (4, -3),
            "a pan of (3.6, -2.6) rounds half up once"
        );
    }
    assert_eq!(view.canonical(), &canonical);
}

#[test]
fn zooming_about_a_point_keeps_that_point_where_it_is() {
    let (store, metrics) = store(&["a", "b", "c", "d"]);
    let mut view = fitted(&store, &metrics, &SPREAD);
    let held = view.cell(ix(&store, 3)).expect("placed");
    view.zoom_about(2.0, SubPt::from(held));
    assert_eq!(view.zoom(), 2.0);
    assert_eq!(view.cell(ix(&store, 3)), Some(held));
    let other = view.cell(ix(&store, 0)).expect("placed");
    assert_ne!(
        other,
        cells(&fitted(&store, &metrics, &SPREAD), &store, 4)[0]
    );
}

#[test]
fn zoom_keeps_to_its_range() {
    let (store, metrics) = store(&["a"]);
    let mut view = fitted(&store, &metrics, &[(0.0, 0.0)]);
    view.zoom_about(100.0, SubPt::new(0.0, 0.0));
    assert_eq!(view.zoom(), MAX_ZOOM);
    view.zoom_about(0.0, SubPt::new(0.0, 0.0));
    assert_eq!(view.zoom(), MIN_ZOOM);
    view.zoom_about(f64::NAN, SubPt::new(0.0, 0.0));
    assert_eq!(view.zoom(), MIN_ZOOM, "a NaN zoom changes nothing");
}

#[test]
fn a_zoom_out_with_room_to_spare_changes_no_cell() {
    let (store, metrics) = store(&["a", "b", "c", "d"]);
    let mut view = fitted(&store, &metrics, &SPREAD);
    let canonical = view.canonical().clone();
    view.zoom_about(0.9, SubPt::new(0.0, 0.0));
    assert_eq!(
        view.resnap_if_crowded(&metrics, &placement_order(&store)),
        []
    );
    assert_eq!(view.canonical(), &canonical);
    assert_eq!(view.ref_zoom(), 1.0);
}

#[test]
fn a_crowding_zoom_out_moves_only_the_nodes_that_meet() {
    // Plan §6's zoom-out re-snap: boxes keep their width as the spacing shrinks, so some meet;
    // they alone move, and the result is the new canonical set at the new zoom. Here three
    // wide labels sit in a row 20 columns apart, and a fourth far below.
    let (store, metrics) = store(&["wide label one", "wide label two", "three", "far"]);
    let points = [(0.0, 0.0), (20.0, 0.0), (40.0, 0.0), (20.0, 60.0)];
    let mut view = fitted(&store, &metrics, &points);
    let order = placement_order(&store);
    view.pan_by((0.3, -0.7));
    view.zoom_about(0.4, SubPt::new(0.0, 0.0));
    let squeezed = cells(&view, &store, 4);
    assert!(any_overlap(&view, &metrics), "the zoom out crowds the row");
    let moved = view.resnap_if_crowded(&metrics, &order);
    assert_ne!(moved, []);
    assert!(!any_overlap(&view, &metrics));
    assert_eq!(view.ref_zoom(), 0.4);
    let after = cells(&view, &store, 4);
    for i in 0..4 {
        if !moved.contains(&ix(&store, i)) {
            assert_eq!(after[i], squeezed[i], "n{i} did not meet anything");
        }
    }
    assert!(
        !moved.contains(&ix(&store, 3)),
        "the far node never met one"
    );
}

#[test]
fn invariant_g_a_refit_keeps_the_kept_node_where_it_was_drawn() {
    // Plan §16.2-G: relayout keeps the focused node within 3 cells of where it was drawn;
    // compensation moves the pan by whole cells, so it stays exactly.
    let (store, metrics) = store(&["a", "b", "c", "d"]);
    let mut view = fitted(&store, &metrics, &SPREAD);
    view.pan_by((7.3, 2.1));
    view.zoom_about(1.7, SubPt::new(30.0, 10.0));
    let kept = ix(&store, 2);
    let before = view.cell(kept).expect("placed");
    let moved = [(5.0, 3.0), (60.0, -10.0), (-4.0, 70.0), (41.0, 39.0)];
    view.refit(
        &world(&store, &moved),
        &metrics,
        &placement_order(&store),
        Some(kept),
    );
    assert_eq!(view.cell(kept), Some(before));
    assert_ne!(
        cells(&view, &store, 4)[0],
        cells(&fitted(&store, &metrics, &SPREAD), &store, 4)[0],
        "the layout itself changed"
    );
}

#[test]
fn a_resnap_moves_the_changed_nodes_and_only_what_they_push() {
    // Plan §4.2: a Geometry delta re-snaps the affected nodes; others keep their cells unless
    // a changed box now meets them, which the result counts.
    let (mut store, mut metrics) = store(&["a", "b", "c", "d"]);
    let points = [(0.0, 0.0), (5.0, 0.0), (0.0, 40.0), (40.0, 40.0)];
    let mut view = fitted(&store, &metrics, &points);
    let order = placement_order(&store);
    let before = cells(&view, &store, 4);
    let ((), mut delta) = store
        .transact(|tx| {
            tx.set_node(&"n0".to_string(), |n| {
                n.label = "a much longer label".into();
            })
        })
        .expect("known");
    metrics.apply(&store, &mut delta);
    let changed: BTreeSet<NodeIx> = delta.reshaped.clone();
    assert_eq!(changed.iter().collect::<Vec<_>>(), [&ix(&store, 0)]);
    let displaced = view.resnap(&changed, &world(&store, &points), &metrics, &order, None);
    assert!(!any_overlap(&view, &metrics));
    let after = cells(&view, &store, 4);
    assert_eq!(
        after[0], before[0],
        "the widened box keeps its fit, being placed first"
    );
    assert_eq!(displaced, [ix(&store, 1)], "b, five columns off, is pushed");
    assert_ne!(after[1], before[1]);
    assert_eq!(&after[2..], &before[2..], "far from the widened box");
}

#[test]
fn a_resize_keeps_the_snap_until_a_refit_spreads_it() {
    // The resize itself moves nothing; the relayout and refit it calls for (owner,
    // "Relayout") use the new grid.
    let (store, metrics) = store(&["a", "b", "c", "d"]);
    let mut view = fitted(&store, &metrics, &SPREAD);
    let before = cells(&view, &store, 4);
    let larger = Grid {
        cols: 160,
        rows: 48,
        ..GRID
    };
    view.resize(larger);
    assert_eq!(view.grid(), larger);
    assert_eq!(cells(&view, &store, 4), before);
    view.refit(
        &world(&store, &SPREAD),
        &metrics,
        &placement_order(&store),
        None,
    );
    let after = cells(&view, &store, 4);
    // The square world fills the rows: the grid's height less the two margin rows and the
    // one-row box, 24 − 3 before and 48 − 3 after.
    let rows = |c: &[CellPt]| c[3].y - c[0].y;
    assert_eq!((rows(&before), rows(&after)), (21, 45));
}

proptest! {
    #[test]
    fn zooming_in_never_makes_boxes_meet(
        points in prop::collection::vec((-50.0f64..50.0, -50.0f64..50.0), 1..20),
        zoom in 1.0f64..4.0,
        pan in (-20.0f64..20.0, -20.0f64..20.0),
    ) {
        // Plan §6: within a level, zooming in only adds space between boxes of fixed width.
        let labels: Vec<String> = (0..points.len()).map(|i| format!("node {i}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let (store, metrics) = store(&refs);
        let mut view = fitted(&store, &metrics, &points);
        prop_assert_eq!(view.boxes(&metrics).count(), points.len());
        prop_assume!(view.canonical().overlapping.is_empty());
        prop_assert!(!any_overlap(&view, &metrics));
        view.pan_by(pan);
        view.zoom_about(zoom, SubPt::new(3.0, 7.0));
        prop_assert!(!any_overlap(&view, &metrics));
    }
}
