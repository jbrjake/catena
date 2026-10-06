//! The Fruchterman-Reingold simulation (plan §8.1), ported from the seed's `layout`:
//! position-based Euler steps (no velocity), repulsion `k² / d` between every pair, attraction
//! `d² / k` along every edge, and a weak gravity toward the running centroid that keeps
//! outliers from owning the bounding box.
//!
//! Two departures make it converge, because the seed's schedule froze a cold layout before it
//! reached equilibrium and a warm restart then relaxed everything at once (the owner's
//! "Relayout" ruling):
//!
//! - **A stable step.** The seed moved a body by its whole force `F` (FR's displacement),
//!   capped by a temperature. Near a balance that overshoots whenever the body is stiffer than
//!   2, which one spring with its other end free already is, so bodies swing as wide as the
//!   temperature allows until it cools to nothing. Here a body moves `F / max(D, 1)`, still
//!   capped by the temperature, where `D` is a Gershgorin bound on how fast every body's
//!   moves change its force: twice the stiffness of its springs, repulsion and gravity (the
//!   other end of each moves too) plus its tether's. That is damped Jacobi relaxation, which
//!   never overshoots: never further than the seed's step, and a body whose forces balance
//!   stays put however hot the run.
//! - **An adaptive temperature** (Hu, "Efficient and high quality force-directed graph
//!   drawing", 2005): it shrinks by `cooling` after a step that raised the total squared force
//!   and grows by as much after five steps in a row that lowered it, so a run slows where it
//!   oscillates instead of freezing on a fixed schedule.
//!
//! Each body has its own ideal distance `kᵢ`: a pair's springs rest, and its repulsion balances
//! them, at `√(kᵢ kⱼ)`, so a wide box keeps its neighbors further off. In a warm run a body may
//! be tethered to where it was by a spring of its own strength.
//!
//! It runs in isotropic world space, where one unit is one cell column on both axes: there is
//! no aspect factor anywhere here, because the grid snapper applies the cell's shape once
//! (plan §6). Each body's box is its measured form in world units, which sets the starting
//! circle. The caller orders the bodies canonically (plan §4.1), so the same graph always runs
//! the same arithmetic; nothing here reads a clock or draws a random number.

use super::params::{ForceParams, Repulsion};
use super::quadtree::{QuadTree, push};
use crate::fmath;
use crate::layout::count;

/// One node as the simulation sees it, in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Body {
    /// Its box's width and height.
    pub(crate) size: (f64, f64),
    /// Its layout weight (plan §4.2): how hard it repels.
    pub(crate) weight: f64,
    /// Its layout edges, counting each parallel edge, for [`Repulsion::DegreeScaled`].
    pub(crate) degree: u32,
    /// Its ideal distance `kᵢ` (see [`ideal_distance`]).
    pub(crate) ideal: f64,
    /// A fixed position: the body exerts force there and never moves.
    pub(crate) pin: Option<(f64, f64)>,
    /// Where the previous layout left it, for a warm start; `None` for a body new to the
    /// layout.
    pub(crate) previous: Option<(f64, f64)>,
    /// The strength of a spring pulling it back toward `previous`; 0 for none.
    pub(crate) tether: f64,
}

/// An edge's pull between two bodies, by index; an edge from a body to itself pulls nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spring {
    pub(crate) ends: (usize, usize),
    /// The edge's weight (plan §4.2): its attraction multiplier.
    pub(crate) weight: f64,
}

/// How hot a run starts and how long it may take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Schedule {
    /// A layout from scratch, or one where every body is free: temperature `max(area) / 2`
    /// for `iterations` steps.
    Cold,
    /// A relayout of a converged layout: `max(area) / 8` for `warm_iterations` steps, with new
    /// bodies ramping their force in.
    Warm,
}

/// The base ideal distance `√(area / n)` for `n` nodes in the core (plan §8.1).
pub(crate) fn base_distance(area: (f64, f64), n: usize) -> f64 {
    (area.0 * area.1 / count(n.max(1))).sqrt()
}

