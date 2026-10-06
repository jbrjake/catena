use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use super::*;

#[test]
fn word_wrap_empty() {
    assert_eq!(word_wrap("", 20), [] as [String; 0]);
}

#[test]
fn word_wrap_fits_single_line() {
    assert_eq!(word_wrap("hello world", 20), vec!["hello world"]);
}

#[test]
fn word_wrap_breaks_on_space() {
    assert_eq!(word_wrap("hello world foo", 11), vec!["hello world", "foo"],);
}

#[test]
fn word_wrap_long_word_char_split() {
    assert_eq!(word_wrap("abcdefghij", 5), vec!["abcde", "fghij"],);
}

#[test]
fn word_wrap_mixed_long_and_short() {
    assert_eq!(
        word_wrap("hi abcdefghij bye", 5),
        vec!["hi", "abcde", "fghij", "bye"],
    );
}

#[test]
fn word_wrap_exact_width() {
    assert_eq!(word_wrap("12345", 5), vec!["12345"]);
}

#[test]
fn word_wrap_width_zero_returns_empty() {
    assert_eq!(word_wrap("hello", 0), [] as [String; 0]);
}

#[test]
fn word_wrap_preserves_multiple_spaces_as_breaks() {
    let result = word_wrap("hello   world", 8);
    assert_eq!(result, vec!["hello", "world"]);
}

#[test]
fn word_wrap_realistic_snippet() {
    let text = "Critical Infrastructure facilities SHARE:CYBERSECURITY ADVISORY";
    let result = word_wrap(text, 20);
    for line in &result {
        assert!(display_width(line) <= 20, "line too long: {line:?}");
    }
    assert_eq!(result[0], "Critical");
    assert_eq!(result[1], "Infrastructure");
}

// ── Display columns and paragraphs (plan §5, §18) ──────────────────────────

#[test]
fn wide_characters_count_two_columns() {
    // The seed counted chars, so four CJK characters (eight columns) passed as "four wide".
    assert_eq!(
        word_wrap("日本語テキスト", 4),
        ["日本", "語テ", "キス", "ト"]
    );
    assert_eq!(word_wrap("😀😀😀", 4), ["😀😀", "😀"]);
    assert_eq!(word_wrap("ab 日本", 4), ["ab", "日本"]);
    assert_eq!(word_wrap("ab 日本", 5), ["ab", "日本"]);
    assert_eq!(word_wrap("ab 日", 5), ["ab 日"]);
}

#[test]
fn combining_marks_ride_with_the_character_before_them() {
    let e_acute = "e\u{301}";
    let word = e_acute.repeat(3);
    assert_eq!(display_width(&word), 3);
    assert_eq!(
        word_wrap(&word, 2),
        [e_acute.repeat(2), e_acute.to_string()]
    );
    assert_eq!(word_wrap(&word, 3), std::slice::from_ref(&word));
}

#[test]
fn a_character_wider_than_the_width_is_dropped() {
    // Only possible at width 1: placing it would break "every line measures ≤ width".
    assert_eq!(word_wrap("a日b", 1), ["a", "b"]);
    assert_eq!(word_wrap("日 x", 1), ["x"]);
}

#[test]
fn an_invisible_word_takes_no_line() {
    // A word of width-0 characters draws nothing, so a line of its own would be a phantom row
    // in a bordered node box.
    assert_eq!(word_wrap("a \u{301}", 1), ["a"]);
    assert_eq!(word_wrap("a \u{301}\u{200d} b", 3), ["a b"]);
    assert_eq!(
        word_wrap("x\u{0}", 4),
        ["x\u{0}"],
        "a visible word keeps its controls"
    );
    assert_eq!(
        word_wrap("\u{301}", 4),
        [""],
        "a paragraph of invisible words is blank, so it keeps its one empty line"
    );
}

#[test]
fn a_dropped_wide_characters_marks_go_with_it() {
    assert_eq!(word_wrap("日\u{301}", 1), [""]);
    assert_eq!(word_wrap("a日\u{301}\u{302}b", 1), ["a", "b"]);
    assert_eq!(word_wrap("ab 日\u{301}", 1), ["a", "b"]);
    assert_eq!(
        word_wrap("ab \u{301}日", 1),
        ["a", "b"],
        "a mark before it is left with no width, so it goes too"
    );
}

#[test]
fn newlines_start_new_lines() {
    assert_eq!(word_wrap("one two\nthree", 20), ["one two", "three"]);
    assert_eq!(word_wrap("a\n\nb", 20), ["a", "", "b"]);
    assert_eq!(
        word_wrap("a\r\nb\n", 20),
        ["a", "b"],
        "CRLF is one break; a trailing one adds nothing"
    );
    assert_eq!(word_wrap("\n", 20), [""]);
    assert_eq!(
        word_wrap("long words here\nx", 6),
        ["long", "words", "here", "x"]
    );
}

fn cells(text: &str) -> Vec<(&str, u16)> {
    text_cells(text).map(|c| (c.symbol, c.width)).collect()
}

