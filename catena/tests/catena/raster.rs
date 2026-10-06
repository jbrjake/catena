//! Braille line quality, checked with the testkit's raster oracle (plan §16.2-E). These live in
//! the integration target because the oracle takes `catena`'s own `SubCellCanvas`, and only
//! here do `catena` and `catena-testkit` share one copy of it.

use std::collections::HashSet;

use catena::raster::{Blitter, SubCellCanvas};
use catena_testkit::braille_asserts::{
    assert_8_connected, assert_endpoints_exact, assert_no_duplicates, lit_pixels,
};

#[test]
fn helpers_compile_smoke() {
    let mut c = SubCellCanvas::new(Blitter::Braille, 10, 10);
    c.draw_line(0, 0, 9, 9);
    let px = lit_pixels(&c);
    assert_eq!(px.len(), 10, "a 45° line lights one pixel per step");
    assert_no_duplicates(&px);
    assert_8_connected(&px);
    assert_endpoints_exact(&px, 0, 0, 9, 9);
}

/// BRAILLE-001: `draw_line_with_hops` must always draw the endpoint pixel even
/// when the endpoint falls inside a hop disc.  Without the fix the hop guard
/// would suppress the very last pixel, making the line appear to end short.
#[test]
fn hop_endpoint_inside_disc_still_draws_endpoint() {
    // Horizontal line from x=0 to x=10, y=0.
    // Place a hop disc centred exactly on the endpoint (10, 0) with radius 2.
    // All pixels in [8..=10] are inside the disc, so the endpoint pixel at
    // x=10 must still be drawn (endpoints are never suppressed by hops).
    let mut canvas = SubCellCanvas::new(Blitter::Braille, 10, 1);
    canvas.draw_line_with_hops(0, 0, 10, 0, &[(10, 0, 2)]);
    let px = lit_pixels(&canvas);
    let set: HashSet<(usize, usize)> = px.iter().copied().collect();
    assert!(
        set.contains(&(10, 0)),
        "endpoint (10,0) must be drawn even though it falls inside the hop disc; lit pixels = {px:?}"
    );
}

#[test]
fn line_is_8_connected_horizontal() {
    let mut c = SubCellCanvas::new(Blitter::Braille, 12, 4);
    c.draw_line(0, 0, 19, 0); // 20 px across row 0
    let px = lit_pixels(&c);
    assert_eq!(
        px.len(),
        20,
        "horizontal line should have 20 distinct pixels"
    );
    assert_8_connected(&px);
    assert_endpoints_exact(&px, 0, 0, 19, 0);
    assert_no_duplicates(&px);
}

#[test]
fn line_is_8_connected_vertical() {
    let mut c = SubCellCanvas::new(Blitter::Braille, 4, 5);
    c.draw_line(0, 0, 0, 15); // 16 px down col 0
    let px = lit_pixels(&c);
    assert_eq!(px.len(), 16, "vertical line should have 16 distinct pixels");
    assert_8_connected(&px);
    assert_endpoints_exact(&px, 0, 0, 0, 15);
    assert_no_duplicates(&px);
}

#[test]
fn line_is_8_connected_diagonal_45() {
    let mut c = SubCellCanvas::new(Blitter::Braille, 10, 6);
    c.draw_line(0, 0, 15, 15); // 45° diagonal
    let px = lit_pixels(&c);
    // A 45° Bresenham produces one pixel per step: 16 pixels
    assert_eq!(px.len(), 16, "45° diagonal should have 16 pixels");
    assert_8_connected(&px);
    assert_endpoints_exact(&px, 0, 0, 15, 15);
    assert_no_duplicates(&px);
}

#[test]
fn line_is_8_connected_steep() {
    let mut c = SubCellCanvas::new(Blitter::Braille, 4, 5);
    c.draw_line(0, 0, 3, 15); // steep: dy >> dx
    let px = lit_pixels(&c);
    assert_8_connected(&px);
    assert_endpoints_exact(&px, 0, 0, 3, 15);
    assert_no_duplicates(&px);
}

#[test]
fn line_endpoints_are_exact_all_octants() {
    // Test all 8 octant directions from a centre point.
    // Canvas: 22×22 cells → 44×88 pixels. Centre: (22, 44).
    let cx: i32 = 22;
    let cy: i32 = 44;
    let endpoints: &[(i32, i32)] = &[
        (cx + 20, cy + 8), // Octant 1: ENE — dx>dy>0
        (cx + 8, cy + 20), // Octant 2: SSE — dy>dx>0
        (cx - 8, cy + 20), // Octant 3: SSW — dy>|dx|>0
        (cx - 20, cy + 8), // Octant 4: WSW — |dx|>dy>0
        (cx - 20, cy - 8), // Octant 5: WNW — |dx|>|dy|>0
        (cx - 8, cy - 20), // Octant 6: NNW — |dy|>|dx|>0
        (cx + 8, cy - 20), // Octant 7: NNE — |dy|>dx>0
        (cx + 20, cy - 8), // Octant 8: ENE — dx>|dy|>0
    ];
    for &(x1, y1) in endpoints {
        let mut c = SubCellCanvas::new(Blitter::Braille, 22, 22);
        c.draw_line(cx, cy, x1, y1);
        let px = lit_pixels(&c);
        assert_endpoints_exact(&px, cx, cy, x1, y1);
        assert_8_connected(&px);
    }
}

