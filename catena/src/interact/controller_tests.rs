#![allow(
    clippy::float_cmp,
    reason = "a zoom glide ends on its target exactly, and clamped steps are exact"
)]

//! The controller's keyboard half and its clock (plan §10.1, §10.3, §10.4); the pointer's
//! half is in `pointer_tests`.

use std::time::Duration;

use super::*;
use crate::geometry::cell::CellPt;
use crate::graph::{EdgeSpec, NodeSpec};
use crate::interact::{InputEvent, KeyCode, KeyInput, Modifiers};
use crate::raster::Blitter;

/// A view of nodes `n0`, `n1`, … pinned at `points`, with undirected `edges`, drawn once at
/// 80 × 24.
pub(in crate::interact) fn pinned(
    points: &[(f64, f64)],
    edges: &[(usize, usize)],
) -> GraphView<String> {
    let mut gv = GraphView::builder().blitter(Blitter::Ascii).build();
    gv.update(|tx| {
        for (i, &at) in points.iter().enumerate() {
            let mut spec = NodeSpec::label(format!("n{i}"));
            spec.pinned = Some(at);
            tx.add_node(format!("n{i}"), spec)?;
        }
        for &(a, b) in edges {
            tx.add_edge(&format!("n{a}"), &format!("n{b}"), EdgeSpec::undirected())?;
        }
        Ok(())
    })
    .expect("fresh keys, known ends");
    gv.render_to_string(80, 24);
    gv
}

pub(in crate::interact) fn ix(gv: &GraphView<String>, i: usize) -> NodeIx {
    gv.node_ix(&format!("n{i}")).expect("live")
}

pub(in crate::interact) fn cell(gv: &GraphView<String>, i: usize) -> CellPt {
    gv.viewport.cell(ix(gv, i)).expect("drawn")
}

/// Ticks 16 ms frames until `ms` milliseconds have passed.
fn run(gv: &mut GraphView<String>, ms: u64) {
    for _ in 0..ms.div_ceil(16) {
        gv.tick(Duration::from_millis(16));
    }
}

/// A plus: `n0` in the middle, then left, right, up and down of it.
const PLUS: [(f64, f64); 5] = [
    (0.0, 0.0),
    (-40.0, 0.0),
    (40.0, 0.0),
    (0.0, -40.0),
    (0.0, 40.0),
];

#[test]
fn cycling_walks_the_sort_order_and_wraps() {
    let mut gv = pinned(&PLUS[..3], &[]);
    let selected = |gv: &mut GraphView<String>, action| {
        gv.act(action)
            .into_iter()
            .find_map(|o| match o {
                Outcome::Selected(ix) => ix,
                _ => None,
            })
            .expect("a selection")
    };
    let (n0, n1, n2) = (ix(&gv, 0), ix(&gv, 1), ix(&gv, 2));
    assert_eq!(
        selected(&mut gv, Action::CycleNext),
        n0,
        "the first, from none"
    );
    assert_eq!(selected(&mut gv, Action::CycleNext), n1);
    assert_eq!(selected(&mut gv, Action::CycleNext), n2);
    assert_eq!(selected(&mut gv, Action::CycleNext), n0, "wrapping");
    assert_eq!(selected(&mut gv, Action::CyclePrev), n2, "wrapping back");
    gv.act(Action::Clear);
    assert_eq!(
        selected(&mut gv, Action::CyclePrev),
        n2,
        "the last, from none"
    );
    assert_eq!(gv.selected(), Some(n2));
    let mut empty: GraphView<String> = GraphView::new();
    assert_eq!(empty.act(Action::CycleNext), []);
}

#[test]
fn keyboard_selection_glides_the_node_to_the_visible_center() {
    // Plan §10.4: 300 ms, eased; the layout never moves (§11.2).
    let mut gv = pinned(&PLUS, &[]);
    let world = gv.force.positions.clone();
    let start = cell(&gv, 1);
    let out = gv.act(Action::CycleNext);
    assert_eq!(out[0], Outcome::Selected(Some(ix(&gv, 0))));
    gv.act(Action::CycleNext);
    assert_eq!(gv.selected(), Some(ix(&gv, 1)));
    let center = SubPt::new(39.5, 11.5);
    assert!(
        (f64::from(start.x) - center.x).abs() > 10.0,
        "n1 starts well off center"
    );
    gv.tick(Duration::from_millis(16));
    let early = cell(&gv, 1);
    assert!(
        early != start && (f64::from(early.x) - center.x).abs() > 3.0,
        "{early:?}"
    );
    run(&mut gv, 300);
    let end = cell(&gv, 1);
    assert!(
        (f64::from(end.x) - center.x).abs() <= 0.5 && (f64::from(end.y) - center.y).abs() <= 0.5,
        "{end:?}"
    );
    assert_eq!(gv.force.positions, world);
}

