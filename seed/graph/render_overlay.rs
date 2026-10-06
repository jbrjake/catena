//! Overlay node rendering + buffer write helpers extracted from `render.rs`.
//!
//! Contains: `render_overlay_nodes`, `render_overlay_collapsed`,
//! `render_overlay_expanded`, `render_canvas_to_buf`, `render_canvas_to_buf_overwrite`,
//! `render_node`, `write_str`, `word_wrap`.

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::braille::BrailleCanvas;
use super::layout_fr::GridPosition;
use super::render::{dim_color, entity_color_full, type_symbol, RenderEntity, RenderOverlayNode};
use crate::app::ColorPalette;
use crate::ui::theme::Theme;

/// Helper: render a BrailleCanvas into a ratatui Buffer, skipping label cells.
///
/// Reads cell bits directly from the canvas flat buffer instead of allocating
/// an intermediate `Vec<Vec<char>>`. `label_grid` is a flat `w * h` bool
/// array (stride = `grid_width`) marking terminal cells to skip.
pub(super) fn render_canvas_to_buf(
    buf: &mut Buffer,
    area: Rect,
    canvas: &BrailleCanvas,
    label_grid: &[bool],
    grid_width: usize,
    style: Style,
) {
    for cy in 0..canvas.cell_rows() {
        let abs_y = area.y + cy as u16;
        if abs_y >= area.y + area.height {
            break;
        }
        for cx in 0..canvas.cell_cols() {
            let bits = canvas.get_cell(cy, cx);
            if bits == 0 {
                continue;
            }
            if label_grid[cy * grid_width + cx] {
                continue;
            }
            let abs_x = area.x + cx as u16;
            if abs_x >= area.x + area.width {
                break;
            }
            // Braille codepoints U+2800..U+28FF are always valid Unicode.
            let ch = char::from_u32(0x2800 + bits as u32).unwrap_or(' ');
            let cell = &mut buf[(abs_x, abs_y)];
            if cell.symbol() == " " {
                cell.set_char(ch);
                cell.set_style(style);
            }
        }
    }
}

/// Like `render_canvas_to_buf` but overwrites existing cell content.
/// Used for highlighted/glow edges that should visually dominate.
pub(super) fn render_canvas_to_buf_overwrite(
    buf: &mut Buffer,
    area: Rect,
    canvas: &BrailleCanvas,
    label_grid: &[bool],
    grid_width: usize,
    style: Style,
) {
    for cy in 0..canvas.cell_rows() {
        let abs_y = area.y + cy as u16;
        if abs_y >= area.y + area.height {
            break;
        }
        for cx in 0..canvas.cell_cols() {
            let bits = canvas.get_cell(cy, cx);
            if bits == 0 {
                continue;
            }
            if label_grid[cy * grid_width + cx] {
                continue;
            }
            let abs_x = area.x + cx as u16;
            if abs_x >= area.x + area.width {
                break;
            }
            let ch = char::from_u32(0x2800 + bits as u32).unwrap_or(' ');
            let cell = &mut buf[(abs_x, abs_y)];
            cell.set_char(ch);
            cell.set_style(style);
        }
    }
}

