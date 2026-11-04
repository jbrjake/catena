use catena::raster::CellGrid;
use ratatui_core::style::Color;

use super::*;
use crate::widget::color;

#[test]
fn the_minimap_draws_the_cores_overview_in_its_area_and_leaves_the_view_alone() {
    let mut gv = catena_testkit::fixtures::community()
        .view(GraphView::builder())
        .expect("fixtures are consistent");
    let before = gv.render_to_string(80, 24);
    let mut grid = CellGrid::new(24, 8);
    gv.render_overview(&mut grid);
    let whole = Rect::new(0, 0, 40, 12);
    let mut buf = Buffer::empty(whole);
    let area = Rect::new(9, 3, 24, 8);
    Minimap::new(&gv).render(area, &mut buf);
    let mut drawn = 0;
    for y in 0..whole.height {
        for x in 0..whole.width {
            let cell = &buf[(x, y)];
            if area.contains((x, y).into()) {
                let want = grid.cell(x - area.x, y - area.y).expect("on");
                assert_eq!(cell.symbol(), want.symbol(), "({x}, {y})");
                assert_eq!(cell.fg, want.fg().map_or(Color::Reset, color), "({x}, {y})");
                drawn += usize::from(want.symbol() != " ");
            } else {
                assert_eq!(cell, &Buffer::empty(whole)[(x, y)], "({x}, {y}) is outside");
            }
        }
    }
    assert!(drawn > 10, "it draws: {drawn}");
    assert_eq!(gv.render_to_string(80, 24), before);
}

#[test]
fn the_minimap_clears_its_area_so_it_can_float_over_the_graph() {
    let gv = catena_testkit::fixtures::community()
        .view(GraphView::builder())
        .expect("fixtures are consistent");
    let area = Rect::new(0, 0, 24, 8);
    let mut buf = Buffer::empty(area);
    for y in 0..8 {
        for x in 0..24 {
            buf[(x, y)].set_symbol("@").set_fg(Color::Red);
        }
    }
    Minimap::new(&gv).render(area, &mut buf);
    let mut grid = CellGrid::new(24, 8);
    gv.render_overview(&mut grid);
    for y in 0..8 {
        for x in 0..24 {
            assert_eq!(buf[(x, y)].symbol(), grid.cell(x, y).expect("on").symbol());
        }
    }
}
