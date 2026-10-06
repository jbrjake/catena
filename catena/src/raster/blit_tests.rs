use super::*;
use crate::raster::{Attrs, CellGrid, CellStyle, PaletteColor, SubCellCanvas, Surface};

/// Every sextant codepoint and the cells its Unicode name lists (`BLOCK SEXTANT-<cells>`), cell
/// 1 top-left, 2 top-right, then down the rows: generated from the Unicode 15.1 database.
const SEXTANT_NAMES: [(u32, &str); 60] = [
    (0x1FB00, "1"),
    (0x1FB01, "2"),
    (0x1FB02, "12"),
    (0x1FB03, "3"),
    (0x1FB04, "13"),
    (0x1FB05, "23"),
    (0x1FB06, "123"),
    (0x1FB07, "4"),
    (0x1FB08, "14"),
    (0x1FB09, "24"),
    (0x1FB0A, "124"),
    (0x1FB0B, "34"),
    (0x1FB0C, "134"),
    (0x1FB0D, "234"),
    (0x1FB0E, "1234"),
    (0x1FB0F, "5"),
    (0x1FB10, "15"),
    (0x1FB11, "25"),
    (0x1FB12, "125"),
    (0x1FB13, "35"),
    (0x1FB14, "235"),
    (0x1FB15, "1235"),
    (0x1FB16, "45"),
    (0x1FB17, "145"),
    (0x1FB18, "245"),
    (0x1FB19, "1245"),
    (0x1FB1A, "345"),
    (0x1FB1B, "1345"),
    (0x1FB1C, "2345"),
    (0x1FB1D, "12345"),
    (0x1FB1E, "6"),
    (0x1FB1F, "16"),
    (0x1FB20, "26"),
    (0x1FB21, "126"),
    (0x1FB22, "36"),
    (0x1FB23, "136"),
    (0x1FB24, "236"),
    (0x1FB25, "1236"),
    (0x1FB26, "46"),
    (0x1FB27, "146"),
    (0x1FB28, "1246"),
    (0x1FB29, "346"),
    (0x1FB2A, "1346"),
    (0x1FB2B, "2346"),
    (0x1FB2C, "12346"),
    (0x1FB2D, "56"),
    (0x1FB2E, "156"),
    (0x1FB2F, "256"),
    (0x1FB30, "1256"),
    (0x1FB31, "356"),
    (0x1FB32, "1356"),
    (0x1FB33, "2356"),
    (0x1FB34, "12356"),
    (0x1FB35, "456"),
    (0x1FB36, "1456"),
    (0x1FB37, "2456"),
    (0x1FB38, "12456"),
    (0x1FB39, "3456"),
    (0x1FB3A, "13456"),
    (0x1FB3B, "23456"),
];

const RED: PaletteColor = PaletteColor::Ansi(1);
const BLUE: PaletteColor = PaletteColor::Ansi(4);

/// Slot 1 is red, slot 2 blue, anything else green.
fn palette(slot: ColorSlot) -> CellStyle {
    CellStyle::new(match slot.0 {
        1 => RED,
        2 => BLUE,
        _ => PaletteColor::Ansi(2),
    })
}

fn blitted(canvas: &SubCellCanvas) -> CellGrid {
    let (cols, rows) = (canvas.cell_cols(), canvas.cell_rows());
    let mut grid = CellGrid::new(
        u16::try_from(cols).expect("small"),
        u16::try_from(rows).expect("small"),
    );
    canvas.blit(&mut grid, palette);
    grid
}

fn at(grid: &CellGrid, x: u16, y: u16) -> (&str, Option<PaletteColor>, Option<PaletteColor>) {
    let cell = grid.cell(x, y).expect("in range");
    (cell.symbol(), cell.fg(), cell.bg())
}

#[test]
fn each_blitter_has_its_sub_cell_resolution() {
    assert_eq!(Blitter::Braille.sub_size(), (2, 4));
    assert_eq!(Blitter::HalfBlock.sub_size(), (1, 2));
    assert_eq!(Blitter::Sextant.sub_size(), (2, 3));
    assert_eq!(Blitter::Ascii.sub_size(), (1, 1));
    for blitter in [
        Blitter::Braille,
        Blitter::HalfBlock,
        Blitter::Sextant,
        Blitter::Ascii,
    ] {
        let canvas = SubCellCanvas::new(blitter, 5, 3);
        let (w, h) = blitter.sub_size();
        assert_eq!(canvas.pixel_width(), 5 * usize::from(w));
        assert_eq!(canvas.pixel_height(), 3 * usize::from(h));
    }
}

