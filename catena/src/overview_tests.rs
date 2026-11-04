//! The overview against hand-computed cells: plan §18's minimap, drawn from the view's layout
//! without changing the view.

use super::*;
use crate::geometry::cell::SubPt;
use crate::graph::NodeSpec;
use crate::raster::{Attrs, Blitter, CellGrid, PaletteColor};

/// A star of nodes `n0`…`n4` pinned in the middle, left, right, above and below, drawn once.
fn star(blitter: Blitter) -> GraphView<String> {
    let mut gv = GraphView::builder().blitter(blitter).build();
    gv.update(|tx| {
        for (i, at) in [
            (0.0, 0.0),
            (-40.0, 0.0),
            (40.0, 0.0),
            (0.0, -40.0),
            (0.0, 40.0),
        ]
        .into_iter()
        .enumerate()
        {
            let mut spec = NodeSpec::label(format!("n{i}"));
            spec.pinned = Some(at);
            tx.add_node(format!("n{i}"), spec)?;
        }
        Ok(())
    })
    .expect("fresh keys");
    gv.render_to_string(80, 24);
    gv
}

/// The star's dots on a 20 × 10 braille overview, worked by hand. The view (80 × 24) fits the
/// star at 0.525 columns a world unit (its 21 free rows over 80 units at aspect 0.5; the widest
/// box, `*[n0]`, is 5), its left node at column 19 and top at row 1.5, so its frame, half a cell
/// past its edge cells, spans x −77.14 to 75.24 and y −47.62 to 43.81. The overview fits that
/// with the nodes: its 18 free columns over 152.38 units (the 8 free rows, 16 columns tall, fit
/// 91.43 more loosely) give 0.23625 pixels a unit either way, centered from pixel 2 across and
/// 9.2 down. So `n0` lands at pixel (20.2, 20.5), cell (10, 5).
const DOTS: [(u16, u16); 5] = [(10, 5), (5, 5), (14, 5), (10, 2), (10, 7)];

fn lit(grid: &CellGrid) -> Vec<(u16, u16)> {
    let (w, h) = grid.size();
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| grid.cell(x, y).is_some_and(|c| c.symbol() != " "))
        .collect()
}

fn colored(grid: &CellGrid, color: PaletteColor) -> Vec<(u16, u16)> {
    lit(grid)
        .into_iter()
        .filter(|&(x, y)| grid.cell(x, y).and_then(crate::raster::Cell::fg) == Some(color))
        .collect()
}

#[test]
fn every_node_is_a_dot_where_the_fit_puts_it_and_the_view_is_untouched() {
    let mut gv = star(Blitter::Braille);
    let before = gv.render_to_string(80, 24);
    let cells: Vec<_> = (0..5)
        .map(|i| {
            gv.viewport
                .cell(gv.node_ix(&format!("n{i}")).expect("live"))
        })
        .collect();
    let mut overview = CellGrid::new(20, 10);
    gv.render_overview(&mut overview);
    let dots = colored(&overview, PaletteColor::Ansi(7));
    assert_eq!(dots, {
        let mut sorted = DOTS.to_vec();
        sorted.sort_by_key(|&(x, y)| (y, x));
        sorted
    });
    for &(x, y) in &DOTS {
        let symbol = overview.cell(x, y).expect("on").symbol().to_string();
        assert_eq!(symbol.chars().count(), 1, "one braille dot at ({x}, {y})");
    }
    assert_eq!(
        gv.render_to_string(80, 24),
        before,
        "the view draws as before"
    );
    let after: Vec<_> = (0..5)
        .map(|i| {
            gv.viewport
                .cell(gv.node_ix(&format!("n{i}")).expect("live"))
        })
        .collect();
    assert_eq!(after, cells);
}

/// The bounds of the viewport rectangle's cells.
fn frame_of(grid: &CellGrid) -> (u16, u16, u16, u16) {
    let cells = colored(grid, PaletteColor::Ansi(14));
    assert!(!cells.is_empty(), "a rectangle is drawn");
    let xs = cells.iter().map(|c| c.0);
    let ys = cells.iter().map(|c| c.1);
    (
        xs.clone().min().expect("cells"),
        ys.clone().min().expect("cells"),
        xs.max().expect("cells"),
        ys.max().expect("cells"),
    )
}

#[test]
fn the_rectangle_frames_what_the_view_shows_and_follows_its_zoom_and_pan() {
    let mut gv = star(Blitter::Braille);
    let mut overview = CellGrid::new(40, 20);
    gv.render_overview(&mut overview);
    let whole = frame_of(&overview);
    // The view shows the whole star, so its frame holds every dot.
    for (x, y) in colored(&overview, PaletteColor::Ansi(7)) {
        assert!(
            (whole.0..=whole.2).contains(&x) && (whole.1..=whole.3).contains(&y),
            "({x}, {y}) outside {whole:?}"
        );
    }
    gv.zoom_about(2.0, SubPt::new(39.5, 11.5));
    let mut zoomed = CellGrid::new(40, 20);
    gv.render_overview(&mut zoomed);
    let near = frame_of(&zoomed);
    let span = |f: (u16, u16, u16, u16)| (f.2 - f.0, f.3 - f.1);
    assert!(
        span(near).0 < span(whole).0 && span(near).1 < span(whole).1,
        "{near:?} inside {whole:?}"
    );
    gv.pan_by((8.0, 0.0));
    let mut panned = CellGrid::new(40, 20);
    gv.render_overview(&mut panned);
    assert!(
        frame_of(&panned).0 < near.0,
        "the drawing moved right, so the view looks further left"
    );
}

#[test]
fn the_selected_node_fills_its_cell_marked() {
    let mut gv = star(Blitter::Braille);
    gv.controller.selected = gv.node_ix(&"n2".to_string());
    let mut overview = CellGrid::new(20, 10);
    gv.render_overview(&mut overview);
    let cell = overview.cell(DOTS[2].0, DOTS[2].1).expect("on");
    assert_eq!(cell.symbol(), "⣿");
    assert!(cell.attrs().contains(Attrs::REVERSED));
}

#[test]
fn a_surface_too_small_or_a_graph_with_nothing_laid_out_draws_nothing() {
    let gv = star(Blitter::Braille);
    for (w, h) in [(0, 0), (1, 10), (10, 1)] {
        let mut overview = CellGrid::new(w, h);
        gv.render_overview(&mut overview);
        assert_eq!(overview, CellGrid::new(w, h), "{w} × {h}");
    }
    let empty: GraphView<String> = GraphView::new();
    let mut overview = CellGrid::new(20, 10);
    empty.render_overview(&mut overview);
    assert_eq!(overview, CellGrid::new(20, 10));
}

#[test]
fn on_an_ascii_overview_nodes_are_the_line_glyphs_dot() {
    let gv = star(Blitter::Ascii);
    let mut overview = CellGrid::new(20, 10);
    gv.render_overview(&mut overview);
    for &(x, y) in &DOTS {
        assert_eq!(overview.cell(x, y).expect("on").symbol(), "·", "({x}, {y})");
    }
}
