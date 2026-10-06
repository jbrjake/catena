use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use super::*;
use crate::raster::text::{char_width, display_width, word_wrap};

fn sanitized(mut spec: NodeSpec, max_label_cols: u16) -> NodeSpec {
    spec.sanitize(max_label_cols);
    spec
}

fn label_cut_to(text: &str, max_label_cols: u16) -> String {
    sanitized(NodeSpec::label(text), max_label_cols).label
}

#[test]
fn a_label_is_cut_to_whole_display_columns() {
    assert_eq!(label_cut_to("abcdef", 4), "abcd");
    assert_eq!(label_cut_to("abc", 4), "abc");
    // A double-width character that would straddle the limit goes whole.
    assert_eq!(label_cut_to("日本語", 5), "日本");
    assert_eq!(label_cut_to("a日", 2), "a");
    // A combining mark takes no column, so it rides with the character before it.
    assert_eq!(label_cut_to("abc\u{301}d", 3), "abc\u{301}");
    assert_eq!(label_cut_to("anything", 0), "");
}

#[test]
fn a_label_is_cut_to_fifteen_bytes_a_column_whatever_its_width() {
    let marks = format!("a{}", "\u{301}".repeat(1000));
    let cut = label_cut_to(&marks, 2);
    assert!(cut.len() <= 30, "{} bytes", cut.len());
    assert!(cut.starts_with('a'));
    assert_eq!(cut.len(), 29, "a, then whole two-byte marks up to the cap");
    let breaks = "\n".repeat(1000);
    assert_eq!(label_cut_to(&breaks, 4).len(), 60);
}

#[test]
fn a_cut_label_gives_back_its_memory() {
    let huge = "x".repeat(1 << 20);
    let spec = sanitized(NodeSpec::label(huge), 8);
    assert_eq!(spec.label, "xxxxxxxx");
    assert!(spec.label.capacity() < 1024, "{}", spec.label.capacity());
}

#[test]
fn a_sort_key_is_cut_like_a_label() {
    let mut spec = NodeSpec::label("short");
    spec.sort_key = Some("abcdefgh".into());
    assert_eq!(sanitized(spec, 3).sort_key.as_deref(), Some("abc"));
}

#[test]
fn weights_are_finite_and_never_negative() {
    let weight = |w: f32| {
        let mut spec = NodeSpec::label("n");
        spec.weight = w;
        sanitized(spec, 8).weight
    };
    assert_eq!(weight(f32::NAN).to_bits(), 1.0f32.to_bits());
    assert_eq!(weight(-3.0).to_bits(), 0.0f32.to_bits());
    assert_eq!(weight(f32::NEG_INFINITY).to_bits(), 0.0f32.to_bits());
    assert_eq!(weight(f32::INFINITY).to_bits(), f32::MAX.to_bits());
    assert_eq!(weight(-0.0).to_bits(), 0.0f32.to_bits());
    assert_eq!(weight(2.5).to_bits(), 2.5f32.to_bits());

    let edge_weight = |w: f32| {
        let mut spec = EdgeSpec::directed();
        spec.weight = w;
        spec.sanitize();
        spec.weight
    };
    assert_eq!(edge_weight(f32::NAN).to_bits(), 1.0f32.to_bits());
    assert_eq!(edge_weight(-1.0).to_bits(), 0.0f32.to_bits());
    assert_eq!(edge_weight(f32::INFINITY).to_bits(), f32::MAX.to_bits());
    assert_eq!(edge_weight(0.25).to_bits(), 0.25f32.to_bits());
}

#[test]
fn pins_are_finite_and_inside_the_cell_range() {
    let pin = |p: (f64, f64)| {
        let mut spec = NodeSpec::label("n");
        spec.pinned = Some(p);
        sanitized(spec, 8)
            .pinned
            .map(|(x, y)| (x.to_bits(), y.to_bits()))
    };
    let bits = |x: f64, y: f64| Some((x.to_bits(), y.to_bits()));
    let limit = f64::from(i32::MAX);
    assert_eq!(pin((f64::NAN, 1.0)), None);
    assert_eq!(pin((1.0, f64::NAN)), None);
    assert_eq!(pin((1e300, f64::NEG_INFINITY)), bits(limit, -limit));
    assert_eq!(pin((-0.0, 3.5)), bits(0.0, 3.5));
    assert_eq!(pin((-7.25, 1e6)), bits(-7.25, 1e6));
}

