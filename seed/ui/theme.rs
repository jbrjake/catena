//! Color palette and theming for the Cylvia TUI.
//!
//! The `Theme` struct controls every visual aspect: colors, border styles,
//! Unicode glyphs, and sizing constants. Six built-in themes ship with Cylvia:
//! Cyberpunk (default), Classic Mac, Corporate, Hot Dog Stand, Matrix,
//! and ANSI-16 (16-color terminal fallback).
//!
//! All render files read colors from `app.theme.colors.*` (semantic slots like
//! `primary`, `accent`, `success`) rather than hardcoded RGB values. Users cycle
//! themes with `T` in the TUI; the order follows the `THEMES` array.
//!
//! # Adding a New Theme
//!
//! ## Theme struct overview
//!
//! A `Theme` has six parts:
//!
//! | Sub-struct | Fields | Purpose |
//! |---|---|---|
//! | `name` | 1 `&'static str` | Human-readable label shown in the status bar |
//! | `ThemeColors` | 29 `Color` + `community_palette: [Color; 8]` | Semantic color slots (primary, accent, bg, etc.) |
//! | `ThemeChrome` | 2 `BorderType` | Border styles for panes and overlays |
//! | `ThemeGlyphs` | ~45 `&'static str` | Unicode or ASCII decorative characters |
//! | `ThemeSizing` | 6 numeric fields | Layout constants (padding, bar widths) |
//! | `ThemeEntityTypes` | 11 `EntityTypeStyle` | Per-entity-type color + symbol |
//!
//! ## Step-by-step
//!
//! 1. **Copy an existing constructor** (e.g. `Theme::cyberpunk()`) as your
//!    starting point. Cyberpunk and Corporate use full Unicode glyphs; Classic
//!    Mac is ASCII-only.
//!
//! 2. **Add a constructor** `pub fn my_theme() -> Self` on `impl Theme`.
//!    Fill in every field — the compiler enforces exhaustive initialization.
//!
//! 3. **Set a unique `name`** — this is displayed in the TUI status bar and
//!    checked for uniqueness in tests.
//!
//! 4. **Register it** in the `THEMES` array in `themes/mod.rs`. Update
//!    the array length (`[fn() -> Theme; N]`) and append your constructor.
//!
//! 5. **Add tests** at minimum:
//!    - The existing `all_themes_construct` test covers construction + name
//!      uniqueness automatically once you update `THEMES`. Update its expected
//!      names vector.
//!    - Add a `*_design_intent` test verifying your key color choices (bg,
//!      primary, bright_text, border types) so future refactors don't
//!      accidentally break the aesthetic.
//!    - If your theme is ASCII-only, add an ASCII glyph assertion test
//!      (see `classic_mac_ascii_only_glyphs`).
//!
//! 6. **Run `cargo test -p nexus-tui`** to confirm no regressions.
//!
//! ## Design tips
//!
//! - `community_palette` needs 8 visually distinct colors that work on your
//!   `bg`. These color graph communities / clusters.
//! - `entity_types` should use your theme's color palette so entities feel
//!   cohesive. Use `dim_text` for the `unknown` type.
//! - `ThemeChrome::border_type` affects all panes; `overlay_border_type`
//!   affects modals (help, confirm, filter). They can differ.
//! - Keep sizing constants identical unless your theme needs tighter/wider
//!   spacing for aesthetic reasons.

use ratatui::style::Color;
use ratatui::widgets::BorderType;

// Re-export presets so `crate::ui::theme::THEMES` and constructors still resolve.
pub use super::theme_presets::*;

// ── Theme system ────────────────────────────────────────────────────────────

/// Visual theme controlling colors, chrome, glyphs, sizing, and entity styling.
///
/// All fields use `&'static str` for glyphs (zero allocations) and `Color` for
/// palette entries. Themes are constructed as compile-time-friendly values via
/// constructor functions.
pub struct Theme {
    /// Human-readable theme name shown in the status bar.
    pub name: &'static str,
    pub colors: ThemeColors,
    pub chrome: ThemeChrome,
    pub glyphs: ThemeGlyphs,
    pub sizing: ThemeSizing,
    pub entity_types: ThemeEntityTypes,
}

// ── ThemeColors ─────────────────────────────────────────────────────────────

/// All colors in the palette, organized by semantic role.
///
/// Render code references `theme.colors.primary` instead of `NEON_YELLOW`,
/// so each theme can redefine what "primary" means in its context.
pub struct ThemeColors {
    // -- Brand / accent --
    /// Primary brand color (headers, mode labels, highlights).
    pub primary: Color,
    /// Secondary brand accent (edit mode, confirm, destructive prompts).
    pub accent: Color,
    /// Positive / success indicator (query results, high confidence).
    pub success: Color,
    /// Informational highlight (selected items, focused borders).
    pub info: Color,
    /// Warning / relation evidence color.
    pub warning: Color,
    /// Danger / destructive action color.
    pub danger: Color,
    /// Special mode indicator (annotations, path explain, category detail).
    pub purple: Color,
    /// Structural element color (borders, filter mode).
    pub blue: Color,
    /// Curved edges, provenance mode, low convergence.
    pub magenta: Color,