#[test]
fn centering_aims_at_what_the_hosts_windows_leave_visible() {
    let mut gv = pinned(&PLUS, &[]);
    gv.set_occlusions(&[CellBox::new(0, 0, 40, 24)]);
    gv.act(Action::CycleNext);
    run(&mut gv, 300);
    let at = cell(&gv, 0);
    assert!(
        (f64::from(at.x) - 59.5).abs() <= 0.5 && (f64::from(at.y) - 11.5).abs() <= 0.5,
        "{at:?}"
    );
}

#[test]
fn a_pan_key_moves_every_node_by_its_step_and_cancels_a_glide() {
    let mut gv = pinned(&PLUS, &[]);
    let world = gv.force.positions.clone();
    let before: Vec<CellPt> = (0..5).map(|i| cell(&gv, i)).collect();
    assert_eq!(gv.act(Action::Pan(Dir::Left)), [Outcome::ViewChanged]);
    gv.act(Action::Pan(Dir::Up));
    for (i, b) in before.iter().enumerate() {
        assert_eq!(cell(&gv, i), CellPt::new(b.x + 4, b.y + 2), "n{i}");
    }
    gv.act(Action::Pan(Dir::Right));
    gv.act(Action::Pan(Dir::Down));
    assert_eq!(cell(&gv, 3), before[3]);
    assert_eq!(gv.force.positions, world);
    gv.act(Action::CycleNext);
    gv.act(Action::CycleNext);
    assert!(gv.controller.pan.is_some(), "a glide toward n1 runs");
    gv.act(Action::Pan(Dir::Left));
    assert!(gv.controller.pan.is_none());
}

#[test]
fn zoom_keys_accumulate_and_glide_about_the_visible_center() {
    let mut gv = pinned(&PLUS, &[]);
    let center = SubPt::new(39.5, 11.5);
    let under = gv.viewport.world_at(center).expect("fitted");
    assert_eq!(gv.act(Action::ZoomIn), [Outcome::ViewChanged]);
    gv.act(Action::ZoomIn);
    gv.tick(Duration::from_millis(16));
    let partway = gv.viewport.zoom();
    assert!(partway > 1.0 && partway < 1.5, "{partway}");
    run(&mut gv, 400);
    assert_eq!(
        gv.viewport.zoom(),
        1.5,
        "two steps of 0.25, snapped at the end"
    );
    assert!(gv.controller.zoom.is_none());
    let now = gv.viewport.world_at(center).expect("fitted");
    assert!((now.0 - under.0).abs() < 1e-9 && (now.1 - under.1).abs() < 1e-9);
}

#[test]
fn a_zoom_step_at_the_clamp_changes_nothing() {
    let mut gv = pinned(&PLUS, &[]);
    for _ in 0..14 {
        gv.act(Action::ZoomOut);
    }
    run(&mut gv, 1000);
    assert_eq!(gv.viewport.zoom(), crate::geometry::MIN_ZOOM);
    assert_eq!(gv.act(Action::ZoomOut), []);
}

#[test]
fn scrolling_zooms_about_the_pointer() {
    let mut gv = pinned(&PLUS, &[]);
    let pointer = SubPt::new(10.0, 5.0);
    let under = gv.viewport.world_at(pointer).expect("fitted");
    assert_eq!(
        gv.handle(InputEvent::Scroll(-1, 10, 5)),
        [Outcome::ViewChanged]
    );
    run(&mut gv, 400);
    assert!((gv.viewport.zoom() - 0.92).abs() < 1e-12);
    let now = gv.viewport.world_at(pointer).expect("fitted");
    assert!((now.0 - under.0).abs() < 1e-9 && (now.1 - under.1).abs() < 1e-9);
    assert_eq!(gv.handle(InputEvent::Scroll(0, 10, 5)), []);
}

