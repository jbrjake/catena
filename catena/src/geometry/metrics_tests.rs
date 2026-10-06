use super::super::testing::at;
use super::*;
use crate::graph::{DeltaClass, EdgeSpec, Limits, NodeShape, NodeSpec};

fn form_at(spec: &NodeSpec, level: usize) -> NodeForm {
    measure(spec, at(level), &SemanticZoomTable::default())
}

fn label(text: &str) -> NodeSpec {
    NodeSpec::label(text)
}

fn pinned(mut spec: NodeSpec) -> NodeSpec {
    spec.pinned = Some((0.0, 0.0));
    spec
}

fn shaped(text: &str, shape: NodeShape) -> NodeSpec {
    NodeSpec {
        shape,
        ..label(text)
    }
}

/// A one-row form as the default glyphs draw it, so tests read like the screen.
pub(super) fn row(form: &NodeForm) -> String {
    let mark = |mark: &Mark| match mark {
        Mark::Text(text) => text.clone(),
        Mark::Dot => "•".to_string(),
    };
    match form.shape() {
        FormShape::Glyph(glyph) => mark(glyph),
        FormShape::Row { pin, icon, label } => {
            let mut out = String::new();
            if *pin {
                out.push('*');
            }
            if let Some(icon) = icon {
                out.push_str(&mark(icon));
            }
            if let Some(label) = label {
                out.push('[');
                out.push_str(&label.text);
                if label.cut {
                    out.push('…');
                }
                out.push(']');
            }
            out
        }
        FormShape::Boxed { .. } => panic!("a boxed form has no one-row text: {form:?}"),
    }
}

/// `(width, height, text)` of a one-row form at each of the six levels.
fn rows(spec: &NodeSpec) -> Vec<(u16, u16, String)> {
    (0..6)
        .map(|level| {
            let form = form_at(spec, level);
            (form.width(), form.height(), row(&form))
        })
        .collect()
}

fn expected(rows: &[(u16, &str)]) -> Vec<(u16, u16, String)> {
    rows.iter()
        .map(|&(width, text)| (width, 1, text.to_string()))
        .collect()
}

// ── Forms by level (plan §5) ────────────────────────────────────────────────

#[test]
fn a_label_node_takes_each_levels_form() {
    assert_eq!(
        rows(&label("Charles Babbage")),
        expected(&[
            (1, "C"),
            (5, "[Ch…]"),
            (14, "[Charles Bab…]"),
            (17, "[Charles Babbage]"),
            (17, "[Charles Babbage]"),
            (17, "[Charles Babbage]"),
        ])
    );
    assert_eq!(
        rows(&label("Ada")),
        expected(&[
            (1, "A"),
            (5, "[Ada]"),
            (5, "[Ada]"),
            (5, "[Ada]"),
            (5, "[Ada]"),
            (5, "[Ada]")
        ]),
        "a label that fits is never cut"
    );
}

#[test]
fn every_level_is_live_up_to_the_label_limit() {
    // Ledger row 27: the seed clamped labels to 22 columns before zoom saw them, so levels 3
    // to 5 never took effect.
    let long = "a".repeat(40);
    let widths: Vec<u16> = rows(&label(&long)).iter().map(|r| r.0).collect();
    assert_eq!(widths, [1, 5, 14, 22, 34, 42]);
}

#[test]
fn a_pinned_node_at_its_cap_shortens_its_label() {
    // Ledger row 4: the seed drew the pin marker one cell past the reserved box.
    assert_eq!(
        rows(&pinned(label("Charles Babbage"))),
        expected(&[
            (1, "C"),
            (5, "*[C…]"),
            (14, "*[Charles Ba…]"),
            (18, "*[Charles Babbage]"),
            (18, "*[Charles Babbage]"),
            (18, "*[Charles Babbage]"),
        ]),
        "below the cap the marker widens the box; at it, the label gives way"
    );
}

