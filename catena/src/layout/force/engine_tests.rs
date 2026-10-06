// `allow`, not `expect`: clippy 1.99's `float_cmp` no longer fires here and earlier releases do.
#![allow(
    clippy::float_cmp,
    reason = "pins, angles and repeat runs are compared exactly"
)]

use super::*;
use crate::geometry::SemanticZoomTable;
use crate::geometry::testing::at;
use crate::graph::{EdgeSpec, Limits, NodeSpec};

type Store = GraphStore<&'static str>;

const FRAME: Frame = Frame {
    area: (80.0, 48.0),
    cell_aspect: 0.5,
};

/// A store with `nodes` (each labelled with its key) and layout edges `edges`, in that order.
fn build(nodes: &[&'static str], edges: &[(&'static str, &'static str)]) -> Store {
    let mut store = GraphStore::new(256, Limits::default());
    store
        .transact(|tx| {
            for &key in nodes {
                tx.add_node(key, NodeSpec::label(key))?;
            }
            for (from, to) in edges {
                tx.add_edge(from, to, EdgeSpec::undirected())?;
            }
            Ok(())
        })
        .expect("fresh keys, known ends");
    store
}

fn metrics(store: &Store) -> ResolvedMetrics {
    let mut metrics = ResolvedMetrics::new(SemanticZoomTable::default(), at(5));
    metrics.measure_all(store);
    metrics
}

/// A cold layout of `store`.
fn cold(store: &Store) -> Vec<Option<(f64, f64)>> {
    cold_state(store).positions
}

/// A cold layout of `store`, with all the layout keeps.
fn cold_state(store: &Store) -> ForceState {
    let mut state = ForceState::default();
    let all = store.nodes_in_order().iter().copied().collect();
    lay_out(
        &ForceParams::default(),
        store,
        &metrics(store),
        FRAME,
        Change::Topology(&all),
        &mut state,
    );
    state
}

fn position(store: &Store, positions: &[Option<(f64, f64)>], key: &'static str) -> (f64, f64) {
    let ix = store.ix_of(&key).expect("live");
    positions[ix.slot()].expect("laid out")
}

/// A node's box in world units: its label in brackets, one row (two units) tall.
fn half_size(key: &str) -> (f64, f64) {
    (crate::layout::count(key.chars().count() + 2) / 2.0, 1.0)
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    (dx * dx + dy * dy).sqrt()
}

#[test]
fn an_empty_graph_lays_out_nothing() {
    assert_eq!(cold(&build(&[], &[])), []);
}

#[test]
fn isolated_nodes_ring_the_core() {
    let store = build(
        &["a", "b", "c", "lone", "solo", "single"],
        &[("a", "b"), ("b", "c"), ("c", "a")],
    );
    let positions = cold(&store);
    let core: Vec<(f64, f64)> = ["a", "b", "c"]
        .iter()
        .map(|k| position(&store, &positions, k))
        .collect();
    let center = centroid(&core);
    let farthest_corner = ["a", "b", "c"]
        .iter()
        .zip(&core)
        .map(|(k, &p)| {
            let (hw, hh) = half_size(k);
            distance(p, center) + (hw * hw + hh * hh).sqrt()
        })
        .fold(0.0f64, f64::max);
    for key in ["lone", "solo", "single"] {
        let d = distance(position(&store, &positions, key), center);
        assert!(
            d > farthest_corner,
            "{key} at {d}, inside the core's {farthest_corner}"
        );
    }
}

#[test]
fn a_node_joined_only_by_a_self_loop_or_a_drawn_edge_rings_the_core_facing_its_neighbor() {
    let mut store = build(
        &["a", "b", "c", "looped", "drawn"],
        &[("a", "b"), ("b", "c")],
    );
    store
        .transact(|tx| {
            tx.add_edge(&"looped", &"looped", EdgeSpec::undirected())?;
            let mut overlay = EdgeSpec::undirected();
            overlay.layout_participating = false;
            tx.add_edge(&"drawn", &"c", overlay)?;
            Ok(())
        })
        .expect("known ends");
    let positions = cold(&store);
    let core: Vec<(f64, f64)> = ["a", "b", "c"]
        .iter()
        .map(|k| position(&store, &positions, k))
        .collect();
    let center = centroid(&core);
    let angle = |p: (f64, f64)| fmath::atan2(p.1 - center.1, p.0 - center.0);
    let drawn = position(&store, &positions, "drawn");
    assert_eq!(
        angle(drawn),
        angle(position(&store, &positions, "c")),
        "the ring's first node sits on its neighbor's bearing"
    );
    let looped = position(&store, &positions, "looped");
    assert!(distance(looped, center) > distance(position(&store, &positions, "a"), center));
}

#[test]
fn islands_pack_left_to_right_largest_first() {
    let store = build(
        &["p1", "p2", "p3", "p4", "t1", "t2", "t3", "q1", "q2"],
        &[
            ("q1", "q2"),
            ("t1", "t2"),
            ("t2", "t3"),
            ("t3", "t1"),
            ("p1", "p2"),
            ("p2", "p3"),
            ("p3", "p4"),
        ],
    );
    let positions = cold(&store);
    let extent = |keys: &[&'static str]| {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &key in keys {
            let ((x, y), (hw, hh)) = (position(&store, &positions, key), half_size(key));
            b = (
                b.0.min(x - hw),
                b.1.min(y - hh),
                b.2.max(x + hw),
                b.3.max(y + hh),
            );
        }
        b
    };
    let path = extent(&["p1", "p2", "p3", "p4"]);
    let triangle = extent(&["t1", "t2", "t3"]);
    let pair = extent(&["q1", "q2"]);
    assert!(
        path.0.abs() < 1e-9,
        "the largest island starts the row: {path:?}"
    );
    assert!(
        (triangle.0 - (path.2 + GUTTER)).abs() < 1e-9,
        "{path:?} then {triangle:?}"
    );
    assert!(
        (pair.0 - (triangle.2 + GUTTER)).abs() < 1e-9,
        "{triangle:?} then {pair:?}"
    );
    let middle = |b: (f64, f64, f64, f64)| f64::midpoint(b.1, b.3);
    assert!((middle(triangle) - middle(path)).abs() < 1e-9);
    assert!((middle(pair) - middle(path)).abs() < 1e-9);
}

