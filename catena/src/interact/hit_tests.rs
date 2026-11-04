use super::*;
use crate::scene::{Layer, SceneItem, StyleId};

const FRAME: (u16, u16) = (80, 30);

/// A node box from `(x, y)` of `width × 1`, anchored at its middle column.
fn node(scene: &mut SceneGraph, ix: u32, x: i32, y: i32, width: u32) {
    scene.push(SceneItem::new(
        Layer::Nodes,
        CellBox::new(x, y, width, 1),
        Payload::NodeBox {
            ix: NodeIx::new(ix),
        },
        StyleId(0),
    ));
}

/// A straight edge between two cells.
fn edge(scene: &mut SceneGraph, ix: u32, from: (i32, i32), to: (i32, i32)) {
    let route = Route::Polyline(vec![
        CellPt::new(from.0, from.1).into(),
        CellPt::new(to.0, to.1).into(),
    ]);
    let bounds = route.bounds().expect("two points");
    scene.push(SceneItem::new(
        Layer::EdgesUnder,
        bounds,
        Payload::EdgePath {
            ix: EdgeIx::new(ix),
            route: EdgeRoute::Path(route),
        },
        StyleId(1),
    ));
}

/// A hit map of `scene`, each node anchored at its box's middle.
fn map(scene: &SceneGraph) -> HitMap {
    let anchors: Vec<(NodeIx, CellPt)> = scene
        .items()
        .iter()
        .filter_map(|item| match item.payload {
            Payload::NodeBox { ix } => {
                let b = item.bounds;
                let mid = i32::try_from(b.width / 2).expect("small");
                Some((ix, CellPt::new(b.x + mid, b.y)))
            }
            _ => None,
        })
        .collect();
    let mut map = HitMap::default();
    map.rebuild(scene, FRAME, 0.5, |ix| {
        anchors.iter().find(|(n, _)| *n == ix).map(|&(_, a)| a)
    });
    map
}

fn at(map: &HitMap, scene: &SceneGraph, x: u16, y: u16) -> Option<Hit<'static>> {
    // Owned copy of the hit, for comparing in asserts.
    map.hit(scene, (x, y)).map(|hit| match hit {
        Hit::Node(ix) => Hit::Node(ix),
        Hit::Edges(edges) => Hit::Edges(Box::leak(edges.to_vec().into_boxed_slice())),
    })
}

#[test]
fn a_point_in_a_box_hits_its_node() {
    let mut scene = SceneGraph::new();
    node(&mut scene, 0, 8, 10, 5);
    let map = map(&scene);
    for x in 8..13 {
        assert_eq!(at(&map, &scene, x, 10), Some(Hit::Node(NodeIx::new(0))));
    }
}

#[test]
fn a_miss_reaches_three_aspect_corrected_cells_from_the_anchor() {
    // The box spans columns 8..13 of row 10, anchored at column 10. Across, three cells from
    // the anchor still hit (column 13 lies past the box); four miss. Down, a row counts as two
    // cells at the default aspect: one row below hits, two miss. The seed's raw five-cell
    // radius would have taken all of these.
    let mut scene = SceneGraph::new();
    node(&mut scene, 0, 8, 10, 5);
    let map = map(&scene);
    let n0 = Some(Hit::Node(NodeIx::new(0)));
    assert_eq!(at(&map, &scene, 13, 10), n0);
    assert_eq!(at(&map, &scene, 14, 10), None);
    assert_eq!(at(&map, &scene, 10, 11), n0);
    assert_eq!(at(&map, &scene, 10, 12), None);
    assert_eq!(at(&map, &scene, 10, 9), n0);
    assert_eq!(at(&map, &scene, 10, 8), None);
}

#[test]
fn the_nearer_anchor_wins_a_miss_and_an_equal_one_goes_to_the_earlier_node() {
    let mut scene = SceneGraph::new();
    node(&mut scene, 0, 0, 10, 3); // columns 0..3, anchor (1, 10)
    node(&mut scene, 1, 6, 10, 3); // columns 6..9, anchor (7, 10)
    let map = map(&scene);
    assert_eq!(
        at(&map, &scene, 3, 10),
        Some(Hit::Node(NodeIx::new(0))),
        "2 against 4"
    );
    assert_eq!(
        at(&map, &scene, 5, 10),
        Some(Hit::Node(NodeIx::new(1))),
        "4 against 2"
    );
    assert_eq!(
        at(&map, &scene, 4, 10),
        Some(Hit::Node(NodeIx::new(0))),
        "3 and 3: a tie"
    );
}

#[test]
fn overlapping_boxes_go_to_the_smaller_then_the_earlier() {
    let mut scene = SceneGraph::new();
    node(&mut scene, 0, 0, 5, 20);
    node(&mut scene, 1, 5, 5, 4);
    node(&mut scene, 2, 5, 5, 4);
    let map = map(&scene);
    assert_eq!(at(&map, &scene, 6, 5), Some(Hit::Node(NodeIx::new(1))));
    assert_eq!(at(&map, &scene, 15, 5), Some(Hit::Node(NodeIx::new(0))));
}

