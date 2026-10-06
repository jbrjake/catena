use super::*;

/// Every cell's glyph, rows joined by `|`.
pub(super) fn raster(canvas: &SubCellCanvas) -> String {
    canvas
        .render()
        .iter()
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("|")
}

fn lit_cells(canvas: &SubCellCanvas) -> usize {
    canvas
        .render()
        .iter()
        .flatten()
        .filter(|&&c| c != '\u{2800}')
        .count()
}

#[test]
fn empty_canvas_renders_blank_braille() {
    let canvas = SubCellCanvas::new(Blitter::Braille, 3, 2);
    let chars = canvas.render();
    assert_eq!(chars.len(), 2);
    assert_eq!(chars[0].len(), 3);
    // Empty Braille = U+2800
    assert_eq!(chars[0][0], '\u{2800}');
}

#[test]
fn single_pixel_top_left() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 1, 1);
    canvas.set_pixel(0, 0); // bit0 = 0x01
    let chars = canvas.render();
    assert_eq!(chars[0][0], '\u{2801}'); // U+2800 + 0x01
}

#[test]
fn single_pixel_bottom_right() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 1, 1);
    canvas.set_pixel(1, 3); // bit7 = 0x80
    let chars = canvas.render();
    assert_eq!(chars[0][0], '\u{2880}'); // U+2800 + 0x80
}

#[test]
fn all_dots_filled() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 1, 1);
    for y in 0..4 {
        for x in 0..2 {
            canvas.set_pixel(x, y);
        }
    }
    let chars = canvas.render();
    assert_eq!(chars[0][0], '\u{28FF}'); // all 8 dots = 0xFF
}

#[test]
fn pixel_resolution_matches_cells() {
    let canvas = SubCellCanvas::new(Blitter::Braille, 10, 5);
    assert_eq!(canvas.pixel_width(), 20);
    assert_eq!(canvas.pixel_height(), 20);
}

#[test]
fn out_of_bounds_clipped() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 1, 1);
    canvas.set_pixel(5, 5); // way out of bounds — should not panic
    let chars = canvas.render();
    assert_eq!(chars[0][0], '\u{2800}'); // still empty
}

#[test]
fn draw_line_horizontal() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 3, 1);
    canvas.draw_line(0, 0, 5, 0);
    let chars = canvas.render();
    // First 3 cells should have top-left dots set
    for c in &chars[0] {
        assert_ne!(*c, '\u{2800}'); // something is set
    }
}

#[test]
fn clear_resets_canvas() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 2, 2);
    canvas.set_pixel(0, 0);
    canvas.set_pixel(3, 7);
    canvas.clear();
    let chars = canvas.render();
    for row in &chars {
        for c in row {
            assert_eq!(*c, '\u{2800}');
        }
    }
}

#[test]
fn draw_dashed_line_has_gaps() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 10, 1);
    // dash_on=2, dash_off=2: draw 2 pixels, skip 2, repeat
    canvas.draw_dashed_line(0, 0, 19, 0, 2, 2);
    let non_empty = lit_cells(&canvas);
    // A fully solid line of 20 pixels at y=0 would fill ~10 cells.
    // A dashed line (50% duty cycle) should fill fewer cells.
    assert!(
        non_empty > 0 && non_empty < 10,
        "Dashed line should have gaps, got {non_empty} non-empty cells"
    );
}

#[test]
fn draw_dashed_bezier_has_gaps() {
    let mut canvas_solid = SubCellCanvas::new(Blitter::Braille, 10, 5);
    canvas_solid.draw_bezier(0, 0, 19, 19);
    let solid_count = lit_cells(&canvas_solid);

    let mut canvas_dashed = SubCellCanvas::new(Blitter::Braille, 10, 5);
    canvas_dashed.draw_dashed_bezier(0, 0, 19, 19, 3, 2);
    let dashed_count = lit_cells(&canvas_dashed);

    assert!(
        dashed_count < solid_count,
        "Dashed bezier ({dashed_count} cells) should have fewer pixels than solid ({solid_count})"
    );
}

#[test]
fn draw_bezier_sets_pixels() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 10, 5);
    canvas.draw_bezier(0, 0, 19, 19);
    let non_empty = lit_cells(&canvas);
    assert!(non_empty > 0, "Bezier should set some pixels");
}

#[test]
fn draw_circle_sets_pixels_symmetrically() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 10, 5);
    canvas.draw_circle(10, 10, 8);
    let non_empty = lit_cells(&canvas);
    assert!(
        non_empty >= 4,
        "Circle should set pixels in multiple cells, got {non_empty}"
    );
}

#[test]
fn draw_circle_entirely_off_screen_draws_nothing() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 5, 5);
    canvas.draw_circle(-10, -10, 5);
    canvas.draw_circle(100, 100, 50);
    assert_eq!(lit_cells(&canvas), 0);
}

