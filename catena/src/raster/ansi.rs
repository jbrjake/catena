//! SGR escape sequences for [`CellGrid::to_ansi_string`].

use super::{Attrs, PaletteColor};

#[cfg(doc)]
use super::CellGrid;

/// Resets every attribute and both colors to the terminal's defaults.
pub(super) const RESET: &str = "\x1b[0m";

/// The colors and attributes a run of cells is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Pen {
    pub(super) fg: Option<PaletteColor>,
    pub(super) bg: Option<PaletteColor>,
    pub(super) attrs: Attrs,
}

impl Pen {
    /// The terminal's default colors, no attributes.
    pub(super) const DEFAULT: Pen = Pen {
        fg: None,
        bg: None,
        attrs: Attrs::NONE,
    };

    /// Appends the one SGR sequence that switches any pen to this one: a reset, then this pen's
    /// attributes, foreground and background.
    pub(super) fn write_sgr(self, out: &mut String) {
        if self == Pen::DEFAULT {
            out.push_str(RESET);
            return;
        }
        out.push_str("\x1b[0");
        for (flag, code) in [
            (Attrs::BOLD, 1),
            (Attrs::DIM, 2),
            (Attrs::ITALIC, 3),
            (Attrs::REVERSED, 7),
        ] {
            if self.attrs.contains(flag) {
                push_code(out, code);
            }
        }
        if let Some(fg) = self.fg {
            push_color(out, fg, 30, 90, 38);
        }
        if let Some(bg) = self.bg {
            push_color(out, bg, 40, 100, 48);
        }
        out.push('m');
    }
}

/// Appends `color` as SGR codes: an ANSI color as `normal + n` or `bright + n − 8`, a palette
/// entry as `extended;5;n` and an RGB color as `extended;2;r;g;b`.
fn push_color(out: &mut String, color: PaletteColor, normal: u8, bright: u8, extended: u8) {
    match color {
        PaletteColor::Ansi(n) => {
            let n = n & 0x0f;
            push_code(out, if n < 8 { normal + n } else { bright + n - 8 });
        }
        PaletteColor::Indexed(n) => {
            for code in [extended, 5, n] {
                push_code(out, code);
            }
        }
        PaletteColor::Rgb(r, g, b) => {
            for code in [extended, 2, r, g, b] {
                push_code(out, code);
            }
        }
    }
}

/// Appends `;` and `code` in decimal.
fn push_code(out: &mut String, code: u8) {
    out.push(';');
    if code >= 100 {
        out.push(char::from(b'0' + code / 100));
    }
    if code >= 10 {
        out.push(char::from(b'0' + code / 10 % 10));
    }
    out.push(char::from(b'0' + code % 10));
}