#[test]
fn a_node_sorts_by_its_sort_key_else_its_label() {
    let mut spec = NodeSpec::label("Ada");
    assert_eq!(spec.order_key(), "Ada");
    spec.sort_key = Some("lovelace".into());
    assert_eq!(spec.order_key(), "lovelace");
}

#[test]
fn labels_lay_out_alike_when_their_widths_and_whitespace_match() {
    assert!(same_layout("Alice", "Bobby"));
    assert!(same_layout("", ""));
    assert!(same_layout("a b", "c d"));
    assert!(same_layout("e\u{301}", "a\u{302}"), "a mark for a mark");
    assert!(same_layout("日本", "😀語"));
    assert!(
        same_layout("x\u{200b}", "x\u{1b}"),
        "any width-0 character for another"
    );

    assert!(!same_layout("Ada", "Adam"));
    assert!(!same_layout("a b", "ab "), "whitespace moved");
    assert!(
        !same_layout("日", "ab"),
        "one wide cell is not two narrow ones"
    );
    assert!(
        !same_layout("a\tb", "a\nb"),
        "a tab separates words, a newline breaks a line"
    );
    assert!(!same_layout("a b", "a\u{a0}b"), "two different spaces");
    assert!(
        !same_layout("\u{301}", ""),
        "an invisible label wraps to an empty line, an empty one to none"
    );
    assert!(
        !same_layout("a \u{301}", "a "),
        "a word of width-0 characters still takes a line"
    );
    assert!(!same_layout("é", "e\u{301}"), "so a mark is never ignored");
}

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x7370_6563),
        failure_persistence: None,
        ..Config::default()
    }
}

fn text() -> impl Strategy<Value = String> {
    let piece = prop::sample::select(vec![
        "a", " ", "日", "😀", "\u{301}", "\n", "\t", "\x1b", "e\u{301}", "\u{200d}",
    ]);
    prop::collection::vec(piece, 0..24).prop_map(|pieces| pieces.concat())
}

proptest! {
    #![proptest_config(config())]

    /// The cut is the longest prefix inside both bounds, and cutting again changes nothing.
    #[test]
    fn a_cut_label_is_the_longest_prefix_inside_both_bounds(text in text(), max in 0u16..12) {
        let cut = label_cut_to(&text, max);
        let cols = usize::from(max);
        prop_assert!(text.starts_with(cut.as_str()));
        prop_assert!(display_width(&cut) <= cols);
        prop_assert!(cut.len() <= cols * 15);
        if let Some(next) = text[cut.len()..].chars().next() {
            let too_wide = display_width(&cut) + char_width(next) > cols;
            let too_long = cut.len() + next.len_utf8() > cols * 15;
            prop_assert!(too_wide || too_long, "{next:?} would have fit");
        }
        prop_assert_eq!(label_cut_to(&cut, max), cut);
    }

    /// Labels lay out alike exactly when their [`relabel`]s are equal, so checking each label
    /// against its own relabel shows that labels that lay out alike word-wrap alike, line for
    /// line, at every width.
    #[test]
    fn labels_that_lay_out_alike_wrap_alike(a in text(), b in text()) {
        let swapped = relabel(&a);
        prop_assert!(same_layout(&a, &swapped), "{:?} vs {:?}", a, swapped);
        let wrapped = |t: &str, width| {
            word_wrap(t, width).iter().map(|line| display_width(line)).collect::<Vec<_>>()
        };
        prop_assert_eq!(same_layout(&a, &b), relabel(&a) == relabel(&b));
        for width in 0..8 {
            prop_assert_eq!(wrapped(&a, width), wrapped(&swapped, width));
        }
    }
}

/// The canonical label of `text`'s layout class: whitespace kept, every other character
/// replaced by one of its width (a zero-width space, `z` or `国`).
fn relabel(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match (c.is_whitespace(), char_width(c)) {
            (true, _) => out.push(c),
            (false, 0) => out.push('\u{200b}'),
            (false, 1) => out.push('z'),
            (false, _) => out.push('国'),
        }
    }
    out
}
