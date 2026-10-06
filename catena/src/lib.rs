//! Interactive node-and-edge graphs for terminal UIs.
//!
//! `catena` is the renderer-agnostic core: graph store, layouts, rasterization, scene graph and
//! interaction. It is a pure state machine. It performs no I/O, reads no clock and draws no
//! random numbers; time enters only through `tick(dt)`, and identical input yields identical
//! frames on every platform. Terminal adapters live in `catena-ratatui`.

mod fmath;
mod layout;
pub mod raster;
