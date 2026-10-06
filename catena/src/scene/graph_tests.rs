use super::*;

fn e(index: u32) -> EdgeIx {
    EdgeIx::new(index)
}

fn line(points: &[(f64, f64)]) -> Route {
    Route::Polyline(points.iter().map(|&(x, y)| SubPt::new(x, y)).collect())
}

fn edge_item(ix: EdgeIx, route: Route, z: Layer) -> SceneItem {
    let bounds = route.bounds().expect("points");
    SceneItem::new(
        z,
        bounds,
        Payload::EdgePath {
            ix,
            route: EdgeRoute::Path(route),
        },
        StyleId(0),
    )
}

#[test]
fn routes_report_their_end_cells_and_bounds() {
    let polyline = line(&[(0.4, 1.6), (5.0, 0.0), (2.5, 3.2)]);
    assert_eq!(
        polyline.ends(),
        Some((CellPt::new(0, 2), CellPt::new(3, 3)))
    );
    assert_eq!(polyline.bounds(), Some(CellBox::new(0, 0, 6, 4)));
    let orthogonal = Route::Orthogonal(vec![
        CellPt::new(1, 1),
        CellPt::new(1, 4),
        CellPt::new(-2, 4),
    ]);
    assert_eq!(
        orthogonal.ends(),
        Some((CellPt::new(1, 1), CellPt::new(-2, 4)))
    );
    assert_eq!(orthogonal.bounds(), Some(CellBox::new(-2, 1, 4, 4)));
    assert_eq!(Route::Polyline(Vec::new()).ends(), None);
    assert_eq!(Route::Orthogonal(Vec::new()).bounds(), None);
}

#[test]
fn route_distance_is_to_the_nearest_segment() {
    let elbow = Route::Orthogonal(vec![
        CellPt::new(0, 0),
        CellPt::new(4, 0),
        CellPt::new(4, 3),
    ]);
    assert!((elbow.distance_to(SubPt::new(2.0, 1.0)) - 1.0).abs() < 1e-12);
    assert!((elbow.distance_to(SubPt::new(5.5, 2.0)) - 1.5).abs() < 1e-12);
    assert!(
        (elbow.distance_to(SubPt::new(-3.0, -4.0)) - 5.0).abs() < 1e-12,
        "past an end"
    );
    let dot = line(&[(1.0, 1.0)]);
    assert!((dot.distance_to(SubPt::new(4.0, 5.0)) - 5.0).abs() < 1e-12);
    assert_eq!(line(&[]).distance_to(SubPt::new(0.0, 0.0)), f64::INFINITY);
}

#[test]
fn shared_segment_hit_returns_its_members() {
    let mut scene = SceneGraph::new();
    let trunk = scene.push_segment(
        Layer::EdgesUnder,
        StyleId(1),
        line(&[(0.0, 5.0), (20.0, 5.0)]),
        vec![e(7), e(2), e(7), e(4)],
    );
    let Some(SceneItem {
        payload: Payload::Segment { id, members, .. },
        bounds,
        ..
    }) = scene.segment(trunk)
    else {
        panic!("segment {trunk:?} has no segment item");
    };
    assert_eq!(*id, trunk);
    assert_eq!(members.len(), 3);
    assert_eq!(members, &[e(2), e(4), e(7)], "sorted, each once");
    assert_eq!(*bounds, CellBox::new(0, 5, 21, 1));

    let hit = scene.edges_at(SubPt::new(10.0, 6.0), EDGE_HIT_CELLS);
    assert_eq!(hit.len(), 3);
    assert_eq!(hit, &[e(2), e(4), e(7)]);
    assert!(
        scene
            .edges_at(SubPt::new(10.0, 7.0), EDGE_HIT_CELLS)
            .is_empty(),
        "2 cells off"
    );
}

