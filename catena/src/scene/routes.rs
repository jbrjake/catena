//! Invariant B in route terms (plan §16.2-B as owner ruling A3 restates it): every input edge
//! maps to exactly one route, and that route's segments connect its source anchor to its target
//! anchor.

use std::collections::BTreeMap;

use super::{EdgeRoute, Payload, Route, SceneGraph, SegmentId};
use crate::geometry::cell::CellPt;
use crate::graph::EdgeIx;

/// An input edge and the anchor cells its route must connect, source to target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeEnds {
    /// The edge.
    pub ix: EdgeIx,
    /// Its source node's anchor cell.
    pub source: CellPt,
    /// Its target node's anchor cell.
    pub target: CellPt,
}

/// How an edge's route breaks invariant B.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RouteFault {
    /// An input edge has no route.
    Missing,
    /// An input edge has more than one route.
    Duplicate,
    /// A route belongs to no input edge.
    Unexpected,
    /// The route does not run from the source anchor to the target anchor without a gap.
    Disconnected,
    /// The chain names a segment the scene does not have.
    UnknownSegment(SegmentId),
    /// The chain uses a segment that does not list the edge as a member.
    NotAMember(SegmentId),
    /// A segment lists the edge as a member, but the edge's chain does not use it.
    StrayMember(SegmentId),
}

/// Every way `scene` breaks invariant B for `edges`, ascending by edge then fault; empty when
/// it holds. A path must start in the source anchor's cell and end in the target's. A chain is
/// followed from the source anchor: each segment must have an end in the cell the walk has
/// reached and moves it to the other end (so a shared segment may run either way), and the
/// last must reach the target anchor.
#[must_use]
pub fn route_faults(scene: &SceneGraph, edges: &[EdgeEnds]) -> Vec<(EdgeIx, RouteFault)> {
    let expected: BTreeMap<EdgeIx, EdgeEnds> = edges.iter().map(|e| (e.ix, *e)).collect();
    let mut routes: BTreeMap<EdgeIx, Vec<&EdgeRoute>> = BTreeMap::new();
    for item in &scene.items {
        if let Payload::EdgePath { ix, route } = &item.payload {
            routes.entry(*ix).or_default().push(route);
        }
    }

    let mut faults = Vec::new();
    for ix in expected.keys() {
        if !routes.contains_key(ix) {
            faults.push((*ix, RouteFault::Missing));
        }
    }
    for (ix, found) in &routes {
        let Some(ends) = expected.get(ix) else {
            faults.push((*ix, RouteFault::Unexpected));
            continue;
        };
        if found.len() > 1 {
            faults.push((*ix, RouteFault::Duplicate));
            continue;
        }
        check_route(scene, ends, found[0], &mut faults);
    }

    for item in &scene.items {
        let Payload::Segment { id, members, .. } = &item.payload else {
            continue;
        };
        for member in members {
            let uses = routes.get(member).is_some_and(|found| {
                found
                    .iter()
                    .any(|route| matches!(route, EdgeRoute::Chain(chain) if chain.contains(id)))
            });
            if !uses {
                faults.push((*member, RouteFault::StrayMember(*id)));
            }
        }
    }

    faults.sort_unstable();
    faults.dedup();
    faults
}

fn check_route(
    scene: &SceneGraph,
    ends: &EdgeEnds,
    route: &EdgeRoute,
    faults: &mut Vec<(EdgeIx, RouteFault)>,
) {
    let connected = match route {
        EdgeRoute::Path(path) => path.ends() == Some((ends.source, ends.target)),
        EdgeRoute::Chain(chain) => {
            let mut at = Some(ends.source);
            let mut known = true;
            for &id in chain {
                let Some(Payload::Segment { route, members, .. }) =
                    scene.segment(id).map(|item| &item.payload)
                else {
                    faults.push((ends.ix, RouteFault::UnknownSegment(id)));
                    known = false;
                    continue;
                };
                if members.binary_search(&ends.ix).is_err() {
                    faults.push((ends.ix, RouteFault::NotAMember(id)));
                }
                at = at.and_then(|cell| across(route, cell));
            }
            // A chain through an unknown segment cannot be followed; that fault says so.
            !known || (!chain.is_empty() && at == Some(ends.target))
        }
    };
    if !connected {
        faults.push((ends.ix, RouteFault::Disconnected));
    }
}

/// The far end of `route` entered at `cell`, or `None` when neither end is there.
fn across(route: &Route, cell: CellPt) -> Option<CellPt> {
    let (first, last) = route.ends()?;
    if first == cell {
        Some(last)
    } else if last == cell {
        Some(first)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "routes_tests.rs"]
mod tests;
