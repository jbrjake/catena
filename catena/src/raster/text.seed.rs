//! Centralized two-pass box layout engine for responsive TUI rendering.
//!
//! Provides `render_box()` for single bordered boxes with word-wrap,
//! `render_row()` for side-by-side box layout, and connector helpers
//! for routing diagrams.  Uses `flex_layout.rs` for space distribution.
//!
//! See `docs/plans/2026-02-24-box-layout-engine-design.md` for architecture.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Border style for a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BorderStyle {
    #[allow(dead_code)] // Part of public API; available for callers needing borderless boxes
    None,
    Single,
    #[allow(dead_code)] // Part of public API; available for callers needing double borders
    Double,
}

/// Content alignment inside a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentAlign {
    #[allow(dead_code)] // Used in Phase 2 puppeteer swap
    Left,
    Center,
}

/// A single line of box content.
#[derive(Debug, Clone)]
pub(crate) struct BoxContent {
    pub text: String,
    pub color: Color,
    pub modifier: Modifier,
}

/// Specification for a single bordered box.
#[derive(Debug, Clone)]
pub(crate) struct BoxSpec {
    pub lines: Vec<BoxContent>,
    pub border: BorderStyle,
    pub align: ContentAlign,
    pub border_color: Color,
    pub min_width: Option<usize>,
}

/// Box-drawing glyphs (extracted from theme for testability).
#[derive(Debug, Clone)]
pub(crate) struct BoxGlyphs {
    pub single_tl: &'static str,
    pub single_tr: &'static str,
    pub single_bl: &'static str,
    pub single_br: &'static str,
    pub single_h: &'static str,
    pub single_v: &'static str,
    pub double_tl: &'static str,
    pub double_tr: &'static str,
    pub double_bl: &'static str,
    pub double_br: &'static str,
    pub double_h: &'static str,
    pub double_v: &'static str,
}

impl BoxGlyphs {
    /// Convert from a `ThemeGlyphs` reference.
    pub fn from_theme(g: &crate::ui::theme::ThemeGlyphs) -> Self {
        Self {
            single_tl: g.box_single_tl,
            single_tr: g.box_single_tr,
            single_bl: g.box_single_bl,
            single_br: g.box_single_br,
            single_h: g.box_single_h,
            single_v: g.box_single_v,
            double_tl: g.box_double_tl,
            double_tr: g.box_double_tr,
            double_bl: g.box_double_bl,
            double_br: g.box_double_br,
            double_h: g.box_double_h,
            double_v: g.box_double_v,
        }
    }
}

/// Word-wrap `text` into lines of at most `width` characters.
///
/// Breaks on whitespace boundaries.  If a single word exceeds `width`,
/// falls back to character-boundary splitting.  Returns an empty vec
/// for empty input or zero width.
pub(crate) fn word_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        let word_len = word.chars().count();

        if word_len > width {
            // Flush current line if non-empty.
            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
            }
            // Character-split the long word.
            let chars: Vec<char> = word.chars().collect();
            for chunk in chars.chunks(width) {
                lines.push(chunk.iter().collect());
            }
            continue;
        }

        let current_len = current_line.chars().count();
        let needed = if current_line.is_empty() {
            word_len
        } else {
            current_len + 1 + word_len
        };

        if needed > width {
            // Current word doesn't fit — start a new line.
            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
            }
            current_line.push_str(word);
        } else {
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

/// Render a single box into `Vec<Line>` at a given width.
///
/// Renders: top border, word-wrapped content lines, bottom border.
/// Width is the outer width (including borders).
pub(crate) fn render_box(spec: &BoxSpec, width: usize, glyphs: &BoxGlyphs) -> Vec<Line<'static>> {
    let min_w = spec.min_width.unwrap_or(4);
    let actual_width = width.max(min_w);
    let inner_w = actual_width.saturating_sub(2);

    let (tl, tr, bl, br, h, v) = match spec.border {
        BorderStyle::None => return render_box_no_border(spec, inner_w),
        BorderStyle::Single => (
            glyphs.single_tl,
            glyphs.single_tr,
            glyphs.single_bl,
            glyphs.single_br,
            glyphs.single_h,
            glyphs.single_v,
        ),
        BorderStyle::Double => (
            glyphs.double_tl,
            glyphs.double_tr,
            glyphs.double_bl,
            glyphs.double_br,
            glyphs.double_h,
            glyphs.double_v,
        ),
    };

    let mut lines = Vec::new();

    // Top border.
    let top = format!("{tl}{}{tr}", h.repeat(inner_w));
    lines.push(Line::from(Span::styled(
        top,
        Style::default().fg(spec.border_color),
    )));

    // Content lines — word-wrap each BoxContent line, then pad/align.
    for content in &spec.lines {
        let wrapped = word_wrap(&content.text, inner_w);
        let wrapped = if wrapped.is_empty() {
            vec![String::new()]
        } else {
            wrapped
        };
        for text in &wrapped {
            let padded = match spec.align {
                ContentAlign::Center => pad_center(text, inner_w),
                ContentAlign::Left => pad_right(text, inner_w),
            };
            let mut spans = vec![
                Span::styled(v.to_string(), Style::default().fg(spec.border_color)),
                Span::styled(
                    padded,
                    Style::default()
                        .fg(content.color)
                        .add_modifier(content.modifier),
                ),
            ];
            spans.push(Span::styled(
                v.to_string(),
                Style::default().fg(spec.border_color),
            ));
            lines.push(Line::from(spans));
        }
    }

    // Bottom border.
    let bot = format!("{bl}{}{br}", h.repeat(inner_w));
    lines.push(Line::from(Span::styled(
        bot,
        Style::default().fg(spec.border_color),
    )));

    lines
}

