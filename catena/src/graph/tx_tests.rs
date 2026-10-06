use std::collections::BTreeSet;

use super::super::testing::{Res, Store, add_all, store, with_nodes};
use super::super::{EdgeClass, Limits, NodeShape};
use super::*;

fn set(ixs: &[NodeIx]) -> BTreeSet<NodeIx> {
    ixs.iter().copied().collect()
}

/// The class of the one commit `f` makes.
fn class_of(
    s: &mut Store,
    f: impl FnOnce(&mut Tx<'_, &'static str>) -> Res<()>,
) -> Option<DeltaClass> {
    s.transact(f).expect("valid operations").1.class
}

#[test]
fn add_node_interns_the_key() {
    let mut s = store();
    let (ix, delta) = s
        .transact(|tx| tx.add_node("ada", NodeSpec::label("Ada")))
        .expect("fresh");
    assert_eq!(s.key(ix), Some(&"ada"));
    assert_eq!(s.ix_of(&"ada"), Some(ix));
    assert_eq!(s.node(ix).expect("live").spec.label, "Ada");
    assert_eq!(delta.class, Some(DeltaClass::Topology));
    assert_eq!(delta.added, set(&[ix]));
}

#[test]
fn a_duplicate_key_fails_whether_committed_or_buffered() {
    let mut s = with_nodes(&["a"]);
    let before = s.clone();
    let err = s.transact(|tx| tx.add_node("a", NodeSpec::label("again")));
    assert_eq!(err.map(|_| ()), Err(GraphError::DuplicateNode("a")));
    let err = s.transact(|tx| add_all(tx, &["b", "c", "b"]));
    assert_eq!(err.map(|_| ()), Err(GraphError::DuplicateNode("b")));
    assert_eq!(s, before, "a failed transaction commits nothing");
}

#[test]
fn a_failed_transaction_leaves_every_slot_id_and_order_as_it_was() {
    let mut s = with_nodes(&["a", "b", "c"]);
    s.transact(|tx| tx.add_edge(&"a", &"b", EdgeSpec::directed()))
        .expect("known ends");
    let before = s.clone();
    let err = s.transact(|tx| {
        add_all(tx, &["x", "y"])?;
        tx.add_edge(&"x", &"a", EdgeSpec::undirected())?;
        tx.remove_node(&"b")?;
        tx.set_node(&"c", |spec| spec.label = "renamed".into())?;
        tx.remove_node(&"nobody")
    });
    assert_eq!(err.map(|_| ()), Err(GraphError::UnknownNode("nobody")));
    assert_eq!(s, before);
}

#[test]
fn a_failed_operation_buffers_nothing() {
    let mut s = with_nodes(&["a"]);
    s.transact(|tx| {
        tx.add_node("b", NodeSpec::label("b"))?;
        assert!(tx.add_node("b", NodeSpec::label("twice")).is_err());
        assert!(tx.add_edge(&"a", &"nobody", EdgeSpec::directed()).is_err());
        assert!(tx.remove_edge(EdgeId(0)).is_err());
        Ok(())
    })
    .expect("the closure's Ok commits");
    assert_eq!((s.node_count(), s.edge_count(), s.next_edge_id), (2, 0, 0));
    assert_eq!(
        s.node(s.ix_of(&"b").expect("live"))
            .expect("live")
            .spec
            .label,
        "b"
    );
}

#[test]
fn an_edge_to_an_unknown_node_names_the_missing_end_source_first() {
    let mut s = with_nodes(&["a"]);
    let err = |s: &mut Store, from, to| {
        s.transact(|tx| tx.add_edge(&from, &to, EdgeSpec::directed()))
            .map(|_| ())
    };
    assert_eq!(err(&mut s, "x", "y"), Err(GraphError::UnknownNode("x")));
    assert_eq!(err(&mut s, "a", "y"), Err(GraphError::UnknownNode("y")));
}

