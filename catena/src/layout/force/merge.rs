//! Joined islands pull together (the owner's "Relayout follow-ups" ruling,
//! `docs/design/owner-rulings.md`): "it's not okay for islands to be stretched, they need to
//! pull together."
//!
//! When a topology change joins islands the last layout placed apart, the relayout's reach
//! holds each island's far side, so on its own it leaves the islands where they were: the
//! joining edges stretch, and the joined nodes are dragged out of their islands toward each
//! other. Freeing the islands would not help. FR's repulsion reaches across any distance, so
//! two joined islands repel by the product of their sizes against the pull of a few joining
//! edges, and a cold layout of the joined graph sets them further apart than the packed
//! islands were.
//!
//! So before the relayout, this pass moves each joined island but one as a rigid body,
//! turning and sliding it until the edges that join it rest. The joining springs pull, and
//! bodies of different islands push only within [`CONTACT`] ideal distances of each other,
//! which keeps the islands from running into each other without pushing them apart from afar.
//! A node new to the layout moves as a point among them. The island with the most survivors
//! (then the one holding the canonically first of them) stays put, as does any island with a
//! pinned node, so a join moves as few nodes as it can and never a pin. The relayout then
//! starts from the moved places, and its reach settles the nodes around the join as it does
//! after any topology change.
//!
//! The steps are the simulation's: damped by each body's stiffness, so a move never
//! overshoots, capped by a temperature that adapts as Hu's does (see [`super::simulation`]).

use super::params::{ForceParams, Repulsion};
use super::quadtree::push;
use super::simulation::{Body, Spring, start_positions};

/// How many of a pair's ideal distances apart two bodies of different islands start to push
/// each other in this pass: Fruchterman and Reingold's own cutoff for their grid variant
/// (1991), where only vertices within `2k` repel.
pub(super) const CONTACT: f64 = 2.0;

/// What moves in the pass.
enum Unit {
    /// An island of the last layout: its members, where they started, and how far it has
    /// slid and turned about its starting centroid.
    Rigid {
        members: Vec<usize>,
        origin: Vec<(f64, f64)>,
        center: (f64, f64),
        shift: (f64, f64),
        turn: f64,
    },
    /// A body new to the layout.
    Point(usize),
}

/// Where each body of one island starts once the islands of the last layout that it joins
/// have pulled together.
///
/// `islands[i]` is body `i`'s island in the last layout, `None` for a body new to it, and
/// `pinned[i]` whether it is pinned. Returns each body's position: a survivor's moved with its
/// island, a new body's where the pass left it. With fewer than two old islands nothing moves.
pub(super) fn pull_together(
    params: &ForceParams,
    bodies: &[Body],
    springs: &[Spring],
    islands: &[Option<usize>],
    pinned: &[bool],
    area: (f64, f64),
) -> Vec<(f64, f64)> {
    let mut positions = start_positions(bodies, springs);
    let (unit_of, units, still) = units(islands, pinned, &positions);
    if units
        .iter()
        .filter(|u| matches!(u, Unit::Rigid { .. }))
        .count()
        + still.len()
        < 2
    {
        return positions;
    }
    let charges: Vec<f64> = bodies
        .iter()
        .map(|b| {
            let mass = match params.repulsion {
                Repulsion::Uniform => b.weight,
                Repulsion::DegreeScaled => b.weight * (f64::from(b.degree) + 1.0),
            };
            mass * b.ideal
        })
        .collect();
    let mut units = units;
    // Islands packed apart can sit further from each other than the frame is wide, so the
    // pass starts hot enough to close its longest joining edge by half in one step.
    let longest_join = springs
        .iter()
        .filter(|s| unit_of[s.ends.0] != unit_of[s.ends.1])
        .map(|s| {
            let (a, b) = (positions[s.ends.0], positions[s.ends.1]);
            let (dx, dy) = (a.0 - b.0, a.1 - b.1);
            (dx * dx + dy * dy).sqrt()
        })
        .fold(0.0, f64::max);
    let mut temperature = (area.0.max(area.1) / 8.0).max(longest_join / 2.0);
    let (mut energy, mut progress) = (f64::INFINITY, 0u32);
    for _ in 0..params.warm_iterations.max(1) {
        let (force, stiffness) = forces(bodies, springs, &charges, &unit_of, &positions);
        let (mut longest, mut total) = (0.0f64, 0.0);
        for unit in &mut units {
            let (moved, squared) = unit.step(&force, &stiffness, temperature, &mut positions);
            longest = longest.max(moved);
            total += squared;
        }
        if total < energy {
            progress += 1;
            if progress >= 5 && params.cooling > 0.0 {
                progress = 0;
                temperature /= params.cooling;
            }
        } else {
            progress = 0;
            temperature *= params.cooling;
        }
        energy = total;
        if longest < params.converge_eps {
            break;
        }
    }
    positions
}