#[test]
fn off_screen_bezier_draws_only_where_it_crosses_the_canvas() {
    // The bow is perpendicular to the direction of travel, so these two curves bulge to
    // opposite sides: the first sweeps through the canvas, the second misses it entirely.
    let mut crossing = SubCellCanvas::new(Blitter::Braille, 5, 5);
    crossing.draw_bezier(-10, -10, 100, 100);
    assert!(lit_cells(&crossing) > 0);
    let mut missing = SubCellCanvas::new(Blitter::Braille, 5, 5);
    missing.draw_bezier(100, 100, -50, -50);
    assert_eq!(lit_cells(&missing), 0);
}

#[test]
fn clear_pixel_clears_set_dot() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 1, 1);
    canvas.set_pixel(0, 0);
    assert_ne!(canvas.render()[0][0], '\u{2800}', "pixel should be set");
    canvas.clear_pixel(0, 0);
    assert_eq!(canvas.render()[0][0], '\u{2800}', "pixel should be cleared");
}

#[test]
fn clear_pixel_leaves_other_dots() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 1, 1);
    canvas.set_pixel(0, 0); // bit0
    canvas.set_pixel(1, 3); // bit7
    canvas.clear_pixel(0, 0);
    // bit0 cleared, bit7 remains → 0x80
    assert_eq!(canvas.render()[0][0], '\u{2880}');
}

#[test]
fn draw_line_with_hops_creates_gap() {
    // Draw a horizontal line with a hop in the middle; compare to solid line.
    let mut canvas_solid = SubCellCanvas::new(Blitter::Braille, 10, 1);
    canvas_solid.draw_line(0, 2, 19, 2);
    let solid_count = lit_cells(&canvas_solid);

    let mut canvas_hop = SubCellCanvas::new(Blitter::Braille, 10, 1);
    canvas_hop.draw_line_with_hops(0, 2, 19, 2, &[(10, 2, 3)]);
    let hop_count = lit_cells(&canvas_hop);

    assert!(
        hop_count < solid_count,
        "Hop line ({hop_count} cells) should have fewer lit cells than solid ({solid_count})"
    );
}

#[test]
fn draw_dashed_bezier_ctrl_has_gaps() {
    // Solid Bézier with explicit control point
    let mut canvas_solid = SubCellCanvas::new(Blitter::Braille, 10, 5);
    canvas_solid.draw_bezier_ctrl(0, 0, 19, 19, 0, 19);
    let solid_count = lit_cells(&canvas_solid);

    // Dashed Bézier with the same control point
    let mut canvas_dashed = SubCellCanvas::new(Blitter::Braille, 10, 5);
    canvas_dashed.draw_dashed_bezier_ctrl(0, 0, 19, 19, 0, 19, 3, 3);
    let dashed_count = lit_cells(&canvas_dashed);

    assert!(
        dashed_count > 0,
        "Dashed bezier ctrl should set some pixels, got 0"
    );
    assert!(
        dashed_count < solid_count,
        "Dashed bezier ctrl ({dashed_count} cells) should have fewer pixels than solid ({solid_count})"
    );
}

#[test]
fn draw_bezier_differs_from_line() {
    let mut canvas_line = SubCellCanvas::new(Blitter::Braille, 10, 5);
    canvas_line.draw_line(0, 0, 19, 19);

    let mut canvas_bezier = SubCellCanvas::new(Blitter::Braille, 10, 5);
    canvas_bezier.draw_bezier(0, 0, 19, 19);

    // At least some cells should differ (the curve bows outward)
    assert_ne!(
        raster(&canvas_line),
        raster(&canvas_bezier),
        "Bezier should produce different pixels than a straight line"
    );
}

/// GRAPH-001: `draw_dashed_line` / `draw_dashed_bezier` / `draw_dashed_bezier_ctrl`
/// must not panic when `dash_on = 0` and `dash_off = 0` (period 0 → modulo by zero).
/// A zero period draws nothing.
#[test]
fn draw_dashed_line_zero_period_draws_nothing() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 5, 2);
    canvas.draw_dashed_line(0, 0, 9, 0, 0, 0);
    assert_eq!(lit_cells(&canvas), 0);
}

#[test]
fn draw_dashed_bezier_zero_period_draws_nothing() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 5, 2);
    canvas.draw_dashed_bezier(0, 0, 9, 7, 0, 0);
    assert_eq!(lit_cells(&canvas), 0);
}

#[test]
fn draw_dashed_bezier_ctrl_zero_period_draws_nothing() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 5, 2);
    canvas.draw_dashed_bezier_ctrl(0, 0, 9, 7, 5, 0, 0, 0);
    assert_eq!(lit_cells(&canvas), 0);
}

