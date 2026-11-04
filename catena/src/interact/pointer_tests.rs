//! The pointer's half of the controller (plan §10.1, §10.2): click, hover with owner ruling
//! A4's hover-lift, and node drag.

use insta::assert_snapshot;

use super::super::controller::tests::{cell, ix, pinned};
use super::*;
use crate::draw::{EDGE, EDGE_LIFTED, EDGE_SELECTED, NODE_SELECTED};
use crate::geometry::cell::{CellBox, CellPt};
use crate::interact::{Button, InputEvent};
use crate::raster::{Attrs, CellGrid, PaletteColor};
use crate::scene::{Layer, Payload, StyleId};

fn x(at: CellPt) -> u16 {
    u16::try_from(at.x).expect("on the frame")
}

fn y(at: CellPt) -> u16 {
    u16::try_from(at.y).expect("on the frame")
}

fn down(at: CellPt) -> InputEvent {
    InputEvent::MouseDown(x(at), y(at), Button::Left)
}

fn up(at: CellPt) -> InputEvent {
    InputEvent::MouseUp(x(at), y(at), Button::Left)
}

fn to(at: CellPt) -> InputEvent {
    InputEvent::MouseMove(x(at), y(at))
}

fn offset(at: CellPt, dx: i32, dy: i32) -> CellPt {
    CellPt::new(at.x + dx, at.y + dy)
}

/// A star: `n0` in the middle joined to the four others, and `n1` to `n3` across a corner.
const STAR: [(f64, f64); 5] = [
    (0.0, 0.0),
    (-40.0, 0.0),
    (40.0, 0.0),
    (0.0, -40.0),
    (0.0, 40.0),
];
const SPOKES: [(usize, usize); 5] = [(0, 1), (0, 2), (0, 3), (0, 4), (1, 3)];

/// What a cell points at: a node, the first edge there, or nothing.
type Target = Option<Result<NodeIx, EdgeIx>>;

fn target(gv: &mut GraphView<String>, at: CellPt) -> Target {
    gv.hit(x(at), y(at)).and_then(|hit| match hit {
        Hit::Node(ix) => Some(Ok(ix)),
        Hit::Edges(edges) => edges.first().copied().map(Err),
    })
}

/// The first cell of the frame, scanning rows, that points at what `want` accepts.
fn find(gv: &mut GraphView<String>, want: impl Fn(Target) -> bool) -> CellPt {
    (0..24)
        .flat_map(|y| (0..80).map(move |x| CellPt::new(x, y)))
        .find(|&at| want(target(gv, at)))
        .expect("such a cell")
}

/// Node `i`'s box as drawn.
fn drawn_box(gv: &GraphView<String>, i: usize) -> CellBox {
    let want = ix(gv, i);
    gv.viewport
        .boxes(&gv.metrics)
        .find_map(|(ix, b)| (ix == want).then_some(b))
        .expect("drawn")
}

/// Every edge's layer and style in the scene, in canonical order.
fn edge_marks(gv: &GraphView<String>) -> Vec<(EdgeIx, Layer, StyleId)> {
    gv.scene()
        .items()
        .iter()
        .filter_map(|item| match item.payload {
            Payload::EdgePath { ix, .. } => Some((ix, item.z, item.style)),
            _ => None,
        })
        .collect()
}

fn edge_between(gv: &GraphView<String>, a: usize, b: usize) -> EdgeIx {
    let (a, b) = (ix(gv, a), ix(gv, b));
    *gv.store
        .out_edges(a)
        .iter()
        .find(|&&e| gv.store.edge(e).is_some_and(|edge| edge.to == b))
        .expect("an edge")
}

