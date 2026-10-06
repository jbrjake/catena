//! Committing a transaction's buffer to the store (plan §4.2).

use super::store::{EdgeSlot, NodeSlot, slot_u32};
use super::tx::{Changes, EdgeEdit, NodeEdit};
use super::{Delta, DeltaClass, GraphStore, Key};

impl<K: Key> GraphStore<K> {
    /// Commits a finished transaction's changes and says what changed. Every change was
    /// validated as it was buffered, so this cannot fail.
    pub(super) fn commit(&mut self, changes: Changes<K>) -> Delta {
        let Some(class) = changes.class else {
            return Delta::default();
        };
        let mut delta = Delta {
            class: Some(class),
            reshaped: changes.reshaped,
            repinned: changes.repinned,
            ..Delta::default()
        };

        // Fresh slots join the tables vacant; the edits below fill the ones that kept a node.
        let base = slot_u32(self.nodes.len());
        self.nodes
            .resize_with(self.nodes.len() + changes.fresh_nodes as usize, || None);
        self.vacant_nodes
            .extend((0..changes.fresh_nodes).map(|i| base + i));
        let base = slot_u32(self.edges.len());
        self.edges
            .resize_with(self.edges.len() + changes.fresh_edges as usize, || None);
        self.vacant_edges
            .extend((0..changes.fresh_edges).map(|i| base + i));

        // Added edges carry ids above every committed one, so removals come first here, and
        // no edge ever takes a slot this transaction freed.
        for (id, edit) in changes.edges {
            match edit {
                EdgeEdit::Removed => {
                    let ix = self.edge_ids.remove(&id).expect("validated: a live edge");
                    self.edges[ix.slot()] = None;
                    self.vacant_edges.insert(ix.0);
                }
                EdgeEdit::Changed(spec) => {
                    let ix = self.edge_ids[&id];
                    self.edges[ix.slot()].as_mut().expect("a live edge").spec = spec;
                }
                EdgeEdit::Added {
                    ix,
                    from,
                    to,
                    spec,
                    alive: true,
                } => {
                    let rank = 0; // set by `reindex`
                    self.edges[ix.slot()] = Some(EdgeSlot {
                        id,
                        from,
                        to,
                        rank,
                        spec,
                    });
                    self.edge_ids.insert(id, ix);
                    self.vacant_edges.remove(&ix.0);
                }
                EdgeEdit::Added { alive: false, .. } => {}
            }
        }

        // Removals first: a key removed and added again may land in a lower slot than it left.
        let mut added = Vec::new();
        for (ix, edit) in changes.nodes {
            match edit {
                NodeEdit::Removed => {
                    let slot = self.nodes[ix.slot()]
                        .take()
                        .expect("validated: a live node");
                    self.lookup.remove(&slot.key);
                    self.vacant_nodes.insert(ix.0);
                    delta.removed.insert(ix);
                }
                NodeEdit::Changed(spec) => {
                    self.nodes[ix.slot()].as_mut().expect("a live node").spec = spec;
                }
                NodeEdit::Added(Some(node)) => added.push((ix, node)),
                NodeEdit::Added(None) => {}
            }
        }
        for (ix, (key, spec)) in added {
            self.lookup.insert(key.clone(), ix);
            self.nodes[ix.slot()] = Some(NodeSlot { key, spec });
            self.vacant_nodes.remove(&ix.0);
            delta.added.insert(ix);
        }

        self.next_edge_id = changes.next_edge_id;
        // Without a topology change the old order lists exactly the live nodes, so it can be
        // checked in place; with one, it is rebuilt anyway.
        if class == DeltaClass::Topology
            || (!changes.resorted.is_empty() && !self.order_holds(&changes.resorted))
        {
            self.reindex();
        }
        delta
    }
}
