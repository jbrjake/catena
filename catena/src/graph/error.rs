//! The core crate's whole error surface (plan §4.3): construction and mutation can fail;
//! rendering, layout, hit-testing and `tick` cannot.

use super::{EdgeId, Key};

/// Why a transaction operation failed. Returning it from the `update` closure discards the
/// whole transaction.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GraphError<K: Key> {
    /// `add_node` with a key the graph already has.
    #[error("node {0:?} already exists")]
    DuplicateNode(K),
    /// A key the graph does not have.
    #[error("no node {0:?}")]
    UnknownNode(K),
    /// An edge id the graph does not have, including one whose edge was removed.
    #[error("no edge {0:?}")]
    UnknownEdge(EdgeId),
    /// The graph would pass an index's range: `"nodes"`, `"edges"` or `"edge ids"`.
    #[error("capacity exceeded: {0}")]
    Capacity(&'static str),
}
