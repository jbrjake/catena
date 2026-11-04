//! Drawing a view: its layout as a scene, and that scene onto a surface (plan §7.4), with
//! invariants B, C and D on real scenes (plan §16.2).

use catena_testkit::fixtures::{FixtureGraph, SplitMix64, generated};
use insta::assert_snapshot;

use crate::geometry::cell::CellPt;
use crate::graph::{EdgeSpec, NodeSpec};
use crate::raster::{Blitter, CellGrid};
use crate::scene::{EdgeEnds, Layer, Payload, SceneGraph, SceneItem, route_faults};
use crate::{GraphView, GraphViewBuilder};

/// A view of `graph`, nodes added in the order `node_order` gives and edges in `edge_order`
/// (indices into the graph's lists), all in one commit.
fn view_of(
    builder: GraphViewBuilder<String>,
    graph: &FixtureGraph,
    node_order: &[usize],
    edge_order: &[usize],
) -> GraphView<String> {
    let mut gv = builder.build();
    gv.update(|tx| {
        for &i in node_order {
            let node = &graph.nodes[i];
            tx.add_node(node.key.clone(), NodeSpec::label(node.label.clone()))?;
        }
        for &e in edge_order {
            let edge = &graph.edges[e];
            tx.add_edge(&edge.source, &edge.target, EdgeSpec::directed())?;
        }
        Ok(())
    })
    .expect("generated graphs are consistent");
    gv
}

fn in_order(graph: &FixtureGraph) -> GraphView<String> {
    let nodes: Vec<usize> = (0..graph.nodes.len()).collect();
    let edges: Vec<usize> = (0..graph.edges.len()).collect();
    view_of(GraphView::builder(), graph, &nodes, &edges)
}

/// Every edge's ends as the view draws them.
fn ends(gv: &GraphView<String>) -> Vec<EdgeEnds> {
    gv.store
        .edges_in_order()
        .iter()
        .filter_map(|&ix| {
            let edge = gv.store.edge(ix)?;
            Some(EdgeEnds {
                ix,
                source: gv.cell(edge.from)?,
                target: gv.cell(edge.to)?,
            })
        })
        .collect()
}

#[test]
fn a_view_draws_each_node_once_and_a_line_for_each_edge() {
    let mut gv: GraphView<String> = GraphView::builder().blitter(Blitter::Ascii).build();
    gv.update(|tx| {
        tx.add_node("a".to_string(), NodeSpec::label("ada"))?;
        tx.add_node("b".to_string(), NodeSpec::label("babbage"))?;
        tx.add_node("c".to_string(), NodeSpec::label("lovelace"))?;
        tx.add_edge(&"a".into(), &"b".into(), EdgeSpec::directed())?;
        tx.add_edge(&"b".into(), &"c".into(), EdgeSpec::directed())?;
        Ok(())
    })
    .expect("fresh keys, known ends");
    let text = gv.render_to_string(60, 20);
    for label in ["ada", "babbage", "lovelace"] {
        assert_eq!(text.matches(label).count(), 1, "{label} in\n{text}");
    }
    let scene = gv.scene();
    let paths = scene
        .items()
        .iter()
        .filter(|item| matches!(item.payload, Payload::EdgePath { .. }))
        .count();
    let boxes = scene
        .items()
        .iter()
        .filter(|item| matches!(item.payload, Payload::NodeBox { .. }))
        .count();
    assert_eq!((boxes, paths), (3, 2));
    let lines = text.chars().filter(|c| "─│╱╲".contains(*c)).count();
    assert!(lines >= 4, "only {lines} line cells in\n{text}");
    assert_snapshot!("three_nodes_ascii", text);
}

#[test]
fn rendering_lays_out_for_the_surface_it_draws_on() {
    let mut gv = in_order(&generated(1, 30));
    let small = gv.render_to_string(40, 12);
    assert_eq!(gv.viewport.grid().cols, 40);
    let large = gv.render_to_string(120, 40);
    assert_eq!(
        (gv.viewport.grid().cols, gv.viewport.grid().rows),
        (120, 40)
    );
    assert_ne!(small, large);
    assert_eq!(large.lines().count(), 40, "a row per surface row");
}

#[test]
fn invariant_b_every_edge_has_one_route_from_its_source_anchor_to_its_target() {
    for seed in 0..6 {
        let graph = generated(seed, 60);
        let mut gv = in_order(&graph);
        gv.resize(120, 40);
        let scene = gv.scene();
        let ends = ends(&gv);
        assert_eq!(
            ends.len(),
            graph.edges.len(),
            "seed {seed}: every edge drawn"
        );
        assert_eq!(route_faults(&scene, &ends), [], "seed {seed}");
    }
}

/// Whether `cell` of `grid` shows anything.
fn shows(grid: &CellGrid, x: u16, y: u16) -> bool {
    grid.cell(x, y).is_some_and(|c| c.symbol() != " ")
}

