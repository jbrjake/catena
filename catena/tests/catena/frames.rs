//! A steady frame allocates nothing (plan §15.2): once a view has drawn into a surface, drawing
//! the same graph into it again touches no heap. Counted per thread by `allocation-counter`'s
//! global allocator, which linking this test target installs.

use catena::GraphView;
use catena::raster::{Blitter, CellGrid};
use catena_testkit::fixtures::generated;

#[test]
fn a_steady_frame_allocates_nothing() {
    let graph = generated(9, 200);
    for blitter in [
        Blitter::Braille,
        Blitter::HalfBlock,
        Blitter::Sextant,
        Blitter::Ascii,
    ] {
        let mut gv = graph
            .view(GraphView::builder().blitter(blitter))
            .expect("fixtures are consistent");
        let mut grid = CellGrid::new(200, 50);
        gv.render(&mut grid);
        gv.render(&mut grid);
        let info = allocation_counter::measure(|| gv.render(&mut grid));
        assert_eq!(
            (info.count_total, info.bytes_total),
            (0, 0),
            "{blitter:?}: the third frame allocated"
        );
    }
}

#[test]
fn a_frame_after_an_edit_draws_the_edit() {
    // The kept scene must not outlive what it shows: an edit, a pan or a new size redraws.
    let mut gv = generated(2, 30)
        .view(GraphView::builder())
        .expect("fixtures are consistent");
    let mut grid = CellGrid::new(100, 30);
    gv.render(&mut grid);
    let before = grid.clone();
    gv.update(|tx| {
        tx.add_node("late".to_string(), catena::NodeSpec::label("late arrival"))?;
        let first = tx_first_key();
        tx.add_edge(&"late".to_string(), &first, catena::EdgeSpec::directed())?;
        Ok(())
    })
    .expect("a fresh key, a known end");
    gv.render(&mut grid);
    assert_ne!(grid, before, "the new node is drawn");
    assert!(grid.to_string().contains("late"), "{grid}");
}

/// A key the generated family 2 is known to hold.
fn tx_first_key() -> String {
    generated(2, 30).nodes[0].key.clone()
}
