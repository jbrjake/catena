//! T5 with the extras (plan §16.6): the graph, its minimap and a legend in one layout.

use catena::GraphView;
use catena::raster::PaletteColor;
use catena_ratatui::{Graph, Legend, Minimap};
use catena_testkit::fixtures::community;
use insta::assert_snapshot;
use ratatui_core::backend::TestBackend;
use ratatui_core::layout::Rect;
use ratatui_core::terminal::Terminal;

#[test]
fn the_graph_composes_with_its_minimap_and_a_legend() {
    let mut gv = community()
        .view(GraphView::builder())
        .expect("fixtures are consistent");
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("infallible");
    let legend = [
        ("node", PaletteColor::Ansi(7)),
        ("edge", PaletteColor::Ansi(8)),
        ("in view", PaletteColor::Ansi(14)),
    ];
    terminal
        .draw(|frame| {
            let side = Rect::new(0, 0, 26, 30);
            frame.render_stateful_widget(Graph::new(), Rect::new(26, 0, 74, 30), &mut gv);
            frame.render_widget(Legend::new(&legend), Rect::new(1, 1, side.width - 2, 3));
            frame.render_widget(Minimap::new(&gv), Rect::new(1, 5, side.width - 2, 10));
        })
        .expect("infallible");
    assert_snapshot!("graph_minimap_legend", terminal.backend().to_string());
}
