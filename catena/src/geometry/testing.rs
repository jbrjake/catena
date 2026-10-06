//! Helpers the tests of node measurement and of what reads it share.

use proptest::prelude::*;

use super::metrics::{NodeForm, measure};
use super::zoom::{SemanticZoom, SemanticZoomTable};
use crate::graph::{NodeShape, NodeSpec};

/// A zoom inside each level of the default table.
const ZOOMS: [f64; 6] = [0.2, 0.33, 1.0, 2.0, 3.0, 4.0];

/// Level `level` of the default table.
pub(crate) fn at(level: usize) -> SemanticZoom {
    SemanticZoomTable::default().level(ZOOMS[level])
}

/// `spec` measured at `level` of `table`: how a test outside `geometry/` gets a form without a
/// graph store.
pub(crate) fn measured(
    spec: &NodeSpec,
    level: SemanticZoom,
    table: &SemanticZoomTable,
) -> NodeForm {
    measure(spec, level, table)
}

/// Each of `specs` measured at level `level` of the default table.
pub(crate) fn forms(specs: &[NodeSpec], level: usize) -> Vec<NodeForm> {
    let table = SemanticZoomTable::default();
    specs
        .iter()
        .map(|spec| measure(spec, at(level), &table))
        .collect()
}

/// The default table at each of its levels, then level 1 of tables capping it at every width
/// from 1 to 24, so every cut point of a short label is reached.
pub(crate) fn every_level_and_cap() -> Vec<(SemanticZoom, SemanticZoomTable)> {
    let upper = [0.30, 0.35, 1.5, 2.5, 3.5];
    let defaults = (0..6).map(|level| (at(level), SemanticZoomTable::default()));
    let caps = (1..=24).map(|cap| {
        let table = SemanticZoomTable::new(upper, [1, cap, cap, cap, cap]).expect("valid");
        (at(1), table)
    });
    defaults.chain(caps).collect()
}

/// Labels mixing ASCII, CJK, emoji, combining marks, controls and breaks.
pub(crate) fn awkward_label() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        Just("a"),
        Just("word"),
        Just("Babbage"),
        Just(" "),
        Just("  "),
        Just("\n"),
        Just("日本"),
        Just("😀"),
        Just("e\u{301}"),
        Just("\u{301}"),
        Just("\u{200d}"),
        Just("x\u{0}y"),
        Just("\t"),
    ];
    prop::collection::vec(piece, 0..16).prop_map(|pieces| pieces.concat())
}

/// Every shape, with box minimums around and past the default caps.
pub(crate) fn any_shape() -> impl Strategy<Value = NodeShape> {
    prop_oneof![
        Just(NodeShape::Label),
        Just(NodeShape::Glyph('●')),
        Just(NodeShape::Glyph('日')),
        (0u16..24, 0u16..6).prop_map(|(min_w, min_h)| NodeShape::Box { min_w, min_h }),
    ]
}

/// A node of `shape` labelled `text`, pinned or not.
pub(crate) fn node(text: &str, shape: NodeShape, pin: bool) -> NodeSpec {
    let mut spec = NodeSpec::label(text);
    spec.shape = shape;
    spec.pinned = pin.then_some((0.0, 0.0));
    spec
}
