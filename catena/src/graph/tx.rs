//! The all-or-nothing transaction (plan §4.2). Operations are validated against the store plus
//! what the transaction has already buffered, and nothing touches the store until the `update`
//! closure returns `Ok`; then [`GraphStore::transact`] commits the buffer in one step.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::spec::same_layout;
use super::{
    Delta, DeltaClass, EdgeId, EdgeIx, EdgeSpec, GraphError, GraphStore, Key, NodeIx, NodeSpec,
};

/// A pending change to one node slot.
#[derive(Debug, Clone)]
pub(super) enum NodeEdit<K> {
    /// The transaction put a node in this slot; `None` once it removed it again.
    Added(Option<(K, NodeSpec)>),
    /// A committed node's new spec.
    Changed(NodeSpec),
    /// The transaction removed this committed node.
    Removed,
}

/// A pending change to one edge.
#[derive(Debug, Clone)]
pub(super) enum EdgeEdit {
    /// The transaction added this edge; `alive` is false once it removed it again.
    Added {
        ix: EdgeIx,
        from: NodeIx,
        to: NodeIx,
        spec: EdgeSpec,
        alive: bool,
    },
    /// A committed edge's new spec.
    Changed(EdgeSpec),
    /// The transaction removed this committed edge.
    Removed,
}

/// Everything a transaction buffered, ready for the commit.
#[derive(Debug)]
pub(super) struct Changes<K> {
    pub(super) nodes: BTreeMap<NodeIx, NodeEdit<K>>,
    pub(super) edges: BTreeMap<EdgeId, EdgeEdit>,
    /// Fresh slots past the end of the slot tables, taken in index order.
    pub(super) fresh_nodes: u32,
    pub(super) fresh_edges: u32,
    pub(super) next_edge_id: u64,
    pub(super) class: Option<DeltaClass>,
    /// Committed nodes whose box may have changed.
    pub(super) reshaped: BTreeSet<NodeIx>,
    /// Committed nodes whose pin changed.
    pub(super) repinned: BTreeSet<NodeIx>,
    /// Committed nodes whose sort key changed.
    pub(super) resorted: BTreeSet<NodeIx>,
}

/// A transaction over a graph, handed to the `update` closure (plan §4.2). Every operation
/// either buffers its change and returns `Ok`, or returns `Err` and buffers nothing; the
/// closure's own `Ok` or `Err` then commits or discards the whole buffer.
///
/// A slot freed by this transaction is not reused before it commits, so no index returned
/// during the transaction ever names two nodes.
#[derive(Debug)]
pub struct Tx<'s, K: Key> {
    store: &'s GraphStore<K>,
    /// Keys this transaction added or removed: the slot each maps to now, or `None`.
    keys: HashMap<K, Option<NodeIx>>,
    /// Edges this transaction added, by each end, for `remove_node`'s cascade.
    added_incident: HashMap<NodeIx, Vec<EdgeId>>,
    /// The lowest vacant node and edge slot not yet taken may be at or after these.
    vacant_node_from: u32,
    vacant_edge_from: u32,
    changes: Changes<K>,
}

impl<'s, K: Key> Tx<'s, K> {
    fn new(store: &'s GraphStore<K>) -> Self {
        Tx {
            store,
            keys: HashMap::new(),
            added_incident: HashMap::new(),
            vacant_node_from: 0,
            vacant_edge_from: 0,
            changes: Changes {
                nodes: BTreeMap::new(),
                edges: BTreeMap::new(),
                fresh_nodes: 0,
                fresh_edges: 0,
                next_edge_id: store.next_edge_id,
                class: None,
                reshaped: BTreeSet::new(),
                repinned: BTreeSet::new(),
                resorted: BTreeSet::new(),
            },
        }
    }

    /// Adds a node and returns its index.
    ///
    /// # Errors
    ///
    /// [`GraphError::DuplicateNode`] if the graph already has `key`; [`GraphError::Capacity`]
    /// if every node index is taken.
    pub fn add_node(&mut self, key: K, mut spec: NodeSpec) -> Result<NodeIx, GraphError<K>> {
        if self.lookup(&key).is_some() {
            return Err(GraphError::DuplicateNode(key));
        }
        let store = self.store;
        let (slot, from, fresh) = next_slot(
            &store.vacant_nodes,
            self.vacant_node_from,
            store.nodes.len(),
            self.changes.fresh_nodes,
            store.limits.nodes,
        )
        .ok_or(GraphError::Capacity("nodes"))?;
        self.vacant_node_from = from;
        self.changes.fresh_nodes = fresh;
        spec.sanitize(store.max_label_cols);
        let ix = NodeIx::new(slot);
        self.keys.insert(key.clone(), Some(ix));
        self.changes
            .nodes
            .insert(ix, NodeEdit::Added(Some((key, spec))));
        self.mark(DeltaClass::Topology);
        Ok(ix)
    }

