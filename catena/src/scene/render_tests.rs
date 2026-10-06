use std::sync::LazyLock;

use super::*;
use crate::geometry::cell::{CellBox, CellPt, SubPt};
use crate::geometry::testing::forms;
use crate::graph::{EdgeIx, NodeSpec};
use crate::raster::{CellGrid, PaletteColor};
use crate::scene::{CountBadge, Decoration, EdgeRoute, Route, SceneItem};

const GLOW: StyleId = StyleId(9);

/// Style `n` is palette color `n`; `GLOW` carries a background.
fn styles(style: StyleId) -> CellStyle {
    let fg = PaletteColor::Indexed(u8::try_from(style.0).unwrap_or(255));
    if style == GLOW {
        CellStyle {
            bg: Some(PaletteColor::Rgb(40, 30, 60)),
            ..CellStyle::new(fg)
        }
    } else {
        CellStyle::new(fg)
    }
}

/// Node 0 is `[node]`, node 1 `[日本 node]`: full labels, at semantic level 5.
static FORMS: LazyLock<Vec<NodeForm>> =
    LazyLock::new(|| forms(&[NodeSpec::label("node"), NodeSpec::label("日本 node")], 5));

fn labels(ix: NodeIx) -> Option<&'static NodeForm> {
    FORMS.get(ix.slot())
}

fn ascii() -> RenderOptions {
    RenderOptions {
        blitter: Blitter::Ascii,
        ..RenderOptions::default()
    }
}

fn render(scene: &SceneGraph, (w, h): (u16, u16), options: &RenderOptions) -> CellGrid {
    let mut grid = CellGrid::new(w, h);
    Compositor::new().render(scene, &mut grid, options, styles, labels);
    grid
}

fn line(points: &[(f64, f64)]) -> Route {
    Route::Polyline(points.iter().map(|&(x, y)| SubPt::new(x, y)).collect())
}

fn cells(points: &[(i32, i32)]) -> Route {
    Route::Orthogonal(points.iter().map(|&(x, y)| CellPt::new(x, y)).collect())
}

fn edge(ix: u32, route: Route, z: Layer, style: StyleId) -> SceneItem {
    let bounds = route.bounds().expect("points");
    SceneItem::new(
        z,
        bounds,
        Payload::EdgePath {
            ix: EdgeIx::new(ix),
            route: EdgeRoute::Path(route),
        },
        style,
    )
}

fn node(ix: u32, bounds: CellBox) -> SceneItem {
    SceneItem::new(
        Layer::Nodes,
        bounds,
        Payload::NodeBox {
            ix: NodeIx::new(ix),
        },
        StyleId(5),
    )
}

fn fg(grid: &CellGrid, x: u16, y: u16) -> Option<PaletteColor> {
    grid.cell(x, y).and_then(crate::raster::Cell::fg)
}

#[test]
fn polylines_on_two_layers_or_merge_and_the_upper_layer_colors_the_crossing() {
    let mut scene = SceneGraph::new();
    scene.push(edge(
        0,
        line(&[(0.0, 1.0), (4.0, 1.0)]),
        Layer::EdgesOver,
        StyleId(2),
    ));
    scene.push(edge(
        1,
        line(&[(2.0, 0.0), (2.0, 2.0)]),
        Layer::EdgesUnder,
        StyleId(1),
    ));
    let grid = render(&scene, (5, 3), &RenderOptions::default());
    // Row y = 1 maps to sub-pixel row 4·1 + 1.5 → 6, the third dot row; column x = 2 to
    // 2·2 + 0.5 → 5, the right dot column (half rounds away from zero).
    assert_eq!(grid.to_string(), "  ⢠\n⠠⠤⢼⠤⠤\n  ⠸");
    assert_eq!(
        fg(&grid, 2, 1),
        Some(PaletteColor::Indexed(2)),
        "EdgesOver is on top"
    );
    assert_eq!(fg(&grid, 2, 0), Some(PaletteColor::Indexed(1)));
}

#[test]
fn an_integer_point_lights_its_own_cell_in_every_blitter() {
    for blitter in [
        Blitter::Braille,
        Blitter::HalfBlock,
        Blitter::Sextant,
        Blitter::Ascii,
    ] {
        let mut scene = SceneGraph::new();
        scene.push(edge(0, line(&[(3.0, 1.0)]), Layer::EdgesUnder, StyleId(1)));
        let grid = render(
            &scene,
            (5, 3),
            &RenderOptions {
                blitter,
                ..RenderOptions::default()
            },
        );
        let lit: Vec<(u16, u16)> = (0..3)
            .flat_map(|y| (0..5).map(move |x| (x, y)))
            .filter(|&(x, y)| grid.cell(x, y).is_some_and(|c| c.symbol() != " "))
            .collect();
        assert_eq!(lit, [(3, 1)], "{blitter:?}");
    }
}

