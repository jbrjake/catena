//! T3 visual snapshots of real scenes (plan §16.4): fixture graphs laid out, drawn through the
//! public API, and hashed cell by cell with their colors. The hashes are committed, so they
//! also hold every platform CI runs on to the same frame (plan §11.5). A changed frame fails
//! and writes `tests/visual/{name}.fail.svg`; `CATENA_UPDATE_SNAPSHOTS=1` re-baselines.

use std::collections::BTreeSet;
use std::time::Duration;

use catena::GraphView;
use catena::geometry::cell::CellBox;
use catena::interact::{Button, InputEvent, Outcome, Redraw};
use catena::raster::{Blitter, CellGrid, SubCellCanvas, polyline_pixels};
use catena::scene::{EdgeRoute, Payload, Route};
use catena_testkit::braille_asserts::{
    assert_8_connected, assert_endpoints_exact, assert_no_duplicates, lit_pixels,
};
use catena_testkit::fixtures::{FixtureGraph, community, generated};
use catena_testkit::oscillation::{diff_count, diff_details, flickers};
use catena_testkit::svg::assert_visual_snapshot;

/// `graph` drawn into `cols × rows` cells with `blitter`.
fn frame(graph: &FixtureGraph, blitter: Blitter, (cols, rows): (u16, u16)) -> CellGrid {
    let mut gv = graph
        .view(GraphView::builder().blitter(blitter))
        .expect("fixtures are consistent");
    let mut grid = CellGrid::new(cols, rows);
    gv.render(&mut grid);
    grid
}

#[test]
fn the_community_graph_draws_as_committed() {
    let graph = community();
    for (blitter, name) in [
        (Blitter::Braille, "community_braille"),
        (Blitter::Ascii, "community_ascii"),
    ] {
        assert_visual_snapshot(&frame(&graph, blitter, (120, 40)), name);
    }
}

#[test]
fn the_community_graph_draws_its_hover_lift_and_selection_as_committed() {
    // Owner ruling A4: hover-lift in the T3 goldens. The first node in canonical order is
    // hovered, its edges lifted; the last is selected.
    let mut gv = community()
        .view(GraphView::builder())
        .expect("fixtures are consistent");
    let mut grid = CellGrid::new(120, 40);
    gv.render(&mut grid);
    let boxes: Vec<CellBox> = gv
        .scene()
        .items()
        .iter()
        .filter(|item| matches!(item.payload, Payload::NodeBox { .. }))
        .map(|item| item.bounds)
        .collect();
    let middle = |b: CellBox| {
        let x = b.x + i32::try_from(b.width / 2).expect("narrow");
        (
            u16::try_from(x).expect("on"),
            u16::try_from(b.y).expect("on"),
        )
    };
    let (x, y) = middle(*boxes.last().expect("nodes"));
    let pressed = gv.handle(InputEvent::MouseDown(x, y, Button::Left));
    assert!(
        matches!(pressed[..], [Outcome::Selected(Some(_))]),
        "{pressed:?}"
    );
    gv.handle(InputEvent::MouseUp(x, y, Button::Left));
    let (x, y) = middle(boxes[0]);
    let hovered = gv.handle(InputEvent::MouseMove(x, y));
    assert!(
        matches!(hovered[..], [Outcome::Hovered(Some(_))]),
        "{hovered:?}"
    );
    gv.render(&mut grid);
    assert_visual_snapshot(&grid, "community_hover_braille");
}

#[test]
fn a_quiescent_view_ticked_and_redrawn_fifty_times_changes_no_cell() {
    // Plan §16.5's steady state: no hidden time, iteration order or reused canvas may leak into
    // a frame, and a tick with nothing animating moves nothing and asks for no frame.
    for graph in [community(), generated(5, 80)] {
        let mut gv = graph
            .view(GraphView::builder())
            .expect("fixtures are consistent");
        let frames: Vec<CellGrid> = (0..50)
            .map(|i| {
                let redraw = gv.tick(Duration::from_millis(16));
                assert_eq!(redraw == Redraw::Idle, i > 0, "tick {i}");
                let mut grid = CellGrid::new(100, 30);
                gv.render(&mut grid);
                grid
            })
            .collect();
        assert!(
            diff_count(&CellGrid::new(100, 30), &frames[0]) > 100,
            "it draws"
        );
        for (i, frame) in frames.iter().enumerate().skip(1) {
            assert_eq!(
                diff_count(&frames[0], frame),
                0,
                "frame {i}: {:?}",
                diff_details(&frames[0], frame)
            );
        }
        assert_eq!(flickers(&frames), Vec::<usize>::new());
    }
}

#[test]
fn invariant_e_every_drawn_edge_is_one_unbroken_line_between_its_ends() {
    // Plan §16.2-E on real routes: each edge of a laid-out graph, drawn alone on a braille
    // canvas, lights exactly its pixel walk clipped to the frame: 8-connected, both ends lit,
    // no pixel walked twice. A route point in cells maps to braille sub-pixels as the
    // compositor centres it, `(2x + 0.5, 4y + 1.5)`.
    let (cols, rows) = (120u16, 40u16);
    let mut checked = 0;
    for seed in 0..3 {
        let graph = generated(seed, 60);
        let mut gv = graph
            .view(GraphView::builder())
            .expect("fixtures are consistent");
        gv.render(&mut CellGrid::new(cols, rows));
        for item in gv.scene().items() {
            let Payload::EdgePath {
                route: EdgeRoute::Path(Route::Polyline(points)),
                ..
            } = &item.payload
            else {
                continue;
            };
            if points.len() < 2 {
                continue;
            }
            let sub: Vec<(f64, f64)> = points
                .iter()
                .map(|p| (p.x * 2.0 + 0.5, p.y * 4.0 + 1.5))
                .collect();
            let mut canvas = SubCellCanvas::new(Blitter::Braille, cols, rows);
            canvas.draw_polyline(&sub);
            let lit = lit_pixels(&canvas);
            let walk: Vec<(i64, i64)> = polyline_pixels(&sub).collect();
            let unique: BTreeSet<(i64, i64)> = walk.iter().copied().collect();
            assert_eq!(
                unique.len(),
                walk.len(),
                "seed {seed}: a pixel walked twice"
            );
            let (w, h) = (i64::from(cols) * 2, i64::from(rows) * 4);
            let on_frame: BTreeSet<(usize, usize)> = walk
                .iter()
                .filter(|&&(x, y)| (0..w).contains(&x) && (0..h).contains(&y))
                .filter_map(|&(x, y)| Some((usize::try_from(x).ok()?, usize::try_from(y).ok()?)))
                .collect();
            assert_no_duplicates(&lit);
            assert_eq!(
                lit.iter().copied().collect::<BTreeSet<_>>(),
                on_frame,
                "seed {seed}: the canvas lit other pixels than the walk"
            );
            assert_8_connected(&lit);
            // Both ends, where they lie on the frame: a node the snapper pushed past an edge
            // has its end off it.
            for end in [walk[0], walk[walk.len() - 1]] {
                if (0..w).contains(&end.0) && (0..h).contains(&end.1) {
                    let (x, y) = (
                        i32::try_from(end.0).expect("on frame"),
                        i32::try_from(end.1).expect("on frame"),
                    );
                    assert_endpoints_exact(&lit, x, y, x, y);
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 100, "only {checked} edges drawn");
}

#[test]
fn a_generated_family_draws_as_committed() {
    let graph = generated(7, 120);
    assert_visual_snapshot(
        &frame(&graph, Blitter::Braille, (120, 40)),
        "generated_7_120_braille",
    );
}