/// Render a single node at its grid position.
/// Nodes with negative or out-of-bounds positions are silently skipped —
/// they live off-screen and only contribute edge lines that extend to the viewport edge.
pub(super) fn render_node(
    buf: &mut Buffer,
    area: Rect,
    entity: &RenderEntity,
    pos: &GridPosition,
    zoom_level: u8,
    palette: ColorPalette,
    theme: &Theme,
) {
    // Skip off-screen nodes (negative grid coordinates or beyond viewport).
    if pos.x < 0 || pos.y < 0 {
        return;
    }
    let abs_x = area.x + pos.x as u16;
    let abs_y = area.y + pos.y as u16;

    // Check bounds
    if abs_x >= area.x + area.width || abs_y >= area.y + area.height {
        return;
    }

    let color = entity_color_full(
        &entity.entity_type,
        entity.community_id,
        entity.confidence,
        entity.convergence_score,
        entity.embedding_community_id,
        entity.user_cluster_id,
        entity.degree_centrality,
        entity.pagerank,
        palette,
        theme,
    );

    // Glow halo: dim colored background on cells surrounding selected/pinned nodes.
    // This creates a soft "bloom" effect in the terminal.
    if entity.selected || entity.pinned {
        let glow_color = dim_color(color);
        let glow_style = Style::default().bg(glow_color);
        let vis_w = super::visual_label_width(pos.label_width, zoom_level);
        // Render one-cell border around the label area.
        for dy in -1i16..=pos.label_height as i16 {
            for dx in -1i16..=(vis_w as i16) {
                let gx = abs_x as i16 + dx;
                let gy = abs_y as i16 + dy;
                if gx < area.x as i16
                    || gx >= (area.x + area.width) as i16
                    || gy < area.y as i16
                    || gy >= (area.y + area.height) as i16
                {
                    continue;
                }
                // Skip the label cells themselves (rendered separately).
                if dy >= 0 && dy < pos.label_height as i16 && dx >= 0 && dx < vis_w as i16 {
                    continue;
                }
                let cell = &mut buf[(gx as u16, gy as u16)];
                // Apply glow background to ALL cells in the halo — not just
                // empty ones. Edge lines (Braille chars) passing through the
                // border get the glow background while keeping their foreground
                // color, creating a consistent highlight around the node.
                let merged = cell.style().patch(glow_style);
                cell.set_style(merged);
            }
        }
    }

    let style = if entity.selected {
        Style::default()
            .fg(Color::Black)
            .bg(color)
            .add_modifier(Modifier::BOLD)
    } else if entity.dimmed {
        // §9.7: dimmed (search-filtered) nodes use theme dim_text, not
        // hardcoded DarkGray, so the fade is consistent across themes.
        Style::default().fg(theme.colors.dim_text)
    } else {
        Style::default().fg(color)
    };

    match zoom_level {
        0 => {
            // Single character by type
            let sym = type_symbol(&entity.entity_type, theme);
            if abs_x < area.x + area.width && abs_y < area.y + area.height {
                let cell = &mut buf[(abs_x, abs_y)];
                cell.set_char(sym);
                cell.set_style(style);
            }
        }
        1 => {
            // [3-char] abbreviated label
            let abbrev: String = entity.name.chars().take(3).collect();
            let label = format!("[{abbrev}]");
            write_str(buf, area, abs_x, abs_y, &label, style);
        }
        _ => {
            // Zoom-aware label: progressively longer names at higher zoom
            let vis_w = super::visual_label_width(pos.label_width, zoom_level);
            let max_name_len = (vis_w as usize).saturating_sub(2);
            let name: String = entity.name.chars().take(max_name_len).collect();
            let pin_marker = if entity.pinned { "*" } else { "" };
            let label = format!("{pin_marker}[{name}]");
            write_str(buf, area, abs_x, abs_y, &label, style);
        }
    }
}

/// Write a string into the buffer at the given position, clipping at area bounds.
pub(super) fn write_str(buf: &mut Buffer, area: Rect, x: u16, y: u16, s: &str, style: Style) {
    let max_x = area.x + area.width;
    for (i, ch) in s.chars().enumerate() {
        let cx = x + i as u16;
        if cx >= max_x || y >= area.y + area.height {
            break;
        }
        let cell = &mut buf[(cx, y)];
        cell.set_char(ch);
        cell.set_style(style);
    }
}