#[test]
fn line_no_duplicates_diagonal() {
    let mut c = SubCellCanvas::new(Blitter::Braille, 10, 6);
    c.draw_line(2, 0, 19, 11); // off-axis: dx=17, dy=11 (not 45°, not steep)
    let px = lit_pixels(&c);
    assert_no_duplicates(&px);
    assert_8_connected(&px);
}

#[test]
fn joined_lines_share_endpoint_pixel() {
    let (ax, ay) = (0i32, 0i32);
    let (bx, by) = (10i32, 8i32);
    let (ex, ey) = (20i32, 0i32);

    // Verify segment A→B individually reaches B
    let mut c_ab = SubCellCanvas::new(Blitter::Braille, 12, 4);
    c_ab.draw_line(ax, ay, bx, by);
    let px_ab = lit_pixels(&c_ab);
    assert_endpoints_exact(&px_ab, ax, ay, bx, by);
    assert_8_connected(&px_ab);

    // Verify segment B→E individually reaches B
    let mut c_be = SubCellCanvas::new(Blitter::Braille, 12, 4);
    c_be.draw_line(bx, by, ex, ey);
    let px_be = lit_pixels(&c_be);
    assert_endpoints_exact(&px_be, bx, by, ex, ey);
    assert_8_connected(&px_be);

    // Verify the combined canvas has B lit and the full path is 8-connected
    let mut c = SubCellCanvas::new(Blitter::Braille, 12, 4);
    c.draw_line(ax, ay, bx, by);
    c.draw_line(bx, by, ex, ey);
    let px = lit_pixels(&c);
    let b_pixel = (10usize, 8usize);
    assert!(
        px.contains(&b_pixel),
        "Junction pixel {b_pixel:?} must be lit"
    );
    assert_8_connected(&px);
}

#[test]
fn dashed_line_on_1_off_0_equals_solid() {
    // dash_on=1, dash_off=0 → period=1, every pixel drawn → identical to draw_line
    let (x0, y0, x1, y1) = (0i32, 0i32, 19i32, 11i32);

    let mut solid = SubCellCanvas::new(Blitter::Braille, 12, 4);
    solid.draw_line(x0, y0, x1, y1);

    let mut dashed = SubCellCanvas::new(Blitter::Braille, 12, 4);
    dashed.draw_dashed_line(x0, y0, x1, y1, 1, 0);

    let solid_px: HashSet<_> = lit_pixels(&solid).into_iter().collect();
    let dashed_px: HashSet<_> = lit_pixels(&dashed).into_iter().collect();
    assert_eq!(solid_px.len(), 20, "a 20-step line lights 20 pixels");
    assert_eq!(
        solid_px, dashed_px,
        "dashed_line(on=1, off=0) must produce identical pixels to draw_line"
    );
}

#[test]
fn hop_line_8_connected_outside_hops() {
    // A line with a hop gap: the pixels OUTSIDE the hop disc must still
    // form two 8-connected sub-segments (each independently connected).
    // We verify this by checking that the overall lit set is NOT fully
    // connected (the gap breaks it), but each side IS connected.
    let (hop_x, hop_r) = (20usize, 3usize);

    let mut c = SubCellCanvas::new(Blitter::Braille, 22, 2);
    c.draw_line_with_hops(0, 0, 39, 0, &[(20, 0, 3)]);

    let all_px = lit_pixels(&c);

    // Split into left and right halves of the gap
    let left: Vec<(usize, usize)> = all_px
        .iter()
        .copied()
        .filter(|&(x, _)| x < hop_x - hop_r)
        .collect();
    let right: Vec<(usize, usize)> = all_px
        .iter()
        .copied()
        .filter(|&(x, _)| x > hop_x + hop_r)
        .collect();

    assert!(!left.is_empty(), "left segment must have pixels");
    assert!(!right.is_empty(), "right segment must have pixels");
    assert_8_connected(&left);
    assert_8_connected(&right);

    // Verify the hop actually created a gap.
    // A disc of radius 3 on a horizontal line at y=hop_y removes exactly
    // 2*hop_r+1 = 7 pixels (x in [17..=23]). Remaining: 40 - 7 = 33.
    assert_eq!(
        all_px.len(),
        33,
        "hop gap should remove exactly 7 pixels from a 40-px horizontal line (got {} pixels)",
        all_px.len()
    );
}

#[test]
fn lit_pixels_decodes_the_unicode_dot_numbering() {
    // One dot at a time: pixel (x, y) in a 1×1 canvas must light exactly the Unicode dot the
    // standard assigns to it, independent of how catena encodes it.
    for y in 0..4 {
        for x in 0..2 {
            let mut c = SubCellCanvas::new(Blitter::Braille, 1, 1);
            c.set_pixel(x, y);
            assert_eq!(lit_pixels(&c), [(x, y)]);
        }
    }
    let mut c = SubCellCanvas::new(Blitter::Braille, 1, 1);
    c.set_pixel(1, 3);
    assert_eq!(c.render()[0][0], '\u{2880}', "dot 8 is bit 7");
}
