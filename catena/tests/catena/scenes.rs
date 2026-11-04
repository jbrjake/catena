//! T3 visual snapshots of real scenes (plan §16.4): fixture graphs laid out, drawn through the
//! public API, and hashed cell by cell with their colors. The hashes are committed, so they
//! also hold every platform CI runs on to the same frame (plan §11.5). A changed frame fails
//! and writes `tests/visual/{name}.fail.svg`; `CATENA_UPDATE_SNAPSHOTS=1` re-baselines.

use catena::GraphView;
use catena::raster::{Blitter, CellGrid};
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
fn a_quiescent_view_redrawn_fifty_times_changes_no_cell() {
    // Plan §16.5's steady state: no hidden time, iteration order or reused canvas may leak into
    // a frame. Until M3's `tick`, nothing advances between frames at all.
    for graph in [community(), generated(5, 80)] {
        let mut gv = graph
            .view(GraphView::builder())
            .expect("fixtures are consistent");
        let frames: Vec<CellGrid> = (0..50)
            .map(|_| {
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
fn a_generated_family_draws_as_committed() {
    let graph = generated(7, 120);
    assert_visual_snapshot(
        &frame(&graph, Blitter::Braille, (120, 40)),
        "generated_7_120_braille",
    );
}
