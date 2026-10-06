//! Interactive node-and-edge graphs for terminal UIs.
//!
//! `catena` is the renderer-agnostic core: graph store, layouts, rasterization, scene graph and
//! interaction. It is a pure state machine. It performs no I/O, reads no clock and draws no
//! random numbers; time enters only through `tick(dt)`, and identical input yields identical
//! frames on every platform. Terminal adapters live in `catena-ratatui`.

// `allow`, not `expect`: fmath is dead only transitively (its callers are dead), which rustc
// 1.97 reports and 1.88 (the MSRV) does not, so an expectation would go unfulfilled there.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "its callers are layout modules that M2 wires in")
)]
mod fmath;
mod layout;
pub mod raster;
