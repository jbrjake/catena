//! The scene graph and its compositor (plan §7.4). A layout emits a [`SceneGraph`]; the
//! renderer interprets it and adds nothing, and hit-testing reads the same scene, so everything
//! drawn has bounds and a payload.

mod graph;
mod nodes;
mod orthogonal;
mod render;
mod routes;

pub use graph::{
    CountBadge, Decoration, EDGE_HIT_CELLS, EdgeRoute, Layer, Payload, Route, SceneGraph,
    SceneItem, SegmentId, StyleId,
};
pub use nodes::NodeGlyphs;
pub use orthogonal::BoxGlyphs;
pub use render::{Compositor, RenderOptions};
pub use routes::{EdgeEnds, RouteFault, route_faults};

#[cfg(test)]
mod golden_tests;