#[test]
fn braille_glyphs_are_the_dot_pattern_offset_from_u2800() {
    for mask in 1..=255u8 {
        assert_eq!(
            Blitter::Braille.glyph(mask, &LineGlyphs::BOX),
            char::from_u32(0x2800 + u32::from(mask)).expect("braille block")
        );
    }
}

#[test]
fn sextant_glyphs_follow_their_unicode_names() {
    let mut seen = std::collections::HashSet::new();
    for (codepoint, cells) in SEXTANT_NAMES {
        let mask = cells
            .bytes()
            .map(|cell| 1u8 << (cell - b'1'))
            .fold(0, |mask, bit| mask | bit);
        let glyph = Blitter::Sextant.glyph(mask, &LineGlyphs::BOX);
        assert_eq!(u32::from(glyph), codepoint, "sextant {cells}");
        assert!(seen.insert(glyph));
    }
    let blocks = [(0b01_0101, '▌'), (0b10_1010, '▐'), (0b11_1111, '█')];
    for (mask, glyph) in blocks {
        assert_eq!(Blitter::Sextant.glyph(mask, &LineGlyphs::BOX), glyph);
        assert!(seen.insert(glyph));
    }
    assert_eq!(seen.len(), 63, "every non-empty pattern has its own glyph");
}

#[test]
fn sub_pixels_set_the_bit_of_their_cell_position() {
    let mut sextant = SubCellCanvas::new(Blitter::Sextant, 2, 2);
    sextant.set_pixel(3, 5);
    assert_eq!(
        sextant.get_cell(1, 1),
        0b10_0000,
        "bottom-right is sextant 6"
    );
    let mut half = SubCellCanvas::new(Blitter::HalfBlock, 2, 2);
    half.set_pixel(1, 3);
    assert_eq!(half.get_cell(1, 1), 0b10, "the lower half");
    let mut ascii = SubCellCanvas::new(Blitter::Ascii, 2, 2);
    ascii.set_pixel(1, 1);
    assert_eq!(ascii.get_cell(1, 1), ascii_bits::DOT);
}

#[test]
fn line_direction_quantizes_the_visual_angle() {
    // With cells twice as tall as wide, (dx, dy) cells look like (dx, 2·dy).
    let cases = [
        ((10.0, 0.0), ascii_bits::H),
        ((-10.0, 2.0), ascii_bits::H),
        ((0.0, -10.0), ascii_bits::V),
        ((4.0, 5.0), ascii_bits::V),
        ((10.0, 5.0), ascii_bits::FALLING),
        ((-10.0, -3.0), ascii_bits::FALLING),
        ((10.0, -5.0), ascii_bits::RISING),
        ((-5.0, 5.0), ascii_bits::RISING),
        ((0.0, 0.0), ascii_bits::DOT),
    ];
    for ((dx, dy), bits) in cases {
        assert_eq!(line_direction(dx, dy, 0.5), bits, "({dx}, {dy})");
    }
    assert_eq!(
        line_direction(10.0, 3.0, 0.5),
        ascii_bits::FALLING,
        "31° on tall cells"
    );
    assert_eq!(
        line_direction(10.0, 3.0, 1.0),
        ascii_bits::H,
        "17° on square cells"
    );
}

#[test]
fn ascii_glyphs_merge_directions_into_junctions() {
    use ascii_bits::{DOT, FALLING, H, RISING, V};
    let cases = [
        (H, '─', '-'),
        (V, '│', '|'),
        (RISING, '╱', '/'),
        (FALLING, '╲', '\\'),
        (DOT, '·', '.'),
        (H | DOT, '─', '-'),
        (H | V, '┼', '+'),
        (RISING | FALLING, '╳', 'X'),
        (H | RISING, '┼', '+'),
        (V | FALLING | DOT, '┼', '+'),
    ];
    for (bits, boxed, ascii) in cases {
        assert_eq!(
            Blitter::Ascii.glyph(bits, &LineGlyphs::BOX),
            boxed,
            "{bits:#x}"
        );
        assert_eq!(
            Blitter::Ascii.glyph(bits, &LineGlyphs::ASCII),
            ascii,
            "{bits:#x}"
        );
    }
}

