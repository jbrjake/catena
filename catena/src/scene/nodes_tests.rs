use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use super::*;
use crate::geometry::SemanticZoomTable;
use crate::geometry::testing::{any_shape, at, awkward_label, every_level_and_cap, measured, node};
use crate::graph::NodeShape;
use crate::raster::{CellGrid, PaletteColor};
use crate::scene::{Compositor, Layer, Payload, RenderOptions, SceneGraph, SceneItem, StyleId};

fn style(_: StyleId) -> CellStyle {
    CellStyle::new(PaletteColor::Indexed(5))
}

/// `form` drawn as node 0 with its top left at `(1, 1)`, in bounds wide enough that only the
/// form itself limits what is drawn, on a grid a cell larger all round.
fn draw(form: &NodeForm, options: &RenderOptions) -> CellGrid {
    let (w, h) = (form.width(), form.height());
    let mut grid = CellGrid::new(w.saturating_add(2), h.saturating_add(2));
    let mut scene = SceneGraph::new();
    scene.push(SceneItem::new(
        Layer::Nodes,
        CellBox::new(1, 1, u32::MAX, u32::MAX),
        Payload::NodeBox {
            ix: crate::graph::NodeIx::new(0),
        },
        StyleId(5),
    ));
    Compositor::new().render(&scene, &mut grid, options, style, |_| Some(form));
    grid
}

fn ascii() -> RenderOptions {
    RenderOptions {
        boxes: BoxGlyphs::ASCII,
        nodes: NodeGlyphs::ASCII,
        ..RenderOptions::default()
    }
}

fn spec_at(spec: &crate::graph::NodeSpec, level: usize) -> NodeForm {
    measured(spec, at(level), &SemanticZoomTable::default())
}

#[test]
fn a_row_draws_marker_glyph_brackets_and_ellipsis() {
    let spec = node("Charles Babbage", NodeShape::Glyph('●'), true);
    let form = spec_at(&spec, 2);
    assert_eq!(
        draw(&form, &RenderOptions::default()).to_string(),
        "\n *●[Charles B…]\n"
    );
    assert_eq!(draw(&form, &ascii()).to_string(), "\n *●[Charles B~]\n");
    let dot = spec_at(&node("日本", NodeShape::Label, false), 0);
    assert_eq!(draw(&dot, &RenderOptions::default()).to_string(), "\n •\n");
    assert_eq!(draw(&dot, &ascii()).to_string(), "\n o\n");
}

#[test]
fn a_box_draws_its_border_pin_marker_and_centered_lines() {
    let spec = node(
        "Charles Babbage",
        NodeShape::Box {
            min_w: 11,
            min_h: 5,
        },
        true,
    );
    let form = spec_at(&spec, 2);
    assert_eq!((form.width(), form.height()), (11, 5));
    assert_eq!(
        draw(&form, &RenderOptions::default()).to_string(),
        "\n ┌*────────┐\n │ Charles │\n │ Babbage │\n │         │\n └─────────┘\n",
        "two lines in three rows sit high; seven columns in nine sit centered"
    );
    assert_eq!(
        draw(&form, &ascii()).to_string(),
        "\n +*--------+\n | Charles |\n | Babbage |\n |         |\n +---------+\n"
    );
}

#[test]
fn a_node_with_no_form_draws_nothing() {
    let mut scene = SceneGraph::new();
    scene.push(SceneItem::new(
        Layer::Nodes,
        CellBox::new(0, 0, 5, 1),
        Payload::NodeBox {
            ix: crate::graph::NodeIx::new(3),
        },
        StyleId(5),
    ));
    let mut grid = CellGrid::new(5, 1);
    Compositor::new().render(&scene, &mut grid, &RenderOptions::default(), style, |_| {
        None
    });
    assert_eq!(grid, CellGrid::new(5, 1));
}

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x6e6f_6465),
        failure_persistence: None,
        ..Config::default()
    }
}

proptest! {
    #![proptest_config(config())]

    /// Invariant N (plan §16.2): the cells a form draws are exactly the cells it measures, so
    /// what the layout reserves is what the renderer fills, CJK, emoji and marks included.
    #[test]
    fn invariant_n_every_form_draws_exactly_its_measured_box(
        text in awkward_label(),
        shape in any_shape(),
        pin in any::<bool>(),
        ascii_glyphs in any::<bool>(),
    ) {
        let spec = node(&text, shape, pin);
        let options = if ascii_glyphs { ascii() } else { RenderOptions::default() };
        for (level, table) in every_level_and_cap() {
            let form = measured(&spec, level, &table);
            let grid = draw(&form, &options);
            let (w, h) = (form.width(), form.height());
            for (y, row) in (0u16..).zip(grid.rows()) {
                for (x, cell) in (0u16..).zip(row) {
                    let inside = (1..=w).contains(&x) && (1..=h).contains(&y);
                    prop_assert_eq!(
                        cell.fg().is_some(),
                        inside,
                        "({}, {}) of {:?} at {:?}:\n{}", x, y, form, level, grid
                    );
                }
            }
        }
    }
}