#[test]
fn removing_a_node_removes_its_edges_committed_and_buffered() {
    let mut s = with_nodes(&["a", "b", "c"]);
    let (ids, _) = s
        .transact(|tx| {
            let ab = tx.add_edge(&"a", &"b", EdgeSpec::directed())?;
            let bc = tx.add_edge(&"b", &"c", EdgeSpec::directed())?;
            let aa = tx.add_edge(&"a", &"a", EdgeSpec::directed())?;
            Ok([ab, bc, aa])
        })
        .expect("known ends");
    let ((), delta) = s.transact(|tx| tx.remove_node(&"a")).expect("known");
    assert_eq!(s.edge_count(), 1);
    assert_eq!(
        [ids[0], ids[1], ids[2]].map(|id| s.edge_ix(id).is_some()),
        [false, true, false]
    );
    assert_eq!(s.in_edges(s.ix_of(&"b").expect("live")).len(), 0);
    assert_eq!(
        (delta.class, delta.removed.len()),
        (Some(DeltaClass::Topology), 1)
    );

    s.transact(|tx| {
        tx.add_node("d", NodeSpec::label("d"))?;
        tx.add_edge(&"d", &"b", EdgeSpec::directed())?;
        tx.add_edge(&"b", &"d", EdgeSpec::directed())?;
        tx.remove_node(&"b")?;
        assert!(!tx.contains_edge(ids[1]), "the committed edge went too");
        Ok(())
    })
    .expect("known keys");
    assert_eq!((s.node_count(), s.edge_count()), (2, 0));
    assert_eq!(
        s.next_edge_id, 5,
        "ids of edges that never committed are still spent"
    );
}

#[test]
fn an_edge_id_is_never_reused_and_a_stale_one_is_unknown() {
    let mut s = with_nodes(&["a", "b"]);
    let first = s
        .transact(|tx| tx.add_edge(&"a", &"b", EdgeSpec::directed()))
        .expect("ends")
        .0;
    s.transact(|tx| tx.remove_edge(first)).expect("live");
    let second = s
        .transact(|tx| tx.add_edge(&"a", &"b", EdgeSpec::directed()))
        .expect("ends")
        .0;
    assert_ne!(first, second);
    assert_eq!(s.edge_ix(first), None);
    let stale = s.transact(|tx| tx.remove_edge(first)).map(|_| ());
    assert_eq!(stale, Err(GraphError::UnknownEdge(first)));
    let stale = s
        .transact(|tx| tx.set_edge(first, |spec| spec.weight = 2.0))
        .map(|_| ());
    assert_eq!(stale, Err(GraphError::UnknownEdge(first)));
    let twice = s.transact(|tx| {
        tx.remove_edge(second)?;
        tx.remove_edge(second)
    });
    assert_eq!(twice.map(|_| ()), Err(GraphError::UnknownEdge(second)));
}

#[test]
fn a_key_removed_and_added_again_in_one_transaction_gets_a_new_slot() {
    let mut s = with_nodes(&["a", "b"]);
    let old = s.ix_of(&"a").expect("live");
    s.transact(|tx| tx.add_edge(&"a", &"b", EdgeSpec::directed()))
        .expect("ends");
    let (new, delta) = s
        .transact(|tx| {
            tx.remove_node(&"a")?;
            assert!(!tx.contains_node(&"a"));
            let new = tx.add_node("a", NodeSpec::label("again"))?;
            assert!(tx.contains_node(&"a"));
            Ok(new)
        })
        .expect("known, then fresh");
    assert_ne!(new, old);
    assert_eq!(s.ix_of(&"a"), Some(new));
    assert_eq!(s.key(old), None);
    assert_eq!(s.edge_count(), 0, "the old node's edges left with it");
    assert_eq!((delta.added, delta.removed), (set(&[new]), set(&[old])));
}

#[test]
fn a_key_added_again_into_a_lower_slot_than_it_left_keeps_its_lookup() {
    let mut s = with_nodes(&["a", "b", "c"]);
    s.transact(|tx| tx.remove_node(&"a")).expect("known");
    let new = s
        .transact(|tx| {
            tx.remove_node(&"c")?;
            tx.add_node("c", NodeSpec::label("c"))
        })
        .expect("known, then fresh")
        .0;
    assert_eq!(new.slot(), 0, "the vacancy a left, below c's old slot 2");
    assert_eq!(s.ix_of(&"c"), Some(new));
    assert_eq!(s.key(new), Some(&"c"));
}

#[test]
fn node_and_edge_slots_have_a_ceiling() {
    let mut s = GraphStore::new(256, Limits { nodes: 2, edges: 2 });
    let full = s.transact(|tx| add_all(tx, &["a", "b", "c"]));
    assert_eq!(full.map(|_| ()), Err(GraphError::Capacity("nodes")));
    s.transact(|tx| add_all(tx, &["a", "b"]))
        .expect("room for two");
    let full = s.transact(|tx| {
        for _ in 0..3 {
            tx.add_edge(&"a", &"b", EdgeSpec::directed())?;
        }
        Ok(())
    });
    assert_eq!(full.map(|_| ()), Err(GraphError::Capacity("edges")));
    s.transact(|tx| tx.remove_node(&"b")).expect("known");
    s.transact(|tx| add_all(tx, &["c"]))
        .expect("the freed slot is room again");
}

