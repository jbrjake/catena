//! The owner's "Relayout" ruling (`docs/design/owner-rulings.md`): a topology change lets the
//! older nodes it affects move as the forces say, a semantic level change relayouts only around
//! the nodes whose boxes collapsed or expanded, and a resize relayouts every node.

use super::super::simulation::net_forces;
use super::*;
use crate::geometry::SemanticZoomTable;
use crate::geometry::testing::at;
use crate::graph::{EdgeSpec, Limits, NodeSpec};

type Store = GraphStore<String>;

/// A slot's world position, if it has one.
type Placement = Option<(f64, f64)>;

const FRAME: Frame = Frame {
    area: (80.0, 48.0),
    cell_aspect: 0.5,
};

/// A store with `nodes` as `(key, label)` and undirected layout edges `edges`.
fn build(nodes: &[(String, String)], edges: &[(String, String)]) -> Store {
    let mut store = GraphStore::new(256, Limits::default());
    store
        .transact(|tx| {
            for (key, label) in nodes {
                tx.add_node(key.clone(), NodeSpec::label(label.clone()))?;
            }
            for (from, to) in edges {
                tx.add_edge(from, to, EdgeSpec::undirected())?;
            }
            Ok(())
        })
        .expect("fresh keys, known ends");
    store
}

/// Nodes labelled with their keys.
fn named(keys: &[&str]) -> Vec<(String, String)> {
    keys.iter()
        .map(|k| (k.to_string(), k.to_string()))
        .collect()
}