#[test]
fn a_cut_leaves_no_blank_before_its_ellipsis_and_no_half_of_a_wide_character() {
    assert_eq!(
        (
            form_at(&label("abcdefghij klmnop"), 2).width(),
            row(&form_at(&label("abcdefghij klmnop"), 2))
        ),
        (13, "[abcdefghij…]".to_string())
    );
    let cjk = label("日本語日本語日本語");
    assert_eq!(
        (form_at(&cjk, 2).width(), row(&form_at(&cjk, 2))),
        (13, "[日本語日本…]".to_string()),
        "a wide character across the cap stays out, and the box is what is drawn"
    );
    for blank in [" \u{301}", " \u{0}"] {
        let spec = label(&format!("abcdefghij{blank}xyz"));
        assert_eq!(
            (form_at(&spec, 2).width(), row(&form_at(&spec, 2))),
            (13, "[abcdefghij…]".to_string()),
            "a blank cell goes whatever rides on it: {blank:?}"
        );
    }
    let marked = label("e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}");
    assert_eq!(
        row(&form_at(&marked, 1)),
        "[e\u{301}e\u{301}…]",
        "a combining mark stays with its base"
    );
}

#[test]
fn level_zero_draws_one_glyph() {
    let glyph = |spec: &NodeSpec| {
        let form = form_at(spec, 0);
        (form.width(), form.height(), form.shape().clone())
    };
    let text = |s: &str| (1, 1, FormShape::Glyph(Mark::Text(s.to_string())));
    let dot = (1, 1, FormShape::Glyph(Mark::Dot));
    assert_eq!(glyph(&label("Ada")), text("A"));
    assert_eq!(glyph(&label("  ada")), text("a"), "the first visible cell");
    assert_eq!(glyph(&label("e\u{301}x")), text("e\u{301}"), "a whole cell");
    assert_eq!(glyph(&label("日本")), dot, "too wide for the cap");
    assert_eq!(glyph(&label("")), dot);
    assert_eq!(glyph(&label(" \t\u{301}")), dot);
    assert_eq!(glyph(&shaped("Ada", NodeShape::Glyph('●'))), text("●"));
    assert_eq!(glyph(&shaped("Ada", NodeShape::Glyph('\u{301}'))), dot);
    assert_eq!(glyph(&shaped("Ada", NodeShape::Glyph('日'))), dot);
    assert_eq!(
        glyph(&pinned(label("Ada"))),
        text("A"),
        "no room for the marker"
    );
    let boxed = shaped("Ada", NodeShape::Box { min_w: 9, min_h: 5 });
    assert_eq!(glyph(&boxed), text("A"), "a box shrinks to a glyph too");

    let wide = SemanticZoomTable::new([0.30, 0.35, 1.5, 2.5, 3.5], [2, 5, 14, 22, 34])
        .expect("a valid table");
    let form = measure(&label("日本"), at(0), &wide);
    assert_eq!(
        (form.width(), form.shape()),
        (2, &FormShape::Glyph(Mark::Text("日".to_string()))),
        "a two-column cap holds a wide glyph"
    );
    assert_eq!(
        measure(&label("Ada"), at(0), &wide).width(),
        1,
        "and only what is drawn"
    );
}

#[test]
fn a_glyph_node_prefixes_its_label_with_its_glyph() {
    let spec = shaped("Ada", NodeShape::Glyph('●'));
    assert_eq!(
        rows(&spec),
        expected(&[
            (1, "●"),
            (5, "●[A…]"),
            (6, "●[Ada]"),
            (6, "●[Ada]"),
            (6, "●[Ada]"),
            (6, "●[Ada]")
        ])
    );
    assert_eq!(row(&form_at(&pinned(spec), 2)), "*●[Ada]");
    let bare = shaped("", NodeShape::Glyph('●'));
    assert_eq!(
        rows(&bare).iter().map(|r| r.2.as_str()).collect::<Vec<_>>(),
        ["●"; 6],
        "with no label it is the glyph alone"
    );
    assert_eq!(row(&form_at(&pinned(bare), 3)), "*●");
}

#[test]
fn an_empty_label_node_draws_empty_brackets() {
    assert_eq!(row(&form_at(&label(""), 5)), "[]");
}