#[test]
fn edge_ids_run_out_at_the_end_of_their_range() {
    let mut s = with_nodes(&["a"]);
    s.next_edge_id = u64::from(u32::MAX);
    let last = s
        .transact(|tx| tx.add_edge(&"a", &"a", EdgeSpec::directed()))
        .expect("one left")
        .0;
    assert_eq!(last, EdgeId(u32::MAX));
    let none = s.transact(|tx| tx.add_edge(&"a", &"a", EdgeSpec::directed()));
    assert_eq!(none.map(|_| ()), Err(GraphError::Capacity("edge ids")));
}

#[test]
fn specs_are_sanitized_as_they_are_set() {
    let mut s = GraphStore::new(4, Limits::default());
    s.transact(|tx| {
        tx.add_node("a", NodeSpec::label("abcdefgh"))?;
        tx.add_node("b", NodeSpec::label("b"))?;
        tx.add_edge(&"a", &"b", EdgeSpec::directed())
    })
    .expect("fresh");
    let id = EdgeId(0);
    s.transact(|tx| {
        tx.set_node(&"b", |spec| spec.weight = f32::NAN)?;
        tx.set_edge(id, |spec| spec.weight = f32::INFINITY)
    })
    .expect("known");
    let spec = |key| {
        s.node(s.ix_of(&key).expect("live"))
            .expect("live")
            .spec
            .clone()
    };
    assert_eq!(spec("a").label, "abcd");
    assert_eq!(spec("b").weight.to_bits(), 1.0f32.to_bits());
    let edge = s.edge(s.edge_ix(id).expect("live")).expect("live");
    assert_eq!(edge.spec.weight.to_bits(), f32::MAX.to_bits());
}

#[test]
fn an_empty_or_unchanging_transaction_is_no_delta() {
    let mut s = with_nodes(&["a", "b"]);
    s.transact(|tx| tx.add_edge(&"a", &"b", EdgeSpec::directed()))
        .expect("ends");
    let before = s.clone();
    assert_eq!(s.transact(|_| Ok(())).expect("empty").1, Delta::default());
    let same = s.transact(|tx| {
        tx.set_node(&"a", |spec| spec.label = "a".into())?;
        tx.set_edge(EdgeId(0), |spec| spec.directed = true)
    });
    assert_eq!(same.expect("known").1, Delta::default());
    assert_eq!(s, before);
}

#[test]
fn node_edits_classify_by_what_they_change() {
    let mut s = with_nodes(&["a", "b"]);
    let a = s.ix_of(&"a").expect("live");
    let property = Some(DeltaClass::Property);
    let geometry = Some(DeltaClass::Geometry);
    assert_eq!(
        class_of(&mut s, |tx| tx.set_node(&"a", |n| n.weight = 3.0)),
        property
    );
    assert_eq!(
        class_of(&mut s, |tx| tx
            .set_node(&"a", |n| n.sort_key = Some("z".into()))),
        property
    );
    assert_eq!(
        class_of(&mut s, |tx| tx.set_node(&"a", |n| n.label = "b".into())),
        property
    );
    assert_eq!(
        class_of(&mut s, |tx| tx.set_node(&"a", |n| n.label = "日".into())),
        geometry
    );
    assert_eq!(
        class_of(&mut s, |tx| tx
            .set_node(&"a", |n| n.shape = NodeShape::Glyph('*'))),
        geometry
    );
    assert_eq!(
        class_of(&mut s, |tx| tx
            .set_node(&"a", |n| n.pinned = Some((1.0, 2.0)))),
        geometry
    );
    assert_eq!(
        class_of(&mut s, |tx| tx.set_node(&"a", |n| n.pinned = None)),
        geometry
    );

    let ((), delta) = s
        .transact(|tx| {
            tx.set_node(&"b", |n| n.weight = 0.5)?;
            tx.set_node(&"a", |n| n.label = "wider".into())
        })
        .expect("known");
    assert_eq!(
        (delta.class, delta.reshaped),
        (geometry, set(&[a])),
        "the strongest wins"
    );
}

#[test]
fn edge_edits_classify_by_what_they_change() {
    let mut s = with_nodes(&["a", "b"]);
    let id = s
        .transact(|tx| tx.add_edge(&"a", &"b", EdgeSpec::directed()))
        .expect("ends")
        .0;
    let property = Some(DeltaClass::Property);
    let topology = Some(DeltaClass::Topology);
    assert_eq!(
        class_of(&mut s, |tx| tx.set_edge(id, |e| e.weight = 0.5)),
        property
    );
    assert_eq!(
        class_of(&mut s, |tx| tx
            .set_edge(id, |e| e.class = EdgeClass::Dashed)),
        property
    );
    assert_eq!(
        class_of(&mut s, |tx| tx.set_edge(id, |e| e.directed = false)),
        topology
    );
    assert_eq!(
        class_of(&mut s, |tx| tx
            .set_edge(id, |e| e.layout_participating = false)),
        topology
    );
}

