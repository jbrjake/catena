//! The store against a slow, obviously correct model: maps keyed by the host's key and the
//! edge id, every order computed by sorting from scratch, every rank by counting.

use std::collections::BTreeMap;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use super::{DeltaClass, EdgeId, EdgeIx, EdgeSpec, GraphError, GraphStore, Limits, NodeSpec, Tx};

/// Four labels: two that lay out alike, one wider, and the empty one.
const LABELS: [&str; 4] = ["a", "b", "日", ""];
/// Eight keys, so keys collide and ties on label are common.
const KEYS: u8 = 8;
/// The most operations in one transaction.
const OPS: usize = 12;

#[derive(Debug, Clone)]
enum Op {
    AddNode(u8, usize),
    RemoveNode(u8),
    AddEdge(u8, u8, bool),
    /// Removes the edge with this id, once [`Model::resolve`] has brought it into range.
    RemoveEdge(u32),
    SetLabel(u8, usize),
    SetSortKey(u8, Option<usize>),
    SetDirected(u32, bool),
}

fn op() -> impl Strategy<Value = Op> {
    let key = 0..KEYS;
    let label = 0..LABELS.len();
    prop_oneof![
        3 => (key.clone(), label.clone()).prop_map(|(k, l)| Op::AddNode(k, l)),
        1 => key.clone().prop_map(Op::RemoveNode),
        4 => (key.clone(), key.clone(), any::<bool>()).prop_map(|(f, t, d)| Op::AddEdge(f, t, d)),
        1 => any::<u32>().prop_map(Op::RemoveEdge),
        2 => (key.clone(), label.clone()).prop_map(|(k, l)| Op::SetLabel(k, l)),
        1 => (key, prop::option::of(label)).prop_map(|(k, l)| Op::SetSortKey(k, l)),
        1 => (any::<u32>(), any::<bool>()).prop_map(|(e, d)| Op::SetDirected(e, d)),
    ]
}

/// The model's graph: nodes by key, edges by id as `(from, to, directed)`.
#[derive(Debug, Clone, Default)]
struct Model {
    nodes: BTreeMap<u8, NodeSpec>,
    edges: BTreeMap<u32, (u8, u8, bool)>,
    next_id: u32,
}

/// Whether two of the four labels lay out alike, by table rather than by `same_layout`.
fn alike(a: &str, b: &str) -> bool {
    let room = |l: &str| match l {
        "a" | "b" => 1,
        "日" => 2,
        _ => 0,
    };
    room(a) == room(b)
}

impl Model {
    /// `op` with any edge id taken modulo two past the last id issued before the transaction,
    /// so it names a live edge, a removed one, or one of the first two the transaction adds.
    fn resolve(&self, op: &Op) -> Op {
        let id = |n: u32| n % (self.next_id + 2);
        match *op {
            Op::RemoveEdge(n) => Op::RemoveEdge(id(n)),
            Op::SetDirected(n, d) => Op::SetDirected(id(n), d),
            ref other => other.clone(),
        }
    }

