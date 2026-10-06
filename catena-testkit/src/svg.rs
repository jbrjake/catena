//! SVG rendering and SHA-256 hashing of a [`CellGrid`]: the T3 visual-snapshot tier, the color
//! oracle that text goldens cannot be (plan §16.4).
//!
//! Converts a grid into a self-contained SVG file suitable for embedding in GitHub markdown for
//! visual inspection, and computes a stable SHA-256 hash of its content for regression
//! comparison without image-diffing.
//!
//! ## Design decisions
//!
//! - Each terminal row produces one `<text>` element containing `<tspan>` children grouped by
//!   (fg colour, bold) so that adjacent cells sharing the same style are emitted as a single
//!   text run. Each tspan carries an absolute `x` coordinate so wide or skipped characters never
//!   drift.
//! - Background rects are emitted per-row *before* the text layer. Only cells whose background
//!   differs from the default dark background get a rect, keeping the SVG small for typical
//!   dark-theme output.
//! - REVERSED swaps fg↔bg. The picture resolves the terminal's default colors first and then
//!   swaps, as a terminal does; the hash swaps first and then resolves by position, which is the
//!   seed's canonicalization, kept exactly (plan §18).
//! - A wide glyph ends its text run and its continuation cell draws nothing, so the next cell
//!   starts a run at its own `x` however wide the font draws the glyph; a skipped blank ends the
//!   run too, so the gap it leaves shows.
//! - All user-visible strings are XML-escaped.
//!
//! ## Snapshots
//!
//! A snapshot is two committed files per name, `{name}.svg` (for eyes) and `{name}.hash` (the
//! gate), in [`VisualSnapshots::dir`]. A frame whose hash differs, or that has no `.hash` yet,
//! fails and writes `{name}.fail.svg` beside them; `CATENA_UPDATE_SNAPSHOTS=1` accepts the
//! current frames instead.

use std::fmt::Write as _;

use catena::raster::{Attrs, Cell, CellGrid, PaletteColor, Surface};
use sha2::{Digest, Sha256};

mod snapshot;

pub use snapshot::{Mismatch, Mode, VisualSnapshots, assert_visual_snapshot, save_visual_gallery};

// ── Font / layout constants ───────────────────────────────────────────────────

/// Pixels per terminal column (Courier New 13 px).
const CHAR_W: f64 = 7.8;
/// Pixels per terminal row, including line leading.
const CHAR_H: f64 = 16.0;
/// Font size used in the SVG `font-size` attribute.
const FONT_SIZE: f64 = 13.0;
/// Height of the decorative title bar above the terminal area.
const TITLE_H: f64 = 22.0;
/// Horizontal padding between SVG edge and terminal content.
const PAD_X: f64 = 4.0;
/// Vertical padding between SVG edge and title bar top.
const PAD_Y: f64 = 4.0;

// ── Default terminal colours ──────────────────────────────────────────────────

