//! The graph store (plan §4.1–§4.2): node and edge slots behind dense indices, the key
//! interning table, and the canonical order every output-bound iteration follows.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use super::{EdgeId, EdgeIx, EdgeSpec, Key, NodeIx, NodeSpec};

/// Slot ceilings: how many node and edge slots the store may hold. The defaults are the
/// index types' range, 2³² − 1 each (plan §4.1); tests lower them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
    pub(crate) nodes: u32,
    pub(crate) edges: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            nodes: u32::MAX,
            edges: u32::MAX,
        }
    }
}

/// A live node.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NodeSlot<K> {
    pub(crate) key: K,
    pub(crate) spec: NodeSpec,
}

/// A live edge.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EdgeSlot {
    pub(crate) id: EdgeId,
    pub(crate) from: NodeIx,
    pub(crate) to: NodeIx,
    /// Its insertion rank among the live edges joining the same two nodes, either way round.
    pub(crate) rank: u32,
    pub(crate) spec: EdgeSpec,
}

/// The canonical order of plan §4.1, rebuilt by [`GraphStore::reindex`] whenever topology or
/// a sort key changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Order {
    /// Live nodes by `(sort key, key)`.
    pub(crate) nodes: Vec<NodeIx>,
    /// Per slot, its node's place in `nodes`; `u32::MAX` for a vacant slot.
    pub(crate) place: Vec<u32>,
    /// Live edges by the places of their lower and higher end, then insertion (`EdgeId`).
    pub(crate) edges: Vec<EdgeIx>,
    /// Per slot, where its outgoing edges start in `out`; one entry more than there are slots.
    pub(crate) out_start: Vec<u32>,
    /// Every node's outgoing edges, each node's run in edge order.
    pub(crate) out: Vec<EdgeIx>,
    /// Per slot, where its incoming edges start in `inc`.
    pub(crate) in_start: Vec<u32>,
    /// Every node's incoming edges, each node's run in edge order.
    pub(crate) inc: Vec<EdgeIx>,
}

/// Nodes and edges in slots, the key interning table, and their canonical order.
#[derive(Debug, Clone)]
pub(crate) struct GraphStore<K> {
    pub(crate) nodes: Vec<Option<NodeSlot<K>>>,
    /// Vacant node slots, reused lowest first.
    pub(crate) vacant_nodes: BTreeSet<u32>,
    /// Key to slot. Used for lookup only: nothing iterates it (plan §4.1).
    pub(crate) lookup: HashMap<K, NodeIx>,
    pub(crate) edges: Vec<Option<EdgeSlot>>,
    pub(crate) vacant_edges: BTreeSet<u32>,
    /// Edge id to slot, for lookup only.
    pub(crate) edge_ids: HashMap<EdgeId, EdgeIx>,
    /// The next id to issue; past `u32::MAX`, ids are exhausted.
    pub(crate) next_edge_id: u64,
    pub(crate) max_label_cols: u16,
    pub(crate) limits: Limits,
    pub(crate) order: Order,
}

/// An edge as the host sees it: its id, its ends' keys, its rank and its spec (plan §12).
#[derive(Debug)]
pub struct EdgeRef<'g, K> {
    id: EdgeId,
    from: &'g K,
    to: &'g K,
    rank: u32,
    spec: &'g EdgeSpec,
}

impl<K> Clone for EdgeRef<'_, K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for EdgeRef<'_, K> {}

impl<'g, K> EdgeRef<'g, K> {
    /// The edge's id.
    #[must_use]
    pub fn id(&self) -> EdgeId {
        self.id
    }

    /// Its source's key.
    #[must_use]
    pub fn from(&self) -> &'g K {
        self.from
    }

    /// Its target's key.
    #[must_use]
    pub fn to(&self) -> &'g K {
        self.to
    }

    /// Its insertion rank, from 0, among the live edges joining the same two nodes either way
    /// round: a reciprocal pair fans like two parallel edges (plan §7.3).
    #[must_use]
    pub fn rank(&self) -> u32 {
        self.rank
    }

    /// Its spec, as stored.
    #[must_use]
    pub fn spec(&self) -> &'g EdgeSpec {
        self.spec
    }
}