/// Each body's unit (`usize::MAX` for one that stays put), the units that move, and the old
/// islands that stay put: the one with the most survivors, then the one holding the first of
/// them, and every island with a pinned body.
fn units(
    islands: &[Option<usize>],
    pinned: &[bool],
    positions: &[(f64, f64)],
) -> (Vec<usize>, Vec<Unit>, Vec<usize>) {
    let mut members: Vec<(usize, Vec<usize>)> = Vec::new();
    for (i, island) in islands.iter().enumerate() {
        if let Some(id) = *island {
            match members.iter_mut().find(|(other, _)| *other == id) {
                Some((_, list)) => list.push(i),
                None => members.push((id, vec![i])),
            }
        }
    }
    // `members` is in order of each island's first body; the first of the largest anchors.
    let anchor = members
        .iter()
        .enumerate()
        .max_by(|(a, (_, x)), (b, (_, y))| x.len().cmp(&y.len()).then(b.cmp(a)))
        .map(|(k, _)| k);
    let mut unit_of = vec![usize::MAX; islands.len()];
    let (mut units, mut still) = (Vec::new(), Vec::new());
    for (k, (id, list)) in members.into_iter().enumerate() {
        if Some(k) == anchor || list.iter().any(|&i| pinned[i]) {
            still.push(id);
            continue;
        }
        for &i in &list {
            unit_of[i] = units.len();
        }
        let origin: Vec<(f64, f64)> = list.iter().map(|&i| positions[i]).collect();
        units.push(Unit::Rigid {
            center: super::simulation::centroid(&origin),
            members: list,
            origin,
            shift: (0.0, 0.0),
            turn: 0.0,
        });
    }
    for (i, island) in islands.iter().enumerate() {
        if island.is_none() && !pinned[i] {
            unit_of[i] = units.len();
            units.push(Unit::Point(i));
        }
    }
    (unit_of, units, still)
}

/// Each moving body's force from bodies outside its unit, and its stiffness: the joining
/// springs, and the contact push between bodies of different islands. Only moving bodies feel
/// anything, so two staying bodies count as one unit, whatever their islands.
fn forces(
    bodies: &[Body],
    springs: &[Spring],
    charges: &[f64],
    unit_of: &[usize],
    positions: &[(f64, f64)],
) -> (Vec<(f64, f64)>, Vec<f64>) {
    let n = bodies.len();
    let (mut force, mut stiffness) = (vec![(0.0, 0.0); n], vec![0.0; n]);
    let apart = |i: usize, j: usize| unit_of[i] != unit_of[j];
    for i in (0..n).filter(|&i| unit_of[i] != usize::MAX) {
        let (x, y) = positions[i];
        for j in (0..n).filter(|&j| j != i && apart(i, j)) {
            let (dx, dy) = (x - positions[j].0, y - positions[j].1);
            let reach = CONTACT * (bodies[i].ideal * bodies[j].ideal).sqrt();
            if dx * dx + dy * dy >= reach * reach {
                continue;
            }
            let push = push(dx, dy, 1.0);
            let scale = charges[i] * charges[j];
            force[i].0 += push.force.0 * scale;
            force[i].1 += push.force.1 * scale;
            stiffness[i] += 2.0 * push.stiffness * scale;
        }
    }
    for spring in springs {
        let (a, b) = spring.ends;
        if a == b || !apart(a, b) {
            continue;
        }
        let (dx, dy) = (
            positions[a].0 - positions[b].0,
            positions[a].1 - positions[b].1,
        );
        let dist = (dx * dx + dy * dy).sqrt().max(0.01);
        let ideal = (bodies[a].ideal * bodies[b].ideal).sqrt();
        let pull = spring.weight * dist * dist / ideal;
        let (ux, uy) = (dx / dist * pull, dy / dist * pull);
        let stiff = 4.0 * spring.weight * dist / ideal;
        force[a] = (force[a].0 - ux, force[a].1 - uy);
        force[b] = (force[b].0 + ux, force[b].1 + uy);
        stiffness[a] += stiff;
        stiffness[b] += stiff;
    }
    (force, stiffness)
}

