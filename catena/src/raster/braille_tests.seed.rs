use super::*;

/// Decode every lit pixel in the canvas.
/// Returns (pixel_x, pixel_y) pairs in row-major scan order.
/// pixel_x = col * 2 + dot_x,  pixel_y = row * 4 + dot_y
fn lit_pixels(canvas: &BrailleCanvas) -> Vec<(usize, usize)> {
    let mut pixels = Vec::new();
    for row in 0..canvas.cell_rows() {
        for col in 0..canvas.cell_cols() {
            let bitmask = canvas.get_cell(row, col);
            if bitmask == 0 {
                continue;
            }
            for (dot_y, bits) in BRAILLE_MAP.iter().enumerate() {
                for (dot_x, &bit) in bits.iter().enumerate() {
                    if bitmask & bit != 0 {
                        pixels.push((col * 2 + dot_x, row * 4 + dot_y));
                    }
                }
            }
        }
    }
    pixels
}

/// Assert the set of pixels is 8-connected: every pixel is reachable from
/// `pixels[0]` by stepping to neighbouring pixels (including diagonals).
/// Panics with a descriptive message if connectivity fails.
fn assert_8_connected(pixels: &[(usize, usize)]) {
    if pixels.len() <= 1 {
        return;
    }
    let set: std::collections::HashSet<(usize, usize)> = pixels.iter().copied().collect();
    let mut visited: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(pixels[0]);
    visited.insert(pixels[0]);
    while let Some((x, y)) = queue.pop_front() {
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 {
                    let nb = (nx as usize, ny as usize);
                    if set.contains(&nb) && visited.insert(nb) {
                        queue.push_back(nb);
                    }
                }
            }
        }
    }
    assert_eq!(
        visited.len(),
        set.len(),
        "Line is not 8-connected: {} pixels total but only {} reachable from {:?}",
        set.len(),
        visited.len(),
        pixels[0]
    );
}

/// Assert the pixel set contains both endpoints (x0, y0) and (x1, y1).
/// Only checks non-negative coordinates (negative endpoints are off-canvas).
fn assert_endpoints_exact(pixels: &[(usize, usize)], x0: i32, y0: i32, x1: i32, y1: i32) {
    let set: std::collections::HashSet<(usize, usize)> = pixels.iter().copied().collect();
    if x0 >= 0 && y0 >= 0 {
        assert!(
            set.contains(&(x0 as usize, y0 as usize)),
            "Start pixel ({x0}, {y0}) not lit; lit count={}",
            set.len()
        );
    }
    if x1 >= 0 && y1 >= 0 {
        assert!(
            set.contains(&(x1 as usize, y1 as usize)),
            "End pixel ({x1}, {y1}) not lit; lit count={}",
            set.len()
        );
    }
}

/// Assert no pixel appears twice in the slice.
/// Note: this verifies the `lit_pixels` decoder does not emit duplicate coords.
/// It cannot detect if `draw_line` visits a pixel coordinate multiple times,
/// since the canvas bitmask is idempotent (OR is a no-op for already-set bits).
fn assert_no_duplicates(pixels: &[(usize, usize)]) {
    let mut seen = std::collections::HashSet::new();
    for &px in pixels {
        assert!(seen.insert(px), "Duplicate pixel at {px:?}");
    }
}

#[test]
fn helpers_compile_smoke() {
    let mut c = BrailleCanvas::new(10, 10);
    c.draw_line(0, 0, 9, 9);
    let px = lit_pixels(&c);
    assert!(!px.is_empty());
    assert_no_duplicates(&px);
    assert_8_connected(&px);
    assert_endpoints_exact(&px, 0, 0, 9, 9);
}

#[test]
fn empty_canvas_renders_blank_braille() {
    let canvas = BrailleCanvas::new(3, 2);
    let chars = canvas.render();
    assert_eq!(chars.len(), 2);
    assert_eq!(chars[0].len(), 3);
    // Empty Braille = U+2800
    assert_eq!(chars[0][0], '\u{2800}');
}

#[test]
fn single_pixel_top_left() {
    let mut canvas = BrailleCanvas::new(1, 1);
    canvas.set_pixel(0, 0); // bit0 = 0x01
    let chars = canvas.render();
    assert_eq!(chars[0][0], '\u{2801}'); // U+2800 + 0x01
}

