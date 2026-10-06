//! Layout engines: force-directed, layered, tree and radial (plan §8, §9). Each runs in
//! isotropic `f64` world space and knows nothing about cell shape (plan §6).

pub(crate) mod force;
pub(crate) mod layered;
pub(crate) mod radial;
pub(crate) mod tree;

/// A count as `f64`. Node and group counts are bounded by `u32` indices (plan §4.1), far below
/// the 2^53 where `f64` stops being exact.
#[expect(
    clippy::cast_precision_loss,
    reason = "counts stay below 2^32, which f64 represents exactly"
)]
pub(crate) fn count(n: usize) -> f64 {
    n as f64
}
