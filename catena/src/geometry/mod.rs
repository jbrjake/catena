//! Geometry: curves, node measurement, grid snapping and the viewport (plan §5, §6).

pub mod cell;
pub mod curve;
mod metrics;
mod snap;
#[cfg(test)]
pub(crate) mod testing;
mod viewport;
mod zoom;

pub(crate) use metrics::ResolvedMetrics;
pub use metrics::{FormShape, Mark, NodeForm, RowLabel};
pub use snap::Fit;
pub(crate) use snap::{Grid, placement_order};
pub(crate) use viewport::Viewport;
pub(crate) use zoom::{DEFAULT_ZOOM, SemanticZoomTable};