#[test]
fn a_click_selects_a_node_an_edge_or_nothing() {
    let mut gv = pinned(&STAR, &SPOKES);
    let n2 = cell(&gv, 2);
    assert_eq!(gv.handle(down(n2)), [Outcome::Selected(Some(ix(&gv, 2)))]);
    assert_eq!(gv.handle(up(n2)), [], "a click is no drag");
    let pin = gv.store.node(ix(&gv, 2)).expect("live").spec.pinned;
    assert_eq!(pin, Some(STAR[2]));
    let on_edge = find(&mut gv, |t| matches!(t, Some(Err(_))));
    let Some(Err(edge)) = target(&mut gv, on_edge) else {
        unreachable!("found as an edge")
    };
    let id = gv.store.edge(edge).expect("live").id;
    assert_eq!(
        gv.handle(down(on_edge)),
        [Outcome::Selected(None), Outcome::EdgeSelected(Some(id))]
    );
    assert_eq!(gv.selected_edge(), Some(id));
    let marks = edge_marks(&gv);
    assert!(
        marks.contains(&(edge, Layer::EdgesOver, EDGE_SELECTED)),
        "{marks:?}"
    );
    let empty = find(&mut gv, |t| t.is_none());
    assert_eq!(gv.handle(down(empty)), [Outcome::EdgeSelected(None)]);
    assert_eq!(gv.handle(down(empty)), []);
    let right = InputEvent::MouseDown(x(n2), y(n2), Button::Right);
    assert_eq!(gv.handle(right), [], "only the primary button selects");
}

#[test]
fn the_selected_node_draws_in_its_own_style() {
    let mut gv = pinned(&STAR, &SPOKES);
    gv.handle(down(cell(&gv, 0)));
    let styles: Vec<(NodeIx, StyleId)> = gv
        .scene()
        .items()
        .iter()
        .filter_map(|item| match item.payload {
            Payload::NodeBox { ix } => Some((ix, item.style)),
            _ => None,
        })
        .collect();
    assert_eq!(styles.len(), 5);
    for (node, style) in styles {
        assert_eq!(style == NODE_SELECTED, node == ix(&gv, 0), "{node:?}");
    }
}

#[test]
fn hovering_a_node_lifts_its_edges_and_hovering_an_edge_lifts_it() {
    // Owner ruling A4: hovering a node or an edge redraws its edges on EdgesOver, highlighted.
    let mut gv = pinned(&STAR, &SPOKES);
    let n1 = cell(&gv, 1);
    assert_eq!(gv.handle(to(n1)), [Outcome::Hovered(Some(ix(&gv, 1)))]);
    assert_eq!(gv.hovered(), Some(ix(&gv, 1)));
    let incident = [edge_between(&gv, 0, 1), edge_between(&gv, 1, 3)];
    let marks = edge_marks(&gv);
    assert_eq!(marks.len(), 5);
    for (edge, layer, style) in &marks {
        let want = if incident.contains(edge) {
            (Layer::EdgesOver, EDGE_LIFTED)
        } else {
            (Layer::EdgesUnder, EDGE)
        };
        assert_eq!((*layer, *style), want, "{edge:?}");
    }
    // Moving within the box changes nothing.
    let b = drawn_box(&gv, 1);
    let corner = CellPt::new(b.x, b.y);
    assert_ne!(corner, n1);
    assert_eq!(gv.handle(to(corner)), []);
    // Off every node and edge: nothing is lifted.
    let empty = find(&mut gv, |t| t.is_none());
    assert_eq!(gv.handle(to(empty)), [Outcome::Hovered(None)]);
    assert!(edge_marks(&gv).iter().all(|m| m.1 == Layer::EdgesUnder));
    // Onto an edge: it alone is lifted, and no node is hovered.
    let on_edge = find(&mut gv, |t| matches!(t, Some(Err(_))));
    let Some(Err(edge)) = target(&mut gv, on_edge) else {
        unreachable!("found as an edge")
    };
    assert_eq!(gv.handle(to(on_edge)), []);
    let lifted: Vec<EdgeIx> = edge_marks(&gv)
        .into_iter()
        .filter(|m| m.1 == Layer::EdgesOver)
        .map(|m| m.0)
        .collect();
    assert_eq!(lifted, [edge]);
}

/// The frame's text, then a map of its styles: `.` unstyled, `n` a node, `e` an edge, `L` a
/// lifted edge, `H` the hovered node, `S` the selected node, `Y` the selected edge.
fn styled(grid: &CellGrid) -> String {
    let (cols, rows) = (80, 24);
    let mut map = String::new();
    for y in 0..rows {
        for x in 0..cols {
            let cell = grid.cell(x, y).expect("on the grid");
            let mark = match (cell.fg(), cell.attrs()) {
                (None, _) => '.',
                (_, a) if a.contains(Attrs::REVERSED) => 'S',
                (Some(PaletteColor::Ansi(15)), a) if a.contains(Attrs::BOLD) => 'H',
                (Some(PaletteColor::Ansi(15)), _) => 'L',
                (Some(PaletteColor::Ansi(11)), _) => 'Y',
                (Some(PaletteColor::Ansi(8)), _) => 'e',
                _ => 'n',
            };
            map.push(mark);
        }
        map.push('\n');
    }
    format!("{grid}\n{map}")
}

