//! The Fruchterman-Reingold simulation (plan §8.1), ported from the seed's `layout` with its
//! tuning intact: position-based Euler steps (no velocity; stability comes from warm starts),
//! each move clamped by a cooling temperature, repulsion `k² / d` between every pair, attraction
//! `d² / k` along every edge, and a weak gravity toward the running centroid that keeps outliers
//! from owning the bounding box.
//!
//! It runs in isotropic world space, where one unit is one cell column on both axes: there is
//! no aspect factor anywhere here, because the grid snapper applies the cell's shape once
//! (plan §6). Each body's box is its measured form in world units, which sets the ideal distance
//! `k` and the starting circle. The caller orders the bodies canonically (plan §4.1), so the
//! same graph always runs the same arithmetic; nothing here reads a clock or draws a random
//! number.

use super::params::{ForceParams, Repulsion};
use super::quadtree::{QuadTree, repulsion};
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
    /// A fixed position: the body exerts force there and never moves.
    pub(crate) pin: Option<(f64, f64)>,
    /// Where the previous layout left it, for a warm start; `None` for a body new to the
    /// layout.
    pub(crate) previous: Option<(f64, f64)>,
}

/// An edge's pull between two bodies, by index; an edge from a body to itself pulls nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spring {
    pub(crate) ends: (usize, usize),
    /// The edge's weight (plan §4.2): its attraction multiplier.
    pub(crate) weight: f64,
}

/// The ideal distance `k = √(area / n)`, times `max(average width / 4, 1)` when
/// `k_label_scale` is on, so connected nodes settle far enough apart for their labels.
pub(crate) fn ideal_distance(params: &ForceParams, area: (f64, f64), widths: &[f64]) -> f64 {
    let n = count(widths.len().max(1));
    let mut k = (area.0 * area.1 / n).sqrt();
    if params.k_label_scale {
        k *= (mean(widths) / 4.0).max(1.0);
    }
    k
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / count(values.len())
    }
}

/// Lays `bodies` out and returns each one's position: its center, in world units.
///
/// The run is warm when any body has a previous position: then the temperature starts at
/// `max(area) / 8` for `warm_iterations` steps, survivors start where they were, and a new
/// body starts beside its placed neighbors and ramps its force in. Otherwise it is cold:
/// `max(area) / 2` for `iterations` steps, from a circle of radius `√n` times the average
/// width. Either way a step whose largest move is under `converge_eps` ends the run.
pub(crate) fn simulate(
    params: &ForceParams,
    bodies: &[Body],
    springs: &[Spring],
    k: f64,
    area: (f64, f64),
) -> Vec<(f64, f64)> {
    let params = params.sanitized();
    let mut run = Run::new(&params, bodies, springs, k, area);
    for step in 0..run.steps {
        if !run.step(step) {
            break;
        }
    }
    run.positions
}

/// One simulation in progress.
pub(super) struct Run<'a> {
    params: &'a ForceParams,
    bodies: &'a [Body],
    springs: &'a [Spring],
    k: f64,
    /// Each body's repelling mass, before any ramp.
    masses: Vec<f64>,
    /// Whether each body is new to a warm layout, and so ramps its force in.
    fresh: Vec<bool>,
    pub(super) positions: Vec<(f64, f64)>,
    displacements: Vec<(f64, f64)>,
    temperature: f64,
    steps: u32,
}

