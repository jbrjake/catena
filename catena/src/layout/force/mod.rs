//! Force-directed layout (plan §8): Fruchterman-Reingold with a Barnes-Hut quadtree above
//! `bh_threshold` nodes. The engine itself lands at M2.

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the force engine (M2) is the quadtree's first caller"
    )
)]
mod quadtree;