#[test]
fn an_edge_hits_within_a_cell_and_a_half_of_its_route() {
    let mut scene = SceneGraph::new();
    edge(&mut scene, 4, (10, 20), (60, 20));
    let map = map(&scene);
    let e4: &[EdgeIx] = &[EdgeIx::new(4)];
    assert_eq!(at(&map, &scene, 30, 20), Some(Hit::Edges(e4)));
    assert_eq!(at(&map, &scene, 30, 21), Some(Hit::Edges(e4)));
    assert_eq!(at(&map, &scene, 30, 22), None, "two cells off");
    assert_eq!(
        at(&map, &scene, 61, 20),
        Some(Hit::Edges(e4)),
        "past its end, within reach"
    );
    assert_eq!(at(&map, &scene, 63, 20), None);
}

#[test]
fn a_node_wins_over_an_edge_under_it_and_near_it() {
    let mut scene = SceneGraph::new();
    edge(&mut scene, 0, (0, 10), (40, 10));
    node(&mut scene, 7, 18, 10, 5);
    let map = map(&scene);
    assert_eq!(
        at(&map, &scene, 19, 10),
        Some(Hit::Node(NodeIx::new(7))),
        "in the box"
    );
    assert_eq!(
        at(&map, &scene, 23, 10),
        Some(Hit::Node(NodeIx::new(7))),
        "near the anchor"
    );
    assert_eq!(
        at(&map, &scene, 30, 10),
        Some(Hit::Edges(&[EdgeIx::new(0)])),
        "past reach"
    );
}

#[test]
fn the_topmost_then_smallest_edge_wins() {
    let mut scene = SceneGraph::new();
    edge(&mut scene, 0, (0, 5), (70, 5));
    edge(&mut scene, 1, (20, 5), (30, 5));
    let over = Route::Polyline(vec![CellPt::new(0, 6).into(), CellPt::new(70, 6).into()]);
    scene.push(SceneItem::new(
        Layer::EdgesOver,
        over.bounds().expect("points"),
        Payload::EdgePath {
            ix: EdgeIx::new(2),
            route: EdgeRoute::Path(over),
        },
        StyleId(1),
    ));
    let map = map(&scene);
    assert_eq!(
        at(&map, &scene, 25, 5),
        Some(Hit::Edges(&[EdgeIx::new(2)])),
        "on top"
    );
    assert_eq!(
        at(&map, &scene, 25, 4),
        Some(Hit::Edges(&[EdgeIx::new(1)])),
        "smaller"
    );
    assert_eq!(at(&map, &scene, 5, 4), Some(Hit::Edges(&[EdgeIx::new(0)])));
}

#[test]
fn a_shared_segment_hits_as_all_its_member_edges() {
    let mut scene = SceneGraph::new();
    let trunk = Route::Polyline(vec![CellPt::new(10, 3).into(), CellPt::new(50, 3).into()]);
    scene.push_segment(
        Layer::EdgesUnder,
        StyleId(1),
        trunk,
        vec![EdgeIx::new(5), EdgeIx::new(2)],
    );
    let map = map(&scene);
    assert_eq!(
        at(&map, &scene, 30, 3),
        Some(Hit::Edges(&[EdgeIx::new(2), EdgeIx::new(5)]))
    );
}

#[test]
fn every_hit_agrees_with_a_scan_of_every_item() {
    // The grid only prunes: against a brute-force oracle over every item, over every cell.
    let mut scene = SceneGraph::new();
    for i in 0..12u32 {
        let x = i32::try_from(i * 6 % 70).expect("small");
        let y = i32::try_from(i * 7 % 28).expect("small");
        node(&mut scene, i, x, y, 4 + i % 5);
        edge(&mut scene, i, (x, y), (70 - x, 28 - y));
    }
    let map = map(&scene);
    // The oracle: every item listed in every bucket, so the grid prunes nothing.
    let mut everything = HitMap {
        cell_aspect: 0.5,
        anchors: map.anchors.clone(),
        ..HitMap::default()
    };
    let all: Vec<(u32, CellBox)> = (0..u32::try_from(scene.items().len()).expect("small"))
        .map(|i| {
            (
                i,
                CellBox::new(0, 0, u32::from(FRAME.0), u32::from(FRAME.1)),
            )
        })
        .collect();
    everything.grid.rebuild(FRAME, all);
    for y in 0..FRAME.1 {
        for x in 0..FRAME.0 {
            assert_eq!(
                map.hit(&scene, (x, y)),
                everything.hit(&scene, (x, y)),
                "({x}, {y})"
            );
        }
    }
}
