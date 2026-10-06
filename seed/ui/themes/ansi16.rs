//! ANSI-16 fallback theme — uses only the 16 standard ANSI colors.

use ratatui::style::Color;
use ratatui::widgets::BorderType;

use super::super::theme::{
    EntityTypeStyle, Theme, ThemeChrome, ThemeColors, ThemeEntityTypes, ThemeGlyphs, ThemeSizing,
};

impl Theme {
    /// ANSI-16 fallback theme — uses only the 16 standard ANSI colors.
    ///
    /// Designed for terminals that don't support 24-bit (truecolor) RGB values.
    /// Uses `Color::Black`, `Color::Red`, `Color::Green`, `Color::Yellow`,
    /// `Color::Blue`, `Color::Magenta`, `Color::Cyan`, `Color::White`,
    /// `Color::DarkGray`, `Color::LightRed`, `Color::LightGreen`,
    /// `Color::LightYellow`, `Color::LightBlue`, `Color::LightMagenta`,
    /// `Color::LightCyan`, and `Color::Gray`. No `Color::Rgb(...)` anywhere.
    /// Uses Unicode glyphs (like Cyberpunk) and Plain borders for simplicity.
    pub fn ansi16() -> Self {
        Self {
            name: "ANSI-16",
            colors: ThemeColors {
                // Bright colors for headers and emphasis
                primary: Color::LightCyan,
                accent: Color::LightMagenta,
                success: Color::LightGreen,
                info: Color::LightBlue,
                warning: Color::LightYellow,
                danger: Color::LightRed,
                purple: Color::Magenta,
                blue: Color::Blue,
                magenta: Color::Magenta,

                // Dark background with standard gray text scale
                bg: Color::Black,
                dim_text: Color::DarkGray,
                bright_text: Color::White,
                hint_key: Color::Gray,
                hint_label: Color::DarkGray,

                window_bg: Color::Black,
                selected_bg: Color::Blue,
                focus_border: Color::LightCyan,
                unfocus_border: Color::DarkGray,
                empty_state: Color::DarkGray,
                status_text: Color::Gray,
                low_confidence: Color::LightRed,
                hidden_window: Color::DarkGray,

                // Graph: Reset lets the terminal decide the background
                graph_bg: Color::Reset,
                edge_layout: Color::DarkGray,
                edge_overlay: Color::Magenta,
                edge_highlight: Color::LightCyan,
                // 8 distinct ANSI colors for community separation
                community_palette: [
                    Color::LightCyan,
                    Color::LightYellow,
                    Color::LightGreen,
                    Color::LightMagenta,
                    Color::LightBlue,
                    Color::LightRed,
                    Color::Cyan,
                    Color::Yellow,
                ],
            },
            chrome: ThemeChrome {
                // Plain borders for maximum terminal compatibility
                border_type: BorderType::Plain,
                overlay_border_type: BorderType::Plain,
            },
            glyphs: ThemeGlyphs {
                lcars_bar_left: "\u{2590}",  // ▐
                lcars_bar_fill: "\u{2593}",  // ▓
                lcars_bar_right: "\u{258C}", // ▌

                progress_fill: "\u{2588}",  // █
                progress_empty: "\u{2591}", // ░

                scroll_up: "\u{25B2}",   // ▲
                scroll_down: "\u{25BC}", // ▼
                enter_key: "\u{23CE}",   // ⏎

                legend_bullet: "\u{25CF}",     // ●
                legend_bullet_dim: "\u{25CC}", // ◌

                window_minimize: "\u{2500}", // ─
                window_maximize: "\u{25A1}", // □

                status_active: "\u{2699}",    // ⚙
                status_done: "\u{2713}",      // ✓
                status_fail: "\u{2717}",      // ✗
                status_cancel: "\u{2715}",    // ✕
                status_skip: "\u{2B0C}",      // ⊌
                status_pending: "\u{00B7}",   // ·
                status_ingesting: "\u{27F3}", // ⟳

                selected_indicator: "\u{25B8}",      // ▸
                section_divider: "\u{2500}\u{2500}", // ──
                overflow_indicator: "\u{22EF}",      // ⋯
                warning_icon: "\u{26A0}",            // ⚠
                error_prefix: "\u{2514}",            // └

                chunk_active: "\u{25B6}",  // ▶
                chunk_done: "\u{2714}",    // ✔
                chunk_pending: "\u{25CB}", // ○

                tooltip_corner_tl: "\u{2554}", // ╔
                tooltip_corner_tr: "\u{2557}", // ╗
                tooltip_divider: "\u{2500}",   // ─
                similarity_marker: "\u{25C6}", // ◆
                centroid_prefix: "\u{2193}",   // ↓

                pipe_branch_continues: "\u{255F}", // ╟
                pipe_branch_terminal: "\u{2559}",  // ╙
                box_double_tl: "\u{2554}",         // ╔
                box_double_tr: "\u{2557}",         // ╗
                box_double_bl: "\u{255A}",         // ╚
                box_double_br: "\u{255D}",         // ╝
                box_double_h: "\u{2550}",          // ═
                box_double_v: "\u{2551}",          // ║
                box_single_tl: "\u{250C}",         // ┌
                box_single_tr: "\u{2510}",         // ┐
                box_single_bl: "\u{2514}",         // └
                box_single_br: "\u{2518}",         // ┘
                box_single_v: "\u{2502}",          // │
                box_single_h: "\u{2500}",          // ─
                bullet: "\u{2022}",                // •

                vertical_divider: "\u{2502}", // │
            },
            sizing: ThemeSizing {
                window_padding: 2,
                touch_button_gap: 1,
                minimize_button_width: 3,
                menu_bar_height: 1,
                status_bar_height: 1,
                lcars_bar_width: 24,
                progress_bar_width: 12,
                chunk_bar_width: 8,
            },
            entity_types: ThemeEntityTypes {
                // Use the 8 bright ANSI colors + DarkGray for unknown
                person: EntityTypeStyle {
                    color: Color::LightCyan,
                    symbol: '\u{25CF}',
                }, // ●
                organization: EntityTypeStyle {
                    color: Color::LightYellow,
                    symbol: '\u{25A0}',
                }, // ■
                location: EntityTypeStyle {
                    color: Color::LightGreen,
                    symbol: '\u{25B2}',
                }, // ▲
                event: EntityTypeStyle {
                    color: Color::LightMagenta,
                    symbol: '\u{25C6}',
                }, // ◆
                document: EntityTypeStyle {
                    color: Color::LightBlue,
                    symbol: '\u{25C7}',
                }, // ◇
                concept: EntityTypeStyle {
                    color: Color::White,
                    symbol: '\u{25CB}',
                }, // ○
                vulnerability: EntityTypeStyle {
                    color: Color::LightRed,
                    symbol: '\u{2022}',
                }, // •
                malware: EntityTypeStyle {
                    color: Color::LightRed,
                    symbol: '\u{2022}',
                }, // •
                threat_actor: EntityTypeStyle {
                    color: Color::Red,
                    symbol: '\u{2022}',
                }, // •
                indicator: EntityTypeStyle {
                    color: Color::Yellow,
                    symbol: '\u{2022}',
                }, // •
                unknown: EntityTypeStyle {
                    color: Color::DarkGray,
                    symbol: '\u{2022}',
                }, // •
            },
        }
    }
}