impl<'a> Run<'a> {
    pub(super) fn new(
        params: &'a ForceParams,
        bodies: &'a [Body],
        springs: &'a [Spring],
        k: f64,
        area: (f64, f64),
    ) -> Self {
        let warm = bodies.iter().any(|b| b.previous.is_some());
        let masses = bodies
            .iter()
            .map(|b| match params.repulsion {
                Repulsion::Uniform => b.weight,
                Repulsion::DegreeScaled => b.weight * (f64::from(b.degree) + 1.0),
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
            k,
            masses,
            fresh,
            positions: start_positions(bodies, springs, k),
            displacements: vec![(0.0, 0.0); bodies.len()],
            temperature: if warm { span / 8.0 } else { span / 2.0 },
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
        let mut longest = 0.0f64;
        for (i, body) in self.bodies.iter().enumerate() {
            if body.pin.is_some() {
                continue;
            }
            let (dx, dy) = self.displacements[i];
            let magnitude = (dx * dx + dy * dy).sqrt().max(0.01);
            let length = magnitude.min(self.temperature);
            let (x, y) = self.positions[i];
            let moved = (x + dx / magnitude * length, y + dy / magnitude * length);
            if moved.0.is_finite() && moved.1.is_finite() {
                self.positions[i] = moved;
                longest = longest.max(length);
            }
        }
        self.temperature *= self.params.cooling;
        longest >= self.params.converge_eps
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

    /// Fills `displacements` with each body's net force at step `step`.
    pub(super) fn forces(&mut self, step: u32) {
        let n = self.bodies.len();
        let k = self.k;
        let exerted: Vec<f64> = (0..n)
            .map(|i| self.masses[i] * self.ramp(i, step))
            .collect();
        self.displacements.fill((0.0, 0.0));

        if n > self.params.bh_threshold {
            let tree = self.tree(&exerted);
            for (i, &(x, y)) in self.positions.iter().enumerate() {
                let (fx, fy) = tree.compute_force(x, y, self.params.theta, k);
                self.displacements[i] = (fx * self.masses[i], fy * self.masses[i]);
            }
        } else {
            for (i, &(x, y)) in self.positions.iter().enumerate() {
                let (mut fx, mut fy) = (0.0, 0.0);
                for (j, &(ox, oy)) in self.positions.iter().enumerate() {
                    if j != i {
                        let (rx, ry) = repulsion(x - ox, y - oy, k);
                        fx += rx * exerted[j];
                        fy += ry * exerted[j];
                    }
                }
                self.displacements[i] = (fx * self.masses[i], fy * self.masses[i]);
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
            let pull = spring.weight * dist * dist / k;
            let (ux, uy) = (dx / dist * pull, dy / dist * pull);
            let (on_a, on_b) = (self.ramp(b, step), self.ramp(a, step));
            self.displacements[a].0 -= ux * on_a;
            self.displacements[a].1 -= uy * on_a;
            self.displacements[b].0 += ux * on_b;
            self.displacements[b].1 += uy * on_b;
        }
    }

    /// A Barnes-Hut tree over the bodies, each with its exerted mass, its bounds grown by one
    /// unit so no body sits on an edge.
    fn tree(&self, exerted: &[f64]) -> QuadTree {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &(x, y) in &self.positions {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        let mut tree = QuadTree::new(x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0);
        for (&(x, y), &mass) in self.positions.iter().zip(exerted) {
            tree.insert_weighted(x, y, mass);
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

/// Where each body starts: at its pin; else where it was; else, in a warm run, half `k` from
/// the mean of its neighbors that already have a place, at a golden-angle turn of its index;
/// else on the starting circle. The circle has radius `√n` times the average width (the seed
/// says average but used each node's own width, so its circle was not one) and is centered on
/// the placed bodies, or on the origin when there are none. No aspect factor: plan §6.
fn start_positions(bodies: &[Body], springs: &[Spring], k: f64) -> Vec<(f64, f64)> {
    let n = bodies.len();
    let placed: Vec<Option<(f64, f64)>> = bodies.iter().map(|b| b.pin.or(b.previous)).collect();
    let anchored: Vec<(f64, f64)> = placed.iter().flatten().copied().collect();
    let center = centroid(&anchored);
    let widths: Vec<f64> = bodies.iter().map(|b| b.size.0).collect();
    let radius = count(n).sqrt() * mean(&widths);

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
                (
                    sx / m + k / 2.0 * fmath::cos(turn),
                    sy / m + k / 2.0 * fmath::sin(turn),
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