#[test]
fn a_long_tick_advances_a_quarter_second_and_a_zero_tick_nothing() {
    // Plan §10.4: a suspended host does not fast-forward a glide into a teleport.
    let mut gv = pinned(&PLUS, &[]);
    gv.act(Action::CycleNext);
    gv.act(Action::CycleNext);
    let mut capped = gv.clone();
    let before = cell(&gv, 1);
    assert_eq!(
        gv.tick(Duration::ZERO),
        Redraw::Needed,
        "a glide is running"
    );
    assert_eq!(cell(&gv, 1), before);
    gv.tick(Duration::from_secs(10));
    capped.tick(MAX_TICK);
    assert_eq!(cell(&gv, 1), cell(&capped, 1));
    assert!(gv.controller.pan.is_some(), "250 ms of a 300 ms glide");
}

#[test]
fn redraw_is_idle_only_when_the_last_frame_shows_the_view() {
    let mut gv = pinned(&PLUS, &[]);
    assert_eq!(gv.tick(Duration::from_millis(16)), Redraw::Idle);
    gv.update(|tx| tx.set_node(&"n0".to_string(), |n| n.label = "renamed".into()))
        .expect("known");
    // A hit-test rebuilds the scene but draws nothing.
    gv.handle(InputEvent::MouseMove(0, 0));
    assert_eq!(gv.tick(Duration::from_millis(16)), Redraw::Needed);
    gv.render_to_string(80, 24);
    assert_eq!(gv.tick(Duration::from_millis(16)), Redraw::Idle);
    gv.act(Action::Pan(Dir::Down));
    assert_eq!(gv.tick(Duration::from_millis(16)), Redraw::Needed);
    gv.render_to_string(80, 24);
    gv.act(Action::ZoomIn);
    gv.render_to_string(80, 24);
    assert_eq!(
        gv.tick(Duration::from_millis(16)),
        Redraw::Needed,
        "zooming"
    );
    run(&mut gv, 400);
    gv.render_to_string(80, 24);
    assert_eq!(gv.tick(Duration::from_millis(16)), Redraw::Idle);
}

#[test]
fn spatial_navigation_moves_along_the_plus_and_stops_at_its_ends() {
    let mut gv = pinned(&PLUS, &[]);
    gv.act(Action::CycleNext);
    let right = gv.act(Action::NavTo(Dir::Right));
    assert_eq!(right[0], Outcome::Selected(Some(ix(&gv, 2))));
    assert_eq!(
        gv.act(Action::NavTo(Dir::Right)),
        [Outcome::Boundary(Dir::Right)]
    );
    assert_eq!(gv.selected(), Some(ix(&gv, 2)));
    let back = gv.act(Action::NavTo(Dir::Left));
    assert_eq!(back[0], Outcome::Selected(Some(ix(&gv, 0))), "the nearer");
    let up = gv.act(Action::NavTo(Dir::Up));
    assert_eq!(up[0], Outcome::Selected(Some(ix(&gv, 3))));
}

#[test]
fn spatial_navigation_with_nothing_selected_starts_from_the_visible_center() {
    let mut gv = pinned(&PLUS, &[]);
    let out = gv.act(Action::NavTo(Dir::Left));
    let Some(Outcome::Selected(Some(chosen))) = out.first().copied() else {
        panic!("{out:?}");
    };
    let at = gv.viewport.cell(chosen).expect("drawn");
    assert!(f64::from(at.x) < 39.5, "{at:?} is left of the center");
}

#[test]
fn toggle_pin_pins_the_selected_node_where_it_is_drawn_and_unpins_it() {
    let mut gv: GraphView<String> = GraphView::builder().blitter(Blitter::Ascii).build();
    gv.update(|tx| {
        for k in ["a", "b", "c"] {
            tx.add_node(k.to_string(), NodeSpec::label(k))?;
        }
        tx.add_edge(&"a".into(), &"b".into(), EdgeSpec::undirected())?;
        tx.add_edge(&"b".into(), &"c".into(), EdgeSpec::undirected())?;
        Ok(())
    })
    .expect("fresh keys");
    assert_eq!(gv.act(Action::TogglePin), [], "nothing selected");
    gv.act(Action::CycleNext);
    let b = gv.node_ix(&"b".to_string()).expect("live");
    gv.act(Action::CycleNext);
    let at = gv.viewport.cell(b).expect("drawn");
    assert_eq!(gv.act(Action::TogglePin), [Outcome::Pinned(b, true)]);
    assert!(gv.store.node(b).expect("live").spec.pinned.is_some());
    assert_eq!(gv.viewport.cell(b), Some(at), "pinned where it was drawn");
    assert_eq!(gv.act(Action::TogglePin), [Outcome::Pinned(b, false)]);
    assert_eq!(gv.store.node(b).expect("live").spec.pinned, None);
}

