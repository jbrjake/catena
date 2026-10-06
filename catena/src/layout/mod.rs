//! Layout engines: force-directed, layered, tree and radial (plan §8, §9). Each runs in
//! isotropic `f64` world space and knows nothing about cell shape (plan §6).

pub(crate) mod force;
pub(crate) mod radial;