#[test]
fn single_pixel_bottom_right() {
    let mut canvas = BrailleCanvas::new(1, 1);
    canvas.set_pixel(1, 3); // bit7 = 0x80
    let chars = canvas.render();
    assert_eq!(chars[0][0], '\u{2880}'); // U+2800 + 0x80
}

#[test]
fn all_dots_filled() {
    let mut canvas = BrailleCanvas::new(1, 1);
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
    let canvas = BrailleCanvas::new(10, 5);
    assert_eq!(canvas.pixel_width(), 20);
    assert_eq!(canvas.pixel_height(), 20);
}

#[test]
fn out_of_bounds_clipped() {
    let mut canvas = BrailleCanvas::new(1, 1);
    canvas.set_pixel(5, 5); // way out of bounds — should not panic
    let chars = canvas.render();
    assert_eq!(chars[0][0], '\u{2800}'); // still empty
}

#[test]
fn draw_line_horizontal() {
    let mut canvas = BrailleCanvas::new(3, 1);
    canvas.draw_line(0, 0, 5, 0);
    let chars = canvas.render();
    // First 3 cells should have top-left dots set
    for c in &chars[0] {
        assert_ne!(*c, '\u{2800}'); // something is set
    }
}

#[test]
fn clear_resets_canvas() {
    let mut canvas = BrailleCanvas::new(2, 2);
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
    let mut canvas = BrailleCanvas::new(10, 1);
    // dash_on=2, dash_off=2: draw 2 pixels, skip 2, repeat
    canvas.draw_dashed_line(0, 0, 19, 0, 2, 2);
    let chars = canvas.render();
    let non_empty: usize = chars[0].iter().filter(|&&c| c != '\u{2800}').count();
    // A fully solid line of 20 pixels at y=0 would fill ~10 cells.
    // A dashed line (50% duty cycle) should fill fewer cells.
    assert!(
        non_empty > 0 && non_empty < 10,
        "Dashed line should have gaps, got {non_empty} non-empty cells"
    );
}

#[test]
fn draw_dashed_bezier_has_gaps() {
    let mut canvas_solid = BrailleCanvas::new(10, 5);
    canvas_solid.draw_bezier(0, 0, 19, 19);
    let solid_count: usize = canvas_solid
        .render()
        .iter()
        .flat_map(|r| r.iter())
        .filter(|&&c| c != '\u{2800}')
        .count();

    let mut canvas_dashed = BrailleCanvas::new(10, 5);
    canvas_dashed.draw_dashed_bezier(0, 0, 19, 19, 3, 2);
    let dashed_count: usize = canvas_dashed
        .render()
        .iter()
        .flat_map(|r| r.iter())
        .filter(|&&c| c != '\u{2800}')
        .count();

    assert!(
        dashed_count < solid_count,
        "Dashed bezier ({dashed_count} cells) should have fewer pixels than solid ({solid_count})"
    );
}

#[test]
fn draw_bezier_sets_pixels() {
    let mut canvas = BrailleCanvas::new(10, 5);
    canvas.draw_bezier(0, 0, 19, 19);
    let chars = canvas.render();
    let non_empty: usize = chars
        .iter()
        .flat_map(|r| r.iter())
        .filter(|&&c| c != '\u{2800}')
        .count();
    assert!(non_empty > 0, "Bezier should set some pixels");
}

#[test]
fn draw_circle_sets_pixels_symmetrically() {
    let mut canvas = BrailleCanvas::new(10, 5);
    canvas.draw_circle(10, 10, 8);
    let chars = canvas.render();
    let non_empty: usize = chars
        .iter()
        .flat_map(|r| r.iter())
        .filter(|&&c| c != '\u{2800}')
        .count();
    assert!(
        non_empty >= 4,
        "Circle should set pixels in multiple cells, got {non_empty}"
    );
}

#[test]
fn draw_circle_off_screen_no_panic() {
    let mut canvas = BrailleCanvas::new(5, 5);
    canvas.draw_circle(-10, -10, 5);
    canvas.draw_circle(100, 100, 50);
    // Should not panic
}

#[test]
fn draw_bezier_off_screen_no_panic() {
    let mut canvas = BrailleCanvas::new(5, 5);
    canvas.draw_bezier(-10, -10, 100, 100);
    canvas.draw_bezier(100, 100, -50, -50);
    // Should not panic
}

