//! Characterization of the harvested primitives against the seed's own output.

use std::collections::HashSet;

use super::tests::raster;
use super::{BRAILLE_BASE, BRAILLE_MAP, BrailleCanvas};

/// The seed's own output for each primitive, captured from `seed/graph/braille.rs` before the
/// port as `render()` rows joined by `|`. Folding the three Bresenham loops into one walker moved
/// no dot. Owner ruling A1 then moved curves and dashes on purpose, which [`expectation`] sorts
/// out.
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

/// What owner ruling A1 leaves of the seed's output for one primitive.
#[derive(Debug, PartialEq)]
enum Expect {
    /// Untouched by A1: solid and hop-gapped lines, circles, and axis-aligned dashes, where arc
    /// length and step count agree. Dot for dot.
    Seed,
    /// A solid curve: A1 joins the seed's samples into an 8-connected polyline, so every dot the
    /// seed plotted is still lit.
    SeedDotsKept,
    /// Dashed by arc length now, so the seed's step- and sample-counted dashes are no reference;
    /// the dashes must still lie on the matching solid primitive.
    DashesOn(&'static str),
}

fn expectation(name: &str) -> Expect {
    match name {
        "dashed_line3" | "dashed_line4" => Expect::Seed,
        _ if name.starts_with("dashed_") => {
            let solid = SEED_RASTERS
                .iter()
                .map(|&(n, _)| n)
                .find(|&n| name.strip_prefix("dashed_") == Some(n))
                .expect("every dashed primitive has a solid twin");
            Expect::DashesOn(solid)
        }
        _ if name.starts_with("bezier") => Expect::SeedDotsKept,
        _ => Expect::Seed,
    }
}

/// The lit pixels of a [`raster`] string.
fn dots(raster: &str) -> HashSet<(usize, usize)> {
    let mut lit = HashSet::new();
    for (row, line) in raster.split('|').enumerate() {
        for (col, glyph) in line.chars().enumerate() {
            let bits = u32::from(glyph) - BRAILLE_BASE;
            for (dy, pair) in BRAILLE_MAP.iter().enumerate() {
                for (dx, &bit) in pair.iter().enumerate() {
                    if bits & u32::from(bit) != 0 {
                        lit.insert((col * 2 + dx, row * 4 + dy));
                    }
                }
            }
        }
    }
    lit
}

#[test]
fn primitives_keep_the_seed_rasters_where_a1_leaves_them() {
    let current = current_rasters();
    assert_eq!(current.len(), SEED_RASTERS.len());
    let mut counts = [0; 3];
    for ((name, got), (seed_name, seed)) in current.iter().zip(SEED_RASTERS) {
        assert_eq!(name, seed_name);
        match expectation(name) {
            Expect::Seed => {
                counts[0] += 1;
                assert_eq!(got, seed, "{name} moved away from the seed's raster");
            }
            Expect::SeedDotsKept => {
                counts[1] += 1;
                let missing: Vec<_> = dots(seed).difference(&dots(got)).copied().collect();
                assert_eq!(missing, [], "{name} lost dots the seed plotted");
                assert!(dots(got).len() > dots(seed).len() || got == seed, "{name}");
            }
            Expect::DashesOn(solid) => {
                counts[2] += 1;
                let (_, solid_raster) = current.iter().find(|(n, _)| n == solid).unwrap();
                let stray: Vec<_> = dots(got).difference(&dots(solid_raster)).copied().collect();
                assert_eq!(stray, [], "{name} lit dots off {solid}");
            }
        }
    }
    assert_eq!(
        counts,
        [16, 7, 10],
        "seed-exact, seed-dots-kept, dashes-on-solid"
    );
}

#[test]
fn the_raster_decoder_reads_back_what_was_drawn() {
    let mut c = BrailleCanvas::new(3, 2);
    for (x, y) in [(0, 0), (1, 3), (4, 7), (5, 2)] {
        c.set_pixel(x, y);
    }
    let expected: HashSet<_> = [(0, 0), (1, 3), (4, 7), (5, 2)].into_iter().collect();
    assert_eq!(dots(&raster(&c)), expected);
}
