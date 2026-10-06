// `allow`, not `expect`: clippy 1.99's `float_cmp` no longer fires here and earlier releases do.
#![allow(
    clippy::float_cmp,
    reason = "a fit that must be exact is compared exactly"
)]

use proptest::prelude::*;

use super::*;
use crate::geometry::testing::{any_shape, at, awkward_label};
use crate::geometry::{ResolvedMetrics, SemanticZoomTable};
use crate::graph::{EdgeSpec, GraphStore, Limits, NodeShape, NodeSpec};

pub(super) type Store = GraphStore<String>;

pub(super) const GRID: Grid = Grid {
    cols: 100,
    rows: 30,
    cell_aspect: 0.5,
};

/// A store of `specs`, keyed `n0`, `n1`, …, with `edges` between them by index.
pub(super) fn store(specs: &[NodeSpec], edges: &[(usize, usize)]) -> Store {
    let mut store = GraphStore::new(256, Limits::default());
    store
        .transact(|tx| {
            for (i, spec) in specs.iter().enumerate() {
                tx.add_node(format!("n{i}"), spec.clone())?;
            }
            for &(a, b) in edges {
                tx.add_edge(&format!("n{a}"), &format!("n{b}"), EdgeSpec::undirected())?;
            }
            Ok(())
        })
        .expect("fresh keys, known ends");
    store
}

pub(super) fn labels(names: &[&str]) -> Vec<NodeSpec> {
    names.iter().map(|&name| NodeSpec::label(name)).collect()
}

pub(super) fn metrics(store: &Store) -> ResolvedMetrics {
    let mut metrics = ResolvedMetrics::new(SemanticZoomTable::default(), at(2));
    metrics.measure_all(store);
    metrics
}

pub(super) fn ix(store: &Store, i: usize) -> NodeIx {
    store.ix_of(&format!("n{i}")).expect("live")
}

/// World positions by slot: node `i` at `points[i]`.
pub(super) fn world(store: &Store, points: &[(f64, f64)]) -> Vec<Option<(f64, f64)>> {
    let mut world = vec![None; store.nodes.len()];
    for (i, &p) in points.iter().enumerate() {
        world[ix(store, i).slot()] = Some(p);
    }
    world
}

/// Fits `points` to `GRID` at zoom 1 and resolves collisions in placement order.
fn snap(store: &Store, metrics: &ResolvedMetrics, points: &[(f64, f64)]) -> Snap {
    snap_in(GRID, store, metrics, points)
}

fn snap_in(grid: Grid, store: &Store, metrics: &ResolvedMetrics, points: &[(f64, f64)]) -> Snap {
    let wanted = fit(&world(store, points), metrics, grid, Fit::Contain, 1.0);
    resolve(&wanted, metrics, &placement_order(store))
}

fn boxes(snap: &Snap, metrics: &ResolvedMetrics) -> Vec<(NodeIx, CellBox)> {
    snap.boxes(metrics).collect()
}

fn intersect(a: CellBox, b: CellBox) -> bool {
    i64::from(a.x) < b.right()
        && i64::from(b.x) < a.right()
        && i64::from(a.y) < b.bottom()
        && i64::from(b.y) < a.bottom()
}

/// Every pair of boxes that intersect.
fn overlaps(boxes: &[(NodeIx, CellBox)]) -> Vec<(NodeIx, NodeIx)> {
    let mut found = Vec::new();
    for (i, &(a, box_a)) in boxes.iter().enumerate() {
        for &(b, box_b) in &boxes[i + 1..] {
            if intersect(box_a, box_b) {
                found.push((a, b));
            }
        }
    }
    found
}

/// Every pair of boxes that share a column and stand closer than two empty rows apart.
fn crowded(boxes: &[(NodeIx, CellBox)]) -> Vec<(NodeIx, NodeIx)> {
    let grown = |b: CellBox| CellBox::new(b.x, b.y - 1, b.width, b.height + 2);
    let mut found = Vec::new();
    for (i, &(a, box_a)) in boxes.iter().enumerate() {
        for &(b, box_b) in &boxes[i + 1..] {
            if intersect(grown(box_a), grown(box_b)) {
                found.push((a, b));
            }
        }
    }
    found
}

#[test]
fn nodes_at_one_point_get_cells_of_their_own() {
    // The seed's `no_collision_on_small_graph` laid five nodes out first; five nodes on one
    // point make every one of them collide.
    let store = store(&labels(&["E0", "E1", "E2", "E3", "E4"]), &[]);
    let metrics = metrics(&store);
    let snap = snap(&store, &metrics, &[(0.0, 0.0); 5]);
    let boxes = boxes(&snap, &metrics);
    assert_eq!(boxes.len(), 5);
    assert_eq!(overlaps(&boxes), []);
    assert_eq!(snap.displaced.len(), 4, "the first keeps its cell");
}

