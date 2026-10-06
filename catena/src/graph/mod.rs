//! The graph model: keys, dense indices, the store and its deltas (plan §4). M1 brings the
//! indices the scene carries; the store arrives with M2.

/// A node's dense index: what scenes, outcomes and every hot path carry instead of the host's
/// key (plan §4.1). Opaque: no arithmetic, and only the store makes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeIx(u32);

/// An edge's dense index (plan §4.1); opaque like [`NodeIx`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeIx(u32);

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the M2 store is the first to make indices")
)]
impl NodeIx {
    pub(crate) const fn new(index: u32) -> Self {
        NodeIx(index)
    }
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the M2 store is the first to make indices")
)]
impl EdgeIx {
    pub(crate) const fn new(index: u32) -> Self {
        EdgeIx(index)
    }
}
