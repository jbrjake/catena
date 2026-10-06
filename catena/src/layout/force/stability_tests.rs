//! Invariant F (plan §16.2-F, §11.3): a warm relayout after adding at most 5% new nodes moves
//! at least 90% of the survivors by at most 2 cells, over the seeded generated families.

use catena_testkit::fixtures::{FixtureGraph, SplitMix64, generated};

use super::*;
use crate::geometry::SemanticZoomTable;
use crate::geometry::testing::at;
use crate::graph::{EdgeSpec, Limits, NodeSpec};

type Store = GraphStore<String>;

/// A slot's world position, if it has one.
type Placement = Option<(f64, f64)>;

/// A 120 × 40-cell viewport, as world units.
const FRAME: Frame = Frame {
    area: (120.0, 80.0),
    cell_aspect: 0.5,
};

/// Adds the graph's nodes that `admit` takes, and the edges that now join two present nodes
/// and touch an admitted one, in one commit; returns that commit's added set.
fn add(store: &mut Store, graph: &FixtureGraph, admit: impl Fn(usize) -> bool) -> BTreeSet<NodeIx> {
    let admitted = |key: &String| {
        graph
            .nodes
            .iter()
            .position(|n| &n.key == key)
            .is_some_and(&admit)
    };
    let ((), delta) = store
        .transact(|tx| {
            for (i, node) in graph.nodes.iter().enumerate() {
                if admit(i) {
                    tx.add_node(node.key.clone(), NodeSpec::label(node.label.clone()))?;
                }
            }
            for edge in &graph.edges {
                let (from, to) = (&edge.source, &edge.target);
                if tx.contains_node(from)
                    && tx.contains_node(to)
                    && (admitted(from) || admitted(to))
                {
                    tx.add_edge(from, to, EdgeSpec::directed())?;
                }
            }
            Ok(())
        })
        .expect("generated graphs are consistent");
    delta.added
}

/// The share of `survivors` that moved at most 2 cells from `before` to `after`, at the scale
/// the snapper's `Fit::Contain` would give the later layout (plan §6): columns from world x,
/// rows from world y times the cell aspect.
fn share_within_two_cells(
    store: &Store,
    metrics: &ResolvedMetrics,
    (before, after): (&[Placement], &[Placement]),
    survivors: &[NodeIx],
) -> f64 {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &ix in store.nodes_in_order() {
        let (x, y) = after[ix.slot()].expect("laid out");
        let form = metrics.form(ix).expect("measured");
        let half_w = f64::from(form.width()) / 2.0;
        let half_h = f64::from(form.height()) / FRAME.cell_aspect / 2.0;
        (x0, y0) = (x0.min(x - half_w), y0.min(y - half_h));
        (x1, y1) = (x1.max(x + half_w), y1.max(y + half_h));
    }
    let scale = (FRAME.area.0 / (x1 - x0)).min(FRAME.area.1 / (y1 - y0));
    let still = survivors
        .iter()
        .filter(|&&ix| {
            let (a, b) = (before[ix.slot()], after[ix.slot()]);
            let ((ax, ay), (bx, by)) = (a.expect("placed"), b.expect("placed"));
            let columns = (bx - ax) * scale;
            let rows = (by - ay) * scale * FRAME.cell_aspect;
            (columns * columns + rows * rows).sqrt() <= 2.0
        })
        .count();
    crate::layout::count(still) / crate::layout::count(survivors.len())
}

/// Lays out `seed`'s graph of `n` nodes less `held_out`, then adds those back and relays out
/// warm; returns the share of survivors that stayed within two cells.
fn relayout_share(seed: u64, n: usize, held_out: &BTreeSet<usize>) -> f64 {
    let params = ForceParams::default();
    let graph = generated(seed, n);
    let mut store: Store = GraphStore::new(256, Limits::default());
    let added = add(&mut store, &graph, |i| !held_out.contains(&i));
    let mut metrics = ResolvedMetrics::new(SemanticZoomTable::default(), at(2));
    metrics.measure_all(&store);
    let mut positions = Vec::new();
    lay_out(&params, &store, &metrics, FRAME, &added, &mut positions);
    let before = positions.clone();
    let survivors: Vec<NodeIx> = store.nodes_in_order().to_vec();

    let added = add(&mut store, &graph, |i| held_out.contains(&i));
    metrics.measure_all(&store);
    lay_out(&params, &store, &metrics, FRAME, &added, &mut positions);
    share_within_two_cells(&store, &metrics, (&before, &positions), &survivors)
}

#[test]
fn a_warm_relayout_after_five_percent_additions_keeps_nine_in_ten_survivors_within_two_cells() {
    for n in [60, 120] {
        for seed in 0..12 {
            let mut rng = SplitMix64::new(seed ^ 0x5eed);
            let n64 = u64::try_from(n).expect("small");
            let held_out: BTreeSet<usize> = (0..n / 20)
                .map(|_| usize::try_from(rng.below(n64)).expect("below n"))
                .collect();
            let share = relayout_share(seed, n, &held_out);
            assert!(
                share >= 0.9,
                "seed {seed}, {n} nodes: only {share:.3} stayed within two cells"
            );
        }
    }
}

#[test]
fn a_warm_relayout_of_an_unchanged_graph_keeps_every_node_within_two_cells() {
    // The seed's cooling (span / 2, × 0.95 a step) freezes the cold layout below
    // `converge_eps` near step 94, before it reaches equilibrium, so a warm restart at
    // span / 8 used to relax it: here as many as 97% of nodes moved more than two cells.
    for seed in 0..8 {
        let share = relayout_share(seed, 120, &BTreeSet::new());
        assert_eq!(
            share, 1.0,
            "seed {seed}: {share:.3} stayed within two cells"
        );
    }
}