#[test]
fn demo_graph_labels_dont_overlap() {
    // The seed's demo data: 16 labels of realistic lengths, here on a 4 × 4 grid fitted to a
    // 40 × 12 viewport, 8 columns and 2 rows apart, tighter than they are wide and than the
    // row gap, so many must move. Every node counts, on screen or off (ledger row 7).
    let names = [
        "Alice",
        "Bob",
        "Carol",
        "ACME Corp",
        "Operation Sunrise",
        "Washington DC",
        "Global Defense Agency",
        "Shadow Protocol",
        "London",
        "Tokyo",
        "Eastern Europe",
        "DNS Tunnels",
        "Spear-Phishing",
        "Critical Infra",
        "Energy Networks",
        "Financial",
    ];
    let edges = [
        (0, 3),
        (1, 4),
        (2, 7),
        (3, 6),
        (5, 8),
        (0, 14),
        (4, 10),
        (6, 9),
        (7, 11),
        (8, 12),
        (9, 13),
    ];
    let store = store(&labels(&names), &edges);
    let metrics = metrics(&store);
    let points: Vec<(f64, f64)> = (0..16)
        .map(|i| (f64::from(i % 4) * 4.0, f64::from(i / 4) * 2.0))
        .collect();
    let cramped = Grid {
        cols: 40,
        rows: 12,
        ..GRID
    };
    let snap = snap_in(cramped, &store, &metrics, &points);
    let boxes = boxes(&snap, &metrics);
    assert_eq!(boxes.len(), 16);
    assert_eq!(overlaps(&boxes), []);
    assert_eq!(crowded(&boxes), []);
    assert!(!snap.displaced.is_empty(), "the grid is too tight to keep");
}

#[test]
fn tall_boxes_at_one_point_do_not_overlap() {
    // The seed's `test_multi_row_collision_detection`: two five-row nodes.
    let tall = NodeShape::Box { min_w: 8, min_h: 5 };
    let mut specs = labels(&["OverlayA", "OverlayB"]);
    for spec in &mut specs {
        spec.shape = tall;
    }
    let store = store(&specs, &[(0, 1)]);
    let metrics = metrics(&store);
    let snap = snap(&store, &metrics, &[(0.0, 0.0), (0.0, 0.0)]);
    let boxes = boxes(&snap, &metrics);
    assert_eq!(boxes.len(), 2);
    assert!(boxes.iter().all(|(_, b)| b.height == 5), "{boxes:?}");
    assert_eq!(overlaps(&boxes), []);
    assert_eq!(crowded(&boxes), []);
}

#[test]
fn boxes_keep_their_measured_heights() {
    // The seed's `test_layout_with_varied_heights`.
    let mut specs = labels(&["Regular", "TallOverlay", "AlsoRegular"]);
    specs[1].shape = NodeShape::Box { min_w: 3, min_h: 6 };
    let store = store(&specs, &[]);
    let metrics = metrics(&store);
    let snap = snap(&store, &metrics, &[(0.0, 0.0), (5.0, 1.0), (9.0, 3.0)]);
    let heights: Vec<u32> = (0..3)
        .map(|i| {
            let wanted = ix(&store, i);
            let (_, b) = snap
                .boxes(&metrics)
                .find(|&(ix, _)| ix == wanted)
                .expect("placed");
            b.height
        })
        .collect();
    assert_eq!(heights, [1, 6, 1]);
}

#[test]
fn bounds_of_no_nodes_is_none() {
    // The seed's `bounding_box_empty`, which answered (0, 0, 0, 0).
    let store = store(&[], &[]);
    let metrics = metrics(&store);
    assert_eq!(snap(&store, &metrics, &[]).bounds(&metrics), None);
}

#[test]
fn bounds_of_one_node_is_its_box() {
    // The seed's `bounding_box_single_node`.
    let store = store(&labels(&["four"]), &[]);
    let metrics = metrics(&store);
    let snap = snap(&store, &metrics, &[(3.0, 3.0)]);
    let (_, b) = snap.boxes(&metrics).next().expect("placed");
    assert_eq!(b.width, 6, "[four]");
    assert_eq!(snap.bounds(&metrics), Some(b));
}

#[test]
fn stacked_nodes_keep_two_empty_rows_between() {
    // Ledger row 28: the seed promised a 2-row gap in a comment and tested label cells only,
    // so stacked nodes touched.
    let store = store(&labels(&["upper", "lower"]), &[]);
    let metrics = metrics(&store);
    let wanted: Vec<Option<SubPt>> = {
        let mut wanted = vec![None; store.nodes.len()];
        wanted[ix(&store, 0).slot()] = Some(SubPt::new(20.0, 10.0));
        wanted[ix(&store, 1).slot()] = Some(SubPt::new(20.0, 11.0));
        wanted
    };
    let snap = resolve(&wanted, &metrics, &placement_order(&store));
    let boxes = boxes(&snap, &metrics);
    assert_eq!(crowded(&boxes), []);
    assert_eq!(
        snap.displaced,
        [ix(&store, 0)],
        "\"lower\" sorts first, so it keeps its cell and \"upper\" moves"
    );
}