#[test]
fn blit_writes_each_lit_cell_in_its_pen_and_leaves_the_rest() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 4, 1);
    canvas.set_pen(ColorSlot(1));
    canvas.draw_line(0, 0, 3, 0);
    let mut grid = CellGrid::new(4, 1);
    grid.put(3, 0, "z", CellStyle::new(BLUE));
    canvas.blit(&mut grid, palette);
    assert_eq!(at(&grid, 0, 0), ("⠉", Some(RED), None));
    assert_eq!(at(&grid, 1, 0), ("⠉", Some(RED), None));
    assert_eq!(
        at(&grid, 3, 0),
        ("z", Some(BLUE), None),
        "an empty cell is not written"
    );
}

#[test]
fn within_a_canvas_the_last_pen_to_touch_a_cell_colors_it() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 3, 3);
    canvas.set_pen(ColorSlot(1));
    canvas.draw_line(0, 4, 5, 4);
    canvas.set_pen(ColorSlot(2));
    canvas.draw_line(2, 0, 2, 11);
    let grid = blitted(&canvas);
    assert_eq!(at(&grid, 0, 1).1, Some(RED));
    assert_eq!(at(&grid, 1, 1).1, Some(BLUE), "the crossing cell");
    assert_eq!(at(&grid, 1, 0).1, Some(BLUE));
}

#[test]
fn half_blocks_carry_a_color_per_half() {
    let mut canvas = SubCellCanvas::new(Blitter::HalfBlock, 4, 1);
    canvas.set_pen(ColorSlot(1));
    canvas.set_pixel(0, 0);
    canvas.set_pixel(2, 0);
    canvas.set_pixel(3, 0);
    canvas.set_pixel(3, 1);
    canvas.set_pen(ColorSlot(2));
    canvas.set_pixel(1, 1);
    canvas.set_pixel(2, 1);
    let grid = blitted(&canvas);
    assert_eq!(at(&grid, 0, 0), ("▀", Some(RED), None));
    assert_eq!(at(&grid, 1, 0), ("▄", Some(BLUE), None));
    assert_eq!(at(&grid, 2, 0), ("▀", Some(RED), Some(BLUE)));
    assert_eq!(
        at(&grid, 3, 0),
        ("█", Some(RED), None),
        "one color fills the cell"
    );
}

#[test]
fn the_blit_keeps_the_pens_attributes() {
    let mut canvas = SubCellCanvas::new(Blitter::Sextant, 1, 1);
    canvas.set_pen(ColorSlot(9));
    canvas.set_pixel(0, 0);
    let mut grid = CellGrid::new(1, 1);
    let dim = |_: ColorSlot| CellStyle {
        attrs: Attrs::DIM,
        ..CellStyle::new(RED)
    };
    canvas.blit(&mut grid, dim);
    let cell = grid.cell(0, 0).expect("in range");
    assert_eq!((cell.symbol(), cell.attrs()), ("\u{1FB00}", Attrs::DIM));
}

#[test]
fn ascii_lines_draw_as_direction_glyphs_with_a_junction_where_they_cross() {
    let mut canvas = SubCellCanvas::new(Blitter::Ascii, 7, 5);
    canvas.draw_line(0, 2, 6, 2);
    canvas.draw_line(3, 0, 3, 4);
    canvas.draw_line(0, 0, 2, 1);
    assert_eq!(
        blitted(&canvas).to_string(),
        "╲  │\n ╲╲│\n───┼───\n   │\n   │"
    );
    canvas.set_line_glyphs(LineGlyphs::ASCII);
    assert_eq!(
        blitted(&canvas).to_string(),
        "\\  |\n \\\\|\n---+---\n   |\n   |"
    );
}

