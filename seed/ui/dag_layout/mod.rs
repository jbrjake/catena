//! DAG layout engine for provenance diagrams.
//!
//! Pure-function layout: `layout_dag(&Dag, width, height) -> Layout`.
//! No ratatui dependency in the core pipeline. Testable with `render_to_string`.
//!
//! See `docs/plans/2026-02-25-dag-layout-engine-design.md` for full design.
//!
//! Phase 2 (puppeteer swap) will wire in DagBuilder, render_to_string,
//! bar builders, and remaining layout helpers. Suppress dead-code warnings
//! until then.

pub(crate) mod app_bridge;
pub(crate) mod builder;
pub(crate) mod engine;
pub(crate) mod models;
mod ordering;
mod positioning;
pub(crate) mod render;
pub(crate) mod render_ascii;
pub(crate) mod render_grid;
mod routing;

// Re-export public API — these are used by puppeteer_diagram.rs via `super::dag_layout::*`.
pub(crate) use app_bridge::build_dag_from_app;
#[allow(unused_imports)]
pub(crate) use builder::DagBuilder;
pub(crate) use engine::layout_dag;
#[allow(unused_imports)]
pub(crate) use models::*;
pub(crate) use render::render_dag_to_lines;
#[allow(unused_imports)]
pub(crate) use render::{render_layout_to_lines, render_to_string};
