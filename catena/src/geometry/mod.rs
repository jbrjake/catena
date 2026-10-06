//! Geometry: curves, node measurement, grid snapping and the viewport (plan §5, §6).

pub mod cell;
pub mod curve;
mod metrics;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "GraphView snaps its layout once the viewport lands (M2 step 4)"
    )
)]
mod snap;
#[cfg(test)]
pub(crate) mod testing;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "GraphView keeps its viewport once the layout goes live (M2 step 4)"
    )
)]
mod viewport;
mod zoom;

pub(crate) use metrics::ResolvedMetrics;
pub use metrics::{FormShape, Mark, NodeForm, RowLabel};
pub(crate) use zoom::{DEFAULT_ZOOM, SemanticZoomTable};
