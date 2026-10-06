//! Raster assertions for braille canvases: the seed's line-quality oracle (plan §16.2-E),
//! harvested from `seed/graph/braille_tests.rs`.

use std::collections::{HashSet, VecDeque};

use catena::raster::{Blitter, SubCellCanvas};

/// Unicode's braille dot numbering, as `(bit, dot_x, dot_y)` within a cell's 2×4 grid, in
/// row-major order: dots 1, 2, 3 and 7 fill the left column top to bottom, dots 4, 5, 6 and 8
/// the right, and dot `n` is bit `n - 1` of the offset from U+2800. Decoding from the standard
/// rather than from `catena`'s own table keeps this oracle independent of the code under test.
const DOTS: [(u8, usize, usize); 8] = [
    (0, 0, 0), // dot 1
    (3, 1, 0), // dot 4
    (1, 0, 1), // dot 2
    (4, 1, 1), // dot 5
    (2, 0, 2), // dot 3
    (5, 1, 2), // dot 6
    (6, 0, 3), // dot 7
    (7, 1, 3), // dot 8
];

/// Decode every lit pixel in a braille canvas.
/// Returns `(pixel_x, pixel_y)` pairs in row-major scan order, where
/// `pixel_x = col * 2 + dot_x` and `pixel_y = row * 4 + dot_y`.
///
/// # Panics
///
/// If the canvas is not a braille canvas, whose masks this decodes.
#[must_use]
pub fn lit_pixels(canvas: &SubCellCanvas) -> Vec<(usize, usize)> {
    assert_eq!(
        canvas.blitter(),
        Blitter::Braille,
        "lit_pixels decodes braille dots"
    );
    let mut pixels = Vec::new();
    for row in 0..canvas.cell_rows() {
        for col in 0..canvas.cell_cols() {
            let bitmask = canvas.get_cell(row, col);
            if bitmask == 0 {
                continue;
            }
            for &(bit, dot_x, dot_y) in &DOTS {
                if bitmask & (1 << bit) != 0 {
                    pixels.push((col * 2 + dot_x, row * 4 + dot_y));
                }
            }
        }
    }
    pixels
}

/// Assert the set of pixels is 8-connected: every pixel is reachable from
/// `pixels[0]` by stepping to neighbouring pixels (including diagonals).
///
/// # Panics
///
/// With a descriptive message if connectivity fails.
#[track_caller]
pub fn assert_8_connected(pixels: &[(usize, usize)]) {
    if pixels.len() <= 1 {
        return;
    }
    let set: HashSet<(usize, usize)> = pixels.iter().copied().collect();
    let mut visited: HashSet<(usize, usize)> = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(pixels[0]);
    visited.insert(pixels[0]);
    while let Some((x, y)) = queue.pop_front() {
        for dy in -1isize..=1 {
            for dx in -1isize..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
                    let nb = (nx, ny);
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
///
/// # Panics
///
/// If an on-canvas endpoint is not lit.
#[track_caller]
pub fn assert_endpoints_exact(pixels: &[(usize, usize)], x0: i32, y0: i32, x1: i32, y1: i32) {
    let set: HashSet<(usize, usize)> = pixels.iter().copied().collect();
    if let (Ok(x), Ok(y)) = (usize::try_from(x0), usize::try_from(y0)) {
        assert!(
            set.contains(&(x, y)),
            "Start pixel ({x0}, {y0}) not lit; lit count={}",
            set.len()
        );
    }
    if let (Ok(x), Ok(y)) = (usize::try_from(x1), usize::try_from(y1)) {
        assert!(
            set.contains(&(x, y)),
            "End pixel ({x1}, {y1}) not lit; lit count={}",
            set.len()
        );
    }
}

/// Assert no pixel appears twice in the slice.
/// Note: this verifies the `lit_pixels` decoder does not emit duplicate coords.
/// It cannot detect if `draw_line` visits a pixel coordinate multiple times,
/// since the canvas bitmask is idempotent (OR is a no-op for already-set bits).
///
/// # Panics
///
/// On the first repeated pixel.
#[track_caller]
pub fn assert_no_duplicates(pixels: &[(usize, usize)]) {
    let mut seen = HashSet::new();
    for &px in pixels {
        assert!(seen.insert(px), "Duplicate pixel at {px:?}");
    }
}