#[test]
fn a_pin_edit_lists_the_node_as_repinned_and_reshaped_only_when_the_marker_flips() {
    let mut s = with_nodes(&["a", "b"]);
    let a = s.ix_of(&"a").expect("live");
    let pin = |s: &mut Store, at: Option<(f64, f64)>| {
        s.transact(|tx| tx.set_node(&"a", |n| n.pinned = at))
            .expect("known")
            .1
    };
    let set_pin = pin(&mut s, Some((1.0, 2.0)));
    assert_eq!(
        (set_pin.class, set_pin.reshaped, set_pin.repinned),
        (Some(DeltaClass::Geometry), set(&[a]), set(&[a])),
        "setting a pin adds the in-box marker"
    );
    let moved = pin(&mut s, Some((3.0, 4.0)));
    assert_eq!(
        (moved.class, moved.reshaped, moved.repinned),
        (Some(DeltaClass::Geometry), set(&[]), set(&[a])),
        "moving one keeps the marker, so the box is as it was"
    );
    let cleared = pin(&mut s, None);
    assert_eq!((cleared.reshaped, cleared.repinned), (set(&[a]), set(&[a])));

    let ((), gone) = s
        .transact(|tx| {
            tx.set_node(&"a", |n| n.pinned = Some((5.0, 6.0)))?;
            tx.remove_node(&"a")
        })
        .expect("known");
    assert!(gone.repinned.is_empty(), "{:?}", gone.repinned);
    let ((), weighed) = s
        .transact(|tx| tx.set_node(&"b", |n| n.weight = 2.0))
        .expect("known");
    assert!(weighed.repinned.is_empty());
}

#[test]
fn reshaped_lists_only_surviving_committed_nodes() {
    let mut s = with_nodes(&["a", "b"]);
    let ((), delta) = s
        .transact(|tx| {
            tx.set_node(&"a", |n| n.label = "much wider".into())?;
            tx.remove_node(&"a")?;
            tx.add_node("c", NodeSpec::label("c"))?;
            tx.set_node(&"c", |n| n.label = "much wider".into())
        })
        .expect("known");
    assert_eq!(delta.class, Some(DeltaClass::Topology));
    assert!(delta.reshaped.is_empty(), "{:?}", delta.reshaped);
    let c = s.ix_of(&"c").expect("live");
    assert_eq!(s.node(c).expect("live").spec.label, "much wider");
    assert_eq!(delta.added, set(&[c]));
}

#[test]
fn deltas_absorb_in_order() {
    let ix = NodeIx::new;
    // Each reshaped node is repinned too, so `repinned` follows the same rules.
    let delta = |class, added: &[NodeIx], removed: &[NodeIx], reshaped: &[NodeIx]| Delta {
        class,
        added: set(added),
        removed: set(removed),
        reshaped: set(reshaped),
        repinned: set(reshaped),
    };
    let mut pending = delta(Some(DeltaClass::Geometry), &[], &[], &[ix(0), ix(1)]);
    pending.absorb(delta(Some(DeltaClass::Property), &[], &[], &[]));
    assert_eq!(
        pending.class,
        Some(DeltaClass::Geometry),
        "the strongest stays"
    );
    pending.absorb(delta(Some(DeltaClass::Topology), &[ix(5)], &[ix(0)], &[]));
    pending.absorb(delta(
        Some(DeltaClass::Topology),
        &[ix(0)],
        &[],
        &[ix(0), ix(1)],
    ));
    assert_eq!(
        pending,
        delta(
            Some(DeltaClass::Topology),
            &[ix(0), ix(5)],
            &[ix(0)],
            &[ix(1)]
        ),
        "a slot that lost its node and gained another is both; a fresh node is not reshaped"
    );
    pending.absorb(delta(
        Some(DeltaClass::Topology),
        &[],
        &[ix(0), ix(1), ix(5)],
        &[],
    ));
    assert_eq!(
        pending,
        delta(Some(DeltaClass::Topology), &[], &[ix(0), ix(1), ix(5)], &[])
    );
    pending.absorb(Delta::default());
    assert_eq!(pending.class, Some(DeltaClass::Topology));
}