fn render_box_no_border(spec: &BoxSpec, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for content in &spec.lines {
        let wrapped = word_wrap(&content.text, width);
        for text in &wrapped {
            let padded = match spec.align {
                ContentAlign::Center => pad_center(text, width),
                ContentAlign::Left => pad_right(text, width),
            };
            lines.push(Line::from(Span::styled(
                padded,
                Style::default()
                    .fg(content.color)
                    .add_modifier(content.modifier),
            )));
        }
    }
    lines
}

fn pad_center(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.chars().take(width).collect();
    }
    let left = (width - len) / 2;
    let right = width - len - left;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(right))
}

fn pad_right(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.chars().take(width).collect();
    }
    format!("{}{}", s, " ".repeat(width - len))
}

/// Position of a rendered box within a row.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoxPosition {
    #[allow(dead_code)] // Part of public API; used by callers constructing positions manually
    pub col: usize,
    #[allow(dead_code)] // Part of public API; used by callers constructing positions manually
    pub width: usize,
    pub center: usize,
}

/// Render boxes side-by-side in a row using space-evenly distribution.
///
/// Returns `(lines, positions)` where positions give each box's column offset,
/// width, and center for connector alignment.
#[allow(dead_code)] // Available for callers; puppeteer uses render_row_at for explicit positioning
pub(crate) fn render_row(
    specs: &[BoxSpec],
    available_width: usize,
    glyphs: &BoxGlyphs,
) -> (Vec<Line<'static>>, Vec<BoxPosition>) {
    if specs.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let n = specs.len();

    // Distribute width evenly with gaps (space-evenly).
    let total_gap_slots = n + 1;
    let per_box_width = if n > 0 {
        let usable = available_width.saturating_sub(total_gap_slots);
        (usable / n).max(4)
    } else {
        available_width
    };

    // Use flex_center_with_gaps for positioning.
    let box_widths: Vec<u16> = (0..n).map(|_| per_box_width as u16).collect();
    let offsets = super::flex_layout::flex_center_with_gaps(available_width as u16, &box_widths);

    // Render each box at its allocated width.
    let rendered_boxes: Vec<Vec<Line<'static>>> = specs
        .iter()
        .map(|spec| render_box(spec, per_box_width, glyphs))
        .collect();

    // Find tallest box.
    let max_height = rendered_boxes.iter().map(|b| b.len()).max().unwrap_or(0);

    // Build positions.
    let positions: Vec<BoxPosition> = offsets
        .iter()
        .map(|&off| BoxPosition {
            col: off as usize,
            width: per_box_width,
            center: off as usize + per_box_width / 2,
        })
        .collect();

    // Merge boxes into rows: for each row index, build a Line by placing
    // each box's content at its column offset.
    let mut lines: Vec<Line<'static>> = Vec::new();
    for row in 0..max_height {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut col = 0usize;

        for (i, rendered) in rendered_boxes.iter().enumerate() {
            let box_col = offsets[i] as usize;

            // Gap before this box.
            if box_col > col {
                spans.push(Span::raw(" ".repeat(box_col - col)));
                col = box_col;
            }

            if row < rendered.len() {
                // Copy spans from this box's line.
                for span in &rendered[row].spans {
                    let span_len = span.content.chars().count();
                    spans.push(span.clone());
                    col += span_len;
                }
            } else {
                // Box is shorter — pad with spaces.
                spans.push(Span::raw(" ".repeat(per_box_width)));
                col += per_box_width;
            }
        }

        lines.push(Line::from(spans));
    }

    (lines, positions)
}

/// Render vertical pipe characters at each box's center column.
#[allow(dead_code)] // Used by box_layout tests; may be needed by future renderers
pub(crate) fn render_vertical_pipes(
    width: usize,
    positions: &[BoxPosition],
    colors: &[Color],
    glyphs: &BoxGlyphs,
) -> Line<'static> {
    let mut items: Vec<(usize, String, Color)> = positions
        .iter()
        .zip(colors.iter())
        .map(|(pos, &color)| (pos.center, glyphs.single_v.to_string(), color))
        .collect();
    items.sort_by_key(|&(col, _, _)| col);
    build_row_truncating(width, &items)
}

