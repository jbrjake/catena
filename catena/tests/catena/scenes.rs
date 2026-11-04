//! T3 visual snapshots of real scenes (plan §16.4): fixture graphs laid out, drawn through the
//! public API, and hashed cell by cell with their colors. The hashes are committed, so they
//! also hold every platform CI runs on to the same frame (plan §11.5). A changed frame fails
//! and writes `tests/visual/{name}.fail.svg`; `CATENA_UPDATE_SNAPSHOTS=1` re-baselines.

use catena::GraphView;
use catena::raster::{Blitter, CellGrid};
use catena_testkit::fixtures::{FixtureGraph, community, generated};
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
fn a_generated_family_draws_as_committed() {
    let graph = generated(7, 120);
    assert_visual_snapshot(
        &frame(&graph, Blitter::Braille, (120, 40)),
        "generated_7_120_braille",
    );
}
