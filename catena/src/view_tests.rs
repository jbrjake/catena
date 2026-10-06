use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;
use crate::ForceParams;
use crate::geometry::NodeForm;
use crate::geometry::cell::CellBox;
use crate::graph::{EdgeSpec, NodeShape, NodeSpec};

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
fn every_commit_is_measured_and_only_boxes_that_changed_re_snap() {
    let mut gv = two_nodes();
    let width = |gv: &GraphView<String>, key: &str| {
        let ix = gv.node_ix(&key.to_string()).expect("live");
        gv.metrics.form(ix).map(NodeForm::width)
    };
    // The view starts at zoom 1.0, semantic level 2: labels cut at 14 columns.
    assert_eq!(
        (width(&gv, "ada"), width(&gv, "babbage")),
        (Some(14), Some(14))
    );
    let (world, canonical) = (gv.force.positions.clone(), gv.viewport.canonical().clone());

    gv.update(|tx| {
        tx.set_node(&"babbage".into(), |n| {
            n.label = "Charles Babbage, FRS".into();
        })
    })
    .expect("known");
    assert_eq!(
        (&gv.force.positions, gv.viewport.canonical()),
        (&world, &canonical),
        "a longer label already cut at the cap moves nothing"
    );

    gv.update(|tx| tx.set_node(&"ada".into(), |n| n.label = "Ada".into()))
        .expect("known");
    assert_eq!(width(&gv, "ada"), Some(5));
    assert_eq!(gv.force.positions, world, "a re-snap runs no layout");
    let ada = gv.node_ix(&"ada".into()).expect("live");
    assert_eq!(
        gv.viewport.canonical().anchors[ada.slot()],
        canonical.anchors[ada.slot()],
        "the narrower box keeps its anchor"
    );
}

#[test]
fn the_layout_catches_up_at_the_end_of_each_update() {
    // Plan §4.2: layout runs synchronously at the end of `update`, so nothing is pending.
    let gv = two_nodes();
    assert_eq!(gv.pending, Delta::default());
    for key in ["ada", "babbage"] {
        let ix = gv.node_ix(&key.to_string()).expect("live");
        assert!(gv.force.positions[ix.slot()].is_some(), "{key} laid out");
        assert!(gv.cell(ix).is_some(), "{key} snapped");
    }
}

/// A view of an `n × n` grid graph of short labels.
fn grid(n: usize) -> GraphView<String> {
    let mut gv = GraphView::new();
    let key = |r: usize, c: usize| format!("{r}.{c}");
    gv.update(|tx| {
        for r in 0..n {
            for c in 0..n {
                tx.add_node(key(r, c), NodeSpec::label(key(r, c)))?;
            }
        }
        for r in 0..n {
            for c in 0..n {
                if c + 1 < n {
                    tx.add_edge(&key(r, c), &key(r, c + 1), EdgeSpec::undirected())?;
                }
                if r + 1 < n {
                    tx.add_edge(&key(r, c), &key(r + 1, c), EdgeSpec::undirected())?;
                }
            }
        }
        Ok(())
    })
    .expect("fresh keys, known ends");
    gv
}

fn intersect(a: CellBox, b: CellBox) -> bool {
    i64::from(a.x) < b.right()
        && i64::from(b.x) < a.right()
        && i64::from(a.y) < b.bottom()
        && i64::from(b.y) < a.bottom()
}

fn drawn(gv: &GraphView<String>) -> Vec<CellBox> {
    gv.viewport.boxes(&gv.metrics).map(|(_, b)| b).collect()
}

#[test]
fn an_update_lays_out_and_snaps_every_node_apart() {
    let gv = grid(5);
    let boxes = drawn(&gv);
    assert_eq!(boxes.len(), 25);
    for (i, &a) in boxes.iter().enumerate() {
        assert!(
            boxes[i + 1..].iter().all(|&b| !intersect(a, b)),
            "{a:?} meets another box"
        );
    }
}

#[test]
fn a_property_edit_moves_no_node() {
    // Plan §11.1: property-only deltas move zero nodes, exactly.
    let mut gv = grid(4);
    let (world, canonical) = (gv.force.positions.clone(), gv.viewport.canonical().clone());
    gv.update(|tx| tx.set_node(&"1.1".to_string(), |n| n.weight = 3.0))
        .expect("known");
    assert_eq!(gv.force.positions, world);
    assert_eq!(gv.viewport.canonical(), &canonical);
}

#[test]
fn pan_and_zoom_within_a_level_run_no_layout() {
    // Plan §11.2: no pan or zoom ever re-runs a layout engine.
    let mut gv = grid(4);
    let world = gv.force.positions.clone();
    gv.pan_by((5.5, -3.0));
    gv.zoom_about(1.4, SubPt::new(10.0, 5.0));
    gv.zoom_about(0.6, SubPt::new(10.0, 5.0));
    assert_eq!(gv.force.positions, world);
}