    /// Applies `op`: `Ok` with its class (`None` when it changes nothing), or the error the
    /// store must return, by kind.
    fn apply(&mut self, op: &Op) -> Result<Option<DeltaClass>, &'static str> {
        use DeltaClass::{Geometry, Property, Topology};
        match *op {
            Op::AddNode(k, l) => {
                if self.nodes.contains_key(&k) {
                    return Err("duplicate");
                }
                self.nodes.insert(k, NodeSpec::label(LABELS[l]));
                Ok(Some(Topology))
            }
            Op::RemoveNode(k) => {
                self.nodes.remove(&k).ok_or("unknown node")?;
                self.edges.retain(|_, &mut (f, t, _)| f != k && t != k);
                Ok(Some(Topology))
            }
            Op::AddEdge(f, t, d) => {
                if !self.nodes.contains_key(&f) || !self.nodes.contains_key(&t) {
                    return Err("unknown node");
                }
                self.edges.insert(self.next_id, (f, t, d));
                self.next_id += 1;
                Ok(Some(Topology))
            }
            Op::RemoveEdge(id) => {
                self.edges.remove(&id).ok_or("unknown edge")?;
                Ok(Some(Topology))
            }
            Op::SetLabel(k, l) => {
                let spec = self.nodes.get_mut(&k).ok_or("unknown node")?;
                let class = if spec.label == LABELS[l] {
                    None
                } else if alike(&spec.label, LABELS[l]) {
                    Some(Property)
                } else {
                    Some(Geometry)
                };
                spec.label = LABELS[l].to_string();
                Ok(class)
            }
            Op::SetSortKey(k, l) => {
                let spec = self.nodes.get_mut(&k).ok_or("unknown node")?;
                let key = l.map(|l| LABELS[l].to_string());
                let class = (spec.sort_key != key).then_some(Property);
                spec.sort_key = key;
                Ok(class)
            }
            Op::SetDirected(id, d) => {
                let edge = self.edges.get_mut(&id).ok_or("unknown edge")?;
                let class = (edge.2 != d).then_some(Topology);
                edge.2 = d;
                Ok(class)
            }
        }
    }

    /// Keys in canonical order, by sorting.
    fn order(&self) -> Vec<u8> {
        let mut keys: Vec<u8> = self.nodes.keys().copied().collect();
        keys.sort_by(|a, b| {
            let (sa, sb) = (&self.nodes[a], &self.nodes[b]);
            sa.order_key().cmp(sb.order_key()).then(a.cmp(b))
        });
        keys
    }

    /// Edges in canonical order as `(id, from, to, rank)`, ranks by counting.
    fn edge_order(&self) -> Vec<(u32, u8, u8, u32)> {
        let order = self.order();
        let place = |k: u8| order.iter().position(|&o| o == k).expect("live end");
        let pair = |f: u8, t: u8| (place(f).min(place(t)), place(f).max(place(t)));
        let mut edges: Vec<(u32, u8, u8, u32)> = self
            .edges
            .iter()
            .map(|(&id, &(f, t, _))| {
                let rank = self
                    .edges
                    .iter()
                    .filter(|&(&other, &(of, ot, _))| other < id && pair(of, ot) == pair(f, t))
                    .count();
                (id, f, t, u32::try_from(rank).expect("small"))
            })
            .collect();
        edges.sort_by_key(|&(id, f, t, _)| (pair(f, t), id));
        edges
    }
}

/// Applies a resolved `op` to the store's transaction, reporting an error by kind.
fn store_op(tx: &mut Tx<'_, u8>, op: &Op) -> Result<(), &'static str> {
    let kind = |e: GraphError<u8>| match e {
        GraphError::DuplicateNode(_) => "duplicate",
        GraphError::UnknownNode(_) => "unknown node",
        GraphError::UnknownEdge(_) => "unknown edge",
        GraphError::Capacity(_) => "capacity",
    };
    let id = EdgeId;
    match *op {
        Op::AddNode(k, l) => tx.add_node(k, NodeSpec::label(LABELS[l])).map(|_| ()),
        Op::RemoveNode(k) => tx.remove_node(&k),
        Op::AddEdge(f, t, d) => {
            let spec = if d {
                EdgeSpec::directed()
            } else {
                EdgeSpec::undirected()
            };
            tx.add_edge(&f, &t, spec).map(|_| ())
        }
        Op::RemoveEdge(n) => tx.remove_edge(id(n)),
        Op::SetLabel(k, l) => tx.set_node(&k, |s| s.label = LABELS[l].to_string()),
        Op::SetSortKey(k, l) => tx.set_node(&k, |s| s.sort_key = l.map(|l| LABELS[l].to_string())),
        Op::SetDirected(n, d) => tx.set_edge(id(n), |s| s.directed = d),
    }
    .map_err(kind)
}

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x6772_6170),
        failure_persistence: None,
        ..Config::default()
    }
}