#[test]
fn each_char_of_width_one_or_two_starts_a_cell() {
    assert_eq!(cells(""), []);
    assert_eq!(cells("ab"), [("a", 1), ("b", 1)]);
    assert_eq!(cells("日本x"), [("日", 2), ("本", 2), ("x", 1)]);
}

#[test]
fn zero_width_chars_join_the_cell_before_them() {
    assert_eq!(
        cells("e\u{301}\u{302}x"),
        [("e\u{301}\u{302}", 1), ("x", 1)]
    );
    assert_eq!(
        cells("👨\u{200d}👩"),
        [("👨\u{200d}", 2), ("👩", 2)],
        "a ZWJ sequence measures as the sum of its parts"
    );
}

#[test]
fn a_leading_zero_width_char_is_dropped() {
    assert_eq!(cells("\u{301}a"), [("a", 1)]);
    assert_eq!(cells("\u{200d}\u{301}"), []);
}

#[test]
fn control_chars_are_dropped_and_end_the_cell_before_them() {
    assert_eq!(
        cells("a\nb\x1b[1m"),
        [("a", 1), ("b", 1), ("[", 1), ("1", 1), ("m", 1)]
    );
    assert_eq!(
        cells("a\u{7}\u{301}b"),
        [("a", 1), ("b", 1)],
        "a mark after a control has no cell to join"
    );
    assert_eq!(cells("\0\t\r\u{7f}\u{85}"), []);
}

#[test]
fn display_width_sums_unicode_widths() {
    assert_eq!(display_width(""), 0);
    assert_eq!(display_width("abc"), 3);
    assert_eq!(display_width("日本"), 4);
    assert_eq!(display_width("e\u{301}"), 1);
    assert_eq!(
        display_width("a\u{200d}b"),
        2,
        "a zero-width joiner takes no column"
    );
}

// ── Properties ─────────────────────────────────────────────────────────────

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x7772_6170),
        failure_persistence: None,
        ..Config::default()
    }
}

/// Text mixing ASCII, CJK, emoji, combining marks, zero-width joiners and line breaks.
fn awkward_text() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        Just("a"),
        Just("word"),
        Just(" "),
        Just("  "),
        Just("\n"),
        Just("日本"),
        Just("😀"),
        Just("e\u{301}"),
        Just("\u{301}"),
        Just("\u{200d}"),
        Just("x\u{0}y"),
        Just("\t"),
    ];
    prop::collection::vec(piece, 0..24).prop_map(|pieces| pieces.concat())
}

/// The §7.1 rule applied char by char into owned strings: the oracle for `text_cells`.
fn cells_by_rule(text: &str) -> Vec<(String, u16)> {
    let mut cells: Vec<(String, u16)> = Vec::new();
    let mut open = false;
    for c in text.chars() {
        match c.width() {
            None => open = false,
            Some(0) => {
                if open {
                    cells.last_mut().expect("an open cell exists").0.push(c);
                }
            }
            Some(w) => {
                cells.push((c.to_string(), u16::try_from(w).expect("1 or 2")));
                open = true;
            }
        }
    }
    cells
}

/// The characters word wrap keeps at `width`, word by word: every character but one too wide
/// for `width` and the width-0 ones riding on it, and none of a word left with no width.
fn kept_chars(text: &str, width: usize) -> String {
    let mut kept = String::new();
    for word in text.split_whitespace() {
        let mut dropping = false;
        let survivors: String = word
            .chars()
            .filter(|&c| {
                let w = char_width(c);
                if w > 0 {
                    dropping = w > width;
                }
                !dropping
            })
            .collect();
        if display_width(&survivors) > 0 {
            kept.push_str(&survivors);
        }
    }
    kept
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn text_cells_follow_the_rule_and_measure_as_display_width(text in awkward_text()) {
        let split: Vec<(String, u16)> =
            text_cells(&text).map(|c| (c.symbol.to_string(), c.width)).collect();
        let oracle = cells_by_rule(&text);
        prop_assert_eq!(split.len(), oracle.len());
        prop_assert_eq!(&split, &oracle);
        let columns: usize = split.iter().map(|&(_, w)| usize::from(w)).sum();
        prop_assert_eq!(columns, display_width(&text));
    }

    #[test]
    fn every_line_fits_and_no_visible_text_is_lost(text in awkward_text(), width in 0usize..12) {
        let lines = word_wrap(&text, width);
        if width == 0 || text.is_empty() {
            prop_assert!(lines.is_empty());
        } else {
            for line in &lines {
                prop_assert!(display_width(line) <= width, "{:?} wider than {}", line, width);
                prop_assert!(
                    line.is_empty() || display_width(line) > 0,
                    "{:?} is a line that draws nothing", line
                );
            }
            let rejoined: String = lines.concat();
            prop_assert_eq!(kept_chars(&rejoined, width), kept_chars(&text, width));
            prop_assert_eq!(lines.len() >= text.lines().count(), true, "one line per paragraph at least");
        }
    }
}
