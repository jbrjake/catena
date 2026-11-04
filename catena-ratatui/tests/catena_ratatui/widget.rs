//! T5, widget composition (plan §16.6): the `Graph` widget in a ratatui layout through
//! `TestBackend`, a resize storm, and the pointer reaching the view in terminal cells.

use catena::geometry::cell::CellBox;
use catena::interact::{Button, InputEvent};
use catena::scene::Payload;
use catena::{GraphView, Outcome};
use catena_ratatui::Graph;
use catena_testkit::fixtures::{SplitMix64, community};
use insta::assert_snapshot;
use ratatui_core::backend::TestBackend;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::terminal::Terminal;

/// A terminal of `cols × rows` with a sidebar of `side` columns and the graph beside it.
fn draw(gv: &mut GraphView<String>, terminal: &mut Terminal<TestBackend>, side: u16) -> Rect {
    let mut graph_area = Rect::default();
    terminal
        .draw(|frame| {
            let all = frame.area();
            let side = side.min(all.width);
            let buf = frame.buffer_mut();
            for (row, text) in ["catena", "", "n/N  cycle", "hjkl pan", "+/-  zoom"]
                .iter()
                .enumerate()
            {
                if let Ok(row) = u16::try_from(row)
                    && row < all.height
                {
                    buf.set_string(0, row, text, ratatui_core::style::Style::default());
                }
            }
            graph_area = Rect::new(side, 0, all.width - side, all.height);
            frame.render_stateful_widget(Graph::new(), graph_area, gv);
        })
        .expect("the test backend cannot fail");
    graph_area
}

fn view() -> GraphView<String> {
    community()
        .view(GraphView::builder())
        .expect("fixtures are consistent")
}

#[test]
fn the_graph_composes_beside_a_sidebar() {
    let mut gv = view();
    let mut terminal = Terminal::new(TestBackend::new(90, 28)).expect("infallible");
    draw(&mut gv, &mut terminal, 14);
    assert_snapshot!("graph_beside_a_sidebar", terminal.backend().to_string());
}

/// Every node box the view's scene places, in surface cells.
fn boxes(gv: &GraphView<String>) -> Vec<CellBox> {
    gv.scene()
        .items()
        .iter()
        .filter(|item| matches!(item.payload, Payload::NodeBox { .. }))
        .map(|item| item.bounds)
        .collect()
}

fn meet(a: CellBox, b: CellBox) -> bool {
    i64::from(a.x) < b.right()
        && i64::from(b.x) < a.right()
        && i64::from(a.y) < b.bottom()
        && i64::from(b.y) < a.bottom()
}

fn holds(b: CellBox, x: i32, y: i32) -> bool {
    b.contains(catena::geometry::cell::CellPt::new(x, y))
}

#[test]
fn a_resize_storm_keeps_boxes_apart_and_every_drawn_cell_in_an_item() {
    // Plan §16.6: ten random resizes, then invariants A (no two boxes meet) and C (every
    // drawn cell lies in some scene item's bounds) on the widget's frame.
    let mut gv = view();
    let mut rng = SplitMix64::new(16_06);
    let mut terminal = Terminal::new(TestBackend::new(90, 28)).expect("infallible");
    for storm in 0..10 {
        let cols = 30 + u16::try_from(rng.below(120)).expect("small");
        let rows = 8 + u16::try_from(rng.below(40)).expect("small");
        terminal.backend_mut().resize(cols, rows);
        let area = draw(&mut gv, &mut terminal, 12);
        let boxes = boxes(&gv);
        assert_eq!(
            boxes.len(),
            gv.node_count(),
            "storm {storm}: every node placed"
        );
        for (i, &a) in boxes.iter().enumerate() {
            assert!(
                boxes[i + 1..].iter().all(|&b| !meet(a, b)),
                "storm {storm} at {cols} × {rows}: {a:?} meets another box"
            );
        }
        let scene = gv.scene();
        let buf: &Buffer = terminal.backend().buffer();
        let mut lit = 0;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if buf[(x, y)].symbol() == " " {
                    continue;
                }
                lit += 1;
                let (sx, sy) = (i32::from(x - area.x), i32::from(y - area.y));
                assert!(
                    scene.items().iter().any(|item| holds(item.bounds, sx, sy)),
                    "storm {storm}: ({x}, {y}) traces to no item"
                );
            }
        }
        assert!(lit > 0, "storm {storm} draws");
    }
}

#[test]
fn a_click_in_terminal_cells_selects_the_node_drawn_there() {
    // Plan §13's loop hands the terminal's coordinates to `handle` unchanged.
    let mut gv = view();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("infallible");
    let area = draw(&mut gv, &mut terminal, 20);
    assert_eq!(area.x, 20);
    let scene = gv.scene();
    let (node, b) = scene
        .items()
        .iter()
        .find_map(|item| match item.payload {
            Payload::NodeBox { ix } if item.bounds.x >= 0 => Some((ix, item.bounds)),
            _ => None,
        })
        .expect("a node on the frame");
    let x = area.x + u16::try_from(b.x).expect("on") + u16::try_from(b.width / 2).expect("narrow");
    let y = area.y + u16::try_from(b.y).expect("on");
    let out = gv.handle(InputEvent::MouseDown(x, y, Button::Left));
    assert_eq!(out, [Outcome::Selected(Some(node))]);
    // A click in the sidebar is the sidebar's.
    assert_eq!(gv.handle(InputEvent::MouseDown(3, 2, Button::Left)), []);
}