impl<K: Key> GraphStore<K> {
    pub(crate) fn new(max_label_cols: u16, limits: Limits) -> Self {
        GraphStore {
            nodes: Vec::new(),
            vacant_nodes: BTreeSet::new(),
            lookup: HashMap::new(),
            edges: Vec::new(),
            vacant_edges: BTreeSet::new(),
            edge_ids: HashMap::new(),
            next_edge_id: 0,
            max_label_cols,
            limits,
            order: Order {
                out_start: vec![0],
                in_start: vec![0],
                ..Order::default()
            },
        }
    }

    pub(crate) fn node(&self, ix: NodeIx) -> Option<&NodeSlot<K>> {
        self.nodes.get(ix.slot()).and_then(Option::as_ref)
    }

    pub(crate) fn key(&self, ix: NodeIx) -> Option<&K> {
        self.node(ix).map(|slot| &slot.key)
    }

    pub(crate) fn ix_of(&self, key: &K) -> Option<NodeIx> {
        self.lookup.get(key).copied()
    }

    pub(crate) fn edge(&self, ix: EdgeIx) -> Option<&EdgeSlot> {
        self.edges.get(ix.slot()).and_then(Option::as_ref)
    }

    pub(crate) fn edge_ix(&self, id: EdgeId) -> Option<EdgeIx> {
        self.edge_ids.get(&id).copied()
    }

    pub(crate) fn edge_ref(&self, ix: EdgeIx) -> Option<EdgeRef<'_, K>> {
        let edge = self.edge(ix)?;
        Some(EdgeRef {
            id: edge.id,
            from: self.key(edge.from)?,
            to: self.key(edge.to)?,
            rank: edge.rank,
            spec: &edge.spec,
        })
    }

    pub(crate) fn node_count(&self) -> usize {
        self.lookup.len()
    }

    pub(crate) fn edge_count(&self) -> usize {
        self.edge_ids.len()
    }

    /// Live nodes in canonical order: by sort key, then key.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the force layout (M2) iterates in this order")
    )]
    pub(crate) fn nodes_in_order(&self) -> &[NodeIx] {
        &self.order.nodes
    }

    /// Live edges in canonical order: by the places of their lower and higher end in node
    /// order, then insertion.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the force layout (M2) iterates in this order")
    )]
    pub(crate) fn edges_in_order(&self) -> &[EdgeIx] {
        &self.order.edges
    }

    /// `ix`'s outgoing edges in canonical order; a self-loop is both outgoing and incoming.
    pub(crate) fn out_edges(&self, ix: NodeIx) -> &[EdgeIx] {
        run(&self.order.out_start, &self.order.out, ix)
    }

    /// `ix`'s incoming edges in canonical order.
    pub(crate) fn in_edges(&self, ix: NodeIx) -> &[EdgeIx] {
        run(&self.order.in_start, &self.order.inc, ix)
    }

    /// Rebuilds the canonical order, ranks and incidence from the slots. Slots are visited in
    /// index order but sorted by a total order, so the result depends on the graph alone.
    pub(crate) fn reindex(&mut self) {
        let GraphStore {
            nodes,
            edges,
            order,
            ..
        } = self;
        order.nodes.clear();
        order.nodes.extend(live(nodes).map(NodeIx::new));
        order
            .nodes
            .sort_unstable_by(|&a, &b| cmp_nodes(nodes, a, b));
        order.place.clear();
        order.place.resize(nodes.len(), u32::MAX);
        for (place, ix) in order.nodes.iter().enumerate() {
            order.place[ix.slot()] = slot_u32(place);
        }

        let place = &order.place;
        let pair = |edge: &EdgeSlot| {
            let (from, to) = (place[edge.from.slot()], place[edge.to.slot()]);
            (from.min(to), from.max(to))
        };
        let slot = |e: EdgeIx| edges[e.slot()].as_ref().expect("a listed edge is live");
        order.edges.clear();
        order.edges.extend(live(edges).map(EdgeIx::new));
        order
            .edges
            .sort_unstable_by_key(|&e| (pair(slot(e)), slot(e).id));
        let mut last = None;
        let mut rank = 0;
        for &e in &order.edges {
            let edge = edges[e.slot()].as_mut().expect("a listed edge is live");
            let (from, to) = (place[edge.from.slot()], place[edge.to.slot()]);
            let here = (from.min(to), from.max(to));
            rank = if last == Some(here) { rank + 1 } else { 0 };
            last = Some(here);
            edge.rank = rank;
        }

        let ends = |e: EdgeIx| {
            let edge = edges[e.slot()].as_ref().expect("a listed edge is live");
            (edge.from, edge.to)
        };
        let slots = nodes.len();
        csr(
            &mut order.out_start,
            &mut order.out,
            slots,
            &order.edges,
            |e| ends(e).0,
        );
        csr(
            &mut order.in_start,
            &mut order.inc,
            slots,
            &order.edges,
            |e| ends(e).1,
        );
    }

    /// Whether the canonical order still holds after the sort keys of `moved` changed: each
    /// still sorts after the node before it and before the node after it. Pairs of unmoved
    /// neighbours kept their keys, so this is every adjacent pair that could have broken.
    pub(crate) fn order_holds(&self, moved: &BTreeSet<NodeIx>) -> bool {
        let sorted = &self.order.nodes;
        moved.iter().all(|&ix| {
            let place = self.order.place[ix.slot()] as usize;
            let before = place.checked_sub(1).map(|p| sorted[p]);
            let after = sorted.get(place + 1).copied();
            before.is_none_or(|b| cmp_nodes(&self.nodes, b, ix).is_lt())
                && after.is_none_or(|a| cmp_nodes(&self.nodes, ix, a).is_lt())
        })
    }
}