/// Render community names at the centroid of each cluster at zoom level 0.
///
/// At this zoom level entities are single characters, so overlaying a short
/// community label at the cluster center helps users orient in the graph
/// without zooming in.
pub(super) fn render_community_labels(
    buf: &mut Buffer,
    area: Rect,
    entities: &[RenderEntity],
    positions: &HashMap<Uuid, GridPosition>,
    community_names: &HashMap<i32, String>,
    theme: &Theme,
) {
    // Accumulate centroid position for each community.
    let mut centers: HashMap<i32, (f64, f64, usize)> = HashMap::new();
    for entity in entities {
        if let (Some(cid), Some(pos)) = (entity.community_id, positions.get(&entity.id)) {
            let entry = centers.entry(cid).or_insert((0.0, 0.0, 0));
            entry.0 += pos.x as f64;
            entry.1 += pos.y as f64;
            entry.2 += 1;
        }
    }

    let style = Style::default()
        .fg(theme.colors.dim_text)
        .add_modifier(Modifier::BOLD);

    for (cid, (sum_x, sum_y, count)) in &centers {
        let name = match community_names.get(cid) {
            Some(n) => n,
            None => continue, // No user-assigned name — skip rather than showing raw IDs.
        };

        // Truncate to 8 chars to avoid overlapping neighboring clusters.
        let label: String = name.chars().take(8).collect();

        let cx = (*sum_x / *count as f64).round() as u16;
        let cy = (*sum_y / *count as f64).round() as u16;

        // Center the label horizontally on the centroid.
        let half_w = label.len() as u16 / 2;
        let x = (area.x + cx).saturating_sub(half_w);
        let y = area.y + cy;

        write_str(buf, area, x, y, &label, style);
    }
}

/// Render overlay nodes (chunks/documents pinned into the graph).
/// Draws overlay-to-entity edges as dashed lines in the theme's info color, then
/// renders collapsed or expanded overlay boxes on top.
pub fn render_overlay_nodes(
    buf: &mut Buffer,
    area: Rect,
    overlays: &[RenderOverlayNode],
    positions: &HashMap<Uuid, GridPosition>,
    zoom_level: u8,
    theme: &Theme,
) {
    if overlays.is_empty() {
        return;
    }

    // Draw dashed edges from overlay nodes to connected entities.
    let w = area.width as usize;
    let h = area.height as usize;
    if w > 0 && h > 0 {
        let mut canvas = BrailleCanvas::new(w, h);
        for ov in overlays {
            let ov_pos = match positions.get(&ov.id) {
                Some(p) => p,
                None => continue,
            };
            let ov_cx = (ov_pos.x as i32 + ov_pos.label_width as i32 / 2) * 2;
            let ov_cy = ov_pos.y as i32 * 4 + 2;

            for eid in &ov.connected_entity_ids {
                if let Some(epos) = positions.get(eid) {
                    let vis_w = super::visual_label_width(epos.label_width, zoom_level);
                    let ecx = (epos.x as i32 + vis_w as i32 / 2) * 2;
                    let ecy = epos.y as i32 * 4 + 2;
                    canvas.draw_dashed_line(ov_cx, ov_cy, ecx, ecy, 3, 2);
                }
            }
        }

        // Render dashed edges in the theme's info color, skipping label cells.
        // Use visual width for entity labels; overlay labels keep full width
        // since they render at their own size regardless of zoom.
        let overlay_ids: HashSet<Uuid> = overlays.iter().map(|o| o.id).collect();
        let mut label_grid = vec![false; w * h];
        for (&id, pos) in positions {
            if pos.x < 0 || pos.y < 0 || pos.y >= h as i16 {
                continue;
            }
            let w_cells = if overlay_ids.contains(&id) {
                pos.label_width
            } else {
                super::visual_label_width(pos.label_width, zoom_level)
            };
            for ly in 0..pos.label_height {
                for lx in 0..w_cells {
                    let cx = pos.x as u16 + lx;
                    let cy = pos.y as u16 + ly;
                    if cx < w as u16 && cy < h as u16 {
                        label_grid[cy as usize * w + cx as usize] = true;
                    }
                }
            }
        }
        render_canvas_to_buf(
            buf,
            area,
            &canvas,
            &label_grid,
            w,
            Style::default().fg(theme.colors.info),
        );
    }

    // Render each overlay node.
    for ov in overlays {
        let pos = match positions.get(&ov.id) {
            Some(p) => p,
            None => continue,
        };
        if ov.expanded {
            render_overlay_expanded(buf, area, ov, pos, theme);
        } else {
            render_overlay_collapsed(buf, area, ov, pos, theme);
        }
    }
}