#[test]
fn invariant_g_a_relayout_keeps_the_focused_node_where_it_is_drawn() {
    // Plan §16.2-G: within 3 cells; anchor compensation holds it exactly.
    let mut gv = grid(4);
    gv.pan_by((2.5, 1.0));
    let focus = gv.node_ix(&"2.1".to_string()).expect("live");
    gv.focus = Some(focus);
    let before = gv.cell(focus).expect("drawn");
    gv.update(|tx| {
        tx.add_node("new".to_string(), NodeSpec::label("a new node"))?;
        tx.add_edge(
            &"new".to_string(),
            &"0.0".to_string(),
            EdgeSpec::undirected(),
        )?;
        tx.add_edge(
            &"new".to_string(),
            &"3.3".to_string(),
            EdgeSpec::undirected(),
        )?;
        Ok(())
    })
    .expect("fresh key, known ends");
    assert_eq!(gv.cell(focus), Some(before));
}

/// Replays what `update` does after an edit that changed `changed`'s boxes or pins, on the
/// viewport as it was: how many nodes the re-snap pushes aside.
fn pushed(
    viewport: &Viewport,
    gv: &GraphView<String>,
    world: &[Option<(f64, f64)>],
    changed: &[&str],
) -> usize {
    let changed: BTreeSet<NodeIx> = changed
        .iter()
        .map(|k| gv.node_ix(&(*k).to_string()).expect("live"))
        .collect();
    let mut probe = viewport.clone();
    let order = placement_order(&gv.store);
    probe
        .resnap(&changed, world, &gv.metrics, &order, None)
        .len()
}

#[test]
fn an_edit_that_pushes_more_than_eight_nodes_aside_relays_out() {
    // Plan §4.2: a Geometry delta re-snaps locally; a cascade past 8 displaced nodes relayouts.
    let mut gv = grid(9);
    let (viewport, world) = (gv.viewport.clone(), gv.force.positions.clone());
    gv.update(|tx| {
        tx.set_node(&"4.4".to_string(), |n| {
            n.shape = NodeShape::Box {
                min_w: 14,
                min_h: 20,
            };
        })
    })
    .expect("known");
    assert!(pushed(&viewport, &gv, &world, &["4.4"]) > 8);
    assert_ne!(gv.force.positions, world, "the layout ran");

    let mut gv = grid(9);
    let (viewport, world) = (gv.viewport.clone(), gv.force.positions.clone());
    gv.update(|tx| tx.set_node(&"4.4".to_string(), |n| n.label = "4.4 wide".into()))
        .expect("known");
    assert!(pushed(&viewport, &gv, &world, &["4.4"]) <= 8);
    assert_eq!(gv.force.positions, world, "a re-snap alone");
}

#[test]
fn crossing_a_semantic_level_relays_out_around_the_reshaped_nodes() {
    // Owner, "Relayout": a level change relayouts only around the boxes that collapsed or
    // expanded. Zooming from 1.0 to 2.0 moves from level 2 to level 3, which widens only the
    // one label longer than 12 columns.
    let mut gv: GraphView<String> = GraphView::new();
    let keys: Vec<String> = (0..12).map(|i| format!("p{i}")).collect();
    gv.update(|tx| {
        for (i, key) in keys.iter().enumerate() {
            let label = if i == 0 {
                "a label that runs long".to_string()
            } else {
                key.clone()
            };
            tx.add_node(key.clone(), NodeSpec::label(label))?;
        }
        for pair in keys.windows(2) {
            tx.add_edge(&pair[0], &pair[1], EdgeSpec::undirected())?;
        }
        Ok(())
    })
    .expect("fresh keys, known ends");
    let world = gv.force.positions.clone();
    gv.zoom_about(2.0, SubPt::new(0.0, 0.0));
    let reach = ForceParams::default().tether_reach;
    let mut held = 0;
    for (hops, key) in keys.iter().enumerate() {
        let ix = gv.node_ix(key).expect("live");
        if u32::try_from(hops).expect("small") > reach {
            held += 1;
            assert_eq!(gv.force.positions[ix.slot()], world[ix.slot()], "{key}");
        }
    }
    assert_eq!(held, 12 - 3);
    let long = gv.node_ix(&keys[0]).expect("live");
    assert_ne!(gv.force.positions[long.slot()], world[long.slot()]);
}

#[test]
fn a_resize_relays_out_every_node() {
    // Owner, "Relayout": a resize is a full relayout.
    let mut gv = grid(4);
    let world = gv.force.positions.clone();
    gv.resize(80, 24);
    assert_eq!(gv.force.positions, world, "the same size is no resize");
    gv.resize(160, 48);
    let moved = world
        .iter()
        .zip(&gv.force.positions)
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(moved, 16);
}