/// The terminal's default background, where a cell has none: `Rgb(12, 10, 18)`.
pub const DEFAULT_BG: &str = "#0c0a12";
/// The terminal's default foreground, where a cell has none: `Rgb(220, 215, 235)`.
pub const DEFAULT_FG: &str = "#dcd7eb";

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Convert a [`CellGrid`] to a self-contained SVG string.
///
/// `title` appears in the decorative title bar and as the SVG `aria-label`.
#[must_use]
pub fn grid_to_svg(grid: &CellGrid, title: &str) -> String {
    let (cols, rows) = grid.size();
    let (cols_f, rows_f) = (f64::from(cols), f64::from(rows));

    // Total SVG dimensions.
    let svg_w = PAD_X * 2.0 + cols_f * CHAR_W;
    let svg_h = PAD_Y * 2.0 + TITLE_H + rows_f * CHAR_H;

    // Y-origin for the top-left of the terminal grid (below the title bar).
    let grid_y = PAD_Y + TITLE_H;
    // X-origin for column 0.
    let grid_x = PAD_X;

    let mut svg = String::new();

    // ── SVG header ────────────────────────────────────────────────────────────
    let label = xml_escape(title);
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{svg_w:.1}" height="{svg_h:.1}" role="img" aria-label="{label}">"#,
    );

    // ── Dark background ───────────────────────────────────────────────────────
    let _ = writeln!(
        svg,
        r#"  <rect width="{svg_w:.1}" height="{svg_h:.1}" fill="{DEFAULT_BG}"/>"#,
    );

    // ── Title bar ─────────────────────────────────────────────────────────────
    let title_text = xml_escape(&format!("{title} | {cols}×{rows}"));
    // Slightly lighter title bar strip.
    let _ = writeln!(
        svg,
        "  <rect x=\"{PAD_X:.1}\" y=\"{PAD_Y:.1}\" width=\"{w:.1}\" height=\"{TITLE_H:.1}\" fill=\"#1a1726\"/>",
        w = cols_f * CHAR_W,
    );
    // Title text, vertically centred in the title bar.
    let _ = writeln!(
        svg,
        "  <text x=\"{x:.1}\" y=\"{y:.1}\" \
font-family=\"Courier New, monospace\" font-size=\"11\" fill=\"#7c7295\">{title_text}</text>",
        x = grid_x + 4.0,
        y = PAD_Y + TITLE_H * 0.68,
    );

    // Dividing line between title and terminal area.
    let _ = writeln!(
        svg,
        "  <line x1=\"{PAD_X:.1}\" y1=\"{grid_y:.1}\" x2=\"{x2:.1}\" y2=\"{grid_y:.1}\" \
stroke=\"#3d3555\" stroke-width=\"1\"/>",
        x2 = PAD_X + cols_f * CHAR_W,
    );

    // ── Terminal rows: background rects ──────────────────────────────────────
    //
    // Emit one pass for backgrounds, a second pass for text.  Keeping them
    // separate avoids SVG stacking issues where text sits behind a later rect.
    for (row, cells) in grid.rows().enumerate() {
        let ry = grid_y + units(row) * CHAR_H;
        // Scan for runs of the same background colour.
        let mut col = 0usize;
        while col < cells.len() {
            let (_, bg_css) = picture_colors(&cells[col]);
            // Skip cells that use the default background — the SVG base rect covers them.
            if bg_css == DEFAULT_BG {
                col += 1;
                continue;
            }
            // Extend this background run as far as the same bg colour holds.
            let run_start = col;
            col += 1;
            while col < cells.len() && picture_colors(&cells[col]).1 == bg_css {
                col += 1;
            }
            let _ = writeln!(
                svg,
                r#"  <rect x="{x:.1}" y="{ry:.1}" width="{w:.1}" height="{CHAR_H:.1}" fill="{bg_css}"/>"#,
                x = grid_x + units(run_start) * CHAR_W,
                w = units(col - run_start) * CHAR_W,
            );
        }
    }

    // ── Terminal rows: text ───────────────────────────────────────────────────
    for (row, cells) in grid.rows().enumerate() {
        // Baseline position: SVG text baseline sits at ~78 % of cell height.
        let baseline_y = grid_y + units(row) * CHAR_H + CHAR_H * 0.78;

        // Build a list of styled spans for this row, then emit as one <text>.
        let spans = collect_row_spans(cells);
        if spans.is_empty() {
            continue;
        }

        let _ = write!(
            svg,
            r#"  <text xml:space="preserve" font-family="Courier New, monospace" font-size="{FONT_SIZE}" y="{baseline_y:.1}">"#,
        );

        for span in &spans {
            let x = grid_x + units(span.start_col) * CHAR_W;
            let bold = if span.bold {
                r#" font-weight="bold""#
            } else {
                ""
            };
            let _ = write!(
                svg,
                r#"<tspan x="{x:.1}" fill="{fg}"{bold}>{text}</tspan>"#,
                fg = span.fg_css,
                text = span.text,
            );
        }

        svg.push_str("</text>\n");
    }

    svg.push_str("</svg>\n");
    svg
}

/// The canonical text the hash is computed over: one line per cell in row-major order,
/// `"{symbol}|{fg_css}|{bg_css}|{attrs_hex}\n"`, with REVERSED colors swapped before they are
/// resolved and a continuation cell's symbol written as a space.
///
/// Using CSS colour strings rather than raw `PaletteColor` variants ensures the hash is stable
/// even if the representation changes, and that visually-identical colours from different
/// variants hash identically.
#[must_use]
pub fn hash_text(grid: &CellGrid) -> String {
    let mut text = String::new();
    for cell in grid.rows().flatten() {
        let (fg, bg) = if cell.attrs().contains(Attrs::REVERSED) {
            (cell.bg(), cell.fg())
        } else {
            (cell.fg(), cell.bg())
        };
        let symbol = if cell.is_continuation() {
            " "
        } else {
            cell.symbol()
        };
        let _ = writeln!(
            text,
            "{symbol}|{}|{}|{:x}",
            color_to_css(fg, true),
            color_to_css(bg, false),
            cell.attrs().bits(),
        );
    }
    text
}

/// A stable 64-character hex SHA-256 hash of the grid's content: of its [`hash_text`].
#[must_use]
pub fn grid_to_hash(grid: &CellGrid) -> String {
    sha256_hex(hash_text(grid).as_bytes())
}

/// The SHA-256 digest of `bytes` as 64 lowercase hex characters.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// A contiguous run of cells on one row sharing the same fg colour and bold flag.
struct StyledSpan {
    /// Column index of the leftmost cell in this span.
    start_col: usize,
    fg_css: String,
    bold: bool,
    text: String,
}