// ── Port regressions (plan §14 row 13 and the overflow family) ─────────────

#[test]
fn get_cell_outside_the_canvas_reads_empty_instead_of_aliasing_or_panicking() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 2, 2);
    canvas.set_pixel(3, 7); // cell (row 1, col 1), bottom-right dot
    canvas.set_pixel(0, 4); // cell (row 1, col 0), top-left dot
    assert_eq!(canvas.get_cell(1, 1), 0x80);
    assert_eq!(canvas.get_cell(1, 0), 0x01);
    // The seed indexed `row * width + col` unchecked: (0, 2) aliased cell (1, 0) and
    // (5, 0) panicked.
    assert_eq!(canvas.get_cell(0, 2), 0);
    assert_eq!(canvas.get_cell(5, 0), 0);
    assert_eq!(canvas.get_cell(usize::MAX, usize::MAX), 0);
}

#[test]
fn try_get_cell_distinguishes_outside_from_empty() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 2, 1);
    canvas.set_pixel(2, 0);
    assert_eq!(canvas.try_get_cell(0, 1), Some(0x01));
    assert_eq!(canvas.try_get_cell(0, 0), Some(0));
    assert_eq!(canvas.try_get_cell(0, 2), None);
    assert_eq!(canvas.try_get_cell(1, 0), None);
}

#[test]
fn a_dash_longer_than_u32_period_draws_solid() {
    // dash_on + dash_off overflowed u32 in the seed and panicked.
    let mut dashed = SubCellCanvas::new(Blitter::Braille, 4, 1);
    dashed.draw_dashed_line(0, 0, 7, 0, u32::MAX, 1);
    let mut solid = SubCellCanvas::new(Blitter::Braille, 4, 1);
    solid.draw_line(0, 0, 7, 0);
    assert_eq!(raster(&dashed), raster(&solid));
}

#[test]
fn a_distant_hop_disc_changes_nothing() {
    // The seed squared the distance to the hop centre in i32 and overflowed.
    let mut hopped = SubCellCanvas::new(Blitter::Braille, 4, 1);
    hopped.draw_line_with_hops(0, 0, 7, 0, &[(i32::MAX, i32::MAX, 1)]);
    let mut plain = SubCellCanvas::new(Blitter::Braille, 4, 1);
    plain.draw_line(0, 0, 7, 0);
    assert_eq!(raster(&hopped), raster(&plain));
}

#[test]
fn primitives_near_the_i32_limits_draw_nothing_on_canvas_and_do_not_overflow() {
    // Each of these overflowed i32 arithmetic in the seed (a sum of endpoints, a centre plus a
    // radius, an absolute difference) and panicked in debug builds.
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 4, 2);
    canvas.draw_bezier(i32::MAX, 0, i32::MAX - 1, 4);
    canvas.draw_dashed_bezier(i32::MIN, 0, i32::MIN + 1, 4, 1, 1);
    canvas.draw_bezier_ctrl(i32::MAX, 0, i32::MAX, 4, i32::MAX, 2);
    canvas.draw_circle(i32::MAX, 0, 3);
    canvas.draw_circle(i32::MIN, i32::MIN, 3);
    canvas.draw_line(i32::MAX - 2, i32::MIN, i32::MAX, i32::MIN + 2);
    assert_eq!(lit_cells(&canvas), 0);
}

#[test]
fn a_segment_across_the_i32_range_draws_only_what_the_canvas_shows() {
    // Unclipped, each of these walks some 4·10⁹ pixels; clipped, the visible stretch and the
    // slack around it.
    let mut line = SubCellCanvas::new(Blitter::Braille, 8, 2);
    line.draw_line(i32::MIN, 0, i32::MAX, 3);
    line.draw_dashed_line(i32::MIN, 0, i32::MAX, 3, 1, 1);
    line.draw_line_with_hops(i32::MIN, 0, i32::MAX, 3, &[(4, 2, 1)]);
    // At x = 0 the line has risen ⌊(6·2³¹ + M) / 2M⌋ = 2 rows (M = 2³² − 1), and it stays there
    // across the canvas.
    let rows: Vec<String> = line
        .render()
        .iter()
        .map(|row| row.iter().collect())
        .collect();
    assert_eq!(rows, ["⠤⠤⠤⠤⠤⠤⠤⠤", "⠀⠀⠀⠀⠀⠀⠀⠀"]);

    let mut bowed = SubCellCanvas::new(Blitter::Braille, 8, 2);
    bowed.draw_bezier(i32::MIN, 0, i32::MAX, 3);
    bowed.draw_dashed_bezier(i32::MIN, 0, i32::MAX, 3, 2, 2);
    assert_eq!(lit_cells(&bowed), 0, "the bow passes some 8·10⁸ rows away");
}
