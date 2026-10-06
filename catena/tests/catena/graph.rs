//! The graph store through the public API, as a host uses it (plan §4.2, §13).

use catena::graph::{EdgeClass, NodeShape};
use catena::{EdgeSpec, GraphError, GraphView, NodeSpec};

#[test]
fn the_plan_13_snippet_builds_a_graph() -> Result<(), GraphError<String>> {
    let mut gv: GraphView<String> = GraphView::builder().build();
    let id = gv.update(|tx| {
        tx.add_node("ada".to_string(), NodeSpec::label("Ada Lovelace"))?;
        tx.add_node("babbage".to_string(), NodeSpec::label("Charles Babbage"))?;
        tx.add_edge(&"ada".into(), &"babbage".into(), EdgeSpec::directed())
    })?;
    assert_eq!((gv.node_count(), gv.edge_count()), (2, 1));
    let edge = gv.edge(id).expect("the edge just added");
    assert_eq!(
        (edge.from().as_str(), edge.to().as_str()),
        ("ada", "babbage")
    );
    let ada = gv.node_ix(&"ada".into()).expect("added");
    assert_eq!(gv.key(ada).map(String::as_str), Some("ada"));
    Ok(())
}

#[test]
fn a_host_edits_specs_through_their_public_fields() -> Result<(), GraphError<u32>> {
    let mut gv: GraphView<u32> = GraphView::new();
    let mut spec = NodeSpec::label("hub");
    spec.sort_key = Some("0".into());
    spec.shape = NodeShape::Box { min_w: 6, min_h: 3 };
    let mut dashed = EdgeSpec::undirected();
    dashed.class = EdgeClass::Dashed;
    dashed.layout_participating = false;
    let id = gv.update(|tx| {
        tx.add_node(1, spec)?;
        tx.add_node(2, NodeSpec::default())?;
        tx.add_edge(&1, &2, dashed)
    })?;
    gv.update(|tx| tx.set_edge(id, |e| e.weight = 2.5))?;
    let edge = gv.edge(id).expect("live");
    assert_eq!(edge.spec().class, EdgeClass::Dashed);
    assert_eq!(edge.spec().weight.to_bits(), 2.5f32.to_bits());
    Ok(())
}

#[test]
fn an_error_inside_the_closure_discards_the_whole_transaction() {
    let mut gv: GraphView<&str> = GraphView::new();
    let result = gv.update(|tx| {
        tx.add_node("a", NodeSpec::label("a"))?;
        tx.add_edge(&"a", &"missing", EdgeSpec::directed())
    });
    assert_eq!(result, Err(GraphError::UnknownNode("missing")));
    assert_eq!(gv.node_count(), 0);
    assert_eq!(
        GraphError::<&str>::UnknownNode("missing").to_string(),
        "no node \"missing\""
    );
}