/// Collect all styled text spans for a single row of the grid.
///
/// Adjacent cells sharing the same (fg, bold) pair are merged into one span.
/// Cells with only whitespace symbols at the default foreground colour are
/// skipped if they carry no styling — the SVG background already covers them.
/// A skipped cell or a wide glyph ends its span, so the next cell starts one at
/// its own absolute `x` instead of wherever the font's advance would put it.
fn collect_row_spans(cells: &[Cell]) -> Vec<StyledSpan> {
    let mut spans: Vec<StyledSpan> = Vec::new();
    // Whether the last span ends in the previous column, so this cell can extend it.
    let mut open = false;

    for (col, cell) in cells.iter().enumerate() {
        // Wide-glyph continuations are drawn by their glyph.
        if cell.is_continuation() {
            continue;
        }
        let sym = cell.symbol();
        let (fg_css, bg_css) = picture_colors(cell);
        // Skip fully empty, unstyled space cells — they are invisible.
        if sym == " " && cell.attrs().is_empty() && bg_css == DEFAULT_BG {
            open = false;
            continue;
        }

        let bold = cell.attrs().contains(Attrs::BOLD);
        let extends = open
            && spans
                .last()
                .is_some_and(|last| last.fg_css == fg_css && last.bold == bold);
        open = cell.width() == 1;

        // Extend the current span if style matches, otherwise start a new one.
        if extends {
            if let Some(last) = spans.last_mut() {
                last.text.push_str(&xml_escape(sym));
            }
            continue;
        }

        spans.push(StyledSpan {
            start_col: col,
            fg_css,
            bold,
            text: xml_escape(sym),
        });
    }

    spans
}

/// The CSS colors a terminal shows for `cell`: its foreground and background, the terminal's
/// defaults where it has none, swapped if it is REVERSED.
fn picture_colors(cell: &Cell) -> (String, String) {
    let fg = color_to_css(cell.fg(), true);
    let bg = color_to_css(cell.bg(), false);
    if cell.attrs().contains(Attrs::REVERSED) {
        (bg, fg)
    } else {
        (fg, bg)
    }
}

/// A count of columns or rows as an SVG coordinate factor. Grids are at most `u16::MAX` cells
/// on a side, so the count is exact.
fn units(count: usize) -> f64 {
    f64::from(u32::try_from(count).unwrap_or(u32::MAX))
}

/// Map a colour to its CSS hex string.
///
/// `None` is the terminal's default; `is_fg` says which default, foreground or background. An
/// ANSI colour maps like the palette entry of its low four bits.
#[must_use]
pub fn color_to_css(color: Option<PaletteColor>, is_fg: bool) -> String {
    match color {
        None => {
            if is_fg {
                DEFAULT_FG.to_string()
            } else {
                DEFAULT_BG.to_string()
            }
        }
        Some(PaletteColor::Ansi(n)) => indexed_to_css(n & 0x0f),
        Some(PaletteColor::Indexed(n)) => indexed_to_css(n),
        Some(PaletteColor::Rgb(r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),
    }
}

/// Convert a 256-colour palette index to a CSS hex string.
///
/// Follows the standard xterm-256 colour cube and greyscale ramp:
/// - 0–7:    standard colours
/// - 8–15:   high-intensity variants
/// - 16–231: 6×6×6 RGB cube
/// - 232–255: 24-step greyscale ramp
fn indexed_to_css(n: u8) -> String {
    match n {
        0 => "#282c34".to_string(),  // Black
        1 => "#e06c75".to_string(),  // Red
        2 => "#98c379".to_string(),  // Green
        3 => "#e5c07b".to_string(),  // Yellow
        4 => "#61afef".to_string(),  // Blue
        5 => "#c678dd".to_string(),  // Magenta
        6 => "#56b6c2".to_string(),  // Cyan
        7 => "#abb2bf".to_string(),  // Gray
        8 => "#5c6370".to_string(),  // DarkGray
        9 => "#ff7b89".to_string(),  // LightRed
        10 => "#b5e8a0".to_string(), // LightGreen
        11 => "#ffd787".to_string(), // LightYellow
        12 => "#88ccff".to_string(), // LightBlue
        13 => "#e0a5f5".to_string(), // LightMagenta
        14 => "#88dfe8".to_string(), // LightCyan
        15 => "#ffffff".to_string(), // White
        16..=231 => {
            // 6×6×6 RGB colour cube.
            // Each channel level i maps to: 0 if i==0, else 55 + i*40.
            let idx = n - 16;
            let r_idx = idx / 36;
            let g_idx = (idx % 36) / 6;
            let b_idx = idx % 6;
            let cube_level = |i: u8| -> u8 {
                if i == 0 {
                    0
                } else {
                    55u8.saturating_add(i.saturating_mul(40))
                }
            };
            format!(
                "#{:02x}{:02x}{:02x}",
                cube_level(r_idx),
                cube_level(g_idx),
                cube_level(b_idx)
            )
        }
        232..=255 => {
            // 24-step greyscale ramp: value = (n − 232) * 10 + 8.
            let v = (n - 232) * 10 + 8;
            format!("#{v:02x}{v:02x}{v:02x}")
        }
    }
}

/// Escape the five XML special characters.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}