/// A node's ideal distance: `base`, times `max(width / 4, 1)` when `k_label_scale` is on, so a
/// node rests far enough from its neighbors for its label. Plan §8.1 scales one `k` by the
/// average width; per node, a box that widens pushes harder and its neighbors make room.
pub(crate) fn ideal_distance(params: &ForceParams, base: f64, width: f64) -> f64 {
    if params.k_label_scale {
        base * (width / 4.0).max(1.0)
    } else {
        base
    }
}

fn mean(values: impl ExactSizeIterator<Item = f64>) -> f64 {
    let n = values.len();
    if n == 0 {
        0.0
    } else {
        values.sum::<f64>() / count(n)
    }
}

/// Lays `bodies` out on `schedule` and returns each one's position: its center, in world
/// units. A step whose largest move is under `converge_eps` ends the run.
pub(crate) fn simulate(
    params: &ForceParams,
    bodies: &[Body],
    springs: &[Spring],
    area: (f64, f64),
    schedule: Schedule,
) -> Vec<(f64, f64)> {
    let params = params.sanitized();
    let mut run = Run::new(&params, bodies, springs, area, schedule);
    for step in 0..run.steps {
        if !run.step(step) {
            break;
        }
    }
    run.positions
}

/// The net force on each body where it would start, from repulsion, springs and gravity, with
/// no pin, tether or ramp: what a test reads to tell whether a layout rests where its forces
/// balance.
#[cfg(test)]
pub(super) fn net_forces(
    params: &ForceParams,
    bodies: &[Body],
    springs: &[Spring],
    area: (f64, f64),
) -> Vec<(f64, f64)> {
    let params = params.sanitized();
    let free: Vec<Body> = bodies
        .iter()
        .map(|b| Body {
            pin: None,
            previous: b.pin.or(b.previous),
            tether: 0.0,
            ..*b
        })
        .collect();
    let mut run = Run::new(&params, &free, springs, area, Schedule::Cold);
    run.forces(u32::MAX);
    run.displacements
}

/// One simulation in progress.
pub(super) struct Run<'a> {
    params: &'a ForceParams,
    bodies: &'a [Body],
    springs: &'a [Spring],
    /// Each body's repelling mass times its ideal distance, before any ramp: two bodies repel
    /// by the product of theirs over `d`, the FR `k² / d` when every ideal distance is `k`.
    charges: Vec<f64>,
    /// Whether each body is new to a warm layout, and so ramps its force in.
    fresh: Vec<bool>,
    pub(super) positions: Vec<(f64, f64)>,
    /// Each body's net force at the last call to `forces`.
    pub(super) displacements: Vec<(f64, f64)>,
    /// Each body's stiffness at the last call to `forces`: a bound on how fast its force
    /// changes as the bodies move (see the module comment).
    stiffness: Vec<f64>,
    temperature: f64,
    /// The total squared force on the movable bodies at the last step.
    energy: f64,
    /// Steps in a row that lowered `energy`.
    progress: u32,
    steps: u32,
}

impl<'a> Run<'a> {
    pub(super) fn new(
        params: &'a ForceParams,
        bodies: &'a [Body],
        springs: &'a [Spring],
        area: (f64, f64),
        schedule: Schedule,
    ) -> Self {
        let warm = schedule == Schedule::Warm;
        let charges = bodies
            .iter()
            .map(|b| {
                let mass = match params.repulsion {
                    Repulsion::Uniform => b.weight,
                    Repulsion::DegreeScaled => b.weight * (f64::from(b.degree) + 1.0),
                };
                mass * b.ideal
            })
            .collect();
        let fresh = bodies
            .iter()
            .map(|b| warm && b.previous.is_none() && b.pin.is_none())
            .collect();
        let span = area.0.max(area.1);
        Run {
            params,
            bodies,
            springs,
            charges,
            fresh,
            positions: start_positions(bodies, springs),
            displacements: vec![(0.0, 0.0); bodies.len()],
            stiffness: vec![0.0; bodies.len()],
            temperature: if warm { span / 8.0 } else { span / 2.0 },
            energy: f64::INFINITY,
            progress: 0,
            steps: if warm {
                params.warm_iterations
            } else {
                params.iterations
            },
        }
    }

