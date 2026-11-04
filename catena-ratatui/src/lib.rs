//! ratatui widgets for `catena` graphs (plan §3, §13).
//!
//! This crate holds no logic. [`Graph`] draws a [`catena::GraphView`] into a ratatui `Buffer`
//! through [`BufferSurface`], the adapter from catena's `Surface`, and with the `crossterm`
//! feature `input::from_crossterm` turns a terminal event into catena's `InputEvent`. It
//! depends on `ratatui-core`, not `ratatui`, so it works across application-side ratatui
//! versions.
//!
//! ```
//! use catena::{GraphView, NodeSpec};
//! use catena_ratatui::Graph;
//! use ratatui_core::{backend::TestBackend, terminal::Terminal};
//!
//! let mut gv: GraphView<String> = GraphView::new();
//! gv.update(|tx| tx.add_node("ada".to_string(), NodeSpec::label("Ada")).map(drop))?;
//! let mut terminal = Terminal::new(TestBackend::new(40, 10))?;
//! terminal.draw(|frame| frame.render_stateful_widget(Graph::new(), frame.area(), &mut gv))?;
//! assert!(terminal.backend().to_string().contains("Ada"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The pointer reaches the view in terminal cells: the area a frame drew the widget in is its
//! surface's origin, which the view subtracts (`catena::raster::Surface::origin`).

// Built for this crate's own tests without the feature too, since the push gate tests the
// workspace without features (`scripts/regression.sh`) and `crossterm` is a dev-dependency.
#[cfg(any(feature = "crossterm", test))]
pub mod input;
mod widget;

pub use widget::{BufferSurface, Graph};