#[test]
fn hover_lift_golden() {
    // Owner ruling A4: in the goldens. The hovered n1 draws bold and bright, its two edges
    // lifted over the rest; the node selected before it stays reversed.
    let mut gv = pinned(&STAR, &SPOKES);
    gv.handle(down(cell(&gv, 2)));
    gv.handle(up(cell(&gv, 2)));
    gv.handle(to(cell(&gv, 1)));
    let mut grid = CellGrid::new(80, 24);
    gv.render(&mut grid);
    assert_snapshot!("hover_lift_ascii", styled(&grid));
}

#[test]
fn dragging_a_node_moves_it_under_the_pointer_and_its_pin_returns() {
    // Plan §10.1: pinned for the drag's duration, `DragCompleted` on release, and the host
    // decides whether the pin persists. The grab keeps the pointer's place on the box.
    let mut gv = pinned(&STAR, &SPOKES);
    gv.update(|tx| tx.set_node(&"n0".to_string(), |n| n.pinned = None))
        .expect("known");
    let (n0, anchor) = (ix(&gv, 0), cell(&gv, 0));
    let far: Vec<CellPt> = (2..5).map(|i| cell(&gv, i)).collect();
    let b = drawn_box(&gv, 0);
    let grip = CellPt::new(b.x, anchor.y);
    assert_eq!(target(&mut gv, grip), Some(Ok(n0)));
    assert_eq!(gv.handle(down(grip)), [Outcome::Selected(Some(n0))]);
    assert_eq!(
        gv.handle(to(offset(grip, -6, 2))),
        [],
        "a drag moves no hover"
    );
    assert!(gv.store.node(n0).expect("live").spec.pinned.is_some());
    assert_eq!(gv.handle(to(offset(grip, -10, 4))), []);
    let dropped = offset(anchor, -10, 4);
    assert_eq!(cell(&gv, 0), dropped);
    let out = gv.handle(up(offset(grip, -10, 4)));
    let [Outcome::DragCompleted(node, world)] = out[..] else {
        panic!("{out:?}");
    };
    assert_eq!(node, n0);
    let under = gv.viewport.world_at(SubPt::from(dropped)).expect("fitted");
    assert!((world.0 - under.0).abs() < 1e-9 && (world.1 - under.1).abs() < 1e-9);
    assert_eq!(
        gv.store.node(n0).expect("live").spec.pinned,
        None,
        "as before"
    );
    assert_eq!(cell(&gv, 0), dropped, "and it stays where it was dropped");
    let others: Vec<CellPt> = (2..5).map(|i| cell(&gv, i)).collect();
    assert_eq!(others, far, "the view did not pan to undo the drag");
}

#[test]
fn a_pinned_node_dragged_stays_pinned_where_it_was_dropped() {
    let mut gv = pinned(&STAR, &SPOKES);
    let n2 = cell(&gv, 2);
    gv.handle(down(n2));
    let out = gv.handle(up(offset(n2, -3, 3)));
    let [Outcome::DragCompleted(_, world)] = out[..] else {
        panic!("a release away from the press drags: {out:?}");
    };
    assert_eq!(cell(&gv, 2), offset(n2, -3, 3));
    assert_eq!(
        gv.store.node(ix(&gv, 2)).expect("live").spec.pinned,
        Some(world)
    );
}

#[test]
fn removing_the_dragged_node_ends_the_drag() {
    let mut gv = pinned(&STAR, &SPOKES);
    let n0 = cell(&gv, 0);
    gv.handle(down(n0));
    gv.handle(to(cell(&gv, 1)));
    gv.update(|tx| tx.remove_node(&"n0".to_string()))
        .expect("known");
    assert_eq!(gv.selected(), None);
    assert_eq!(gv.hovered(), None);
    gv.handle(to(offset(n0, 2, 2)));
    assert_eq!(gv.handle(up(offset(n0, 2, 2))), []);
}