proptest! {
    #![proptest_config(config())]

    /// Every transaction, committed or discarded, leaves the store equal to the model: the
    /// same nodes and edges, the same canonical orders, ranks and incident lists, the same
    /// class of change, and no more slots than the most nodes it ever held at once.
    #[test]
    fn the_store_matches_a_model_through_any_transactions(
        txs in prop::collection::vec((prop::collection::vec(op(), 0..OPS), any::<bool>()), 1..12)
    ) {
        let mut s = GraphStore::<u8>::new(256, Limits::default());
        let mut model = Model::default();
        for (ops, commit) in &txs {
            let ops: Vec<Op> = ops.iter().map(|op| model.resolve(op)).collect();
            let mut next = model.clone();
            let expected: Vec<_> = ops.iter().map(|op| next.apply(op)).collect();
            let mut got = Vec::new();
            let result = s.transact(|tx| {
                for op in &ops {
                    got.push(store_op(tx, op));
                }
                if *commit { Ok(()) } else { Err(GraphError::Capacity("discarded")) }
            });
            let kinds: Vec<Result<(), &str>> = expected.iter().map(|r| r.map(|_| ())).collect();
            prop_assert_eq!(got, kinds);
            if *commit {
                let class = expected.iter().filter_map(|r| r.ok().flatten()).max();
                prop_assert_eq!(result.expect("committed").1.class, class);
                model = next;
            } else {
                prop_assert!(result.is_err());
            }
            check(&s, &model)?;
        }
    }
}

fn check(s: &GraphStore<u8>, model: &Model) -> Result<(), TestCaseError> {
    let keys: Vec<u8> = s
        .nodes_in_order()
        .iter()
        .map(|&ix| *s.key(ix).expect("live"))
        .collect();
    prop_assert_eq!(keys, model.order());
    for (k, spec) in &model.nodes {
        let ix = s.ix_of(k).expect("modelled node");
        prop_assert_eq!(&s.node(ix).expect("live").spec, spec);
    }

    let edges: Vec<(u32, u8, u8, u32)> = s
        .edges_in_order()
        .iter()
        .map(|&e| {
            let edge = s.edge(e).expect("live");
            (
                edge.id.0,
                *s.key(edge.from).expect("end"),
                *s.key(edge.to).expect("end"),
                edge.rank,
            )
        })
        .collect();
    let expected = model.edge_order();
    prop_assert_eq!(&edges, &expected);
    for &(id, ..) in &expected {
        let spec = &s
            .edge(s.edge_ix(EdgeId(id)).expect("modelled edge"))
            .expect("live")
            .spec;
        prop_assert_eq!(spec.directed, model.edges[&id].2);
    }
    for id in 0..model.next_id {
        prop_assert_eq!(
            s.edge_ix(EdgeId(id)).is_some(),
            model.edges.contains_key(&id)
        );
    }
    prop_assert_eq!(s.next_edge_id, u64::from(model.next_id));

    for &k in model.nodes.keys() {
        let ix = s.ix_of(&k).expect("live");
        let ids = |list: &[EdgeIx]| {
            list.iter()
                .map(|&e| s.edge(e).expect("live").id.0)
                .collect::<Vec<_>>()
        };
        let out: Vec<u32> = expected.iter().filter(|e| e.1 == k).map(|e| e.0).collect();
        let inc: Vec<u32> = expected.iter().filter(|e| e.2 == k).map(|e| e.0).collect();
        prop_assert_eq!(ids(s.out_edges(ix)), out);
        prop_assert_eq!(ids(s.in_edges(ix)), inc);
    }

    let vacant: Vec<u32> = (0..s.nodes.len())
        .filter(|&i| s.nodes[i].is_none())
        .map(|i| u32::try_from(i).expect("small"))
        .collect();
    prop_assert_eq!(s.vacant_nodes.iter().copied().collect::<Vec<_>>(), vacant);
    let vacant: Vec<u32> = (0..s.edges.len())
        .filter(|&i| s.edges[i].is_none())
        .map(|i| u32::try_from(i).expect("small"))
        .collect();
    prop_assert_eq!(s.vacant_edges.iter().copied().collect::<Vec<_>>(), vacant);
    prop_assert!(
        s.nodes.len() <= usize::from(KEYS) + OPS,
        "{} node slots",
        s.nodes.len()
    );
    Ok(())
}
