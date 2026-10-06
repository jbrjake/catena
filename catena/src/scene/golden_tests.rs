//! T2 golden text snapshots of primitive scenes (plan §16.3): `CellGrid::to_string` under
//! `insta`, with no redaction, because a frame is deterministic. `cargo insta review` accepts a
//! change.

use std::sync::LazyLock;

use insta::assert_snapshot;

use super::*;
use crate::geometry::NodeForm;
use crate::geometry::cell::{CellBox, CellPt, SubPt};
use crate::geometry::curve::{Bezier, tessellate};
use crate::geometry::testing::forms;
use crate::graph::{EdgeIx, NodeIx, NodeShape, NodeSpec};
use crate::raster::{Blitter, CellGrid, CellStyle, PaletteColor};

/// The six nodes the primitive scenes draw, at full detail (semantic level 5).
static FORMS: LazyLock<Vec<NodeForm>> = LazyLock::new(|| {
    let labels = ["ada", "babbage", "lovelace", "日本語", "root", "leaf"];
    forms(&labels.map(NodeSpec::label), 5)
});

fn style(id: StyleId) -> CellStyle {
    CellStyle::new(PaletteColor::Indexed(u8::try_from(id.0).unwrap_or(255)))
}

fn render(scene: &SceneGraph, size: (u16, u16), blitter: Blitter) -> String {
    render_forms(scene, size, blitter, &FORMS)
}

/// `scene` drawn with node `i`'s form `forms[i]`.
fn render_forms(
    scene: &SceneGraph,
    size: (u16, u16),
    blitter: Blitter,
    forms: &[NodeForm],
) -> String {
    let mut grid = CellGrid::new(size.0, size.1);
    let options = RenderOptions {
        blitter,
        ..RenderOptions::default()
    };
    Compositor::new().render(scene, &mut grid, &options, style, |ix| forms.get(ix.slot()));
    grid.to_string()
}

/// One of the six nodes, its box from `(x, y)`; returns its anchor.
fn node(scene: &mut SceneGraph, ix: u32, x: i32, y: i32) -> CellPt {
    place(scene, ix, &FORMS[NodeIx::new(ix).slot()], x, y)
}

/// Node `ix` with its form's box from `(x, y)`; returns its anchor.
fn place(scene: &mut SceneGraph, ix: u32, form: &NodeForm, x: i32, y: i32) -> CellPt {
    let size = (u32::from(form.width()), u32::from(form.height()));
    scene.push(SceneItem::new(
        Layer::Nodes,
        CellBox::new(x, y, size.0, size.1),
        Payload::NodeBox {
            ix: NodeIx::new(ix),
        },
        StyleId(7),
    ));
    let anchor = form.anchor();
    CellPt::new(x + anchor.x, y + anchor.y)
}

fn path(scene: &mut SceneGraph, ix: u32, z: Layer, route: Route) {
    let bounds = route.bounds().expect("points");
    scene.push(SceneItem::new(
        z,
        bounds,
        Payload::EdgePath {
            ix: EdgeIx::new(ix),
            route: EdgeRoute::Path(route),
        },
        StyleId(1),
    ));
}

fn straight(from: CellPt, to: CellPt) -> Route {
    Route::Polyline(vec![from.into(), to.into()])
}

fn curved(from: CellPt, ctrl: (f64, f64), to: CellPt) -> Route {
    let (from, to): (SubPt, SubPt) = (from.into(), to.into());
    let mut points = Vec::new();
    tessellate(
        &[Bezier::Quadratic {
            from: (from.x, from.y),
            ctrl,
            to: (to.x, to.y),
        }],
        &mut points,
    );
    Route::Polyline(points.into_iter().map(|(x, y)| SubPt::new(x, y)).collect())
}

/// Three nodes joined by straight edges, and a curved edge to a fourth.
fn triangle() -> SceneGraph {
    let mut scene = SceneGraph::new();
    let ada = node(&mut scene, 0, 2, 1);
    let babbage = node(&mut scene, 1, 22, 2);
    let lovelace = node(&mut scene, 2, 8, 9);
    let japanese = node(&mut scene, 3, 26, 10);
    path(&mut scene, 0, Layer::EdgesUnder, straight(ada, babbage));
    path(
        &mut scene,
        1,
        Layer::EdgesUnder,
        straight(babbage, lovelace),
    );
    path(&mut scene, 2, Layer::EdgesUnder, straight(lovelace, ada));
    path(
        &mut scene,
        3,
        Layer::EdgesOver,
        curved(babbage, (34.0, 3.0), japanese),
    );
    scene
}

#[test]
fn t2_triangle_in_each_blitter() {
    let scene = triangle();
    for (blitter, name) in [
        (Blitter::Braille, "t2_triangle_braille"),
        (Blitter::HalfBlock, "t2_triangle_half_block"),
        (Blitter::Sextant, "t2_triangle_sextant"),
        (Blitter::Ascii, "t2_triangle_ascii"),
    ] {
        assert_snapshot!(name, render(&scene, (34, 12), blitter));
    }
}

