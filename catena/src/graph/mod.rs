//! The graph model: keys, dense indices, the store and its deltas (plan §4).
//!
//! The host names nodes by its own key `K` and edges by the [`EdgeId`] that adding one returns.
//! Inside, every node and edge lives in a slot behind a dense index ([`NodeIx`], [`EdgeIx`]),
//! which is what scenes, outcomes and every hot path carry. Mutation goes through one
//! all-or-nothing transaction ([`Tx`]), and each commit is classified by the strongest change
//! it made (`Topology > Geometry > Property`), which is how a property edit is kept from moving
//! the graph.
//!
//! Slots are reused, lowest first, once the transaction that freed them commits, so an index is
//! only meaningful until the next commit; [`EdgeId`]s are never reused. Nothing reaches the
//! output in slot order: every iteration that can is in the canonical order of plan §4.1, nodes
//! by `(sort key, key)` and edges by their endpoints' places in that order, then insertion.

mod commit;
mod delta;
mod error;
mod spec;
mod store;
mod tx;

pub(crate) use delta::{Delta, DeltaClass};
pub use error::GraphError;
pub use spec::{EdgeClass, EdgeSpec, NodeShape, NodeSpec};
pub use store::EdgeRef;
pub(crate) use store::{GraphStore, Limits};
pub use tx::Tx;

#[cfg(test)]
mod oracle_tests;
#[cfg(test)]
mod testing;

/// A host's node identity: `String`, `u64`, a UUID, anything with these traits (plan §4.1).
///
/// `Ord` is the determinism backbone: wherever order reaches the output, ties between nodes
/// break by key. `Hash` serves the interning table, which is used for lookup only.
pub trait Key: Clone + Eq + std::hash::Hash + Ord + std::fmt::Debug + 'static {}

impl<T: Clone + Eq + std::hash::Hash + Ord + std::fmt::Debug + 'static> Key for T {}

/// A node's dense index: what scenes, outcomes and every hot path carry instead of the host's
/// key (plan §4.1). Opaque: no arithmetic, and only the store makes one. A removed node's slot
/// is reused, so an index names the same node only until the next commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeIx(u32);

/// An edge's dense index (plan §4.1); opaque, and reused after removal, like [`NodeIx`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeIx(u32);

/// The host's handle on an edge, returned by [`Tx::add_edge`] (plan §4.1). Ids come from a
/// counter that never goes back, so an id held past its edge's removal fails as
/// [`GraphError::UnknownEdge`] instead of naming a newer edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeId(u32);

impl NodeIx {
    pub(crate) const fn new(index: u32) -> Self {
        NodeIx(index)
    }

    /// The slot this index names.
    pub(crate) const fn slot(self) -> usize {
        self.0 as usize
    }
}

impl EdgeIx {
    pub(crate) const fn new(index: u32) -> Self {
        EdgeIx(index)
    }

    /// The slot this index names.
    pub(crate) const fn slot(self) -> usize {
        self.0 as usize
    }
}
