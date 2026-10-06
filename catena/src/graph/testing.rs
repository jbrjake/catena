//! Helpers the graph module's tests share.

use super::{EdgeIx, GraphError, GraphStore, Limits, NodeSpec, Tx};

pub(super) type Store = GraphStore<&'static str>;
pub(super) type Res<T> = Result<T, GraphError<&'static str>>;

pub(super) fn store() -> Store {
    GraphStore::new(256, Limits::default())
}

/// A store holding `keys`, each labelled with its own name, added in one commit.
pub(super) fn with_nodes(keys: &[&'static str]) -> Store {
    let mut s = store();
    s.transact(|tx| add_all(tx, keys)).expect("fresh keys");
    s
}

pub(super) fn add_all(tx: &mut Tx<'_, &'static str>, keys: &[&'static str]) -> Res<()> {
    for &key in keys {
        tx.add_node(key, NodeSpec::label(key))?;
    }
    Ok(())
}

/// The store's live keys in canonical order.
pub(super) fn keys_in_order(s: &Store) -> Vec<&'static str> {
    s.nodes_in_order()
        .iter()
        .map(|&ix| *s.key(ix).expect("live node"))
        .collect()
}

/// Each edge as `(from, to, rank)`.
pub(super) fn named(s: &Store, edges: &[EdgeIx]) -> Vec<(&'static str, &'static str, u32)> {
    edges
        .iter()
        .map(|&e| {
            let edge = s.edge_ref(e).expect("live edge");
            (*edge.from(), *edge.to(), edge.rank())
        })
        .collect()
}
