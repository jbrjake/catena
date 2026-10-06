use super::*;
use crate::geometry::cell::{CellBox, SubPt};
use crate::scene::{Layer, Payload, SceneItem, StyleId};

fn e(index: u32) -> EdgeIx {
    EdgeIx::new(index)
}

fn c(x: i32, y: i32) -> CellPt {
    CellPt::new(x, y)
}

fn line(from: CellPt, to: CellPt) -> Route {
    Route::Polyline(vec![SubPt::from(from), SubPt::from(to)])
}

fn ends(ix: EdgeIx, source: CellPt, target: CellPt) -> EdgeEnds {
    EdgeEnds { ix, source, target }
}

fn edge(scene: &mut SceneGraph, ix: EdgeIx, route: EdgeRoute) {
    scene.push(SceneItem::new(
        Layer::EdgesUnder,
        CellBox::default(),
        Payload::EdgePath { ix, route },
        StyleId(0),
    ));
}

/// Edge 0 draws its own path A→B. Edges 1 (C→D) and 2 (F→E) share a trunk from J to K, which
/// edge 2 runs backwards: its chain goes F→K, K→J, J→E.
fn bundle() -> (SceneGraph, Vec<EdgeEnds>) {
    let (node_a, node_b) = (c(0, 0), c(10, 0));
    let (node_c, node_d) = (c(0, 10), c(30, 10));
    let (node_e, node_f) = (c(0, 14), c(30, 14));
    let (join, knot) = (c(8, 12), c(22, 12));
    let mut scene = SceneGraph::new();
    edge(&mut scene, e(0), EdgeRoute::Path(line(node_a, node_b)));
    let z = Layer::EdgesUnder;
    let style = StyleId(0);
    let c_j = scene.push_segment(z, style, line(node_c, join), vec![e(1)]);
    let trunk = scene.push_segment(z, style, line(join, knot), vec![e(1), e(2)]);
    let k_d = scene.push_segment(z, style, line(knot, node_d), vec![e(1)]);
    let elbow = Route::Orthogonal(vec![node_f, c(22, 14), knot]);
    let f_k = scene.push_segment(z, style, elbow, vec![e(2)]);
    let j_e = scene.push_segment(z, style, line(node_e, join), vec![e(2)]);
    edge(&mut scene, e(1), EdgeRoute::Chain(vec![c_j, trunk, k_d]));
    edge(&mut scene, e(2), EdgeRoute::Chain(vec![f_k, trunk, j_e]));
    let edges = vec![
        ends(e(0), node_a, node_b),
        ends(e(1), node_c, node_d),
        ends(e(2), node_f, node_e),
    ];
    (scene, edges)
}

#[test]
fn invariant_b_every_edge_has_one_connected_route() {
    let (scene, edges) = bundle();
    assert_eq!(route_faults(&scene, &edges), []);
}

#[test]
fn a_missing_duplicate_or_unexpected_route_is_a_fault() {
    let (mut scene, mut edges) = bundle();
    edges.push(ends(e(3), c(0, 0), c(1, 1)));
    edge(&mut scene, e(0), EdgeRoute::Path(line(c(0, 0), c(10, 0))));
    edge(&mut scene, e(9), EdgeRoute::Path(line(c(0, 0), c(1, 0))));
    let faults = route_faults(&scene, &edges);
    assert_eq!(faults.len(), 3);
    assert_eq!(
        faults,
        [
            (e(0), RouteFault::Duplicate),
            (e(3), RouteFault::Missing),
            (e(9), RouteFault::Unexpected),
        ]
    );
}

#[test]
fn a_path_must_run_from_the_source_anchor_to_the_target_anchor() {
    let mut scene = SceneGraph::new();
    edge(&mut scene, e(0), EdgeRoute::Path(line(c(10, 0), c(0, 0))));
    edge(&mut scene, e(1), EdgeRoute::Path(line(c(0, 0), c(9, 0))));
    edge(
        &mut scene,
        e(2),
        EdgeRoute::Path(Route::Polyline(Vec::new())),
    );
    let edges = [
        ends(e(0), c(0, 0), c(10, 0)),
        ends(e(1), c(0, 0), c(10, 0)),
        ends(e(2), c(0, 0), c(10, 0)),
    ];
    let faults = route_faults(&scene, &edges);
    assert_eq!(faults.len(), 3);
    assert!(
        faults
            .iter()
            .all(|&(_, fault)| fault == RouteFault::Disconnected),
        "{faults:?}"
    );
}

#[test]
fn a_chain_with_a_gap_or_the_wrong_end_is_disconnected() {
    let (mut scene, edges) = bundle();
    let mut gapped = scene.clone();
    // Edge 1 skips the trunk, so C→J is followed by K→D.
    let Some(SceneItem {
        payload:
            Payload::EdgePath {
                route: EdgeRoute::Chain(chain),
                ..
            },
        ..
    }) = gapped
        .items
        .iter_mut()
        .find(|item| item.edges() == [e(1)] && matches!(item.payload, Payload::EdgePath { .. }))
    else {
        panic!("edge 1 is chained");
    };
    chain.remove(1);
    let faults = route_faults(&gapped, &edges);
    assert!(
        faults.contains(&(e(1), RouteFault::Disconnected)),
        "{faults:?}"
    );

    let wrong_end = [edges[0], ends(e(1), c(0, 10), c(30, 11)), edges[2]];
    assert_eq!(
        route_faults(&scene, &wrong_end),
        [(e(1), RouteFault::Disconnected)]
    );

    edge(&mut scene, e(5), EdgeRoute::Chain(Vec::new()));
    let with_empty = [edges[0], edges[1], edges[2], ends(e(5), c(0, 0), c(1, 0))];
    assert_eq!(
        route_faults(&scene, &with_empty),
        [(e(5), RouteFault::Disconnected)]
    );
}

#[test]
fn segment_membership_must_match_the_chains() {
    let (scene, edges) = bundle();
    let mut items = scene.items.clone();
    // The trunk (segment 1) forgets edge 2 and claims edge 0, which draws its own path.
    for item in &mut items {
        if let Payload::Segment { members, .. } = &mut item.payload
            && members.len() == 2
        {
            *members = vec![e(0), e(1)];
        }
    }
    let mut altered = SceneGraph::new();
    for item in items {
        altered.push(item);
    }
    let faults = route_faults(&altered, &edges);
    assert_eq!(faults.len(), 2);
    assert!(
        faults.contains(&(e(0), RouteFault::StrayMember(SegmentId(1)))),
        "{faults:?}"
    );
    assert!(
        faults.contains(&(e(2), RouteFault::NotAMember(SegmentId(1)))),
        "{faults:?}"
    );
}

#[test]
fn a_chain_naming_no_segment_is_a_fault() {
    let mut scene = SceneGraph::new();
    edge(&mut scene, e(0), EdgeRoute::Chain(vec![SegmentId(4)]));
    let faults = route_faults(&scene, &[ends(e(0), c(0, 0), c(1, 0))]);
    assert_eq!(faults, [(e(0), RouteFault::UnknownSegment(SegmentId(4)))]);
}
