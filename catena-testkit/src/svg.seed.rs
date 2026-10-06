// SVG renderer for ratatui TestBackend buffers.
//
// Converts a ratatui `Buffer` snapshot into a self-contained SVG file
// suitable for embedding in GitHub markdown for visual inspection. Also
// computes a stable SHA-256 hash of the buffer content for regression
// comparison without image-diffing.
//
// ## Design decisions
//
// - Each terminal row produces one `<text>` element containing `<tspan>`
//   children grouped by (fg_color, is_bold) so that adjacent cells sharing
//   the same style are emitted as a single text run. Each tspan carries an
//   absolute `x` coordinate so wide or skipped characters never drift.
// - Background rects are emitted per-row *before* the text layer. Only
//   cells whose background differs from Cylvia's default dark background
//   get a rect, keeping the SVG small for typical dark-theme output.
// - REVERSED modifier swaps fg↔bg before any color resolution.
// - Wide-char continuation cells (skip == true) output a single space so
//   they consume the correct column width without repeating the glyph.
// - All user-visible strings are XML-escaped.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use sha2::{Digest, Sha256};

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

// ── Cylvia brand colours ──────────────────────────────────────────────────────

/// Default terminal background — `Color::Rgb(12, 10, 18)`.
const DEFAULT_BG: &str = "#0c0a12";
/// Default terminal foreground — `Color::Rgb(220, 215, 235)`.
const DEFAULT_FG: &str = "#dcd7eb";

// ── Public API ────────────────────────────────────────────────────────────────