    /// Takes step `step` (from 0) and returns whether the run goes on: `false` once every move
    /// was shorter than `converge_eps`.
    pub(super) fn step(&mut self, step: u32) -> bool {
        self.forces(step);
        let (mut longest, mut energy) = (0.0f64, 0.0);
        for (i, body) in self.bodies.iter().enumerate() {
            if body.pin.is_some() {
                continue;
            }
            let (dx, dy) = self.displacements[i];
            energy += dx * dx + dy * dy;
            let magnitude = (dx * dx + dy * dy).sqrt().max(0.01);
            let length = (magnitude / self.stiffness[i].max(1.0)).min(self.temperature);
            let (x, y) = self.positions[i];
            let moved = (x + dx / magnitude * length, y + dy / magnitude * length);
            if moved.0.is_finite() && moved.1.is_finite() {
                self.positions[i] = moved;
                longest = longest.max(length);
            }
        }
        self.adapt(energy);
        longest >= self.params.converge_eps
    }

    /// Hu's step control: cool after a step that raised `energy`, warm up after five that
    /// lowered it.
    fn adapt(&mut self, energy: f64) {
        let cooling = self.params.cooling;
        if energy < self.energy {
            self.progress += 1;
            if self.progress >= 5 && cooling > 0.0 {
                self.progress = 0;
                self.temperature /= cooling;
            }
        } else {
            self.progress = 0;
            self.temperature *= cooling;
        }
        self.energy = energy;
    }

    /// How much of its force body `i` exerts at step `step`: a body new to a warm layout ramps
    /// from `1 / ramp_in_iterations` to all of it, so one arrival does not shove its
    /// neighborhood (plan §8.3).
    fn ramp(&self, i: usize, step: u32) -> f64 {
        let ramp = self.params.ramp_in_iterations;
        if self.fresh[i] && step < ramp {
            f64::from(step + 1) / f64::from(ramp)
        } else {
            1.0
        }
    }

    /// Fills `displacements` with each movable body's net force at step `step`, and
    /// `stiffness` with how fast that force changes as it moves. A pinned body's own force is
    /// never used, so it is not summed.
    pub(super) fn forces(&mut self, step: u32) {
        let n = self.bodies.len();
        let exerted: Vec<f64> = (0..n)
            .map(|i| self.charges[i] * self.ramp(i, step))
            .collect();
        self.displacements.fill((0.0, 0.0));
        self.stiffness.fill(2.0 * self.params.gravity);
        let movable = |i: usize| self.bodies[i].pin.is_none();

        if n > self.params.bh_threshold {
            let tree = self.tree(&exerted);
            for (i, &(x, y)) in self.positions.iter().enumerate() {
                if movable(i) {
                    let push = tree.compute_push(x, y, self.params.theta, 1.0);
                    let charge = self.charges[i];
                    self.displacements[i] = (push.force.0 * charge, push.force.1 * charge);
                    self.stiffness[i] += 2.0 * push.stiffness * charge;
                }
            }
        } else {
            for (i, &(x, y)) in self.positions.iter().enumerate() {
                if !movable(i) {
                    continue;
                }
                let (mut fx, mut fy, mut stiffness) = (0.0, 0.0, 0.0);
                for (j, &(ox, oy)) in self.positions.iter().enumerate() {
                    if j != i {
                        let push = push(x - ox, y - oy, 1.0);
                        fx += push.force.0 * exerted[j];
                        fy += push.force.1 * exerted[j];
                        stiffness += push.stiffness * exerted[j];
                    }
                }
                let charge = self.charges[i];
                self.displacements[i] = (fx * charge, fy * charge);
                self.stiffness[i] += 2.0 * stiffness * charge;
            }
        }

        let (cx, cy) = centroid(&self.positions);
        for (i, &(x, y)) in self.positions.iter().enumerate() {
            self.displacements[i].0 -= self.params.gravity * (x - cx);
            self.displacements[i].1 -= self.params.gravity * (y - cy);
        }

        for spring in self.springs {
            let (a, b) = spring.ends;
            if a == b {
                continue;
            }
            let (dx, dy) = (
                self.positions[a].0 - self.positions[b].0,
                self.positions[a].1 - self.positions[b].1,
            );
            let dist = (dx * dx + dy * dy).sqrt().max(0.01);
            let ideal = (self.bodies[a].ideal * self.bodies[b].ideal).sqrt();
            let pull = spring.weight * dist * dist / ideal;
            let stiffness = 4.0 * spring.weight * dist / ideal;
            let (ux, uy) = (dx / dist * pull, dy / dist * pull);
            let (on_a, on_b) = (self.ramp(b, step), self.ramp(a, step));
            self.displacements[a].0 -= ux * on_a;
            self.displacements[a].1 -= uy * on_a;
            self.displacements[b].0 += ux * on_b;
            self.displacements[b].1 += uy * on_b;
            self.stiffness[a] += stiffness * on_a;
            self.stiffness[b] += stiffness * on_b;
        }

        for (i, body) in self.bodies.iter().enumerate() {
            if let Some((px, py)) = body.previous.filter(|_| body.tether > 0.0) {
                let (x, y) = self.positions[i];
                self.displacements[i].0 -= body.tether * (x - px);
                self.displacements[i].1 -= body.tether * (y - py);
                self.stiffness[i] += body.tether;
            }
        }
    }

