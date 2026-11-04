//! `fuzz_topology` (plan §16.7, target 2): arbitrary transactions of mutations, interleaved with
//! input and renders. No panic; after every commit each live key maps to an index that maps
//! back to it and no other index is live (no `NodeIx` leaks after removals), every committed
//! edge is live exactly while it has not been removed, with live ends (edge conservation); and
//! every frame drawn holds invariants A, B and C.
#![no_main]

use arbitrary::Arbitrary;
use catena::graph::EdgeId;
use catena::{EdgeSpec, GraphView, NodeSpec};
use catena_fuzz::{Event, apply, check_frame};
use libfuzzer_sys::fuzz_target;

/// One edit inside a transaction. Nodes are keyed `k0` to `k255`; an edge is named by its place
/// among the edges committed so far, which may have been removed since.
#[derive(Arbitrary, Debug)]
enum Mutation {
    AddNode(u8, String),
    RemoveNode(u8),
    AddEdge(u8, u8, bool),
    RemoveEdge(u16),
    SetLabel(u8, String),
    Pin(u8, Option<(f64, f64)>),
    Weigh(u8, f32),
}

#[derive(Arbitrary, Debug)]
enum Op {
    /// One all-or-nothing transaction.
    Commit(Vec<Mutation>),
    Input(Event),
}

fn key(k: u8) -> String {
    format!("k{k}")
}

/// Checks the store's keys and edges against what was committed.
fn check_store(gv: &GraphView<String>, edges: &[EdgeId]) {
    let mut live = 0;
    for k in 0..=u8::MAX {
        if let Some(ix) = gv.node_ix(&key(k)) {
            live += 1;
            assert_eq!(gv.key(ix), Some(&key(k)), "an index maps back to its key");
        }
    }
    assert_eq!(
        live,
        gv.node_count(),
        "no node outside the keys, no leaked index"
    );
    let mut alive = 0;
    for &id in edges {
        if let Some(edge) = gv.edge(id) {
            alive += 1;
            assert_eq!(edge.id(), id);
            assert!(gv.node_ix(edge.from()).is_some() && gv.node_ix(edge.to()).is_some());
        }
    }
    assert_eq!(alive, gv.edge_count(), "every live edge was committed");
}

fuzz_target!(|ops: Vec<Op>| {
    let mut gv: GraphView<String> = GraphView::new();
    let mut edges: Vec<EdgeId> = Vec::new();
    let mut size = (80, 24);
    for op in ops.iter().take(64) {
        match op {
            Op::Commit(mutations) => {
                let mut added = Vec::new();
                let committed = gv.update(|tx| {
                    for m in mutations.iter().take(32) {
                        match m {
                            Mutation::AddNode(k, label) => {
                                tx.add_node(key(*k), NodeSpec::label(label.clone()))?;
                            }
                            Mutation::RemoveNode(k) => tx.remove_node(&key(*k))?,
                            Mutation::AddEdge(a, b, directed) => {
                                let spec = if *directed {
                                    EdgeSpec::directed()
                                } else {
                                    EdgeSpec::undirected()
                                };
                                added.push(tx.add_edge(&key(*a), &key(*b), spec)?);
                            }
                            Mutation::RemoveEdge(i) => {
                                if !edges.is_empty() {
                                    tx.remove_edge(edges[usize::from(*i) % edges.len()])?;
                                }
                            }
                            Mutation::SetLabel(k, label) => {
                                tx.set_node(&key(*k), |n| n.label.clone_from(label))?;
                            }
                            Mutation::Pin(k, pin) => tx.set_node(&key(*k), |n| n.pinned = *pin)?,
                            Mutation::Weigh(k, w) => tx.set_node(&key(*k), |n| n.weight = *w)?,
                        }
                    }
                    Ok(())
                });
                if committed.is_ok() {
                    edges.extend(added);
                }
                check_store(&gv, &edges);
            }
            Op::Input(event) => apply(&mut gv, event, &mut size),
        }
    }
    check_frame(&mut gv, size);
});