    /// Removes a node and every edge incident to it.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnknownNode`] if the graph has no `key`.
    pub fn remove_node(&mut self, key: &K) -> Result<(), GraphError<K>> {
        let ix = self
            .lookup(key)
            .ok_or_else(|| GraphError::UnknownNode(key.clone()))?;
        let store = self.store;
        // The committed incidence of the slot: empty when the slot was vacant at the start.
        let incident = store.out_edges(ix).iter().chain(store.in_edges(ix));
        for edge in incident.filter_map(|&e| store.edge(e)) {
            self.changes.edges.insert(edge.id, EdgeEdit::Removed);
        }
        for id in self.added_incident.remove(&ix).unwrap_or_default() {
            if let Some(EdgeEdit::Added { alive, .. }) = self.changes.edges.get_mut(&id) {
                *alive = false;
            }
        }
        match self.changes.nodes.get_mut(&ix) {
            Some(edit @ NodeEdit::Added(_)) => *edit = NodeEdit::Added(None),
            _ => {
                self.changes.nodes.insert(ix, NodeEdit::Removed);
            }
        }
        self.changes.reshaped.remove(&ix);
        self.changes.repinned.remove(&ix);
        self.changes.resorted.remove(&ix);
        self.keys.insert(key.clone(), None);
        self.mark(DeltaClass::Topology);
        Ok(())
    }

    /// Adds an edge from `from` to `to` and returns its id. Parallel edges and self-loops are
    /// edges like any other.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnknownNode`] if either end is missing, `from` checked first;
    /// [`GraphError::Capacity`] if edge ids or edge indices are exhausted.
    pub fn add_edge(
        &mut self,
        from: &K,
        to: &K,
        mut spec: EdgeSpec,
    ) -> Result<EdgeId, GraphError<K>> {
        let source = self
            .lookup(from)
            .ok_or_else(|| GraphError::UnknownNode(from.clone()))?;
        let target = self
            .lookup(to)
            .ok_or_else(|| GraphError::UnknownNode(to.clone()))?;
        let id = u32::try_from(self.changes.next_edge_id)
            .map(EdgeId)
            .map_err(|_| GraphError::Capacity("edge ids"))?;
        let store = self.store;
        let (slot, vacant_from, fresh) = next_slot(
            &store.vacant_edges,
            self.vacant_edge_from,
            store.edges.len(),
            self.changes.fresh_edges,
            store.limits.edges,
        )
        .ok_or(GraphError::Capacity("edges"))?;
        self.vacant_edge_from = vacant_from;
        self.changes.fresh_edges = fresh;
        self.changes.next_edge_id += 1;
        spec.sanitize();
        self.changes.edges.insert(
            id,
            EdgeEdit::Added {
                ix: EdgeIx::new(slot),
                from: source,
                to: target,
                spec,
                alive: true,
            },
        );
        self.added_incident.entry(source).or_default().push(id);
        if target != source {
            self.added_incident.entry(target).or_default().push(id);
        }
        self.mark(DeltaClass::Topology);
        Ok(id)
    }

    /// Removes an edge.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnknownEdge`] if the graph has no edge `id`.
    pub fn remove_edge(&mut self, id: EdgeId) -> Result<(), GraphError<K>> {
        match self.changes.edges.get_mut(&id) {
            Some(EdgeEdit::Added { alive, .. }) if *alive => *alive = false,
            Some(edit @ EdgeEdit::Changed(_)) => *edit = EdgeEdit::Removed,
            None if self.store.edge_ix(id).is_some() => {
                self.changes.edges.insert(id, EdgeEdit::Removed);
            }
            _ => return Err(GraphError::UnknownEdge(id)),
        }
        self.mark(DeltaClass::Topology);
        Ok(())
    }

    /// Edits a node's spec in place. An edit that leaves the spec as it was is no change.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnknownNode`] if the graph has no `key`.
    pub fn set_node(
        &mut self,
        key: &K,
        f: impl FnOnce(&mut NodeSpec),
    ) -> Result<(), GraphError<K>> {
        let unknown = || GraphError::UnknownNode(key.clone());
        let ix = self.lookup(key).ok_or_else(unknown)?;
        let store = self.store;
        let current = match self.changes.nodes.get(&ix) {
            Some(NodeEdit::Added(Some((_, spec))) | NodeEdit::Changed(spec)) => spec,
            // A live key names a live slot, so this is a committed node untouched so far.
            _ => &store.node(ix).ok_or_else(unknown)?.spec,
        };
        let mut next = current.clone();
        f(&mut next);
        next.sanitize(store.max_label_cols);
        let Some(class) = node_change(current, &next) else {
            return Ok(());
        };
        let reshaped = box_may_change(current, &next);
        let repinned = current.pinned != next.pinned;
        let resorted = current.order_key() != next.order_key();
        if let Some(NodeEdit::Added(Some((_, spec)))) = self.changes.nodes.get_mut(&ix) {
            *spec = next;
        } else {
            self.changes.nodes.insert(ix, NodeEdit::Changed(next));
            if reshaped {
                self.changes.reshaped.insert(ix);
            }
            if repinned {
                self.changes.repinned.insert(ix);
            }
            if resorted {
                self.changes.resorted.insert(ix);
            }
        }
        self.mark(class);
        Ok(())
    }

