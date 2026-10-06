//! What a commit changed, classified so that a property edit never moves the graph
//! (plan §4.2, §11.1).

use std::collections::BTreeSet;

use super::NodeIx;

/// How strong a change is, weakest first, so the strongest of several is their `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum DeltaClass {
    /// Re-render only; positions are untouched. Label text that lays out as before, a sort
    /// key, a node or edge weight, an edge class.
    Property,
    /// A local re-snap of the reshaped nodes. Label text that lays out differently, a shape,
    /// a pin.
    Geometry,
    /// An incremental relayout. A node or edge added or removed, an edge's direction or
    /// layout participation flipped.
    Topology,
}

/// What one commit, or several absorbed in order, changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Delta {
    /// The strongest change; `None` when nothing changed.
    pub(crate) class: Option<DeltaClass>,
    /// Slots that gained a node, which enters the layout fresh (ramped in when warm, §8.3).
    pub(crate) added: BTreeSet<NodeIx>,
    /// Slots that lost their node, whose layout state is dropped. A slot can be in both sets:
    /// its old node left and a new one took the slot.
    pub(crate) removed: BTreeSet<NodeIx>,
    /// Surviving nodes whose box may have changed, to re-snap. Never an added slot.
    pub(crate) reshaped: BTreeSet<NodeIx>,
}

impl Delta {
    /// Folds `later`, a delta committed after this one, into this one.
    pub(crate) fn absorb(&mut self, later: Delta) {
        self.class = self.class.max(later.class);
        for ix in later.removed {
            self.added.remove(&ix);
            self.reshaped.remove(&ix);
            self.removed.insert(ix);
        }
        self.added.extend(later.added);
        for ix in later.reshaped {
            if !self.added.contains(&ix) {
                self.reshaped.insert(ix);
            }
        }
    }
}
