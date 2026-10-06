//! [`GraphView`], the one object a host holds (plan §4.2, §13): the graph store now, and with
//! later milestones its positions, controller and caches.

use std::marker::PhantomData;

use crate::graph::{Delta, EdgeId, EdgeRef, GraphError, GraphStore, Key, Limits, NodeIx, Tx};

/// The default ceiling on a label's display columns (plan §4.1).
pub const DEFAULT_MAX_LABEL_COLS: u16 = 256;

/// An interactive graph keyed by the host's `K` (plan §4.2).
#[derive(Debug, Clone)]
pub struct GraphView<K: Key> {
    store: GraphStore<K>,
    /// Every commit since the layout last caught up, absorbed in order.
    pending: Delta,
}

/// Configures a [`GraphView`]; `GraphView::builder().build()` is a complete configuration.
#[derive(Debug, Clone)]
pub struct GraphViewBuilder<K: Key> {
    max_label_cols: u16,
    key: PhantomData<fn() -> K>,
}

impl<K: Key> GraphViewBuilder<K> {
    /// Cuts every label and sort key to `cols` display columns as it enters the graph
    /// (default [`DEFAULT_MAX_LABEL_COLS`]), so one hostile label cannot make a frame
    /// O(label).
    #[must_use]
    pub fn max_label_cols(mut self, cols: u16) -> Self {
        self.max_label_cols = cols;
        self
    }

    /// The configured view, with an empty graph.
    #[must_use]
    pub fn build(self) -> GraphView<K> {
        GraphView {
            store: GraphStore::new(self.max_label_cols, Limits::default()),
            pending: Delta::default(),
        }
    }
}

impl<K: Key> Default for GraphView<K> {
    fn default() -> Self {
        GraphView::builder().build()
    }
}

impl<K: Key> GraphView<K> {
    /// A builder with every default.
    #[must_use]
    pub fn builder() -> GraphViewBuilder<K> {
        GraphViewBuilder {
            max_label_cols: DEFAULT_MAX_LABEL_COLS,
            key: PhantomData,
        }
    }

    /// A view with every default: `GraphView::builder().build()`.
    #[must_use]
    pub fn new() -> Self {
        GraphView::default()
    }

    /// Runs one all-or-nothing transaction. When `f` returns `Ok`, everything it did commits
    /// at once; when it returns `Err` (or panics), nothing it did happened.
    ///
    /// # Errors
    ///
    /// Whatever `f` returns.
    pub fn update<T>(
        &mut self,
        f: impl FnOnce(&mut Tx<'_, K>) -> Result<T, GraphError<K>>,
    ) -> Result<T, GraphError<K>> {
        let (out, delta) = self.store.transact(f)?;
        self.pending.absorb(delta);
        Ok(out)
    }

    /// The key of the node at `ix`, or `None` if that slot holds no node.
    #[must_use]
    pub fn key(&self, ix: NodeIx) -> Option<&K> {
        self.store.key(ix)
    }

    /// The index of the node `key`, valid until the next commit.
    #[must_use]
    pub fn node_ix(&self, key: &K) -> Option<NodeIx> {
        self.store.ix_of(key)
    }

    /// The edge `id`: its ends, rank and spec.
    #[must_use]
    pub fn edge(&self, id: EdgeId) -> Option<EdgeRef<'_, K>> {
        self.store.edge_ref(self.store.edge_ix(id)?)
    }

    /// How many nodes the graph has.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.store.node_count()
    }

    /// How many edges the graph has.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.store.edge_count()
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
