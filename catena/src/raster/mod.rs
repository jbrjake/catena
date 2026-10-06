//! Rasterization: the cell surface, sub-cell canvases, blitters and drawing primitives (plan §7).

mod ansi;
mod braille;
mod grid;
mod polyline;
mod surface;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "ResolvedMetrics and label rendering (M1, M2) are its callers"
    )
)]
mod text;

pub use braille::BrailleCanvas;
pub use grid::{Cell, CellGrid, MAX_CELLS};
pub use polyline::polyline_pixels;
pub use surface::{Attrs, CellStyle, PaletteColor, Surface};
pub use text::{TextCell, TextCells, text_cells};
