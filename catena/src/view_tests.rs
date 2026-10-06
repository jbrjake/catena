use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;
use crate::graph::{DeltaClass, EdgeSpec, NodeSpec};

fn two_nodes() -> GraphView<String> {
    let mut gv = GraphView::new();
    gv.update(|tx| {
        tx.add_node("ada".to_string(), NodeSpec::label("Ada Lovelace"))?;
        tx.add_node("babbage".to_string(), NodeSpec::label("Charles Babbage"))?;
        Ok(())
    })
    .expect("fresh keys");
    gv
}

#[test]
fn update_returns_the_closures_value_and_commits() {
    let mut gv = two_nodes();
    let id = gv
        .update(|tx| tx.add_edge(&"ada".into(), &"babbage".into(), EdgeSpec::directed()))
        .expect("known ends");
    assert_eq!((gv.node_count(), gv.edge_count()), (2, 1));
    let edge = gv.edge(id).expect("live");
    assert_eq!(
        (edge.id(), edge.from().as_str(), edge.to().as_str()),
        (id, "ada", "babbage")
    );
    assert_eq!(edge.rank(), 0);
    assert_eq!(edge.spec(), &EdgeSpec::directed());
}

#[test]
fn a_failing_update_changes_nothing_and_returns_the_error() {
    let mut gv = two_nodes();
    let err = gv.update(|tx| {
        tx.remove_node(&"ada".into())?;
        tx.add_node("babbage".to_string(), NodeSpec::label("again"))
    });
    assert_eq!(err, Err(GraphError::DuplicateNode("babbage".to_string())));
    assert_eq!(gv.node_count(), 2);
    assert!(gv.node_ix(&"ada".into()).is_some());
}

#[test]
fn a_panicking_update_changes_nothing() {
    let mut gv = two_nodes();
    let pending = gv.pending.clone();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        gv.update(|tx| -> Result<(), _> {
            tx.remove_node(&"ada".into())?;
            panic!("the host's closure panics mid-transaction");
        })
    }));
    assert!(outcome.is_err());
    assert_eq!(gv.node_count(), 2);
    assert_eq!(gv.pending, pending);
}

#[test]
fn indices_map_back_to_keys_until_their_node_leaves() {
    let mut gv = two_nodes();
    let ada = gv.node_ix(&"ada".into()).expect("live");
    assert_eq!(gv.key(ada).map(String::as_str), Some("ada"));
    gv.update(|tx| tx.remove_node(&"ada".into()))
        .expect("known");
    assert_eq!(gv.key(ada), None);
    assert_eq!(gv.node_ix(&"ada".into()), None);
}

#[test]
fn the_builder_sets_the_label_ceiling() {
    let mut gv: GraphView<u64> = GraphView::builder().max_label_cols(3).build();
    gv.update(|tx| tx.add_node(7, NodeSpec::label("seven")))
        .expect("fresh");
    assert_eq!(
        gv.store
            .node(gv.node_ix(&7).expect("live"))
            .expect("live")
            .spec
            .label,
        "sev"
    );
    let default: GraphView<u64> = GraphView::new();
    assert_eq!(default.store.max_label_cols, DEFAULT_MAX_LABEL_COLS);
}

#[test]
fn commits_accumulate_until_the_layout_catches_up() {
    let mut gv = two_nodes();
    assert_eq!(gv.pending.class, Some(DeltaClass::Topology));
    assert_eq!(gv.pending.added.len(), 2);
    gv.update(|tx| tx.set_node(&"ada".into(), |n| n.weight = 2.0))
        .expect("known");
    assert_eq!(
        gv.pending.class,
        Some(DeltaClass::Topology),
        "absorbed, not replaced"
    );
}