#[test]
fn a_pin_from_the_keyboard_holds_a_node_drawn_off_its_layout_spot_where_it_is_drawn() {
    // A zoom out crowds n1 against the pinned n0, and the re-snap moves n1 off where its
    // layout position falls. Pinning it there would put it back where it meets n0, and n1,
    // with an edge more than n0, would claim the cell first and push n0 aside.
    let mut gv = pinned(&[(0.0, 0.0), (3.0, 0.0), (40.0, 20.0)], &[(1, 2)]);
    gv.update(|tx| tx.set_node(&"n1".to_string(), |n| n.pinned = None))
        .expect("known");
    gv.zoom_about(0.4, SubPt::new(0.0, 0.0));
    let n1 = ix(&gv, 1);
    let at = cell(&gv, 1);
    let layout = gv.force.positions[n1.slot()].expect("placed");
    let drawn = gv.viewport.world_at(SubPt::from(at)).expect("fitted");
    let off = (drawn.0 - layout.0).abs().max((drawn.1 - layout.1).abs());
    assert!(
        off > 1.0,
        "n1 was pushed off its spot: {drawn:?}, {layout:?}"
    );
    gv.act(Action::CycleNext);
    gv.act(Action::CycleNext);
    assert_eq!(gv.selected(), Some(n1));
    let n0 = cell(&gv, 0);
    assert_eq!(gv.act(Action::TogglePin), [Outcome::Pinned(n1, true)]);
    assert_eq!((cell(&gv, 1), cell(&gv, 0)), (at, n0));
}

#[test]
fn clear_drops_the_selection_and_bundling_toggles() {
    let mut gv = pinned(&PLUS, &[]);
    gv.act(Action::CycleNext);
    assert_eq!(gv.act(Action::Clear), [Outcome::Selected(None)]);
    assert_eq!(gv.act(Action::Clear), []);
    assert!(gv.bundling());
    assert_eq!(gv.act(Action::ToggleBundling), []);
    assert!(!gv.bundling());
    gv.act(Action::ToggleBundling);
    assert!(gv.bundling());
}

#[test]
fn keys_go_through_the_key_table() {
    let mut gv = pinned(&PLUS, &[]);
    let n = InputEvent::Key(KeyInput::new(KeyCode::Char('n'), Modifiers::NONE));
    let out = gv.handle(n);
    assert_eq!(out[0], Outcome::Selected(Some(ix(&gv, 0))));
    gv.keymap_mut()
        .unbind(KeyInput::new(KeyCode::Char('n'), Modifiers::NONE));
    assert_eq!(gv.handle(n), []);
    assert_eq!(
        gv.keymap()
            .action(KeyInput::new(KeyCode::Char('n'), Modifiers::NONE)),
        None
    );
    assert_eq!(gv.selected(), Some(ix(&gv, 0)));
}

#[test]
fn a_resize_event_lays_out_for_the_new_size() {
    let mut gv = pinned(&PLUS, &[]);
    assert_eq!(
        gv.handle(InputEvent::Resize(120, 40)),
        [Outcome::ViewChanged]
    );
    assert_eq!(
        (gv.viewport.grid().cols, gv.viewport.grid().rows),
        (120, 40)
    );
    assert_eq!(gv.handle(InputEvent::Resize(120, 40)), []);
}

#[test]
fn a_commit_that_removes_the_selected_node_forgets_it() {
    // A slot is reused after the commit that freed it, so a kept index would come to name
    // another node.
    let mut gv = pinned(&PLUS, &[(0, 1)]);
    gv.act(Action::CycleNext);
    gv.update(|tx| tx.remove_node(&"n0".to_string()))
        .expect("known");
    assert_eq!(gv.selected(), None);
    gv.update(|tx| tx.add_node("new".to_string(), NodeSpec::label("new")))
        .expect("fresh key");
    assert_eq!(gv.selected(), None, "the reused slot is not selected");
}