#[test]
fn the_topmost_layer_then_the_smallest_bounds_wins_a_hit() {
    let mut scene = SceneGraph::new();
    scene.push(edge_item(
        e(1),
        line(&[(0.0, 0.0), (10.0, 0.0)]),
        Layer::EdgesOver,
    ));
    scene.push(edge_item(
        e(2),
        line(&[(0.0, 0.0), (30.0, 0.0)]),
        Layer::EdgesUnder,
    ));
    scene.push(edge_item(
        e(3),
        line(&[(0.0, 0.0), (20.0, 0.0)]),
        Layer::EdgesOver,
    ));
    assert_eq!(scene.edges_at(SubPt::new(5.0, 0.0), 1.5), &[e(1)]);
    assert_eq!(scene.edges_at(SubPt::new(15.0, 0.0), 1.5), &[e(3)]);
    assert_eq!(scene.edges_at(SubPt::new(25.0, 0.0), 1.5), &[e(2)]);
    let mut tie = SceneGraph::new();
    tie.push(edge_item(
        e(5),
        line(&[(0.0, 0.0), (9.0, 0.0)]),
        Layer::EdgesUnder,
    ));
    tie.push(edge_item(
        e(4),
        line(&[(0.0, 1.0), (9.0, 1.0)]),
        Layer::EdgesUnder,
    ));
    assert_eq!(
        tie.edges_at(SubPt::new(3.0, 0.5), 1.5),
        &[e(5)],
        "the earlier item"
    );
}

#[test]
fn a_chained_edge_is_hit_through_its_segments_not_itself() {
    let mut scene = SceneGraph::new();
    let s = scene.push_segment(
        Layer::EdgesUnder,
        StyleId(0),
        line(&[(0.0, 0.0), (8.0, 0.0)]),
        vec![e(1)],
    );
    scene.push(SceneItem::new(
        Layer::EdgesOver,
        CellBox::new(0, 0, 9, 1),
        Payload::EdgePath {
            ix: e(1),
            route: EdgeRoute::Chain(vec![s]),
        },
        StyleId(0),
    ));
    assert_eq!(scene.edges_at(SubPt::new(4.0, 0.0), 1.5), &[e(1)]);
    assert_eq!(scene.items()[1].edges(), &[e(1)]);
}

#[test]
fn only_edges_and_segments_stand_for_edges() {
    let node = SceneItem::new(
        Layer::Nodes,
        CellBox::new(0, 0, 5, 1),
        Payload::NodeBox { ix: NodeIx::new(3) },
        StyleId(0),
    );
    assert_eq!(node.edges(), []);
    let glow = SceneItem::new(
        Layer::Glow,
        CellBox::new(0, 0, 5, 1),
        Payload::Decoration(Decoration::Glow),
        StyleId(0),
    );
    assert_eq!(glow.edges(), []);
    let mut scene = SceneGraph::new();
    scene.push(node);
    scene.push(glow);
    assert!(
        scene
            .edges_at(SubPt::new(1.0, 0.0), EDGE_HIT_CELLS)
            .is_empty(),
        "boxes are not routes"
    );
}

#[test]
fn draw_order_is_by_layer_then_insertion() {
    let mut scene = SceneGraph::new();
    for (index, z) in [
        Layer::Labels,
        Layer::EdgesUnder,
        Layer::Nodes,
        Layer::EdgesUnder,
    ]
    .into_iter()
    .enumerate()
    {
        let ix = e(u32::try_from(index).expect("small"));
        scene.push(edge_item(ix, line(&[(0.0, 0.0)]), z));
    }
    let order: Vec<EdgeIx> = scene.in_draw_order().map(|item| item.edges()[0]).collect();
    assert_eq!(order, [e(1), e(3), e(2), e(0)]);
}

#[test]
fn pushing_a_segment_item_assigns_the_next_id() {
    let mut scene = SceneGraph::new();
    let first = scene.push_segment(Layer::EdgesUnder, StyleId(0), line(&[(0.0, 0.0)]), vec![]);
    let stale = SceneItem::new(
        Layer::EdgesUnder,
        CellBox::default(),
        Payload::Segment {
            id: first,
            route: line(&[(1.0, 1.0)]),
            members: vec![e(9)],
        },
        StyleId(0),
    );
    scene.push(stale);
    let Some(Payload::Segment { id, .. }) = scene.items().last().map(|item| &item.payload) else {
        panic!("the last item is a segment");
    };
    assert_ne!(*id, first);
    assert_eq!(scene.segment(*id).map(SceneItem::edges), Some(&[e(9)][..]));
    scene.clear();
    assert_eq!(scene.items(), []);
    assert_eq!(scene.segment(first), None);
}