    /// A Barnes-Hut tree over the bodies, each with its exerted charge, its bounds grown by
    /// one unit so no body sits on an edge.
    fn tree(&self, exerted: &[f64]) -> QuadTree {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &(x, y) in &self.positions {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        let mut tree = QuadTree::new(x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0);
        for (&(x, y), &charge) in self.positions.iter().zip(exerted) {
            tree.insert_weighted(x, y, charge);
        }
        tree
    }
}

/// The mean of `points`; the origin for none.
pub(crate) fn centroid(points: &[(f64, f64)]) -> (f64, f64) {
    if points.is_empty() {
        return (0.0, 0.0);
    }
    let n = count(points.len());
    let (sx, sy) = points
        .iter()
        .fold((0.0, 0.0), |(sx, sy), &(x, y)| (sx + x, sy + y));
    (sx / n, sy / n)
}

/// The golden angle, in radians: successive multiples of it never line up, so bodies placed
/// around the same neighbors fan out.
const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;

/// Where each body starts: at its pin; else where it was; else, half its ideal distance from
/// the mean of its neighbors that already have a place, at a golden-angle turn of its index;
/// else on the starting circle. The circle has radius `√n` times the average width (the seed
/// says average but used each node's own width, so its circle was not one) and is centered on
/// the placed bodies, or on the origin when there are none. No aspect factor: plan §6.
fn start_positions(bodies: &[Body], springs: &[Spring]) -> Vec<(f64, f64)> {
    let n = bodies.len();
    let placed: Vec<Option<(f64, f64)>> = bodies.iter().map(|b| b.pin.or(b.previous)).collect();
    let anchored: Vec<(f64, f64)> = placed.iter().flatten().copied().collect();
    let center = centroid(&anchored);
    let radius = count(n).sqrt() * mean(bodies.iter().map(|b| b.size.0));

    let mut neighbors = vec![(0.0, 0.0, 0u32); n];
    for spring in springs {
        let (a, b) = spring.ends;
        for (from, to) in [(a, b), (b, a)] {
            if let (Some((x, y)), true) = (placed[to], placed[from].is_none() && from != to) {
                let sum = &mut neighbors[from];
                *sum = (sum.0 + x, sum.1 + y, sum.2 + 1);
            }
        }
    }

    (0..n)
        .map(|i| {
            if let Some(at) = placed[i] {
                return at;
            }
            let index = count(i);
            let (sx, sy, placed_neighbors) = neighbors[i];
            if placed_neighbors > 0 {
                let turn = index * GOLDEN_ANGLE;
                let m = f64::from(placed_neighbors);
                let offset = bodies[i].ideal / 2.0;
                (
                    sx / m + offset * fmath::cos(turn),
                    sy / m + offset * fmath::sin(turn),
                )
            } else {
                let angle = std::f64::consts::TAU * index / count(n);
                (
                    center.0 + radius * fmath::cos(angle),
                    center.1 + radius * fmath::sin(angle),
                )
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "simulation_tests.rs"]
mod tests;