#[test]
fn orthogonal_routes_draw_box_glyphs_whatever_the_blitter() {
    let mut scene = SceneGraph::new();
    scene.push(edge(
        0,
        cells(&[(0, 0), (3, 0), (3, 2)]),
        Layer::EdgesUnder,
        StyleId(1),
    ));
    scene.push(edge(
        1,
        cells(&[(0, 1), (5, 1)]),
        Layer::EdgesUnder,
        StyleId(2),
    ));
    for options in [RenderOptions::default(), ascii()] {
        let grid = render(&scene, (6, 3), &options);
        assert_eq!(grid.to_string(), "───┐\n───┼──\n   │");
        assert_eq!(
            fg(&grid, 3, 1),
            Some(PaletteColor::Indexed(2)),
            "the later route"
        );
    }
    let boxed = RenderOptions {
        boxes: BoxGlyphs::ASCII,
        ..ascii()
    };
    assert_eq!(
        render(&scene, (6, 3), &boxed).to_string(),
        "---+\n---+--\n   |"
    );
}

#[test]
fn edges_never_overprint_node_text() {
    let mut scene = SceneGraph::new();
    scene.push(edge(
        0,
        line(&[(0.0, 1.0), (9.0, 1.0)]),
        Layer::EdgesUnder,
        StyleId(1),
    ));
    scene.push(edge(
        1,
        cells(&[(4, 0), (4, 2)]),
        Layer::EdgesUnder,
        StyleId(1),
    ));
    scene.push(node(0, CellBox::new(2, 1, 6, 1)));
    let grid = render(&scene, (10, 3), &ascii());
    assert_eq!(grid.to_string(), "    │\n──[node]──\n    │");
    assert_eq!(fg(&grid, 2, 1), Some(PaletteColor::Indexed(5)));
}

#[test]
fn a_node_label_clips_to_its_box_and_to_the_surface() {
    let mut scene = SceneGraph::new();
    scene.push(node(1, CellBox::new(-2, 0, 7, 1)));
    scene.push(node(0, CellBox::new(4, 1, 2, 1)));
    scene.push(node(0, CellBox::new(6, 2, 9, 1)));
    let grid = render(&scene, (8, 3), &ascii());
    assert_eq!(
        grid.to_string(),
        " 本 n\n    [n\n      [n",
        "the half-visible 日 and the overhanging tail are dropped"
    );
}

#[test]
fn glow_tints_the_background_and_keeps_the_glyph() {
    let mut scene = SceneGraph::new();
    scene.push(edge(
        0,
        line(&[(0.0, 0.0), (3.0, 0.0)]),
        Layer::EdgesUnder,
        StyleId(1),
    ));
    scene.push(SceneItem::new(
        Layer::Glow,
        CellBox::new(1, 0, 2, 2),
        Payload::Decoration(Decoration::Glow),
        GLOW,
    ));
    scene.push(node(0, CellBox::new(2, 1, 4, 1)));
    let grid = render(&scene, (6, 2), &ascii());
    assert_eq!(grid.to_string(), "────\n  [nod");
    let halo = Some(PaletteColor::Rgb(40, 30, 60));
    let at = |x, y| grid.cell(x, y).expect("in range");
    assert_eq!((at(1, 0).symbol(), at(1, 0).bg()), ("─", halo));
    assert_eq!(
        fg(&grid, 1, 0),
        Some(PaletteColor::Indexed(1)),
        "the edge keeps its color"
    );
    assert_eq!(
        (at(2, 1).symbol(), at(2, 1).bg()),
        ("[", halo),
        "text drawn over a halo keeps it"
    );
    assert_eq!(at(4, 1).bg(), None, "outside the halo");
    assert_eq!(at(0, 0).bg(), None);
}

