//! Rasterization: sub-cell canvases, blitters and drawing primitives (plan §7.2, §7.3).

mod braille;
mod polyline;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "ResolvedMetrics and label rendering (M1, M2) are its callers"
    )
)]
mod text;

pub use braille::BrailleCanvas;
pub use polyline::polyline_pixels;
