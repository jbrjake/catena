//! The owner's "Relayout follow-ups" ruling (`docs/design/owner-rulings.md`): "it's not okay
//! for islands to be stretched, they need to pull together."

use super::*;

/// Nodes as `(key, label)`, or edges as `(from, to)`.
type Pairs = Vec<(String, String)>;

/// A `rows × cols` grid of nodes keyed and labelled `{prefix}{r}{c}`, as nodes and edges.
fn grid(prefix: &str, rows: usize, cols: usize) -> (Pairs, Pairs) {
    let key = |r: usize, c: usize| format!("{prefix}{r}{c}");
    let (mut nodes, mut edges) = (Vec::new(), Vec::new());
    for r in 0..rows {
        for c in 0..cols {
            nodes.push((key(r, c), key(r, c)));
            if c + 1 < cols {
                edges.push((key(r, c), key(r, c + 1)));
            }
            if r + 1 < rows {
                edges.push((key(r, c), key(r + 1, c)));
            }
        }
    }
    (nodes, edges)
}

/// The key starting with `prefix` whose node sits furthest left (or right) in `positions`.
fn extreme(store: &Store, positions: &[Placement], prefix: &str, right: bool) -> String {
    let x = |ix: NodeIx| positions[ix.slot()].expect("laid out").0;
    let side = |ix: NodeIx| if right { x(ix) } else { -x(ix) };
    store
        .nodes_in_order()
        .iter()
        .copied()
        .filter(|&ix| store.key(ix).is_some_and(|k| k.starts_with(prefix)))
        .max_by(|&a, &b| side(a).total_cmp(&side(b)))
        .and_then(|ix| store.key(ix).cloned())
        .expect("some node has the prefix")
}

/// Two grid islands laid out apart and then joined by one new node, `x`.
struct Joined {
    store: Store,
    /// The layout before `x` came, and after.
    before: Vec<Placement>,
    after: Vec<Placement>,
    /// The node of the larger grid (A) and of the smaller (B) that `x` joins.
    ends: (String, String),
    /// Both grids' own edges.
    edges: Pairs,
}

/// A 3 × 4 grid (A) and a 2 × 3 grid (B), laid out as islands with A left of B, then a new
/// node `x` joining A's and B's facing nodes, or with `far` their furthest ones.
fn join_grids(far: bool) -> Joined {
    let (mut nodes, mut edges) = grid("a", 3, 4);
    let (b_nodes, b_edges) = grid("b", 2, 3);
    nodes.extend(b_nodes);
    edges.extend(b_edges);
    let mut store = build(&nodes, &edges);
    let mut metrics = ResolvedMetrics::new(SemanticZoomTable::default(), at(2));
    metrics.measure_all(&store);
    let mut state = first(&store, &metrics, FRAME);
    let before = state.positions.clone();
    let ends = (
        extreme(&store, &before, "a", !far),
        extreme(&store, &before, "b", far),
    );
    let ((), delta) = store
        .transact(|tx| {
            tx.add_node("x".to_string(), NodeSpec::label("x"))?;
            tx.add_edge(&"x".to_string(), &ends.0, EdgeSpec::undirected())?;
            tx.add_edge(&"x".to_string(), &ends.1, EdgeSpec::undirected())?;
            Ok(())
        })
        .expect("fresh key, known ends");
    metrics.measure_all(&store);
    // The pull waits on the owner's ruling about its cost to invariant F (`pull_joined`).
    let params = ForceParams {
        pull_joined: true,
        ..ForceParams::default()
    };
    lay_out(
        &params,
        &store,
        &metrics,
        FRAME,
        Change::Topology(&delta.added),
        &mut state,
    );
    Joined {
        store,
        before,
        after: state.positions,
        ends,
        edges,
    }
}

#[test]
fn a_node_joining_two_islands_pulls_them_together() {
    // Joined on their facing sides, and on their far sides, where only turning B brings the
    // joined nodes face to face.
    for far in [false, true] {
        let Joined {
            store,
            before,
            after,
            ends: (ka, kb),
            edges,
        } = join_grids(far);

        // Stretched means drawn longer than the islands' own edges: a joining edge longer than
        // their median edge, or a joined node dragged out of its island, which lengthens the
        // edges tying it there. Without the pull, x's edges ended up to 1.57 medians long and
        // a joined node's edge grew from 13.7 to 36.7; with it, 1.41 and 1.23×.
        let length = |p: &[Placement], (a, b): (&str, &str)| {
            distance(place(&store, p, a), place(&store, p, b))
        };
        let mut own: Vec<f64> = edges.iter().map(|(a, b)| length(&before, (a, b))).collect();
        own.sort_by(f64::total_cmp);
        let usual = own[own.len() / 2];
        for end in [ka.as_str(), kb.as_str()] {
            let drawn = length(&after, ("x", end));
            assert!(
                drawn <= 1.5 * usual,
                "far sides {far}: x–{end} ends {drawn:.1} long; the islands' edges were {usual:.1}"
            );
        }
        let tying: Vec<(&str, &str)> = edges
            .iter()
            .filter(|(a, b)| [a, b].iter().any(|k| **k == ka || **k == kb))
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        assert!(
            tying.len() >= 2 + 2,
            "far sides {far}: each joined node has at least two edges of its own grid"
        );
        for edge in tying {
            let (was, is) = (length(&before, edge), length(&after, edge));
            assert!(
                is <= was * 4.0 / 3.0,
                "far sides {far}: {}–{} grew from {was:.1} to {is:.1}",
                edge.0,
                edge.1
            );
        }

        assert_only_the_smaller_island_moved_whole(&store, &before, &after, (&ka, &kb));
    }
}

/// A, the larger island, keeps its place past the reach of `x`; B moves as one body, so the
/// distances between its nodes past the reach are unchanged.
fn assert_only_the_smaller_island_moved_whole(
    store: &Store,
    before: &[Placement],
    after: &[Placement],
    (ka, kb): (&str, &str),
) {
    let reach = ForceParams::default().tether_reach;
    // A grid node's hops from x: its grid distance to the joined node, plus that node's hop.
    let hops_from = |key: &str, end: &str| {
        let (r, c) = (key.as_bytes()[1] - b'0', key.as_bytes()[2] - b'0');
        let (er, ec) = (end.as_bytes()[1] - b'0', end.as_bytes()[2] - b'0');
        u32::from(r.abs_diff(er) + c.abs_diff(ec)) + 1
    };
    let mut rigid = Vec::new();
    for &ix in store.nodes_in_order() {
        let key = store.key(ix).expect("live").clone();
        if key.starts_with('a') && hops_from(&key, ka) > reach {
            assert_eq!(after[ix.slot()], before[ix.slot()], "{key} of A moved");
        }
        if key.starts_with('b') && hops_from(&key, kb) > reach {
            rigid.push(ix);
        }
    }
    assert!(rigid.len() >= 2, "{} of B past the reach", rigid.len());
    for (n, &p) in rigid.iter().enumerate() {
        for &q in &rigid[n + 1..] {
            let was = distance(
                before[p.slot()].expect("placed"),
                before[q.slot()].expect("placed"),
            );
            let is = distance(
                after[p.slot()].expect("placed"),
                after[q.slot()].expect("placed"),
            );
            assert!(
                (is - was).abs() <= 1e-6 * was.max(1.0),
                "B's shape changed, {was:.3} became {is:.3}"
            );
        }
    }
}
