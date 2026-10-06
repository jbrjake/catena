//! Interactive node-and-edge graphs for terminal UIs.
//!
//! `catena` is the renderer-agnostic core: graph store, layouts, rasterization, scene graph and
//! interaction. It is a pure state machine. It performs no I/O, reads no clock and draws no
//! random numbers; time enters only through `tick(dt)`, and identical input yields identical
//! frames on every platform. Terminal adapters live in `catena-ratatui`.

mod fmath;
pub mod geometry;
pub mod graph;
mod layout;
pub mod raster;
pub mod scene;
mod view;

pub use geometry::Fit;
pub use graph::{EdgeSpec, GraphError, NodeSpec};
pub use layout::LayoutKind;
pub use layout::force::{ForceParams, Repulsion};
pub use view::{DEFAULT_CELL_ASPECT, DEFAULT_MAX_LABEL_COLS, GraphView, GraphViewBuilder};