#[test]
fn a_cap_too_narrow_for_brackets_falls_back_to_the_glyph() {
    let narrow = SemanticZoomTable::new([0.30, 0.35, 1.5, 2.5, 3.5], [1, 2, 14, 22, 34])
        .expect("a valid table");
    let form = measure(&label("Ada"), at(1), &narrow);
    assert_eq!(
        (form.width(), form.shape()),
        (1, &FormShape::Glyph(Mark::Text("A".to_string())))
    );
    let boxed = shaped("Ada", NodeShape::Box { min_w: 0, min_h: 0 });
    assert_eq!(measure(&boxed, at(1), &narrow).width(), 1);
    assert_eq!(
        measure(&label("A"), at(1), &narrow).width(),
        1,
        "`[A]` takes three"
    );
}

// ── Boxes ───────────────────────────────────────────────────────────────────

fn boxed_at(spec: &NodeSpec, level: usize) -> (u16, u16, bool, Vec<String>) {
    let form = form_at(spec, level);
    let FormShape::Boxed { pin, lines } = form.shape() else {
        panic!("not a box: {form:?}");
    };
    (form.width(), form.height(), *pin, lines.clone())
}

fn lines(texts: &[&str]) -> Vec<String> {
    texts.iter().map(ToString::to_string).collect()
}

#[test]
fn a_box_wraps_its_label_to_its_capped_width_and_shrinks_around_it() {
    let spec = shaped("Charles Babbage", NodeShape::Box { min_w: 0, min_h: 0 });
    assert_eq!(
        boxed_at(&spec, 2),
        (9, 4, false, lines(&["Charles", "Babbage"])),
        "inside 14 columns the label wraps at 12, and the box fits the wider line"
    );
    assert_eq!(
        boxed_at(&spec, 5),
        (17, 3, false, lines(&["Charles Babbage"]))
    );
    let paragraphs = shaped("one\n\nthree", NodeShape::Box { min_w: 0, min_h: 0 });
    assert_eq!(
        boxed_at(&paragraphs, 5),
        (7, 5, false, lines(&["one", "", "three"]))
    );
}

#[test]
fn a_box_honors_its_minimums_under_its_cap() {
    let spec = shaped(
        "Hi",
        NodeShape::Box {
            min_w: 20,
            min_h: 5,
        },
    );
    assert_eq!(boxed_at(&spec, 5), (20, 5, false, lines(&["Hi"])));
    assert_eq!(
        boxed_at(&spec, 2),
        (14, 5, false, lines(&["Hi"])),
        "the level's cap wins over the minimum width"
    );
    let empty = shaped("", NodeShape::Box { min_w: 0, min_h: 0 });
    assert_eq!(
        boxed_at(&empty, 5),
        (3, 3, false, lines(&[])),
        "the least box has one blank cell inside its border"
    );
}

#[test]
fn a_pinned_box_marks_its_border_and_keeps_its_size() {
    let spec = shaped("Ada", NodeShape::Box { min_w: 0, min_h: 0 });
    assert_eq!(boxed_at(&spec, 2), (5, 3, false, lines(&["Ada"])));
    assert_eq!(boxed_at(&pinned(spec), 2), (5, 3, true, lines(&["Ada"])));
}

#[test]
fn a_box_draws_no_phantom_rows_for_invisible_words() {
    let spec = shaped("a \u{301}", NodeShape::Box { min_w: 3, min_h: 0 });
    assert_eq!(boxed_at(&spec, 1), (3, 3, false, lines(&["a"])));
}

#[test]
fn the_anchor_is_the_middle_cell_rounding_up_and_left() {
    let anchor = |spec: &NodeSpec, level: usize| {
        let form = form_at(spec, level);
        (form.width(), form.height(), form.anchor())
    };
    assert_eq!(anchor(&label("Ada"), 2), (5, 1, CellPt::new(2, 0)));
    assert_eq!(anchor(&label("Ad"), 2), (4, 1, CellPt::new(1, 0)));
    assert_eq!(anchor(&label("Ada"), 0), (1, 1, CellPt::new(0, 0)));
    let spec = shaped("Charles Babbage", NodeShape::Box { min_w: 0, min_h: 0 });
    assert_eq!(anchor(&spec, 2), (9, 4, CellPt::new(4, 1)));
}

// ── ResolvedMetrics over a store ────────────────────────────────────────────

type Store = GraphStore<&'static str>;