#[test]
fn clear_pixel_clears_set_dot() {
    let mut canvas = BrailleCanvas::new(1, 1);
    canvas.set_pixel(0, 0);
    assert_ne!(canvas.render()[0][0], '\u{2800}', "pixel should be set");
    canvas.clear_pixel(0, 0);
    assert_eq!(canvas.render()[0][0], '\u{2800}', "pixel should be cleared");
}

#[test]
fn clear_pixel_leaves_other_dots() {
    let mut canvas = BrailleCanvas::new(1, 1);
    canvas.set_pixel(0, 0); // bit0
    canvas.set_pixel(1, 3); // bit7
    canvas.clear_pixel(0, 0);
    // bit0 cleared, bit7 remains → 0x80
    assert_eq!(canvas.render()[0][0], '\u{2880}');
}

#[test]
fn draw_line_with_hops_creates_gap() {
    // Draw a horizontal line with a hop in the middle; compare to solid line.
    let mut canvas_solid = BrailleCanvas::new(10, 1);
    canvas_solid.draw_line(0, 2, 19, 2);
    let solid_count: usize = canvas_solid.render()[0]
        .iter()
        .filter(|&&c| c != '\u{2800}')
        .count();

    let mut canvas_hop = BrailleCanvas::new(10, 1);
    canvas_hop.draw_line_with_hops(0, 2, 19, 2, &[(10, 2, 3)]);
    let hop_count: usize = canvas_hop.render()[0]
        .iter()
        .filter(|&&c| c != '\u{2800}')
        .count();

    assert!(
        hop_count < solid_count,
        "Hop line ({hop_count} cells) should have fewer lit cells than solid ({solid_count})"
    );
}

/// BRAILLE-001: draw_line_with_hops must always draw the endpoint pixel even
/// when the endpoint falls inside a hop disc.  Without the fix the hop guard
/// would suppress the very last pixel, making the line appear to end short.
#[test]
fn hop_endpoint_inside_disc_still_draws_endpoint() {
    // Horizontal line from x=0 to x=10, y=0.
    // Place a hop disc centred exactly on the endpoint (10, 0) with radius 2.
    // All pixels in [8..=10] are inside the disc, so the endpoint pixel at
    // x=10 must still be drawn (endpoints are never suppressed by hops).
    let mut canvas = BrailleCanvas::new(10, 1);
    canvas.draw_line_with_hops(0, 0, 10, 0, &[(10, 0, 2)]);
    let px = lit_pixels(&canvas);
    let set: std::collections::HashSet<(usize, usize)> = px.iter().copied().collect();
    assert!(
        set.contains(&(10, 0)),
        "endpoint (10,0) must be drawn even though it falls inside the hop disc; lit pixels = {:?}",
        px
    );
}

#[test]
fn draw_dashed_bezier_ctrl_has_gaps() {
    // Solid Bézier with explicit control point
    let mut canvas_solid = BrailleCanvas::new(10, 5);
    canvas_solid.draw_bezier_ctrl(0, 0, 19, 19, 0, 19);
    let solid_count: usize = canvas_solid
        .render()
        .iter()
        .flat_map(|r| r.iter())
        .filter(|&&c| c != '\u{2800}')
        .count();

    // Dashed Bézier with the same control point
    let mut canvas_dashed = BrailleCanvas::new(10, 5);
    canvas_dashed.draw_dashed_bezier_ctrl(0, 0, 19, 19, 0, 19, 3, 3);
    let dashed_count: usize = canvas_dashed
        .render()
        .iter()
        .flat_map(|r| r.iter())
        .filter(|&&c| c != '\u{2800}')
        .count();

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
    let mut canvas_line = BrailleCanvas::new(10, 5);
    canvas_line.draw_line(0, 0, 19, 19);
    let line_chars = canvas_line.render();

    let mut canvas_bezier = BrailleCanvas::new(10, 5);
    canvas_bezier.draw_bezier(0, 0, 19, 19);
    let bezier_chars = canvas_bezier.render();

    // At least some cells should differ (the curve bows outward)
    let mut differs = false;
    for (lr, br) in line_chars.iter().zip(bezier_chars.iter()) {
        for (lc, bc) in lr.iter().zip(br.iter()) {
            if lc != bc {
                differs = true;
            }
        }
    }
    assert!(
        differs,
        "Bezier should produce different pixels than a straight line"
    );
}