    // -- Text & background --
    /// Main window / terminal background.
    pub bg: Color,
    /// Dim / deemphasized text.
    pub dim_text: Color,
    /// Primary readable text.
    pub bright_text: Color,
    /// Keybinding hint bracket color.
    pub hint_key: Color,
    /// Keybinding hint label color (dimmer than hint_key).
    pub hint_label: Color,

    // -- Semantic UI --
    /// Background for overlay/popup windows.
    pub window_bg: Color,
    /// Background for selected list items.
    pub selected_bg: Color,
    /// Border color for focused panes.
    pub focus_border: Color,
    /// Border color for unfocused panes.
    pub unfocus_border: Color,
    /// Empty state placeholder text.
    pub empty_state: Color,
    /// Status message text.
    pub status_text: Color,
    /// Low confidence / error indicator (distinct from `danger`).
    pub low_confidence: Color,
    /// Hidden / invisible window button text.
    pub hidden_window: Color,

    // -- Graph --
    /// Background fill for the graph canvas area.
    pub graph_bg: Color,
    /// Layout (straight) edge color.
    pub edge_layout: Color,
    /// Overlay (curved) edge color.
    pub edge_overlay: Color,
    /// Highlighted / selected edge color.
    pub edge_highlight: Color,
    /// Community color palette for graph node coloring.
    pub community_palette: [Color; 8],
}

// ── ThemeChrome ─────────────────────────────────────────────────────────────

/// Border styles and window chrome.
pub struct ThemeChrome {
    /// Border type for floating windows and panes.
    pub border_type: BorderType,
    /// Border type for modal overlays (help, confirm, filter, etc.).
    pub overlay_border_type: BorderType,
}

// ── ThemeGlyphs ─────────────────────────────────────────────────────────────

/// Unicode (or ASCII) decorative characters used throughout the UI.
pub struct ThemeGlyphs {
    // -- LCARS bar (inspector pane) --
    /// Left edge of LCARS bar (e.g. "▐").
    pub lcars_bar_left: &'static str,
    /// Fill character for LCARS bar (e.g. "▓").
    pub lcars_bar_fill: &'static str,
    /// Right edge of LCARS bar (e.g. "▌").
    pub lcars_bar_right: &'static str,

    // -- Progress bars --
    /// Filled block in progress bars (e.g. "█").
    pub progress_fill: &'static str,
    /// Empty block in progress bars (e.g. "░").
    pub progress_empty: &'static str,

    // -- Scroll / navigation --
    /// Scroll up indicator (e.g. "▲").
    pub scroll_up: &'static str,
    /// Scroll down indicator (e.g. "▼").
    pub scroll_down: &'static str,
    /// Enter / return key symbol (e.g. "⏎").
    pub enter_key: &'static str,

    // -- Legend --
    /// Bullet for colored legend entries (e.g. "●").
    pub legend_bullet: &'static str,
    /// Dim bullet for "Others" / unclustered (e.g. "◌").
    pub legend_bullet_dim: &'static str,

    // -- Window chrome --
    /// Minimized window icon (e.g. "─").
    pub window_minimize: &'static str,
    /// Maximize window icon (e.g. "□").
    pub window_maximize: &'static str,

    // -- Status icons --
    /// Active work / gear icon (e.g. "⚙").
    pub status_active: &'static str,
    /// Done / completed icon (e.g. "✓").
    pub status_done: &'static str,
    /// Failed icon (e.g. "✗").
    pub status_fail: &'static str,
    /// Cancelled icon (e.g. "✕").
    pub status_cancel: &'static str,
    /// Skipped icon (e.g. "⊌").
    pub status_skip: &'static str,
    /// Pending icon (e.g. "·").
    pub status_pending: &'static str,
    /// Ingesting / spinner icon (e.g. "⟳").
    pub status_ingesting: &'static str,

    // -- List / selection --
    /// Selected item indicator (e.g. "▸").
    pub selected_indicator: &'static str,
    /// Section heading prefix (e.g. "──").
    pub section_divider: &'static str,
    /// Overflow / "more items" indicator (e.g. "⋯").
    pub overflow_indicator: &'static str,
    /// Warning icon (e.g. "⚠").
    pub warning_icon: &'static str,
    /// Error message line prefix (e.g. "└").
    pub error_prefix: &'static str,

    // -- Document / ingest --
    /// Active chunk marker (e.g. "▶").
    pub chunk_active: &'static str,
    /// Processed chunk marker (e.g. "✔").
    pub chunk_done: &'static str,
    /// Future / unprocessed chunk (e.g. "○").
    pub chunk_pending: &'static str,

