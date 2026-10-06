use super::super::testing::{Store, add_all, keys_in_order, named, store, with_nodes};
use super::*;

#[test]
fn nodes_order_by_sort_key_then_key() {
    let mut s = store();
    s.transact(|tx| {
        tx.add_node("c", NodeSpec::label("same"))?;
        tx.add_node("a", NodeSpec::label("same"))?;
        tx.add_node("b", NodeSpec::label("first"))?;
        Ok(())
    })
    .expect("fresh keys");
    assert_eq!(keys_in_order(&s), ["b", "a", "c"], "label, then key");

    s.transact(|tx| tx.set_node(&"c", |spec| spec.sort_key = Some("0".into())))
        .expect("known key");
    assert_eq!(
        keys_in_order(&s),
        ["c", "b", "a"],
        "a sort key outranks the label"
    );

    s.transact(|tx| tx.set_node(&"a", |spec| spec.label = "fir".into()))
        .expect("known key");
    assert_eq!(
        keys_in_order(&s),
        ["c", "a", "b"],
        "a relabelled node moves"
    );

    s.transact(|tx| tx.set_node(&"b", |spec| spec.label = "fiq".into()))
        .expect("known key");
    assert_eq!(
        keys_in_order(&s),
        ["c", "b", "a"],
        "and so does its neighbour"
    );

    s.transact(|tx| tx.set_node(&"c", |spec| spec.sort_key = Some("zz".into())))
        .expect("known key");
    assert_eq!(
        keys_in_order(&s),
        ["b", "a", "c"],
        "the first node, sorting past the one after it"
    );

    s.transact(|tx| tx.set_node(&"b", |spec| spec.weight = 2.0))
        .expect("known key");
    assert_eq!(keys_in_order(&s), ["b", "a", "c"], "a weight moves nothing");
}

#[test]
fn node_order_does_not_depend_on_insertion_order() {
    let forward = with_nodes(&["d", "b", "a", "c", "e"]);
    let backward = with_nodes(&["e", "c", "a", "b", "d"]);
    assert_eq!(keys_in_order(&forward), ["a", "b", "c", "d", "e"]);
    assert_eq!(keys_in_order(&forward), keys_in_order(&backward));
}

/// a, b, c with `c→a, a→b, b→a, a→b, a→a, b→c`, added in that order.
fn triangle_with_parallels() -> Store {
    let mut s = with_nodes(&["a", "b", "c"]);
    s.transact(|tx| {
        for (from, to) in [
            ("c", "a"),
            ("a", "b"),
            ("b", "a"),
            ("a", "b"),
            ("a", "a"),
            ("b", "c"),
        ] {
            tx.add_edge(&from, &to, EdgeSpec::directed())?;
        }
        Ok(())
    })
    .expect("known ends");
    s
}

#[test]
fn edges_order_by_their_ends_places_then_insertion_with_ranks_per_unordered_pair() {
    let s = triangle_with_parallels();
    assert_eq!(
        named(&s, s.edges_in_order()),
        [
            ("a", "a", 0),
            ("a", "b", 0),
            ("b", "a", 1),
            ("a", "b", 2),
            ("c", "a", 0),
            ("b", "c", 0),
        ]
    );
}

#[test]
fn ranks_close_up_when_a_parallel_edge_leaves() {
    let mut s = triangle_with_parallels();
    let first = s.edges_in_order()[1];
    let id = s.edge(first).expect("live").id;
    s.transact(|tx| tx.remove_edge(id)).expect("known edge");
    assert_eq!(
        named(&s, s.edges_in_order()),
        [
            ("a", "a", 0),
            ("b", "a", 0),
            ("a", "b", 1),
            ("c", "a", 0),
            ("b", "c", 0),
        ]
    );
}

#[test]
fn incident_lists_follow_edge_order_and_a_self_loop_is_in_both() {
    let s = triangle_with_parallels();
    let ix = |key| s.ix_of(&key).expect("live");
    assert_eq!(
        named(&s, s.out_edges(ix("a"))),
        [("a", "a", 0), ("a", "b", 0), ("a", "b", 2)]
    );
    assert_eq!(
        named(&s, s.in_edges(ix("a"))),
        [("a", "a", 0), ("b", "a", 1), ("c", "a", 0)]
    );
    assert_eq!(
        named(&s, s.out_edges(ix("b"))),
        [("b", "a", 1), ("b", "c", 0)]
    );
    assert_eq!(
        named(&s, s.in_edges(ix("b"))),
        [("a", "b", 0), ("a", "b", 2)]
    );
    assert_eq!(named(&s, s.out_edges(ix("c"))), [("c", "a", 0)]);
    assert_eq!(named(&s, s.in_edges(ix("c"))), [("b", "c", 0)]);
    assert!(
        s.out_edges(NodeIx::new(99)).is_empty(),
        "a slot past the table"
    );
}

#[test]
fn a_freed_slot_is_reused_lowest_first_once_its_transaction_commits() {
    let mut s = with_nodes(&["a", "b", "c", "d"]);
    let slot = |s: &Store, key| s.ix_of(&key).expect("live").slot();
    assert_eq!([0, 1, 2, 3], ["a", "b", "c", "d"].map(|k| slot(&s, k)));

    let fresh = s
        .transact(|tx| {
            tx.remove_node(&"d")?;
            tx.remove_node(&"b")?;
            tx.add_node("e", NodeSpec::label("e"))
        })
        .expect("known keys")
        .0;
    assert_eq!(
        fresh.slot(),
        4,
        "slots freed in this transaction wait for its commit"
    );

    s.transact(|tx| add_all(tx, &["f", "g", "h"]))
        .expect("fresh keys");
    assert_eq!([1, 3, 5], ["f", "g", "h"].map(|k| slot(&s, k)));
    assert_eq!(s.nodes.len(), 6);
}

#[test]
fn churn_reuses_slots_instead_of_growing() {
    let mut s = store();
    for _ in 0..50 {
        s.transact(|tx| {
            add_all(tx, &["x", "y"])?;
            tx.add_edge(&"x", &"y", EdgeSpec::directed())?;
            Ok(())
        })
        .expect("fresh keys");
        s.transact(|tx| {
            tx.remove_node(&"x")?;
            tx.remove_node(&"y")
        })
        .expect("known keys");
    }
    assert_eq!((s.nodes.len(), s.edges.len()), (2, 1));
    assert_eq!(s.next_edge_id, 50, "edge ids still never repeat");
    assert_eq!((s.node_count(), s.edge_count()), (0, 0));
}