/// A root, a bus bar and three leaves joined by orthogonal connectors, as the tree and layered
/// engines will draw them.
#[test]
fn t2_orthogonal_tree() {
    let mut scene = SceneGraph::new();
    let root = node(&mut scene, 4, 12, 0);
    // The middle leaf sits under the root, so its drop crosses the bar: a cross between tees.
    let leaves = [(2, 4), (12, 4), (22, 4)].map(|(x, y)| node(&mut scene, 5, x, y));
    let bar = 2;
    for (ix, leaf) in (0..).zip(leaves) {
        let route = Route::Orthogonal(vec![
            CellPt::new(root.x, root.y + 1),
            CellPt::new(root.x, bar),
            CellPt::new(leaf.x, bar),
            CellPt::new(leaf.x, leaf.y - 1),
        ]);
        path(&mut scene, ix, Layer::EdgesUnder, route);
    }
    assert_snapshot!(render(&scene, (28, 5), Blitter::Braille));
}

/// Edges crossing on two layers, under node text, with a parallel-edge badge.
#[test]
fn t2_crossing_layers_mask_and_badge() {
    let mut scene = SceneGraph::new();
    let ada = node(&mut scene, 0, 1, 4);
    let lovelace = node(&mut scene, 2, 20, 4);
    let root = node(&mut scene, 4, 10, 0);
    let leaf = node(&mut scene, 5, 11, 8);
    path(&mut scene, 0, Layer::EdgesUnder, straight(ada, lovelace));
    let over = straight(root, leaf);
    let bounds = over.bounds().expect("points");
    scene.push(
        SceneItem::new(
            Layer::EdgesOver,
            bounds,
            Payload::EdgePath {
                ix: EdgeIx::new(1),
                route: EdgeRoute::Path(over),
            },
            StyleId(2),
        )
        .with_badge(CountBadge {
            count: 4,
            at: CellPt::new(12, 6),
        }),
    );
    for (blitter, name) in [
        (Blitter::Braille, "t2_crossing_braille"),
        (Blitter::Ascii, "t2_crossing_ascii"),
    ] {
        assert_snapshot!(name, render(&scene, (30, 9), blitter));
    }
}

/// Two edges sharing a trunk segment, drawn once (owner ruling A3).
#[test]
fn t2_shared_trunk() {
    let mut scene = SceneGraph::new();
    let sources = [node(&mut scene, 0, 0, 0), node(&mut scene, 1, 0, 8)];
    let targets = [node(&mut scene, 2, 30, 0), node(&mut scene, 3, 32, 8)];
    let (join, knot) = (CellPt::new(10, 4), CellPt::new(22, 4));
    let trunk = scene.push_segment(
        Layer::EdgesUnder,
        StyleId(3),
        straight(join, knot),
        vec![EdgeIx::new(0), EdgeIx::new(1)],
    );
    for (ix, (source, target)) in (0..).zip(sources.into_iter().zip(targets)) {
        let edge = EdgeIx::new(ix);
        let first = scene.push_segment(
            Layer::EdgesUnder,
            StyleId(1),
            straight(source, join),
            vec![edge],
        );
        let last = scene.push_segment(
            Layer::EdgesUnder,
            StyleId(1),
            straight(knot, target),
            vec![edge],
        );
        let bounds = CellBox::around([source, target]).expect("two cells");
        scene.push(SceneItem::new(
            Layer::EdgesUnder,
            bounds,
            Payload::EdgePath {
                ix: edge,
                route: EdgeRoute::Chain(vec![first, trunk, last]),
            },
            StyleId(1),
        ));
    }
    let ends: Vec<EdgeEnds> = (0..)
        .zip(sources.into_iter().zip(targets))
        .map(|(ix, (source, target))| EdgeEnds {
            ix: EdgeIx::new(ix),
            source,
            target,
        })
        .collect();
    assert_eq!(
        route_faults(&scene, &ends),
        [],
        "the golden scene keeps invariant B"
    );
    for (blitter, name) in [
        (Blitter::Braille, "t2_shared_trunk_braille"),
        (Blitter::Ascii, "t2_shared_trunk_ascii"),
    ] {
        assert_snapshot!(name, render(&scene, (40, 9), blitter));
    }
}

/// One graph at semantic levels 0, 2 and 5 (plan §5): a plain label, a pinned glyph node, a
/// pinned CJK label and a pinned box, each in the form its level measures, with edges between
/// their anchors kept off their text.
#[test]
fn t2_node_forms_by_level() {
    let specs = [
        NodeSpec::label("Ada Lovelace"),
        NodeSpec {
            shape: NodeShape::Glyph('●'),
            pinned: Some((0.0, 0.0)),
            ..NodeSpec::label("Charles Babbage")
        },
        NodeSpec {
            pinned: Some((0.0, 0.0)),
            ..NodeSpec::label("日本語テキスト")
        },
        NodeSpec {
            shape: NodeShape::Box { min_w: 0, min_h: 0 },
            pinned: Some((0.0, 0.0)),
            ..NodeSpec::label("Analytical Engine")
        },
    ];
    for level in [0, 2, 5] {
        let forms = forms(&specs, level);
        let mut scene = SceneGraph::new();
        let [ada, babbage, cjk, engine] = [(0, 0), (24, 0), (0, 6), (22, 5)]
            .iter()
            .zip(0..)
            .map(|(&(x, y), ix)| place(&mut scene, ix, &forms[NodeIx::new(ix).slot()], x, y))
            .collect::<Vec<_>>()[..]
        else {
            unreachable!("four nodes")
        };
        for (ix, (from, to)) in
            (0..).zip([(ada, babbage), (ada, cjk), (babbage, engine), (cjk, engine)])
        {
            path(&mut scene, ix, Layer::EdgesUnder, straight(from, to));
        }
        let name = format!("t2_node_forms_level_{level}");
        assert_snapshot!(
            name,
            render_forms(&scene, (44, 10), Blitter::Braille, &forms)
        );
    }
}