/// The canonical node order: sort key (the label when absent), then key. Total, since keys
/// are unique.
fn cmp_nodes<K: Key>(nodes: &[Option<NodeSlot<K>>], a: NodeIx, b: NodeIx) -> Ordering {
    let node = |ix: NodeIx| nodes[ix.slot()].as_ref().expect("a listed node is live");
    let (a, b) = (node(a), node(b));
    a.spec
        .order_key()
        .cmp(b.spec.order_key())
        .then_with(|| a.key.cmp(&b.key))
}

/// The indices of a slot table's occupied slots, ascending.
fn live<T>(slots: &[Option<T>]) -> impl Iterator<Item = u32> + '_ {
    slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.is_some())
        .map(|(i, _)| slot_u32(i))
}

/// A slot index as the `u32` an index holds; the slot limits keep every table in range.
pub(super) fn slot_u32(slot: usize) -> u32 {
    u32::try_from(slot).expect("slot tables stay within the u32 limits")
}

/// Fills a compressed incidence list: `start[n]..start[n + 1]` is node `n`'s run of `list`,
/// in the order `edges` gives, by the end `end` picks.
fn csr(
    start: &mut Vec<u32>,
    list: &mut Vec<EdgeIx>,
    slots: usize,
    edges: &[EdgeIx],
    end: impl Fn(EdgeIx) -> NodeIx,
) {
    start.clear();
    start.resize(slots + 1, 0);
    for &e in edges {
        start[end(e).slot() + 1] += 1;
    }
    for n in 0..slots {
        start[n + 1] += start[n];
    }
    list.clear();
    list.resize(edges.len(), EdgeIx::new(0));
    // Each node's start serves as its fill cursor and ends up at the next node's start.
    for &e in edges {
        let cursor = &mut start[end(e).slot()];
        list[*cursor as usize] = e;
        *cursor += 1;
    }
    start.copy_within(0..slots, 1);
    start[0] = 0;
}

/// Field by field, for tests that check a discarded transaction left no trace.
#[cfg(test)]
impl<K: Key> PartialEq for GraphStore<K> {
    fn eq(&self, other: &Self) -> bool {
        self.nodes == other.nodes
            && self.vacant_nodes == other.vacant_nodes
            && self.lookup == other.lookup
            && self.edges == other.edges
            && self.vacant_edges == other.vacant_edges
            && self.edge_ids == other.edge_ids
            && self.next_edge_id == other.next_edge_id
            && self.max_label_cols == other.max_label_cols
            && self.limits == other.limits
            && self.order == other.order
    }
}

/// Node `ix`'s run of a CSR list, empty for a slot past the table.
fn run<'a>(start: &[u32], list: &'a [EdgeIx], ix: NodeIx) -> &'a [EdgeIx] {
    match (start.get(ix.slot()), start.get(ix.slot() + 1)) {
        (Some(&from), Some(&to)) => &list[from as usize..to as usize],
        _ => &[],
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