#[test]
fn a_layout_does_not_depend_on_insertion_order() {
    let nodes = [
        "ada",
        "babbage",
        "lovelace",
        "menabrea",
        "somerville",
        "hopper",
        "lone",
    ];
    let edges = [
        ("ada", "babbage"),
        ("babbage", "lovelace"),
        ("lovelace", "ada"),
        ("menabrea", "ada"),
        ("somerville", "hopper"),
    ];
    let forward = build(&nodes, &edges);
    let mut reversed_nodes = nodes;
    reversed_nodes.reverse();
    let mut reversed_edges = edges;
    reversed_edges.reverse();
    let backward = build(&reversed_nodes, &reversed_edges);
    let (a, b) = (cold(&forward), cold(&backward));
    for key in nodes {
        assert_eq!(
            position(&forward, &a, key),
            position(&backward, &b, key),
            "{key}"
        );
    }
}

#[test]
fn pins_hold_in_the_core_and_on_the_ring() {
    let mut store = build(&["a", "b", "c", "lone"], &[("a", "b"), ("b", "c")]);
    store
        .transact(|tx| {
            tx.set_node(&"b", |n| n.pinned = Some((7.0, -3.0)))?;
            tx.set_node(&"lone", |n| n.pinned = Some((-50.0, 2.5)))
        })
        .expect("known");
    let positions = cold(&store);
    assert_eq!(position(&store, &positions, "b"), (7.0, -3.0));
    assert_eq!(position(&store, &positions, "lone"), (-50.0, 2.5));
    assert_ne!(
        position(&store, &positions, "a"),
        position(&store, &positions, "c")
    );
}

#[test]
fn only_live_slots_have_positions() {
    let mut store = build(&["a", "b", "c"], &[("a", "b")]);
    store.transact(|tx| tx.remove_node(&"b")).expect("known");
    let positions = cold(&store);
    assert_eq!(positions.len(), 3);
    let b_slot = 1;
    assert_eq!(positions[b_slot], None);
    assert_eq!(positions.iter().flatten().count(), 2);
}

#[test]
fn a_reused_slot_starts_fresh() {
    let mut store = build(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
    let mut state = cold_state(&store);
    let old_c = position(&store, &state.positions, "c");
    let ((), delta) = store
        .transact(|tx| {
            tx.remove_node(&"c")?;
            Ok(())
        })
        .expect("known");
    lay_out(
        &ForceParams::default(),
        &store,
        &metrics(&store),
        FRAME,
        Change::Topology(&delta.added),
        &mut state,
    );
    let ((), delta) = store
        .transact(|tx| {
            tx.add_node("d", NodeSpec::label("d"))?;
            tx.add_edge(&"d", &"a", EdgeSpec::undirected())?;
            Ok(())
        })
        .expect("fresh");
    let d = store.ix_of(&"d").expect("live");
    assert_eq!(d.slot(), 2, "d takes c's freed slot");
    assert_eq!(delta.added.iter().collect::<Vec<_>>(), [&d]);
    let mut graph = Graph::of(&store, &metrics(&store), FRAME);
    let params = ForceParams::default();
    let base = graph.scale(&params, FRAME.area);
    let starts = mobility::starts(
        &params,
        &graph.view(),
        Change::Topology(&delta.added),
        &state,
        base,
    );
    assert_eq!(
        starts.previous[graph.local[d.slot()]],
        None,
        "not c's old place, {old_c:?}"
    );
}
