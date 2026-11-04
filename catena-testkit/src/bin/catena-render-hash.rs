//! `catena-render-hash <fixture> <cols> <rows> <blitter>`: lays out a named fixture, draws it,
//! and prints its frame's SHA-256 hash (plan §16.5). The cross-run determinism test spawns it
//! twice and compares, so ASLR or hash-map order escaping into a frame shows up as two
//! different hashes, and committed hashes hold every platform to the same frame.
//!
//! `<fixture>` is `community` or `generated:<seed>:<n>`; `<blitter>` is `braille`,
//! `halfblock`, `sextant` or `ascii`.

use std::error::Error;

use catena_testkit::frames::{blitter, fixture, frame};
use catena_testkit::svg::grid_to_hash;

const USAGE: &str = "usage: catena-render-hash <community|generated:SEED:N> <cols> <rows> \
                     <braille|halfblock|sextant|ascii>";

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [name, cols, rows, blit] = args.as_slice() else {
        return Err(USAGE.into());
    };
    let graph = fixture(name).ok_or_else(|| format!("unknown fixture {name:?}; {USAGE}"))?;
    let size = (cols.parse()?, rows.parse()?);
    let blit = blitter(blit).ok_or_else(|| format!("unknown blitter {blit:?}; {USAGE}"))?;
    let grid = frame(&graph, blit, size).map_err(|e| format!("{e:?}"))?;
    println!("{}", grid_to_hash(&grid));
    Ok(())
}
