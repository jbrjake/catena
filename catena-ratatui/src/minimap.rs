//! The [`Minimap`] widget (plan §3), behind `widget-extras`: the view's overview in an area.

use catena::GraphView;
use catena::graph::Key;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::Widget;

use crate::BufferSurface;

/// The whole graph small, with the rectangle its view shows, drawn by
/// `GraphView::render_overview`: `frame.render_widget(Minimap::new(&gv), area)`. It reads the
/// view and changes nothing, and it clears its area first, so it can float over the
/// [`crate::Graph`] widget drawn before it.
pub struct Minimap<'v, K: Key> {
    view: &'v GraphView<K>,
}

impl<'v, K: Key> Minimap<'v, K> {
    /// The minimap of `view`.
    #[must_use]
    pub const fn new(view: &'v GraphView<K>) -> Self {
        Minimap { view }
    }
}

impl<K: Key> std::fmt::Debug for Minimap<'_, K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Minimap")
    }
}

impl<K: Key> Widget for Minimap<'_, K> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        crate::widget::clear(buf, area);
        self.view
            .render_overview(&mut BufferSurface::new(buf, area));
    }
}

#[cfg(test)]
#[path = "minimap_tests.rs"]
mod tests;