/// Render a collapsed overlay node as a single-line label: `[| Chunk 3: first... |]`
fn render_overlay_collapsed(
    buf: &mut Buffer,
    area: Rect,
    overlay: &RenderOverlayNode,
    pos: &GridPosition,
    theme: &Theme,
) {
    if pos.x < 0 || pos.y < 0 {
        return;
    }
    let abs_x = area.x + pos.x as u16;
    let abs_y = area.y + pos.y as u16;
    if abs_x >= area.x + area.width || abs_y >= area.y + area.height {
        return;
    }

    let max_w = (pos.label_width as usize).saturating_sub(4);
    let truncated: String = overlay.label.chars().take(max_w).collect();
    let label = format!("[|{truncated}|]");

    let info = theme.colors.info;
    let style = if overlay.focused {
        Style::default()
            .fg(Color::Black)
            .bg(info)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(info)
    };

    write_str(buf, area, abs_x, abs_y, &label, style);
}

/// Render an expanded overlay node as a bordered box with word-wrapped content.
/// Max dimensions: 40w x 12h. Title bar shows source info.
fn render_overlay_expanded(
    buf: &mut Buffer,
    area: Rect,
    overlay: &RenderOverlayNode,
    pos: &GridPosition,
    theme: &Theme,
) {
    if pos.x < 0 || pos.y < 0 {
        return;
    }
    let abs_x = area.x + pos.x as u16;
    let abs_y = area.y + pos.y as u16;
    if abs_x >= area.x + area.width || abs_y >= area.y + area.height {
        return;
    }

    let box_w = (pos.label_width as usize).clamp(10, 40);
    let box_h = (pos.label_height as usize).clamp(3, 12);
    let inner_w = box_w.saturating_sub(2);
    let inner_h = box_h.saturating_sub(2);

    let info = theme.colors.info;
    let border_style = if overlay.focused {
        Style::default().fg(info).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(info)
    };
    let content_style = Style::default().fg(theme.colors.bright_text);
    let title_style = Style::default().fg(info).add_modifier(Modifier::BOLD);

    // Draw top border with title
    let kind_label = if overlay.is_document { "Doc" } else { "Chunk" };
    let title: String = format!(" {kind_label}: {} ", overlay.document_source)
        .chars()
        .take(inner_w)
        .collect();
    let fill: String = "\u{2500}".repeat(inner_w.saturating_sub(title.len()));
    let top = format!("\u{256d}{title}{fill}\u{256e}");
    write_str(buf, area, abs_x, abs_y, &top, border_style);

    // Word-wrap content into lines
    let content_lines = word_wrap(&overlay.content, inner_w);
    let scroll = overlay
        .scroll_offset
        .min(content_lines.len().saturating_sub(inner_h));

    // Draw content rows
    for row in 0..inner_h {
        let row_y = abs_y + 1 + row as u16;
        if row_y >= area.y + area.height {
            break;
        }
        // Left border
        write_str(buf, area, abs_x, row_y, "\u{2502}", border_style);

        // Content
        if let Some(line) = content_lines.get(scroll + row) {
            let padded = format!("{line:<width$}", width = inner_w);
            write_str(buf, area, abs_x + 1, row_y, &padded, content_style);
        } else {
            let empty = " ".repeat(inner_w);
            write_str(buf, area, abs_x + 1, row_y, &empty, content_style);
        }

        // Right border
        let right_x = abs_x + box_w as u16 - 1;
        if right_x < area.x + area.width {
            write_str(buf, area, right_x, row_y, "\u{2502}", border_style);
        }
    }

    // Draw bottom border
    let bottom_y = abs_y + box_h as u16 - 1;
    if bottom_y < area.y + area.height {
        let bottom = format!("\u{2570}{}\u{256f}", "\u{2500}".repeat(inner_w));
        write_str(buf, area, abs_x, bottom_y, &bottom, border_style);
    }

    // Draw title on top border (overwrite)
    write_str(buf, area, abs_x + 1, abs_y, &title, title_style);
}

/// Simple word-wrap: breaks content into lines of at most `width` characters.
fn word_wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            if current.is_empty() {
                current = word.to_string();
            } else if current.len() + 1 + word.len() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current);
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
#[path = "render_overlay_tests.rs"]
mod tests;
