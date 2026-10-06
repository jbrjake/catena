//! Geometry: curves, node measurement, grid snapping and the viewport (plan §5, §6).

pub mod cell;
pub mod curve;
mod metrics;
#[cfg(test)]
pub(crate) mod testing;
mod zoom;

pub(crate) use metrics::ResolvedMetrics;
pub use metrics::{FormShape, Mark, NodeForm, RowLabel};
pub(crate) use zoom::{DEFAULT_ZOOM, SemanticZoomTable};