#[test]
fn count_badge_rides_any_item() {
    let badge = |x, y| CountBadge {
        count: 3,
        at: CellPt::new(x, y),
    };
    let mut scene = SceneGraph::new();
    scene.push(node(0, CellBox::new(0, 0, 8, 1)).with_badge(badge(6, 0)));
    scene.push(
        edge(
            0,
            line(&[(0.0, 2.0), (9.0, 2.0)]),
            Layer::EdgesUnder,
            StyleId(1),
        )
        .with_badge(badge(4, 2)),
    );
    let trunk = scene.push_segment(
        Layer::EdgesUnder,
        StyleId(2),
        cells(&[(0, 4), (9, 4)]),
        vec![EdgeIx::new(1)],
    );
    scene.push(
        SceneItem::new(
            Layer::EdgesUnder,
            CellBox::new(0, 4, 10, 1),
            Payload::EdgePath {
                ix: EdgeIx::new(1),
                route: EdgeRoute::Chain(vec![trunk]),
            },
            StyleId(2),
        )
        .with_badge(badge(1, 4)),
    );
    scene.push(
        SceneItem::new(
            Layer::Glow,
            CellBox::new(0, 6, 4, 1),
            Payload::Decoration(Decoration::Glow),
            GLOW,
        )
        .with_badge(badge(0, 6)),
    );
    let grid = render(&scene, (10, 7), &ascii());
    assert_eq!(
        grid.to_string(),
        "[node]×3\n\n────×3────\n\n─×3───────\n\n×3"
    );
}

#[test]
fn a_badge_clips_to_its_item() {
    let mut scene = SceneGraph::new();
    scene.push(node(0, CellBox::new(0, 0, 6, 1)).with_badge(CountBadge {
        count: 1234,
        at: CellPt::new(3, 0),
    }));
    assert_eq!(render(&scene, (10, 1), &ascii()).to_string(), "[no×12");
}

#[test]
fn a_chained_edge_draws_only_through_its_segments() {
    let mut scene = SceneGraph::new();
    let a = scene.push_segment(
        Layer::EdgesUnder,
        StyleId(1),
        line(&[(0.0, 0.0), (3.0, 0.0)]),
        vec![EdgeIx::new(0)],
    );
    let b = scene.push_segment(
        Layer::EdgesUnder,
        StyleId(1),
        line(&[(3.0, 0.0), (3.0, 2.0)]),
        vec![EdgeIx::new(0)],
    );
    scene.push(SceneItem::new(
        Layer::EdgesOver,
        CellBox::new(0, 0, 9, 9),
        Payload::EdgePath {
            ix: EdgeIx::new(0),
            route: EdgeRoute::Chain(vec![a, b]),
        },
        StyleId(7),
    ));
    let grid = render(&scene, (5, 3), &ascii());
    // Both segments light (3, 0), so its ascii directions OR into a junction.
    assert_eq!(grid.to_string(), "───┼\n   │\n   │");
    assert_eq!(
        fg(&grid, 0, 0),
        Some(PaletteColor::Indexed(1)),
        "the segments' style, not the edge's"
    );
}

#[test]
fn a_reused_compositor_renders_like_a_fresh_one() {
    let mut busy = SceneGraph::new();
    busy.push(edge(
        0,
        line(&[(0.0, 0.0), (9.0, 4.0)]),
        Layer::EdgesUnder,
        StyleId(1),
    ));
    busy.push(edge(
        1,
        cells(&[(0, 3), (9, 3)]),
        Layer::EdgesOver,
        StyleId(2),
    ));
    busy.push(node(0, CellBox::new(1, 1, 4, 1)));
    let mut quiet = SceneGraph::new();
    quiet.push(edge(
        2,
        line(&[(9.0, 0.0), (0.0, 4.0)]),
        Layer::EdgesUnder,
        StyleId(3),
    ));
    for options in [RenderOptions::default(), ascii()] {
        let mut compositor = Compositor::new();
        let mut reused = CellGrid::new(10, 5);
        compositor.render(&busy, &mut reused, &options, styles, labels);
        let mut reused = CellGrid::new(10, 5);
        compositor.render(&quiet, &mut reused, &options, styles, labels);
        assert_eq!(
            reused,
            render(&quiet, (10, 5), &options),
            "{:?}",
            options.blitter
        );
    }
}

#[test]
fn rendering_is_total_on_empty_scenes_and_surfaces() {
    let mut scene = SceneGraph::new();
    scene.push(edge(
        0,
        line(&[(-1e300, f64::NAN), (1e300, 3.0)]),
        Layer::EdgesUnder,
        StyleId(1),
    ));
    scene.push(node(
        0,
        CellBox::new(i32::MAX, i32::MIN, u32::MAX, u32::MAX),
    ));
    for size in [(0, 0), (0, 5), (5, 0), (3, 3)] {
        let _ = render(&scene, size, &RenderOptions::default());
        let _ = render(&SceneGraph::new(), size, &ascii());
    }
}