#[test]
fn off_screen_nodes_take_part_in_collisions() {
    // Ledger row 7: the seed resolved collisions only for nodes inside the viewport, so two
    // off-screen nodes could share cells and pop into view overlapping.
    let store = store(&labels(&["far", "away"]), &[]);
    let metrics = metrics(&store);
    let mut wanted = vec![None; store.nodes.len()];
    wanted[ix(&store, 0).slot()] = Some(SubPt::new(-500.0, 900.0));
    wanted[ix(&store, 1).slot()] = Some(SubPt::new(-500.0, 900.0));
    let snap = resolve(&wanted, &metrics, &placement_order(&store));
    let boxes = boxes(&snap, &metrics);
    assert_eq!(boxes.len(), 2);
    assert_eq!(overlaps(&boxes), []);
}

#[test]
fn the_spiral_stops_after_fifty_rings_on_its_last_candidate() {
    // Plan §6: one spiral, bounded by 50 rings (the seed's two copies stopped after 50
    // attempts, 25 rings); past it a node keeps its last candidate and overlaps.
    let start = CellPt::new(0, 0);
    let walk: Vec<CellPt> = spiral(start, (3, 4)).collect();
    assert_eq!(walk.len(), 2 * 50);
    assert_eq!(
        &walk[..4],
        [(0, 4), (-3, 4), (-3, -4), (3, -4)].map(|(x, y)| CellPt::new(x, y))
    );
    let distinct: std::collections::BTreeSet<CellPt> = walk.iter().copied().collect();
    assert_eq!(distinct.len(), walk.len(), "never revisits a cell");

    let n = 2 * 50 + 2;
    let names: Vec<String> = (0..n).map(|i| format!("{i:03}")).collect();
    let store = store(&names.iter().map(NodeSpec::label).collect::<Vec<_>>(), &[]);
    let metrics = metrics(&store);
    let wanted = vec![Some(SubPt::new(0.0, 0.0)); store.nodes.len()];
    let snap = resolve(&wanted, &metrics, &placement_order(&store));
    assert_eq!(snap.displaced.len(), n - 1);
    assert_eq!(
        snap.overlapping.len(),
        1,
        "only the last finds no free candidate"
    );
    let stranded = *snap.overlapping.first().expect("one");
    let lost = snap.anchors[stranded.slot()].expect("placed anyway");
    let own_walk = spiral(start, (5, 1 + GAP)).last();
    assert_eq!(
        Some(lost),
        own_walk,
        "a [NNN] box steps 5 across and 3 down"
    );
}

#[test]
fn hubs_and_pins_claim_cells_first() {
    // Degree-descending placement (plan §6), pinned nodes before all, ties in canonical order
    // (here the labels' order: hub, leaf, other, pinned).
    let mut specs = labels(&["leaf", "hub", "other", "pinned"]);
    specs[3].pinned = Some((0.0, 0.0));
    let store = store(&specs, &[(1, 0), (1, 2), (1, 3)]);
    let order = placement_order(&store);
    assert_eq!(
        order,
        [ix(&store, 3), ix(&store, 1), ix(&store, 0), ix(&store, 2)],
        "the pin, the hub of degree 3, then the leaves of degree 1 in canonical order"
    );
}

proptest! {
    #[test]
    fn invariant_a_no_two_boxes_intersect(
        nodes in prop::collection::vec(
            (awkward_label(), any_shape(), -30.0f64..30.0, -30.0f64..30.0),
            0..30,
        ),
        clump in 0.0f64..1.0,
    ) {
        // Plan §16.2-A: after a snap no two node boxes intersect, and stacked ones keep two
        // rows apart. `clump` squeezes the world toward one point, so collisions are common.
        let specs: Vec<NodeSpec> = nodes
            .iter()
            .map(|(label, shape, _, _)| {
                let mut spec = NodeSpec::label(label.clone());
                spec.shape = *shape;
                spec
            })
            .collect();
        let points: Vec<(f64, f64)> = nodes.iter().map(|&(_, _, x, y)| (x * clump, y * clump)).collect();
        let store = store(&specs, &[]);
        let metrics = metrics(&store);
        let snap = snap(&store, &metrics, &points);
        let boxes = boxes(&snap, &metrics);
        prop_assert_eq!(boxes.len(), nodes.len());
        prop_assert!(snap.overlapping.is_empty());
        prop_assert_eq!(overlaps(&boxes), []);
        prop_assert_eq!(crowded(&boxes), []);
    }

}