/// Render horizontal routing segments with labels, dynamically truncated.
///
/// Each segment runs from some start column to the box center, with format:
/// `── label ──┐`
#[allow(dead_code)] // Available for callers; puppeteer now uses centered labels instead
pub(crate) fn render_labeled_routing(
    width: usize,
    positions: &[BoxPosition],
    labels: &[String],
    colors: &[Color],
    glyphs: &BoxGlyphs,
) -> Line<'static> {
    let mut all_items: Vec<(usize, String, Color)> = Vec::new();

    for (i, pos) in positions.iter().enumerate() {
        let label = labels.get(i).map(|s| s.as_str()).unwrap_or("???");
        let color = colors.get(i).copied().unwrap_or(Color::Gray);
        let ec = pos.center;

        // Available space for the segment: from seg_start to ec (inclusive).
        // Format: "── label ──┐" needs at least 8 chars (2 dash + space + 1 char + space + 2 dash + turn).
        let min_segment = 8;
        let max_label_space = ec.saturating_sub(min_segment);

        // Truncate label to fit.
        let truncated = if label.chars().count() <= max_label_space {
            label.to_string()
        } else if max_label_space > 3 {
            let t: String = label.chars().take(max_label_space - 3).collect();
            format!("{t}...")
        } else {
            // Not enough space even for "x..." — just use what fits.
            label.chars().take(max_label_space.max(1)).collect()
        };

        let segment = format!(
            "{h}{h} {label} {h}{h}{turn}",
            h = glyphs.single_h,
            label = truncated,
            turn = glyphs.single_tr,
        );
        let segment_len = segment.chars().count();
        let seg_start = ec.saturating_sub(segment_len.saturating_sub(1));

        all_items.push((seg_start, segment, color));
    }

    all_items.sort_by_key(|&(col, _, _)| col);
    build_row_truncating(width, &all_items)
}

/// Like puppeteer's build_row but truncates overflowing items instead of skipping.
pub(crate) fn build_row_truncating(
    width: usize,
    items: &[(usize, String, Color)],
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut col = 0usize;

    for (item_col, text, color) in items {
        let item_col = *item_col;
        let text_len = text.chars().count();

        if item_col >= width {
            continue; // starts past the edge — skip
        }

        // Gap before this item.
        if item_col > col {
            spans.push(Span::raw(" ".repeat(item_col - col)));
            col = item_col;
        }
        if col > item_col {
            continue; // overlap — skip
        }

        // Truncate if it would overflow.
        let available = width - col;
        let display_text = if text_len > available {
            text.chars().take(available).collect::<String>()
        } else {
            text.clone()
        };
        let display_len = display_text.chars().count();

        spans.push(Span::styled(display_text, Style::default().fg(*color)));
        col += display_len;
    }

    Line::from(spans)
}

/// Render boxes side-by-side at caller-specified column offsets and width.
///
/// Unlike `render_row` which computes its own positions via flex,
/// this function places each box at the given offset with the given width.
/// Used when the caller has already computed positions (e.g., to match
/// routing lines in the puppeteer diagram).
#[allow(dead_code)] // Used by box_layout tests; may be needed by future renderers
pub(crate) fn render_row_at(
    specs: &[BoxSpec],
    offsets: &[usize],
    box_width: usize,
    _total_width: usize,
    glyphs: &BoxGlyphs,
) -> (Vec<Line<'static>>, Vec<BoxPosition>) {
    if specs.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // Render each box at the allocated width.
    let rendered_boxes: Vec<Vec<Line<'static>>> = specs
        .iter()
        .map(|spec| render_box(spec, box_width, glyphs))
        .collect();

    // Find tallest box.
    let max_height = rendered_boxes.iter().map(|b| b.len()).max().unwrap_or(0);

    // Build positions from caller-specified offsets.
    let positions: Vec<BoxPosition> = offsets
        .iter()
        .map(|&off| BoxPosition {
            col: off,
            width: box_width,
            center: off + box_width / 2,
        })
        .collect();

    // Merge boxes into rows at the given offsets.
    let mut lines: Vec<Line<'static>> = Vec::new();
    for row in 0..max_height {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut col = 0usize;

        for (i, rendered) in rendered_boxes.iter().enumerate() {
            let box_col = if i < offsets.len() { offsets[i] } else { col };

            // Gap before this box.
            if box_col > col {
                spans.push(Span::raw(" ".repeat(box_col - col)));
                col = box_col;
            }

            if row < rendered.len() {
                for span in &rendered[row].spans {
                    let span_len = span.content.chars().count();
                    spans.push(span.clone());
                    col += span_len;
                }
            } else {
                // Box is shorter — pad with spaces.
                spans.push(Span::raw(" ".repeat(box_width)));
                col += box_width;
            }
        }

        lines.push(Line::from(spans));
    }

    (lines, positions)
}

#[cfg(test)]
#[path = "box_layout_tests.rs"]
mod tests;