fn store_with(nodes: &[(&'static str, &str)]) -> Store {
    let mut store = GraphStore::new(256, Limits::default());
    store
        .transact(|tx| {
            for &(key, text) in nodes {
                tx.add_node(key, label(text))?;
            }
            tx.add_edge(&nodes[0].0, &nodes[1].0, EdgeSpec::directed())?;
            Ok(())
        })
        .expect("fresh keys");
    store
}

fn width_of(metrics: &ResolvedMetrics, store: &Store, key: &'static str) -> Option<u16> {
    let ix = store.ix_of(&key)?;
    metrics.form(ix).map(NodeForm::width)
}

#[test]
fn metrics_measure_every_live_node_at_their_level() {
    let store = store_with(&[("a", "Ada"), ("b", "Charles Babbage")]);
    let mut metrics = ResolvedMetrics::new(SemanticZoomTable::default(), at(2));
    metrics.measure_all(&store);
    assert_eq!(metrics.level(), at(2));
    assert_eq!(width_of(&metrics, &store, "a"), Some(5));
    assert_eq!(width_of(&metrics, &store, "b"), Some(14));

    let b = store.ix_of(&"b").expect("live");
    assert_eq!(
        metrics
            .set_level(&store, at(5))
            .into_iter()
            .collect::<Vec<_>>(),
        [b],
        "a new level measures again, and only b's box grows"
    );
    assert_eq!(width_of(&metrics, &store, "b"), Some(17));
    assert_eq!(width_of(&metrics, &store, "a"), Some(5));
    assert!(
        metrics.set_level(&store, at(5)).is_empty(),
        "the same level is no change"
    );
    assert!(
        metrics
            .set_level(&store, SemanticZoomTable::default().level(3.9))
            .is_empty(),
        "another zoom in the same level is no change"
    );
}

#[test]
fn applying_a_delta_keeps_only_the_reshapes_that_changed_a_box() {
    let mut store = store_with(&[("a", "Ada"), ("b", "Charles Babbage"), ("c", "c")]);
    let mut metrics = ResolvedMetrics::new(SemanticZoomTable::default(), at(2));
    metrics.measure_all(&store);
    let ix = |store: &Store, key| store.ix_of(&key).expect("live");

    let ((), mut delta) = store
        .transact(|tx| {
            // Geometry by the store's reckoning, but both cut to the same 14 columns.
            tx.set_node(&"b", |n| n.label = "Charles Babbage!".into())?;
            // A box that does change.
            tx.set_node(&"a", |n| n.label = "Ada Lovelace".into())?;
            // A property edit: new text in the same box.
            tx.set_node(&"c", |n| n.label = "d".into())
        })
        .expect("known");
    assert_eq!(delta.class, Some(DeltaClass::Geometry));
    assert_eq!(delta.reshaped.len(), 2);
    metrics.apply(&store, &mut delta);
    assert_eq!(delta.reshaped.len(), 1);
    assert_eq!(delta.reshaped.first(), Some(&ix(&store, "a")));
    assert_eq!(width_of(&metrics, &store, "a"), Some(14));
    let c = metrics.form(ix(&store, "c")).expect("live");
    assert_eq!(row(c), "[d]", "a relabeled node is measured again");

    let ((), mut delta) = store
        .transact(|tx| {
            tx.remove_node(&"c")?;
            tx.add_node("e", label("Eve"))?;
            tx.set_node(&"a", |n| n.pinned = Some((1.0, 1.0)))
        })
        .expect("known");
    let repinned = delta.repinned.clone();
    metrics.apply(&store, &mut delta);
    assert_eq!(
        delta.reshaped.len(),
        0,
        "at its cap the marker shortens the label instead"
    );
    assert_eq!(delta.repinned, repinned, "a pin edit still re-snaps");
    assert_eq!(width_of(&metrics, &store, "e"), Some(5));
    assert_eq!(width_of(&metrics, &store, "c"), None);
    let mut fresh = ResolvedMetrics::new(SemanticZoomTable::default(), at(2));
    fresh.measure_all(&store);
    assert_eq!(
        metrics, fresh,
        "following deltas ends where measuring afresh does"
    );
}
