//! The force layout's parameters (plan §8.1). The defaults are the seed's proven values, and
//! every one is honored as given: the seed floored cold runs to 100 iterations and warm runs to
//! 30 whatever was asked, and here the presets carry that wisdom instead.

/// How nodes push each other apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Repulsion {
    /// Classic Fruchterman-Reingold: every node repels every other by `k² / d`, scaled by
    /// their weights.
    #[default]
    Uniform,
    /// ForceAtlas2-flavored: each node's repelling mass is its weight times its degree plus
    /// one, so hubs push harder and clusters read apart.
    DegreeScaled,
}

/// The force layout's parameters (plan §8.1).
///
/// A parameter outside its range is clamped when a layout runs, never refused: a NaN takes
/// its default, `cooling` keeps to `0..=1`, and the others to finite non-negative values.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ForceParams {
    /// Steps of a cold layout; 100.
    pub iterations: u32,
    /// Steps of a warm relayout; 50, the seed orchestrator's budget.
    pub warm_iterations: u32,
    /// The temperature's multiplier per step; 0.95.
    pub cooling: f64,
    /// The pull toward the running centroid, per unit of distance; 0.10.
    pub gravity: f64,
    /// The Barnes-Hut opening criterion; 0.80.
    pub theta: f64,
    /// The node count above which repulsion is approximated by a quadtree; 500. At or below
    /// it the exact sum is cheaper.
    pub bh_threshold: usize,
    /// How nodes repel.
    pub repulsion: Repulsion,
    /// A step whose largest move is shorter than this, in world units, ends the run; 0.5.
    pub converge_eps: f64,
    /// Whether the ideal distance `k` grows with the average label width, so nodes settle
    /// where their labels survive; `true`.
    pub k_label_scale: bool,
    /// Steps over which a node new to a warm layout ramps its force from 0 to full; 10.
    pub ramp_in_iterations: u32,
}

impl Default for ForceParams {
    fn default() -> Self {
        ForceParams::quality()
    }
}

impl ForceParams {
    /// The defaults: 100 cold iterations.
    #[must_use]
    pub fn quality() -> Self {
        ForceParams {
            iterations: 100,
            warm_iterations: 50,
            cooling: 0.95,
            gravity: 0.10,
            theta: 0.80,
            bh_threshold: 500,
            repulsion: Repulsion::Uniform,
            converge_eps: 0.5,
            k_label_scale: true,
            ramp_in_iterations: 10,
        }
    }

    /// The defaults with 30 cold iterations, the seed's warm floor.
    #[must_use]
    pub fn fast() -> Self {
        ForceParams {
            iterations: 30,
            ..ForceParams::quality()
        }
    }

    /// These parameters with every float in its range (see the type's doc comment).
    pub(crate) fn sanitized(&self) -> Self {
        let defaults = ForceParams::quality();
        let finite = |v: f64, default: f64| {
            if v.is_nan() {
                default
            } else {
                v.clamp(0.0, f64::MAX)
            }
        };
        ForceParams {
            cooling: if self.cooling.is_nan() {
                defaults.cooling
            } else {
                self.cooling.clamp(0.0, 1.0)
            },
            gravity: finite(self.gravity, defaults.gravity),
            theta: finite(self.theta, defaults.theta),
            converge_eps: finite(self.converge_eps, defaults.converge_eps),
            ..self.clone()
        }
    }
}