/// Convert a ratatui `Buffer` to a self-contained SVG string.
///
/// `title` appears in the decorative title bar and as the SVG `aria-label`.
pub fn buffer_to_svg(buf: &Buffer, title: &str) -> String {
    let area = buf.area;
    let cols = area.width as usize;
    let rows = area.height as usize;

    // Total SVG dimensions.
    let svg_w = PAD_X * 2.0 + cols as f64 * CHAR_W;
    let svg_h = PAD_Y * 2.0 + TITLE_H + rows as f64 * CHAR_H;

    // Y-origin for the top-left of the terminal grid (below the title bar).
    let grid_y = PAD_Y + TITLE_H;
    // X-origin for column 0.
    let grid_x = PAD_X;

    let mut svg = String::with_capacity(cols * rows * 20);

    // ── SVG header ────────────────────────────────────────────────────────────
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w:.1}" height="{h:.1}" role="img" aria-label="{label}">"#,
        w = svg_w,
        h = svg_h,
        label = xml_escape(title),
    ));
    svg.push('\n');

    // ── Dark background ───────────────────────────────────────────────────────
    svg.push_str(&format!(
        r#"  <rect width="{w:.1}" height="{h:.1}" fill="{bg}"/>"#,
        w = svg_w,
        h = svg_h,
        bg = DEFAULT_BG,
    ));
    svg.push('\n');

    // ── Title bar ─────────────────────────────────────────────────────────────
    let title_text = format!("{} | {}×{}", title, cols, rows);
    // Slightly lighter title bar strip.
    svg.push_str(&format!(
        "  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" fill=\"#1a1726\"/>",
        x = PAD_X,
        y = PAD_Y,
        w = cols as f64 * CHAR_W,
        h = TITLE_H,
    ));
    svg.push('\n');
    // Title text, vertically centred in the title bar.
    svg.push_str(&format!(
        "  <text x=\"{x:.1}\" y=\"{y:.1}\" \
font-family=\"Courier New, monospace\" font-size=\"11\" fill=\"#7c7295\">{text}</text>",
        x = grid_x + 4.0,
        y = PAD_Y + TITLE_H * 0.68,
        text = xml_escape(&title_text),
    ));
    svg.push('\n');

    // Dividing line between title and terminal area.
    svg.push_str(&format!(
        "  <line x1=\"{x1:.1}\" y1=\"{y:.1}\" x2=\"{x2:.1}\" y2=\"{y:.1}\" \
stroke=\"#3d3555\" stroke-width=\"1\"/>",
        x1 = PAD_X,
        y = grid_y,
        x2 = PAD_X + cols as f64 * CHAR_W,
    ));
    svg.push('\n');

    // ── Terminal rows: background rects ──────────────────────────────────────
    //
    // Emit one pass for backgrounds, a second pass for text.  Keeping them
    // separate avoids SVG stacking issues where text sits behind a later rect.
    for row in 0..rows {
        let ry = grid_y + row as f64 * CHAR_H;
        // Scan for runs of the same background colour.
        let mut col = 0usize;
        while col < cols {
            let cell = &buf[(col as u16 + area.x, row as u16 + area.y)];
            let (_eff_fg, eff_bg) = effective_colors(cell.fg, cell.bg, cell.modifier);
            let bg_css = color_to_css(eff_bg, false);
            // Skip cells that use the default background — the SVG base rect covers them.
            if bg_css == DEFAULT_BG {
                col += 1;
                continue;
            }
            // Extend this background run as far as the same bg colour holds.
            let run_start = col;
            col += 1;
            while col < cols {
                let next = &buf[(col as u16 + area.x, row as u16 + area.y)];
                let (_, next_bg) = effective_colors(next.fg, next.bg, next.modifier);
                if color_to_css(next_bg, false) != bg_css {
                    break;
                }
                col += 1;
            }
            let run_len = col - run_start;
            svg.push_str(&format!(
                r#"  <rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" fill="{fill}"/>"#,
                x = grid_x + run_start as f64 * CHAR_W,
                y = ry,
                w = run_len as f64 * CHAR_W,
                h = CHAR_H,
                fill = bg_css,
            ));
            svg.push('\n');
        }
    }

    // ── Terminal rows: text ───────────────────────────────────────────────────
    for row in 0..rows {
        // Baseline position: SVG text baseline sits at ~78 % of cell height.
        let baseline_y = grid_y + row as f64 * CHAR_H + CHAR_H * 0.78;

        // Build a list of styled spans for this row, then emit as one <text>.
        let spans = collect_row_spans(buf, &area, row);
        if spans.is_empty() {
            continue;
        }

        svg.push_str(&format!(
            r#"  <text xml:space="preserve" font-family="Courier New, monospace" font-size="{fs}" y="{y:.1}">"#,
            fs = FONT_SIZE,
            y = baseline_y,
        ));

        for span in &spans {
            let x = grid_x + span.start_col as f64 * CHAR_W;
            let mut attrs = format!(r#" x="{x:.1}" fill="{fg}""#, fg = span.fg_css);
            if span.bold {
                attrs.push_str(r#" font-weight="bold""#);
            }
            svg.push_str(&format!("<tspan{attrs}>{text}</tspan>", text = span.text));
        }

        svg.push_str("</text>\n");
    }

    svg.push_str("</svg>\n");
    svg
}

/// Compute a stable 64-character hex SHA-256 hash of the buffer content.
///
/// The hash is computed over a canonical text encoding of the buffer in
/// row-major order. Each cell contributes one line:
/// `"{symbol}|{fg_css}|{bg_css}|{modifier_hex}\n"`
///
/// Using CSS colour strings rather than raw `Color` discriminants ensures the
/// hash is stable even if ratatui's internal representation changes, and that
/// visually-identical colours from different `Color` variants hash identically.
pub fn buffer_to_hash(buf: &Buffer) -> String {
    let area = buf.area;
    let mut hasher = Sha256::new();

    for row in 0..area.height {
        for col in 0..area.width {
            let cell = &buf[(col + area.x, row + area.y)];
            let (eff_fg, eff_bg) = effective_colors(cell.fg, cell.bg, cell.modifier);
            let sym = if cell.skip { " " } else { cell.symbol() };
            let fg_css = color_to_css(eff_fg, true);
            let bg_css = color_to_css(eff_bg, false);
            let mods = cell.modifier.bits();
            let entry = format!("{}|{}|{}|{:x}\n", sym, fg_css, bg_css, mods);
            hasher.update(entry.as_bytes());
        }
    }

    format!("{:x}", hasher.finalize())
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

/// Collect all styled text spans for a single row of the buffer.
///
/// Adjacent cells sharing the same (fg, bold) pair are merged into one span.
/// Cells with only whitespace symbols at the default foreground colour are
/// skipped if they carry no styling — the SVG background already covers them.
fn collect_row_spans(buf: &Buffer, area: &Rect, row: usize) -> Vec<StyledSpan> {
    let cols = area.width as usize;
    let mut spans: Vec<StyledSpan> = Vec::new();

    for col in 0..cols {
        let cell = &buf[(col as u16 + area.x, row as u16 + area.y)];
        // Wide-char continuations: render a space so the column width is preserved.
        let sym = if cell.skip { " " } else { cell.symbol() };
        // Skip fully empty, unstyled space cells — they are invisible.
        if sym == " " && cell.modifier.is_empty() {
            let (_, raw_bg) = effective_colors(cell.fg, cell.bg, cell.modifier);
            if color_to_css(raw_bg, false) == DEFAULT_BG {
                // Try to extend the previous span's start_col by emitting nothing —
                // but absolute x on each tspan handles gaps automatically.
                // Just push nothing and let the next non-space tspan set its own x.
                continue;
            }
        }

        let (eff_fg, _) = effective_colors(cell.fg, cell.bg, cell.modifier);
        let fg_css = color_to_css(eff_fg, true);
        let bold = cell.modifier.contains(Modifier::BOLD);

        // Extend the current span if style matches, otherwise start a new one.
        if let Some(last) = spans.last_mut() {
            if last.fg_css == fg_css && last.bold == bold {
                last.text.push_str(&xml_escape(sym));
                continue;
            }
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

/// Apply `REVERSED` modifier: swap fg and bg when the bit is set.
///
/// Returns `(effective_fg, effective_bg)` after any swap.
fn effective_colors(fg: Color, bg: Color, modifier: Modifier) -> (Color, Color) {
    if modifier.contains(Modifier::REVERSED) {
        (bg, fg)
    } else {
        (fg, bg)
    }
}

/// Map a ratatui `Color` to its CSS hex string.
///
/// `is_fg` controls which default colour is used when the colour is `Reset`.
pub fn color_to_css(color: Color, is_fg: bool) -> String {
    match color {
        Color::Reset => {
            if is_fg {
                DEFAULT_FG.to_string()
            } else {
                DEFAULT_BG.to_string()
            }
        }
        Color::Black => "#282c34".to_string(),
        Color::Red => "#e06c75".to_string(),
        Color::Green => "#98c379".to_string(),
        Color::Yellow => "#e5c07b".to_string(),
        Color::Blue => "#61afef".to_string(),
        Color::Magenta => "#c678dd".to_string(),
        Color::Cyan => "#56b6c2".to_string(),
        Color::Gray => "#abb2bf".to_string(),
        Color::DarkGray => "#5c6370".to_string(),
        Color::LightRed => "#ff7b89".to_string(),
        Color::LightGreen => "#b5e8a0".to_string(),
        Color::LightYellow => "#ffd787".to_string(),
        Color::LightBlue => "#88ccff".to_string(),
        Color::LightMagenta => "#e0a5f5".to_string(),
        Color::LightCyan => "#88dfe8".to_string(),
        Color::White => "#ffffff".to_string(),
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        Color::Indexed(n) => indexed_to_css(n),
    }
}

/// Convert a 256-colour palette index to a CSS hex string.
///
/// Follows the standard xterm-256 colour cube and greyscale ramp:
/// - 0–7:    standard colours (same palette as the named `Color` variants)
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
            format!("#{:02x}{:02x}{:02x}", v, v, v)
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

/// Convert a ratatui `Buffer` to a PNG image as a byte vector.
///
/// Internally calls `buffer_to_svg()` to produce an SVG string, then
/// rasterises it with `resvg`. The returned bytes are a valid standalone PNG.
///
/// Font rendering uses system fonts loaded via `usvg::fontdb`. If no
/// system Courier New is available, a monospace fallback is used — this does
/// not affect character-grid layout or the accuracy of visual assessment.
pub fn buffer_to_png(buf: &Buffer, title: &str) -> Vec<u8> {
    use resvg::{tiny_skia, usvg};

    let svg_str = buffer_to_svg(buf, title);

    // Load system fonts so text renders correctly.
    // In resvg 0.44 the fontdb is embedded inside Options; populate it via
    // the `fontdb_mut()` accessor rather than constructing a separate Database.
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();

    let tree = usvg::Tree::from_str(&svg_str, &opt)
        .expect("buffer_to_png: SVG parse failed (buffer_to_svg should always produce valid SVG)");

    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .expect("buffer_to_png: pixmap allocation failed — SVG dimensions are zero?");

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .expect("buffer_to_png: PNG encode failed")
}

#[cfg(test)]
mod png_tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn buffer_to_png_produces_valid_png_header() {
        // 4×2 buffer with one non-default cell.
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        buf[(0, 0)].set_symbol("A");

        let png = buffer_to_png(&buf, "test");

        // PNG files always start with the 8-byte PNG signature.
        assert_eq!(
            &png[..8],
            b"\x89PNG\r\n\x1a\n",
            "expected PNG signature at start of output"
        );
        assert!(
            png.len() > 100,
            "PNG output suspiciously small: {} bytes",
            png.len()
        );
    }
}