fn pairs(edges: &[(&str, &str)]) -> Vec<(String, String)> {
    edges
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

fn ix(store: &Store, key: &str) -> NodeIx {
    store.ix_of(&key.to_string()).expect("live")
}

fn place(store: &Store, positions: &[Placement], key: &str) -> (f64, f64) {
    positions[ix(store, key).slot()].expect("laid out")
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    (dx * dx + dy * dy).sqrt()
}

fn length(v: (f64, f64)) -> f64 {
    distance(v, (0.0, 0.0))
}

/// A first layout of `store` at `level`.
fn first(store: &Store, metrics: &ResolvedMetrics, frame: Frame) -> ForceState {
    let mut state = ForceState::default();
    let all = store.nodes_in_order().iter().copied().collect();
    lay_out(
        &ForceParams::default(),
        store,
        metrics,
        frame,
        Change::Topology(&all),
        &mut state,
    );
    state
}

/// The net force on each core node of `store` where `positions` puts it, by slot: what decides
/// whether a node rests where its forces balance.
fn net_force(
    store: &Store,
    metrics: &ResolvedMetrics,
    frame: Frame,
    positions: &[Placement],
) -> Vec<Option<(f64, f64)>> {
    let params = ForceParams::default();
    let mut graph = Graph::of(store, metrics, frame);
    graph.scale(&params, frame.area);
    graph.previous = graph.order.iter().map(|ix| positions[ix.slot()]).collect();
    let islands = graph.islands();
    let mut out = vec![None; store.nodes.len()];
    for island in &islands {
        let (bodies, springs) = graph.bodies(island);
        let forces = net_forces(&params, &bodies, &springs, frame.area);
        for (&i, &f) in island.iter().zip(&forces) {
            out[graph.order[i].slot()] = Some(f);
        }
    }
    out
}

#[test]
fn new_nodes_lead_their_neighbors_to_be_reassessed() {
    // Two triangles at the ends of a path, so their outer corners sit far apart; then one new
    // node joins those corners. Each corner must answer the new edge's pull by moving until
    // its forces balance again, not stay where it was with the pull unanswered.
    let mut store = build(
        &named(&[
            "a1", "a2", "a3", "p1", "p2", "p3", "p4", "p5", "b1", "b2", "b3",
        ]),
        &pairs(&[
            ("a1", "a2"),
            ("a2", "a3"),
            ("a3", "a1"),
            ("a1", "p1"),
            ("p1", "p2"),
            ("p2", "p3"),
            ("p3", "p4"),
            ("p4", "p5"),
            ("p5", "b1"),
            ("b1", "b2"),
            ("b2", "b3"),
            ("b3", "b1"),
        ]),
    );
    let mut metrics = ResolvedMetrics::new(SemanticZoomTable::default(), at(2));
    metrics.measure_all(&store);
    let mut state = first(&store, &metrics, FRAME);
    let before = state.positions.clone();

    let ((), delta) = store
        .transact(|tx| {
            tx.add_node("x".to_string(), NodeSpec::label("x"))?;
            tx.add_edge(&"x".to_string(), &"a2".to_string(), EdgeSpec::undirected())?;
            tx.add_edge(&"x".to_string(), &"b2".to_string(), EdgeSpec::undirected())?;
            Ok(())
        })
        .expect("fresh key, known ends");
    metrics.measure_all(&store);
    lay_out(
        &ForceParams::default(),
        &store,
        &metrics,
        FRAME,
        Change::Topology(&delta.added),
        &mut state,
    );

    let after = net_force(&store, &metrics, FRAME, &state.positions);
    for corner in ["a2", "b2"] {
        let mut stayed = state.positions.clone();
        stayed[ix(&store, corner).slot()] = before[ix(&store, corner).slot()];
        let unanswered = net_force(&store, &metrics, FRAME, &stayed);
        let slot = ix(&store, corner).slot();
        let (rest, pull) = (
            length(after[slot].expect("core")),
            length(unanswered[slot].expect("core")),
        );
        assert!(
            rest <= pull / 4.0,
            "{corner}: net force {rest:.2} where it settled, {pull:.2} had it stayed put"
        );
    }
}

#[test]
fn a_level_change_moves_only_nodes_near_reshaped_ones() {
    // A 5 × 5 grid whose middle node has a long label: cut to 14 columns at level 2, whole
    // (45 columns) at level 5. Only that box changes, so only the nodes near it may move.
    let long = "the quick brown fox jumps over the lazy dog";
    let key = |r: usize, c: usize| format!("g{r}{c}");
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for r in 0..5 {
        for c in 0..5 {
            let label = if (r, c) == (2, 2) {
                long.to_string()
            } else {
                key(r, c)
            };
            nodes.push((key(r, c), label));
            if c < 4 {
                edges.push((key(r, c), key(r, c + 1)));
            }
            if r < 4 {
                edges.push((key(r, c), key(r + 1, c)));
            }
        }
    }
    let store = build(&nodes, &edges);
    let mut metrics = ResolvedMetrics::new(SemanticZoomTable::default(), at(2));
    metrics.measure_all(&store);
    let mut state = first(&store, &metrics, FRAME);
    let before = state.positions.clone();

    let reshaped = metrics.set_level(&store, at(5));
    let wide = ix(&store, "g22");
    assert_eq!(reshaped.iter().collect::<Vec<_>>(), [&wide]);
    lay_out(
        &ForceParams::default(),
        &store,
        &metrics,
        FRAME,
        Change::Reshaped(&reshaped),
        &mut state,
    );

    let reach = ForceParams::default().tether_reach;
    let mut held = 0;
    for r in 0..5usize {
        for c in 0..5usize {
            let hops = r.abs_diff(2) + c.abs_diff(2);
            if u32::try_from(hops).expect("small") > reach {
                held += 1;
                assert_eq!(
                    place(&store, &state.positions, &key(r, c)),
                    place(&store, &before, &key(r, c)),
                    "{} is {hops} hops from the widened box",
                    key(r, c)
                );
            }
        }
    }
    assert_eq!(held, 25 - 1 - 4 - 8, "the nodes past the reach");

    // The box grew by 31 columns, 15.5 on each side; its neighbors make room for at least
    // half of that.
    let growth = f64::from(45 - 14) / 2.0;
    let spread = |positions: &[Placement]| {
        let center = place(&store, positions, "g22");
        ["g12", "g21", "g23", "g32"]
            .iter()
            .map(|k| distance(place(&store, positions, k), center))
            .sum::<f64>()
            / 4.0
    };
    let (was, is) = (spread(&before), spread(&state.positions));
    assert!(
        is - was >= growth / 2.0,
        "the neighbors sat {was:.2} from it and now {is:.2}"
    );
}

#[test]
fn a_resize_relays_out_every_node() {
    // A 4 × 4 grid with one node knocked out of place: a resize frees every node, so the
    // stray one settles back and the whole layout spreads into the larger frame.
    let key = |r: usize, c: usize| format!("n{r}{c}");
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for r in 0..4 {
        for c in 0..4 {
            nodes.push((key(r, c), key(r, c)));
            if c < 3 {
                edges.push((key(r, c), key(r, c + 1)));
            }
            if r < 3 {
                edges.push((key(r, c), key(r + 1, c)));
            }
        }
    }
    let store = build(&nodes, &edges);
    let mut metrics = ResolvedMetrics::new(SemanticZoomTable::default(), at(2));
    metrics.measure_all(&store);
    let mut state = first(&store, &metrics, FRAME);
    let stray = ix(&store, "n11").slot();
    let (x, y) = state.positions[stray].expect("laid out");
    state.positions[stray] = Some((x + 60.0, y - 40.0));
    let before = state.positions.clone();
    let knocked = length(net_force(&store, &metrics, FRAME, &before)[stray].expect("core"));

    let larger = Frame {
        area: (FRAME.area.0 * 2.0, FRAME.area.1 * 2.0),
        ..FRAME
    };
    lay_out(
        &ForceParams::default(),
        &store,
        &metrics,
        larger,
        Change::Resize,
        &mut state,
    );

    for &ix in store.nodes_in_order() {
        assert_ne!(
            state.positions[ix.slot()],
            before[ix.slot()],
            "{:?} kept its place",
            store.key(ix)
        );
    }
    let settled =
        length(net_force(&store, &metrics, larger, &state.positions)[stray].expect("core"));
    assert!(
        settled <= knocked / 10.0,
        "the stray node felt {knocked:.2} and still feels {settled:.2}"
    );
    let mean_edge = |positions: &[Placement]| {
        let mut sum = 0.0;
        for (a, b) in &edges {
            sum += distance(place(&store, positions, a), place(&store, positions, b));
        }
        sum / crate::layout::count(edges.len())
    };
    let ratio = mean_edge(&state.positions) / mean_edge(&first(&store, &metrics, FRAME).positions);
    assert!(
        (ratio - 2.0).abs() <= 0.2,
        "a frame of four times the area doubles the ideal distance; edges grew {ratio:.3}×"
    );
}
