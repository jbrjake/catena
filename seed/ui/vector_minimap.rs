//! Graph minimap rendering for the embeddings pane.
//!
//! Contains `render_graph_minimap()` which maps graph canonical positions to
//! braille dots and draws a viewport rectangle showing the current zoom/pan state.

use std::collections::HashMap;

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;

use crate::app::App;
use crate::graph::braille::BrailleCanvas;
use crate::graph::render::entity_color_full;

use super::vector_pane::{draw_viewport_rect, MINIMAP_PAD};

/// Render the graph as a braille-dot minimap with viewport rectangle.
/// Used in the floating Embeddings window when layout is swapped.
///
/// Maps graph canonical positions to braille dots (colored by active palette)
/// and draws a viewport rectangle showing the current zoom/pan state.
/// `main_area` is the theoretical fullscreen graph area for viewport calculation.
pub fn render_graph_minimap(frame: &mut Frame, app: &App, area: Rect, main_area: Rect) {
    if area.width < 2 || area.height < 2 || app.canonical_positions.is_empty() {
        return;
    }

    let cw = area.width as usize;
    let ch = area.height as usize;
    let mut canvas = BrailleCanvas::new(cw, ch);
    let mut cell_colors: HashMap<(usize, usize), (Color, u8)> = HashMap::new();

    // Compute bounding box of canonical positions.
    let mut min_x = i16::MAX;
    let mut max_x = i16::MIN;
    let mut min_y = i16::MAX;
    let mut max_y = i16::MIN;
    for pos in app.canonical_positions.values() {
        min_x = min_x.min(pos.x);
        max_x = max_x.max(pos.x);
        min_y = min_y.min(pos.y);
        max_y = max_y.max(pos.y);
    }
    let range_x = (max_x - min_x).max(1) as f32;
    let range_y = (max_y - min_y).max(1) as f32;

    // Map positions to canvas with padding (consistent with MINIMAP_PAD).
    let pad_x = ((cw as f32) * MINIMAP_PAD).round() as usize;
    let pad_y = ((ch as f32) * MINIMAP_PAD).round() as usize;
    let usable_w = cw.saturating_sub(pad_x * 2).max(1);
    let usable_h = ch.saturating_sub(pad_y * 2).max(1);

    // Build entity info for coloring.
    let focus_entity = app.effective_focus_entity();

    for entity in &app.entities {
        if let Some(pos) = app.canonical_positions.get(&entity.id) {
            let nx = ((pos.x - min_x) as f32 / range_x * usable_w as f32).round() as usize;
            let ny = ((pos.y - min_y) as f32 / range_y * usable_h as f32).round() as usize;
            let cell_x = (pad_x + nx).min(cw.saturating_sub(1));
            let cell_y = (pad_y + ny).min(ch.saturating_sub(1));

            let color = entity_color_full(
                &entity.entity_type,
                entity.community_id,
                entity.confidence,
                entity.convergence_score,
                entity.embedding_community_id,
                entity.user_cluster_id,
                entity.degree_centrality,
                entity.pagerank,
                app.color_palette,
                &app.theme,
            );

            let is_selected = focus_entity == Some(entity.id);
            if is_selected {
                for dy in 0..4 {
                    for dx in 0..2 {
                        canvas.set_pixel(cell_x * 2 + dx, cell_y * 4 + dy);
                    }
                }
                cell_colors.insert((cell_x, cell_y), (color, 2));
            } else {
                let px = cell_x * 2;
                let py = cell_y * 4 + 2;
                canvas.set_pixel(px, py);
                cell_colors.entry((cell_x, cell_y)).or_insert((color, 0));
            }
        }
    }

    // Draw viewport rect showing the current graph zoom/pan on this minimap.
    draw_viewport_rect(&mut canvas, app, cw, ch, main_area);

    // Render canvas to frame buffer.
    let chars = canvas.render();
    let viewport_style = Style::default().fg(app.theme.colors.bright_text);
    let empty_style = Style::default();

    for (cy, row) in chars.iter().enumerate() {
        for (cx, &ch_char) in row.iter().enumerate() {
            let abs_x = area.x + cx as u16;
            let abs_y = area.y + cy as u16;
            if abs_x >= area.x + area.width || abs_y >= area.y + area.height {
                continue;
            }
            let cell = &mut frame.buffer_mut()[(abs_x, abs_y)];
            if ch_char == '\u{2800}' {
                cell.set_char(' ');
                cell.set_style(empty_style);
            } else {
                let style = if let Some(&(color, highlight)) = cell_colors.get(&(cx, cy)) {
                    match highlight {
                        2 => Style::default().fg(color).add_modifier(Modifier::BOLD),
                        _ => Style::default().fg(color),
                    }
                } else {
                    viewport_style
                };
                cell.set_char(ch_char);
                cell.set_style(style);
            }
        }
    }
}
