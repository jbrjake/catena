use super::*;

fn grid(size: (u16, u16), items: &[(u32, CellBox)]) -> SpatialGrid {
    let mut grid = SpatialGrid::default();
    grid.rebuild(size, items.iter().copied());
    grid
}

#[test]
fn an_item_is_listed_in_every_bucket_its_bounds_reach_and_no_other() {
    // 48 × 24 cells: 3 × 3 buckets of 16 × 8. The item spans columns 10..20 and rows 5..10, so
    // buckets (0,0), (1,0), (0,1) and (1,1).
    let grid = grid((48, 24), &[(7, CellBox::new(10, 5, 10, 5))]);
    let mut reached = Vec::new();
    for y in 0..24u16 {
        for x in 0..48u16 {
            if grid.query(x, y) == [7] {
                reached.push((x / 16, y / 8));
            } else {
                assert_eq!(grid.query(x, y), [] as [u32; 0], "({x}, {y})");
            }
        }
    }
    reached.sort_unstable();
    reached.dedup();
    assert_eq!(reached, [(0, 0), (0, 1), (1, 0), (1, 1)]);
}

#[test]
fn every_cell_inside_an_items_bounds_finds_it() {
    // The oracle: a linear scan of who contains each cell, against the grid's candidates.
    let items = [
        (0, CellBox::new(0, 0, 1, 1)),
        (1, CellBox::new(15, 7, 2, 2)),
        (2, CellBox::new(30, 3, 40, 1)),
        (3, CellBox::new(-5, -5, 8, 8)),
        (4, CellBox::new(60, 20, 30, 30)),
    ];
    let grid = grid((70, 25), &items);
    for y in 0..25u16 {
        for x in 0..70u16 {
            let at = crate::geometry::cell::CellPt::new(i32::from(x), i32::from(y));
            for &(id, b) in &items {
                if b.contains(at) {
                    assert!(
                        grid.query(x, y).contains(&id),
                        "({x}, {y}) misses item {id}"
                    );
                }
            }
        }
    }
}

#[test]
fn items_off_the_frame_or_empty_are_not_indexed_and_queries_off_it_find_nothing() {
    let grid = grid(
        (32, 16),
        &[
            (0, CellBox::new(40, 0, 5, 5)),
            (1, CellBox::new(0, -9, 5, 5)),
            (2, CellBox::new(3, 3, 0, 4)),
        ],
    );
    for y in 0..16 {
        for x in 0..32 {
            assert_eq!(grid.query(x, y), [] as [u32; 0]);
        }
    }
    assert_eq!(grid.query(32, 0), [] as [u32; 0]);
    assert_eq!(grid.query(u16::MAX, u16::MAX), [] as [u32; 0]);
}

#[test]
fn a_rebuild_forgets_the_last_items_and_keeps_insertion_order() {
    let mut grid = grid((16, 8), &[(9, CellBox::new(0, 0, 4, 4))]);
    grid.rebuild(
        (16, 8),
        [(3, CellBox::new(0, 0, 2, 2)), (1, CellBox::new(1, 1, 2, 2))],
    );
    assert_eq!(grid.query(1, 1), [3, 1]);
    grid.rebuild((0, 0), []);
    assert_eq!(grid.query(0, 0), [] as [u32; 0]);
}

#[test]
fn a_frame_past_max_cells_is_indexed_only_as_far_as_it_can_be_drawn() {
    // A resize event can name any u16 size (found by fuzz_events: 65535 × 65535 cost 33.5
    // million buckets, 800 MB). Nothing past `MAX_CELLS` is drawn, as a `CellGrid` and a canvas
    // keep their width and lose rows; the index stops there too.
    let grid = grid((u16::MAX, u16::MAX), &[(4, CellBox::new(100, 30, 6, 2))]);
    let cells = usize::from(BUCKET_W) * usize::from(BUCKET_H);
    assert!(
        grid.buckets.len() <= crate::raster::MAX_CELLS / cells + usize::from(grid.cols),
        "{} buckets",
        grid.buckets.len()
    );
    assert_eq!(grid.query(102, 31), [4]);
    assert_eq!(grid.query(102, u16::MAX - 1), [] as [u32; 0]);
}
