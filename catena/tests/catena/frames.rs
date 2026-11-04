//! A steady frame allocates nothing (plan §15.2): once a view has drawn into a surface, drawing
//! the same graph into it again touches no heap. Counted per thread by `allocation-counter`'s
//! global allocator, which linking this test target installs.

use std::time::Duration;

use catena::GraphView;
use catena::interact::{InputEvent, Redraw};
use catena::raster::{Blitter, CellGrid};
use catena::scene::Payload;
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
fn a_pointer_moving_within_one_target_allocates_nothing() {
    // Plan §15.2: a frame of mouse moves clones no keys. Once the hover has settled, moves that
    // keep it hit-test the kept scene, change nothing, and return an empty list.
    let mut gv = generated(9, 200)
        .view(GraphView::builder())
        .expect("fixtures are consistent");
    let mut grid = CellGrid::new(200, 50);
    gv.render(&mut grid);
    let node = gv
        .scene()
        .items()
        .iter()
        .find(|item| matches!(item.payload, Payload::NodeBox { .. }) && item.bounds.width > 2)
        .map(|item| item.bounds)
        .expect("a node wide enough to move within");
    let (x, y) = (
        u16::try_from(node.x).expect("on"),
        u16::try_from(node.y).expect("on"),
    );
    assert_eq!(gv.handle(InputEvent::MouseMove(x, y)).len(), 1, "hovered");
    gv.render(&mut grid);
    let mut outcomes = 0;
    let info = allocation_counter::measure(|| {
        for dx in [1, 2, 1, 0] {
            outcomes += gv.handle(InputEvent::MouseMove(x + dx, y)).len();
        }
    });
    assert_eq!(outcomes, 0, "the hover held");
    assert_eq!((info.count_total, info.bytes_total), (0, 0));
    assert_eq!(gv.tick(Duration::from_millis(16)), Redraw::Idle);
}

#[test]
fn a_resize_to_the_largest_size_holds_its_memory_to_what_can_be_drawn() {
    // A resize event can name any u16 size; fuzz_events found 65535 × 65535 driving a
    // hit-test's index to 800 MB. Nothing past `MAX_CELLS` (4 Mi cells) is drawn, so what the
    // view keeps for a frame stays within a small multiple of it.
    let mut gv = generated(5, 40)
        .view(GraphView::builder())
        .expect("fixtures are consistent");
    let info = allocation_counter::measure(|| {
        gv.handle(InputEvent::Resize(u16::MAX, u16::MAX));
        gv.handle(InputEvent::MouseMove(10, 10));
        gv.act(catena::interact::Action::CycleNext);
        gv.tick(Duration::from_millis(16));
    });
    let ceiling = 64 << 20;
    assert!(
        info.bytes_max < ceiling,
        "peak {} MiB",
        info.bytes_max >> 20
    );
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