/// A move of `(dx, dy)` shortened to at most `cap`, and its length.
fn capped(dx: f64, dy: f64, cap: f64) -> ((f64, f64), f64) {
    let length = (dx * dx + dy * dy).sqrt();
    if length > cap && length > 0.0 {
        ((dx / length * cap, dy / length * cap), cap)
    } else {
        ((dx, dy), length)
    }
}

impl Unit {
    /// Moves this unit one damped step under `force`, capped by `temperature`, and writes its
    /// bodies' new places into `positions`; returns the longest move of any of them and the
    /// squared net force it answered.
    fn step(
        &mut self,
        force: &[(f64, f64)],
        stiffness: &[f64],
        temperature: f64,
        positions: &mut [(f64, f64)],
    ) -> (f64, f64) {
        match self {
            Unit::Point(i) => {
                let (fx, fy) = force[*i];
                let damp = stiffness[*i].max(1.0);
                let ((dx, dy), moved) = capped(fx / damp, fy / damp, temperature);
                let (x, y) = positions[*i];
                if (x + dx).is_finite() && (y + dy).is_finite() {
                    positions[*i] = (x + dx, y + dy);
                }
                (moved, fx * fx + fy * fy)
            }
            Unit::Rigid {
                members,
                origin,
                center,
                shift,
                turn,
            } => {
                let at = (center.0 + shift.0, center.1 + shift.1);
                let (mut fx, mut fy, mut torque, mut damp, mut inertia, mut far) =
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0f64);
                for &i in members.iter() {
                    let (rx, ry) = (positions[i].0 - at.0, positions[i].1 - at.1);
                    let (f, s) = (force[i], stiffness[i]);
                    (fx, fy) = (fx + f.0, fy + f.1);
                    torque += rx * f.1 - ry * f.0;
                    damp += s;
                    inertia += s * (rx * rx + ry * ry);
                    far = far.max((rx * rx + ry * ry).sqrt());
                }
                let ((dx, dy), slid) = capped(fx / damp.max(1.0), fy / damp.max(1.0), temperature);
                let mut spin = torque / inertia.max(1.0);
                if far > 0.0 && spin.abs() * far > temperature {
                    spin = spin.signum() * temperature / far;
                }
                let next_shift = (shift.0 + dx, shift.1 + dy);
                let next_turn = *turn + spin;
                if !(next_shift.0.is_finite() && next_shift.1.is_finite() && next_turn.is_finite())
                {
                    return (0.0, 0.0);
                }
                (*shift, *turn) = (next_shift, next_turn);
                let (sine, cosine) = (crate::fmath::sin(*turn), crate::fmath::cos(*turn));
                for (&i, &(ox, oy)) in members.iter().zip(origin.iter()) {
                    let (rx, ry) = (ox - center.0, oy - center.1);
                    positions[i] = (
                        center.0 + shift.0 + rx * cosine - ry * sine,
                        center.1 + shift.1 + rx * sine + ry * cosine,
                    );
                }
                (slid + spin.abs() * far, fx * fx + fy * fy)
            }
        }
    }
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