    /// Edits an edge's spec in place. An edit that leaves the spec as it was is no change.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnknownEdge`] if the graph has no edge `id`.
    pub fn set_edge(
        &mut self,
        id: EdgeId,
        f: impl FnOnce(&mut EdgeSpec),
    ) -> Result<(), GraphError<K>> {
        let store = self.store;
        let current = match self.changes.edges.get(&id) {
            Some(
                EdgeEdit::Added {
                    spec, alive: true, ..
                }
                | EdgeEdit::Changed(spec),
            ) => spec,
            Some(_) => return Err(GraphError::UnknownEdge(id)),
            None => {
                let edge = store.edge_ix(id).and_then(|ix| store.edge(ix));
                &edge.ok_or(GraphError::UnknownEdge(id))?.spec
            }
        };
        let mut next = current.clone();
        f(&mut next);
        next.sanitize();
        let class = if current.directed != next.directed
            || current.layout_participating != next.layout_participating
        {
            DeltaClass::Topology
        } else if *current != next {
            DeltaClass::Property
        } else {
            return Ok(());
        };
        if let Some(EdgeEdit::Added { spec, .. }) = self.changes.edges.get_mut(&id) {
            *spec = next;
        } else {
            self.changes.edges.insert(id, EdgeEdit::Changed(next));
        }
        self.mark(class);
        Ok(())
    }

    /// Whether the graph, as this transaction has changed it so far, has a node `key`.
    #[must_use]
    pub fn contains_node(&self, key: &K) -> bool {
        self.lookup(key).is_some()
    }

    /// Whether the graph, as this transaction has changed it so far, has an edge `id`.
    #[must_use]
    pub fn contains_edge(&self, id: EdgeId) -> bool {
        match self.changes.edges.get(&id) {
            Some(EdgeEdit::Added { alive, .. }) => *alive,
            Some(EdgeEdit::Changed(_)) => true,
            Some(EdgeEdit::Removed) => false,
            None => self.store.edge_ix(id).is_some(),
        }
    }

    /// The slot `key` names, as this transaction has changed the graph so far.
    fn lookup(&self, key: &K) -> Option<NodeIx> {
        match self.keys.get(key) {
            Some(&now) => now,
            None => self.store.ix_of(key),
        }
    }

    fn mark(&mut self, class: DeltaClass) {
        self.changes.class = self.changes.class.max(Some(class));
    }
}

/// The slot the next add takes, the lowest vacant one at or after `vacant_from` first and then
/// the next fresh one, with the `vacant_from` and fresh count after taking it; `None` at the
/// limit.
fn next_slot(
    vacant: &BTreeSet<u32>,
    vacant_from: u32,
    len: usize,
    fresh: u32,
    limit: u32,
) -> Option<(u32, u32, u32)> {
    if let Some(&slot) = vacant.range(vacant_from..).next() {
        return Some((slot, slot + 1, fresh));
    }
    let slot = u32::try_from(len).ok()?.checked_add(fresh)?;
    (slot < limit).then_some((slot, vacant_from, fresh + 1))
}

/// How a node edit changes the graph: geometry when its box may change or its pin moves,
/// property when only what is drawn in it or how it sorts does, nothing when the spec is
/// unchanged.
fn node_change(old: &NodeSpec, new: &NodeSpec) -> Option<DeltaClass> {
    if box_may_change(old, new) || old.pinned != new.pinned {
        Some(DeltaClass::Geometry)
    } else if old != new {
        Some(DeltaClass::Property)
    } else {
        None
    }
}

/// Whether a node's measured box may differ between two specs: its shape, the layout of its
/// label, or whether it carries the in-box pin marker (plan §5) changed.
fn box_may_change(old: &NodeSpec, new: &NodeSpec) -> bool {
    old.shape != new.shape
        || old.pinned.is_some() != new.pinned.is_some()
        || !same_layout(&old.label, &new.label)
}

impl<K: Key> GraphStore<K> {
    /// Runs `f` as one transaction: commits what it buffered if it returns `Ok`, and
    /// discards it otherwise. Returns `f`'s value and what the commit changed.
    pub(crate) fn transact<T>(
        &mut self,
        f: impl FnOnce(&mut Tx<'_, K>) -> Result<T, GraphError<K>>,
    ) -> Result<(T, Delta), GraphError<K>> {
        let mut tx = Tx::new(self);
        let out = f(&mut tx)?;
        let changes = tx.changes;
        Ok((out, self.commit(changes)))
    }
}

#[cfg(test)]
#[path = "tx_tests.rs"]
mod tests;
