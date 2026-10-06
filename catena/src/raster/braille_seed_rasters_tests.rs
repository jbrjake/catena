//! Characterization of the harvested primitives against the seed's own output.

use super::BrailleCanvas;
use super::tests::raster;

/// The seed's own output for each primitive, captured from `seed/graph/braille.rs` before the
/// port as `render()` rows joined by `|`. Folding the three Bresenham loops into one walker and
/// the four Bézier loops into one sampler must not move a single dot.
#[rustfmt::skip]
const SEED_RASTERS: [(&str, &str); 33] = [
    ("line0", "⠉⠒⠤⢄⡀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠈⠉⠒⠢⢄⣀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠒|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("dashed_line0", "⠉⠂⠠⢄⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠉⠂⠠⢄⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠂|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("hop_line0", "⠉⠒⠤⢄⡀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠈⠉⠒⠢⢄⣀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠐|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("line1", "⠀⠢⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠈⠢⡀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠈⠢⡀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠑⢄⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠑⢄⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠑⢄"),
    ("dashed_line1", "⠀⠢⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠠⡀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠈⠀⡀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠑⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠑⠄⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠐⢄"),
    ("hop_line1", "⠀⠂⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠈⠢⡀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠈⠂⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⢀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠑⢄⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠑⢄"),
    ("line2", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡠|⠀⠀⠀⠀⠀⠀⠀⠀⠀⡠⠊⠀|⠀⠀⠀⠀⠀⠀⠀⡠⠊⠀⠀⠀|⠀⠀⠀⠀⠀⡠⠊⠀⠀⠀⠀⠀|⠀⠀⠀⡠⠊⠀⠀⠀⠀⠀⠀⠀|⠀⡠⠊⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("dashed_line2", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠊⠀|⠀⠀⠀⠀⠀⠀⠀⠠⠊⠀⠀⠀|⠀⠀⠀⠀⠀⡠⠂⠀⠀⠀⠀⠀|⠀⠀⠀⡠⠀⠀⠀⠀⠀⠀⠀⠀|⠀⡀⠈⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("hop_line2", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡠|⠀⠀⠀⠀⠀⠀⠀⠀⠀⡠⠊⠀|⠀⠀⠀⠀⠀⠀⠀⠠⠊⠀⠀⠀|⠀⠀⠀⠀⠀⡀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⡠⠊⠀⠀⠀⠀⠀⠀⠀|⠀⡠⠊⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("line3", "⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("dashed_line3", "⠀⠀⠀⠸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢰⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢠⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢈⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠘⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠸⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("hop_line3", "⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⢈⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("line4", "⠀⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("dashed_line4", "⠀⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("hop_line4", "⠀⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("bezier0", "⠡⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠡⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠐⡀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠡⢀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠂⠄⡀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠈⠐⠠⢀"),
    ("dashed_bezier0", "⠡⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠐⡀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠁⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠂⠄⡀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠠⢀"),
    ("bezier1", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⢀⠠|⠀⠀⠀⠀⠀⠀⠠⠐⠈⠀⠀⠀|⠀⠀⠀⠀⠠⠂⠁⠀⠀⠀⠀⠀|⠀⠀⢀⠊⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠌⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠈⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("dashed_bezier1", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⢀⠠|⠀⠀⠀⠀⠀⠀⠠⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠂⠁⠀⠀⠀⠀⠀|⠀⠀⢀⠂⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠈⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("bezier2", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠐⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("dashed_bezier2", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠐⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("bezier3", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠠⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠈⠈⠐⠠⠠⠠⢀⢀⢀⢀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("dashed_bezier3", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠠⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠐⠠⠠⠀⠀⢀⢀⢀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("bezier_ctrl0", "⡁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠂⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠨⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠡⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠁⢂⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠈⠐⠠⠠⠀⡀⡀⢀"),
    ("dashed_bezier_ctrl0", "⡁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠡⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠈⠐⠀⠀⠀⠀⡀⢀"),
    ("bezier_ctrl1", "⠀⠀⠀⠀⠀⠀⠀⠀⣀⢀⡀⡀|⠀⠀⠀⠀⠀⠠⠐⠁⠀⠀⠀⠀|⠀⠀⠀⠀⠄⠁⠀⠀⠀⠀⠀⠀|⠀⠀⠠⠈⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠂⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("dashed_bezier_ctrl1", "⠀⠀⠀⠀⠀⠀⠀⠀⢀⢀⠀⠀|⠀⠀⠀⠀⠀⠠⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠁⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠂⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("bezier_ctrl2", "⠀⠀⡀⠄⠂⠂⠂⠂⠂⠄⡀⠀|⠂⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠁|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("dashed_bezier_ctrl2", "⠀⠀⡀⠄⠀⠀⠀⠂⠂⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("circle0", "⠀⠀⠀⠀⢀⣀⣀⠀⠀⠀⠀⠀|⠀⠀⡠⠊⠁⠀⠀⠉⠢⡀⠀⠀|⠀⢰⠁⠀⠀⠀⠀⠀⠀⢱⠀⠀|⠀⠘⡄⠀⠀⠀⠀⠀⠀⡜⠀⠀|⠀⠀⠈⠢⢄⣀⣀⠤⠊⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("circle1", "⠀⠀⡸⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠒⠊⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("circle2", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
    ("circle3", "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀|⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"),
];

/// Draws exactly what produced [`SEED_RASTERS`], in the same order.
fn current_rasters() -> Vec<(String, String)> {
    let fresh = || BrailleCanvas::new(12, 6);
    let mut out = Vec::new();
    let lines = [
        (0, 0, 23, 9),
        (23, 23, 2, 1),
        (-5, 30, 30, -5),
        (7, 0, 7, 23),
        (3, 3, 3, 3),
    ];
    for (i, &(x0, y0, x1, y1)) in lines.iter().enumerate() {
        let mut c = fresh();
        c.draw_line(x0, y0, x1, y1);
        out.push((format!("line{i}"), raster(&c)));
        let mut c = fresh();
        c.draw_dashed_line(x0, y0, x1, y1, 3, 2);
        out.push((format!("dashed_line{i}"), raster(&c)));
        let mut c = fresh();
        c.draw_line_with_hops(x0, y0, x1, y1, &[(12, 12, 3), (x1, y1, 2)]);
        out.push((format!("hop_line{i}"), raster(&c)));
    }
    let curves = [
        (0, 0, 23, 23),
        (23, 2, 1, 20),
        (5, 5, 5, 5),
        (-10, 3, 30, 15),
    ];
    for (i, &(x0, y0, x1, y1)) in curves.iter().enumerate() {
        let mut c = fresh();
        c.draw_bezier(x0, y0, x1, y1);
        out.push((format!("bezier{i}"), raster(&c)));
        let mut c = fresh();
        c.draw_dashed_bezier(x0, y0, x1, y1, 3, 2);
        out.push((format!("dashed_bezier{i}"), raster(&c)));
    }
    let controlled = [
        (0, 0, 23, 23, 0, 23),
        (2, 20, 22, 3, 12, 0),
        (-6, 10, 30, 10, 12, -8),
    ];
    for (i, &(x0, y0, x1, y1, cx, cy)) in controlled.iter().enumerate() {
        let mut c = fresh();
        c.draw_bezier_ctrl(x0, y0, x1, y1, cx, cy);
        out.push((format!("bezier_ctrl{i}"), raster(&c)));
        let mut c = fresh();
        c.draw_dashed_bezier_ctrl(x0, y0, x1, y1, cx, cy, 2, 3);
        out.push((format!("dashed_bezier_ctrl{i}"), raster(&c)));
    }
    let circles = [(11, 11, 8), (0, 0, 5), (20, 3, 0), (4, 4, -1)];
    for (i, &(x, y, r)) in circles.iter().enumerate() {
        let mut c = fresh();
        c.draw_circle(x, y, r);
        out.push((format!("circle{i}"), raster(&c)));
    }
    out
}

#[test]
fn primitives_match_the_seed_rasters() {
    let current = current_rasters();
    assert_eq!(current.len(), SEED_RASTERS.len());
    for ((name, got), (seed_name, seed)) in current.iter().zip(SEED_RASTERS) {
        assert_eq!(name, seed_name);
        assert_eq!(got, seed, "{name} moved away from the seed's raster");
    }
}