#[test]
fn invariant_c_every_drawn_cell_lies_in_an_item_that_drew_it() {
    for seed in 0..4 {
        let mut gv = in_order(&generated(seed, 40));
        let mut grid = CellGrid::new(100, 30);
        gv.render(&mut grid);
        let scene = gv.scene();
        let mut lit = 0;
        for y in 0..30 {
            for x in 0..100 {
                if shows(&grid, x, y) {
                    lit += 1;
                    let at = CellPt::new(i32::from(x), i32::from(y));
                    assert!(
                        scene.items().iter().any(|item| item.bounds.contains(at)),
                        "seed {seed}: ({x}, {y}) is drawn but in no item's bounds"
                    );
                }
            }
        }
        assert!(lit > 100, "seed {seed}: only {lit} cells drawn");

        // And each item alone draws only inside its own bounds.
        for item in scene.items() {
            let mut alone = SceneGraph::new();
            alone.push(item.clone());
            let mut grid = CellGrid::new(100, 30);
            let metrics = &gv.metrics;
            crate::scene::Compositor::new().render(
                &alone,
                &mut grid,
                &gv.render_options,
                crate::draw::style,
                |ix| metrics.form(ix),
            );
            for y in 0..30 {
                for x in 0..100 {
                    let at = CellPt::new(i32::from(x), i32::from(y));
                    assert!(
                        !shows(&grid, x, y) || item.bounds.contains(at),
                        "seed {seed}: {:?} drew ({x}, {y}) outside {:?}",
                        item.payload,
                        item.bounds
                    );
                }
            }
        }
    }
}

#[test]
fn a_node_items_bounds_are_its_measured_box_where_it_is_drawn() {
    // Plan §7.4's completeness rule (ledger row 2): rendered box == scene bounds == reserved
    // box. The compositor clips a node's text to its item's bounds, so bounds a column short
    // would cut its label without drawing outside them.
    let graph = generated(3, 40);
    let gv = in_order(&graph);
    let scene = gv.scene();
    let mut checked = 0;
    for item in scene.items() {
        if let Payload::NodeBox { ix } = item.payload {
            let form = gv.metrics.form(ix).expect("measured");
            let anchor = gv.cell(ix).expect("placed");
            assert_eq!(
                (item.bounds.width, item.bounds.height),
                (u32::from(form.width()), u32::from(form.height())),
                "{:?}",
                gv.key(ix)
            );
            assert_eq!(
                CellPt::new(item.bounds.x, item.bounds.y),
                CellPt::new(anchor.x - form.anchor().x, anchor.y - form.anchor().y),
                "{:?}: the box sits around its anchor",
                gv.key(ix)
            );
            checked += 1;
        }
    }
    assert_eq!(checked, graph.nodes.len());
}

#[test]
fn nodes_draw_over_edges() {
    let gv = in_order(&generated(2, 20));
    let scene = gv.scene();
    let layers: Vec<Layer> = scene
        .in_draw_order()
        .map(|item: &SceneItem| item.z)
        .collect();
    let first_node = layers.iter().position(|&z| z == Layer::Nodes);
    let last_edge = layers.iter().rposition(|&z| z == Layer::EdgesUnder);
    assert!(first_node.is_some() && last_edge.is_some());
    assert!(last_edge < first_node, "edges draw under nodes");
}

/// `0..n` in an order `rng` shuffles.
fn shuffled(n: usize, rng: &mut SplitMix64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let j = usize::try_from(rng.below(u64::try_from(i + 1).expect("small"))).expect("small");
        order.swap(i, j);
    }
    order
}

#[test]
fn invariant_d_any_insertion_order_draws_the_same_frame() {
    // Parallel edges keep their insertion rank by contract (plan §4.1), so the shuffle moves
    // whole groups of edges between the same pair of nodes and keeps each group's order.
    for seed in 0..4 {
        let graph = generated(seed, 50);
        let mut rng = SplitMix64::new(seed ^ 0xd);
        let nodes = shuffled(graph.nodes.len(), &mut rng);
        let mut pairs: Vec<(&str, &str)> = Vec::new();
        for edge in &graph.edges {
            let pair = (edge.source.as_str(), edge.target.as_str());
            if !pairs.contains(&pair) {
                pairs.push(pair);
            }
        }
        let mut edges = Vec::new();
        for p in shuffled(pairs.len(), &mut rng) {
            let (source, target) = pairs[p];
            edges.extend(
                (0..graph.edges.len()).filter(|&e| {
                    graph.edges[e].source == source && graph.edges[e].target == target
                }),
            );
        }
        assert_eq!(edges.len(), graph.edges.len());
        let mut shuffled_view = view_of(GraphView::builder(), &graph, &nodes, &edges);
        let mut ordered_view = in_order(&graph);
        assert_eq!(
            shuffled_view.render_to_string(100, 30),
            ordered_view.render_to_string(100, 30),
            "seed {seed}"
        );
    }
}
