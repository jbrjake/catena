//! Plain-text ASCII renderer for Layout (test/debug output).
//!
//! `render_to_string()` converts a Layout to a plain ASCII string with no colors.
//! Used for testing and visual verification of the layout engine.

#![allow(dead_code)]

use super::models::*;

// ── Plain-text renderer (test harness) ───────────────────────────────────────

/// Render a Layout to a plain ASCII string. No colors, no ratatui.
/// Used for testing and visual verification.
pub(crate) fn render_to_string(layout: &Layout) -> String {
    let w = layout.total_cols;
    let h = layout.total_rows;
    let mut grid: Vec<Vec<char>> = vec![vec![' '; w]; h];

    // Draw boxes.
    for bx in &layout.boxes {
        let (tl, tr, bl, br, hc, vc) = match bx.border {
            BoxBorder::Single => ('┌', '┐', '└', '┘', '─', '│'),
            BoxBorder::Double => ('╔', '╗', '╚', '╝', '═', '║'),
        };
        let inner = bx.width.saturating_sub(2);

        if bx.row < h {
            set_char(&mut grid, bx.row, bx.col, tl);
            for c in 1..=inner {
                set_char(&mut grid, bx.row, bx.col + c, hc);
            }
            set_char(&mut grid, bx.row, bx.col + inner + 1, tr);
        }

        for (i, line) in bx.content.iter().enumerate() {
            let r = bx.row + 1 + i;
            if r >= h {
                break;
            }
            set_char(&mut grid, r, bx.col, vc);
            for (j, ch) in line.chars().enumerate() {
                if j < inner {
                    set_char(&mut grid, r, bx.col + 1 + j, ch);
                }
            }
            set_char(&mut grid, r, bx.col + inner + 1, vc);
        }

        let bot_row = bx.row + bx.height.saturating_sub(1);
        if bot_row < h {
            set_char(&mut grid, bot_row, bx.col, bl);
            for c in 1..=inner {
                set_char(&mut grid, bot_row, bx.col + c, hc);
            }
            set_char(&mut grid, bot_row, bx.col + inner + 1, br);
        }
    }

    // Draw edges.
    for seg in &layout.edges {
        match seg {
            EdgeSegment::HBar {
                row,
                col_start,
                col_end,
                ..
            } => {
                for c in *col_start..=*col_end {
                    set_char_if_space(&mut grid, *row, c, '─');
                }
            }
            EdgeSegment::VPipe {
                col,
                row_start,
                row_end,
                ..
            } => {
                for r in *row_start..=*row_end {
                    set_char_if_space(&mut grid, r, *col, '│');
                }
            }
            EdgeSegment::Junction { col, row, kind, .. } => {
                set_char(&mut grid, *row, *col, kind.to_char());
            }
        }
    }

    grid.iter()
        .map(|row| {
            let s: String = row.iter().collect();
            s.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn set_char(grid: &mut [Vec<char>], row: usize, col: usize, ch: char) {
    if row < grid.len() && col < grid[0].len() {
        grid[row][col] = ch;
    }
}

fn set_char_if_space(grid: &mut [Vec<char>], row: usize, col: usize, ch: char) {
    if row < grid.len() && col < grid[0].len() && grid[row][col] == ' ' {
        grid[row][col] = ch;
    }
}
