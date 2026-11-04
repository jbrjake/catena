//! Frames by name: a fixture graph laid out and drawn at a size, named the way the
//! `catena-render-hash` bin takes them on its command line (plan §16.5).

use catena::raster::{Blitter, CellGrid};
use catena::{GraphError, GraphView};

use crate::fixtures::{FixtureGraph, community, generated};

/// The most nodes a `generated:<seed>:<n>` name may ask for: a ceiling on untrusted sizes.
pub const MAX_GENERATED_NODES: usize = 5_000;

/// The fixture `name` names: `community`, or `generated:<seed>:<n>` with `n` at most
/// [`MAX_GENERATED_NODES`]. `None` for anything else.
#[must_use]
pub fn fixture(name: &str) -> Option<FixtureGraph> {
    if name == "community" {
        return Some(community());
    }
    let mut parts = name.strip_prefix("generated:")?.split(':');
    let seed: u64 = parts.next()?.parse().ok()?;
    let n: usize = parts.next()?.parse().ok()?;
    if parts.next().is_some() || n > MAX_GENERATED_NODES {
        return None;
    }
    Some(generated(seed, n))
}

/// The blitter `name` names: `braille`, `halfblock`, `sextant` or `ascii`.
#[must_use]
pub fn blitter(name: &str) -> Option<Blitter> {
    match name {
        "braille" => Some(Blitter::Braille),
        "halfblock" => Some(Blitter::HalfBlock),
        "sextant" => Some(Blitter::Sextant),
        "ascii" => Some(Blitter::Ascii),
        _ => None,
    }
}

/// `graph` laid out for and drawn into `cols × rows` cells with `blitter`.
///
/// # Errors
///
/// The commit's error when the graph is inconsistent (see [`FixtureGraph::view`]).
pub fn frame(
    graph: &FixtureGraph,
    blitter: Blitter,
    (cols, rows): (u16, u16),
) -> Result<CellGrid, GraphError<String>> {
    let mut gv = graph.view(GraphView::builder().blitter(blitter))?;
    let mut grid = CellGrid::new(cols, rows);
    gv.render(&mut grid);
    Ok(grid)
}