    // -- Embeddings / vector pane --
    /// Tooltip header corner (e.g. "╔").
    pub tooltip_corner_tl: &'static str,
    /// Tooltip header end (e.g. "╗").
    pub tooltip_corner_tr: &'static str,
    /// Tooltip header divider (e.g. "─").
    pub tooltip_divider: &'static str,
    /// Similarity entry marker (e.g. "◆").
    pub similarity_marker: &'static str,
    /// Centroid fallback prefix (e.g. "↓").
    pub centroid_prefix: &'static str,

    // -- Inspector pipe diagram --
    /// Branch continues (e.g. "╟").
    pub pipe_branch_continues: &'static str,
    /// Terminal branch (e.g. "╙").
    pub pipe_branch_terminal: &'static str,
    /// Double-line box top-left (e.g. "╔").
    pub box_double_tl: &'static str,
    /// Double-line box top-right (e.g. "╗").
    pub box_double_tr: &'static str,
    /// Double-line box bottom-left (e.g. "╚").
    pub box_double_bl: &'static str,
    /// Double-line box bottom-right (e.g. "╝").
    pub box_double_br: &'static str,
    /// Double-line horizontal (e.g. "═").
    pub box_double_h: &'static str,
    /// Double-line vertical (e.g. "║").
    pub box_double_v: &'static str,
    /// Single-line box top-left (e.g. "┌").
    pub box_single_tl: &'static str,
    /// Single-line box top-right (e.g. "┐").
    pub box_single_tr: &'static str,
    /// Single-line box bottom-left (e.g. "└").
    pub box_single_bl: &'static str,
    /// Single-line box bottom-right (e.g. "┘").
    pub box_single_br: &'static str,
    /// Light vertical line (e.g. "│").
    pub box_single_v: &'static str,
    /// Light horizontal line (e.g. "─").
    pub box_single_h: &'static str,
    /// Bullet point for lists (e.g. "•").
    pub bullet: &'static str,

    // -- Dividers --
    /// Vertical divider between sections (e.g. "│").
    pub vertical_divider: &'static str,
}

// ── ThemeSizing ─────────────────────────────────────────────────────────────

/// Layout sizing constants that vary by theme.
pub struct ThemeSizing {
    /// Padding from screen edges for floating windows.
    pub window_padding: u16,
    /// Gap between touch bar buttons.
    pub touch_button_gap: u16,
    /// Width of the `[-]` minimize button.
    pub minimize_button_width: u16,
    /// Height of the menu bar row (1 for most themes, 2 for mac).
    pub menu_bar_height: u16,
    /// Height of the status bar row.
    pub status_bar_height: u16,
    /// Base width of LCARS metric bars in the inspector.
    pub lcars_bar_width: usize,
    /// Number of blocks in active work progress bars.
    pub progress_bar_width: usize,
    /// Number of blocks in per-file chunk progress bars.
    pub chunk_bar_width: usize,
}

// ── ThemeEntityTypes ────────────────────────────────────────────────────────

/// Per-entity-type visual styling (color + symbol).
pub struct EntityTypeStyle {
    pub color: Color,
    pub symbol: char,
}

/// Entity type visual definitions used by the graph renderer and legend.
pub struct ThemeEntityTypes {
    pub person: EntityTypeStyle,
    pub organization: EntityTypeStyle,
    pub location: EntityTypeStyle,
    pub event: EntityTypeStyle,
    pub document: EntityTypeStyle,
    pub concept: EntityTypeStyle,
    pub vulnerability: EntityTypeStyle,
    pub malware: EntityTypeStyle,
    pub threat_actor: EntityTypeStyle,
    pub indicator: EntityTypeStyle,
    pub unknown: EntityTypeStyle,
}

impl ThemeEntityTypes {
    /// Look up color by entity type string (matches `graph::render::type_color`).
    pub fn type_color(&self, entity_type: &str) -> Color {
        match entity_type {
            "person" => self.person.color,
            "organization" | "org" => self.organization.color,
            "location" => self.location.color,
            "event" => self.event.color,
            "document" => self.document.color,
            "concept" => self.concept.color,
            "vulnerability" => self.vulnerability.color,
            "malware" => self.malware.color,
            "threat_actor" => self.threat_actor.color,
            "indicator" => self.indicator.color,
            _ => self.unknown.color,
        }
    }

    /// Look up symbol by entity type string (matches `graph::render::type_symbol`).
    pub fn type_symbol(&self, entity_type: &str) -> char {
        match entity_type {
            "person" => self.person.symbol,
            "organization" | "org" => self.organization.symbol,
            "location" => self.location.symbol,
            "event" => self.event.symbol,
            "document" => self.document.symbol,
            "concept" => self.concept.symbol,
            _ => self.unknown.symbol,
        }
    }
}