/// GRAPH-001: draw_dashed_line / draw_dashed_bezier / draw_dashed_bezier_ctrl
/// must not panic when dash_on=0 and dash_off=0 (period=0 → modulo by zero).
#[test]
fn draw_dashed_line_zero_period_no_panic() {
    let mut canvas = BrailleCanvas::new(5, 2);
    // Both zero → period=0 → step % period would panic before the fix.
    canvas.draw_dashed_line(0, 0, 9, 0, 0, 0);
    // Canvas may or may not have pixels set, but must not panic.
}

#[test]
fn draw_dashed_bezier_zero_period_no_panic() {
    let mut canvas = BrailleCanvas::new(5, 2);
    canvas.draw_dashed_bezier(0, 0, 9, 7, 0, 0);
}

#[test]
fn draw_dashed_bezier_ctrl_zero_period_no_panic() {
    let mut canvas = BrailleCanvas::new(5, 2);
    canvas.draw_dashed_bezier_ctrl(0, 0, 9, 7, 5, 0, 0, 0);
}

#[test]
fn line_is_8_connected_horizontal() {
    let mut c = BrailleCanvas::new(12, 4);
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
    let mut c = BrailleCanvas::new(4, 5);
    c.draw_line(0, 0, 0, 15); // 16 px down col 0
    let px = lit_pixels(&c);
    assert_eq!(px.len(), 16, "vertical line should have 16 distinct pixels");
    assert_8_connected(&px);
    assert_endpoints_exact(&px, 0, 0, 0, 15);
    assert_no_duplicates(&px);
}

#[test]
fn line_is_8_connected_diagonal_45() {
    let mut c = BrailleCanvas::new(10, 6);
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
    let mut c = BrailleCanvas::new(4, 5);
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
        let mut c = BrailleCanvas::new(22, 22);
        c.draw_line(cx, cy, x1, y1);
        let px = lit_pixels(&c);
        assert_endpoints_exact(&px, cx, cy, x1, y1);
        assert_8_connected(&px);
    }
}

#[test]
fn line_no_duplicates_diagonal() {
    let mut c = BrailleCanvas::new(10, 6);
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
    let mut c_ab = BrailleCanvas::new(12, 4);
    c_ab.draw_line(ax, ay, bx, by);
    let px_ab = lit_pixels(&c_ab);
    assert_endpoints_exact(&px_ab, ax, ay, bx, by);
    assert_8_connected(&px_ab);

    // Verify segment B→E individually reaches B
    let mut c_be = BrailleCanvas::new(12, 4);
    c_be.draw_line(bx, by, ex, ey);
    let px_be = lit_pixels(&c_be);
    assert_endpoints_exact(&px_be, bx, by, ex, ey);
    assert_8_connected(&px_be);

    // Verify the combined canvas has B lit and the full path is 8-connected
    let mut c = BrailleCanvas::new(12, 4);
    c.draw_line(ax, ay, bx, by);
    c.draw_line(bx, by, ex, ey);
    let px = lit_pixels(&c);
    let b_pixel = (bx as usize, by as usize);
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

    let mut solid = BrailleCanvas::new(12, 4);
    solid.draw_line(x0, y0, x1, y1);

    let mut dashed = BrailleCanvas::new(12, 4);
    dashed.draw_dashed_line(x0, y0, x1, y1, 1, 0);

    let solid_px: std::collections::HashSet<_> = lit_pixels(&solid).into_iter().collect();
    let dashed_px: std::collections::HashSet<_> = lit_pixels(&dashed).into_iter().collect();
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
    let x0: i32 = 0;
    let y0: i32 = 0;
    let x1: i32 = 39;
    let y1: i32 = 0;
    let hop_x: i32 = 20;
    let hop_y: i32 = 0;
    let hop_r: i32 = 3;

    let mut c = BrailleCanvas::new(22, 2);
    c.draw_line_with_hops(x0, y0, x1, y1, &[(hop_x, hop_y, hop_r)]);

    let all_px = lit_pixels(&c);

    // Split into left and right halves of the gap
    let left: Vec<(usize, usize)> = all_px
        .iter()
        .copied()
        .filter(|&(x, _)| (x as i32) < hop_x - hop_r)
        .collect();
    let right: Vec<(usize, usize)> = all_px
        .iter()
        .copied()
        .filter(|&(x, _)| (x as i32) > hop_x + hop_r)
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
