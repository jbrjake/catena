//! Per-theme constructor modules and the `THEMES` cycling array.
//!
//! Each submodule adds a single constructor to `impl Theme`. The `THEMES`
//! array lists them in cycling order (press `T` in the TUI).

mod ansi16;
mod classic_mac;
mod corporate;
mod cyberpunk;
mod hot_dog_stand;
mod matrix;

use super::theme::Theme;

/// Available built-in themes, used for cycling with `T`.
pub const THEMES: [fn() -> Theme; 6] = [
    Theme::cyberpunk,
    Theme::classic_mac,
    Theme::corporate,
    Theme::hot_dog_stand,
    Theme::matrix,
    Theme::ansi16,
];