#[test]
fn an_ascii_polyline_turns_its_glyph_at_each_vertex_and_dashes_keep_theirs() {
    let mut canvas = SubCellCanvas::new(Blitter::Ascii, 6, 4);
    canvas.draw_polyline(&[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)]);
    assert_eq!(blitted(&canvas).to_string(), "─────\n    │\n    │\n    │");
    let mut dashed = SubCellCanvas::new(Blitter::Ascii, 10, 1);
    dashed.draw_dashed_line(0, 0, 9, 0, 2, 2);
    assert_eq!(blitted(&dashed).to_string(), "──  ──  ──");
}

#[test]
fn an_ascii_circle_draws_its_tangents() {
    let mut canvas = SubCellCanvas::new(Blitter::Ascii, 9, 5);
    canvas.draw_circle(4, 2, 2);
    let grid = blitted(&canvas);
    assert_eq!(at(&grid, 4, 0).0, "─", "the top's tangent is horizontal");
    assert_eq!(at(&grid, 4, 4).0, "─");
    assert_eq!(at(&grid, 2, 2).0, "│", "the left's tangent is vertical");
    assert_eq!(at(&grid, 6, 2).0, "│");
}

#[test]
fn merging_keeps_the_lower_dots_and_takes_the_upper_color() {
    // The seed's paint-behind blit dropped the lower layer's dots where an overlay edge crossed
    // (plan §14 row 15); the merge ORs them.
    let mut under = SubCellCanvas::new(Blitter::Braille, 3, 1);
    under.set_pen(ColorSlot(1));
    under.draw_line(0, 0, 5, 0);
    let mut over = SubCellCanvas::new(Blitter::Braille, 3, 1);
    over.set_pen(ColorSlot(2));
    over.draw_line(2, 3, 3, 3);
    under.merge_from(&over);
    assert_eq!(under.get_cell(0, 1), 0x09 | 0xC0, "both layers' dots");
    let grid = blitted(&under);
    assert_eq!(at(&grid, 0, 0), ("⠉", Some(RED), None));
    assert_eq!(
        at(&grid, 1, 0),
        ("⣉", Some(BLUE), None),
        "the upper layer colors the cell"
    );
}

#[test]
fn half_blocks_merge_each_half_on_its_own() {
    let mut under = SubCellCanvas::new(Blitter::HalfBlock, 1, 1);
    under.set_pen(ColorSlot(1));
    under.set_pixel(0, 0);
    under.set_pixel(0, 1);
    let mut over = SubCellCanvas::new(Blitter::HalfBlock, 1, 1);
    over.set_pen(ColorSlot(2));
    over.set_pixel(0, 1);
    under.merge_from(&over);
    assert_eq!(at(&blitted(&under), 0, 0), ("▀", Some(RED), Some(BLUE)));
}

#[test]
fn a_canvas_of_another_shape_does_not_merge() {
    let mut braille = SubCellCanvas::new(Blitter::Braille, 2, 1);
    let mut sextant = SubCellCanvas::new(Blitter::Sextant, 2, 1);
    sextant.set_pixel(0, 0);
    braille.merge_from(&sextant);
    let mut wider = SubCellCanvas::new(Blitter::Braille, 3, 1);
    wider.set_pixel(0, 0);
    braille.merge_from(&wider);
    assert_eq!(braille.render(), [['\u{2800}'; 2]]);
}

#[test]
fn a_masked_cell_is_not_written() {
    let mut canvas = SubCellCanvas::new(Blitter::Ascii, 4, 1);
    canvas.draw_line(0, 0, 3, 0);
    let mut grid = CellGrid::new(4, 1);
    grid.put(1, 0, "L", CellStyle::new(BLUE));
    canvas.blit_masked(&mut grid, palette, &[false, true, true, false]);
    assert_eq!(grid.to_string(), "─L ─");
}

#[test]
fn a_canvas_request_past_the_cell_ceiling_is_bounded() {
    let mut canvas = SubCellCanvas::new(Blitter::Braille, u16::MAX, u16::MAX);
    assert_eq!(
        (canvas.cell_cols(), canvas.cell_rows()),
        (usize::from(u16::MAX), 64)
    );
    assert!(canvas.cell_cols() * canvas.cell_rows() <= crate::raster::MAX_CELLS);
    canvas.clear_and_resize(u16::MAX, u16::MAX);
    assert_eq!(canvas.cell_rows(), 64);
}
