//! Force-directed layout (plan §8): Fruchterman-Reingold with a Barnes-Hut quadtree above
//! `bh_threshold` nodes.

mod params;
mod quadtree;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "GraphView runs the force layout once the viewport lands (M2 step 4)"
    )
)]
mod simulation;

pub use params::{ForceParams, Repulsion};
