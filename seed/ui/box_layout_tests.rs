use super::*;

#[test]
fn word_wrap_empty() {
    assert!(word_wrap("", 20).is_empty());
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
    assert!(word_wrap("hello", 0).is_empty());
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
        assert!(line.chars().count() <= 20, "line too long: {line:?}");
    }
    assert_eq!(result[0], "Critical");
    assert_eq!(result[1], "Infrastructure");
}

fn default_glyphs() -> BoxGlyphs {
    BoxGlyphs {
        single_tl: "\u{250C}",
        single_tr: "\u{2510}",
        single_bl: "\u{2514}",
        single_br: "\u{2518}",
        single_h: "\u{2500}",
        single_v: "\u{2502}",
        double_tl: "\u{2554}",
        double_tr: "\u{2557}",
        double_bl: "\u{255A}",
        double_br: "\u{255D}",
        double_h: "\u{2550}",
        double_v: "\u{2551}",
    }
}

#[test]
fn render_box_single_border_contains_content() {
    let spec = BoxSpec {
        lines: vec![BoxContent {
            text: "Hello".into(),
            color: Color::White,
            modifier: Modifier::empty(),
        }],
        border: BorderStyle::Single,
        align: ContentAlign::Center,
        border_color: Color::Gray,
        min_width: None,
    };
    let rendered = render_box(&spec, 20, &default_glyphs());
    let texts: Vec<String> = rendered
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect();
    assert_eq!(texts.len(), 3);
    assert!(
        texts[0].starts_with('\u{250C}'),
        "top should start with \u{250C}"
    );
    assert!(
        texts[0].ends_with('\u{2510}'),
        "top should end with \u{2510}"
    );
    assert!(texts[1].contains("Hello"), "content should contain Hello");
    assert!(
        texts[2].starts_with('\u{2514}'),
        "bottom should start with \u{2514}"
    );
}

#[test]
fn render_box_double_border() {
    let spec = BoxSpec {
        lines: vec![BoxContent {
            text: "Test".into(),
            color: Color::White,
            modifier: Modifier::empty(),
        }],
        border: BorderStyle::Double,
        align: ContentAlign::Center,
        border_color: Color::Yellow,
        min_width: None,
    };
    let rendered = render_box(&spec, 20, &default_glyphs());
    let texts: Vec<String> = rendered
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect();
    assert!(
        texts[0].starts_with('\u{2554}'),
        "top should start with \u{2554}"
    );
    assert!(
        texts[0].ends_with('\u{2557}'),
        "top should end with \u{2557}"
    );
}

#[test]
fn render_box_wraps_long_content() {
    let spec = BoxSpec {
        lines: vec![BoxContent {
            text: "hello world this is a long line of text".into(),
            color: Color::White,
            modifier: Modifier::empty(),
        }],
        border: BorderStyle::Single,
        align: ContentAlign::Left,
        border_color: Color::Gray,
        min_width: None,
    };
    let rendered = render_box(&spec, 15, &default_glyphs());
    assert!(
        rendered.len() > 3,
        "long content should wrap to multiple lines"
    );
}

#[test]
fn render_box_respects_min_width() {
    let spec = BoxSpec {
        lines: vec![BoxContent {
            text: "Hi".into(),
            color: Color::White,
            modifier: Modifier::empty(),
        }],
        border: BorderStyle::Single,
        align: ContentAlign::Center,
        border_color: Color::Gray,
        min_width: Some(20),
    };
    let rendered = render_box(&spec, 30, &default_glyphs());
    let top_line: String = rendered[0]
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    assert!(top_line.chars().count() >= 20);
}

#[test]
fn render_row_two_boxes_side_by_side() {
    let boxes = vec![
        BoxSpec {
            lines: vec![BoxContent {
                text: "Alice".into(),
                color: Color::Green,
                modifier: Modifier::empty(),
            }],
            border: BorderStyle::Single,
            align: ContentAlign::Center,
            border_color: Color::Gray,
            min_width: None,
        },
        BoxSpec {
            lines: vec![BoxContent {
                text: "Bob".into(),
                color: Color::Blue,
                modifier: Modifier::empty(),
            }],
            border: BorderStyle::Single,
            align: ContentAlign::Center,
            border_color: Color::Gray,
            min_width: None,
        },
    ];
    let (rendered, positions) = render_row(&boxes, 60, &default_glyphs());
    let texts: Vec<String> = rendered
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect();
    let all_text = texts.join("\n");
    assert!(all_text.contains("Alice"), "should contain Alice");
    assert!(all_text.contains("Bob"), "should contain Bob");
    assert_eq!(rendered.len(), 3);
    assert_eq!(positions.len(), 2);
}

#[test]
fn render_row_tallest_box_sets_height() {
    let boxes = vec![
        BoxSpec {
            lines: vec![BoxContent {
                text: "short".into(),
                color: Color::White,
                modifier: Modifier::empty(),
            }],
            border: BorderStyle::Single,
            align: ContentAlign::Center,
            border_color: Color::Gray,
            min_width: None,
        },
        BoxSpec {
            lines: vec![
                BoxContent {
                    text: "line 1".into(),
                    color: Color::White,
                    modifier: Modifier::empty(),
                },
                BoxContent {
                    text: "line 2".into(),
                    color: Color::White,
                    modifier: Modifier::empty(),
                },
                BoxContent {
                    text: "line 3".into(),
                    color: Color::White,
                    modifier: Modifier::empty(),
                },
            ],
            border: BorderStyle::Single,
            align: ContentAlign::Left,
            border_color: Color::Gray,
            min_width: None,
        },
    ];
    let (rendered, _) = render_row(&boxes, 60, &default_glyphs());
    assert_eq!(
        rendered.len(),
        5,
        "row height should match tallest box (3 content + 2 borders)"
    );
}

#[test]
fn render_vertical_pipes_at_centers() {
    let positions = vec![
        BoxPosition {
            col: 5,
            width: 10,
            center: 10,
        },
        BoxPosition {
            col: 25,
            width: 10,
            center: 30,
        },
    ];
    let colors = vec![Color::Green, Color::Magenta];
    let line = render_vertical_pipes(50, &positions, &colors, &default_glyphs());
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text.chars().nth(10), Some('\u{2502}'));
    assert_eq!(text.chars().nth(30), Some('\u{2502}'));
}

#[test]
fn render_labeled_routing_shows_full_label_when_space() {
    let positions = vec![BoxPosition {
        col: 20,
        width: 20,
        center: 30,
    }];
    let labels = vec!["MANAGES".to_string()];
    let colors = vec![Color::Green];
    let line = render_labeled_routing(80, &positions, &labels, &colors, &default_glyphs());
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains("MANAGES"),
        "should show full label when space permits"
    );
}

#[test]
fn render_labeled_routing_truncates_when_tight() {
    let positions = vec![BoxPosition {
        col: 0,
        width: 10,
        center: 5,
    }];
    let labels = vec!["very_long_relationship_name".to_string()];
    let colors = vec![Color::Green];
    let line = render_labeled_routing(20, &positions, &labels, &colors, &default_glyphs());
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    // Should be truncated -- not showing the full label.
    assert!(
        !text.contains("very_long_relationship_name"),
        "should truncate long label"
    );
}

#[test]
fn render_row_at_places_boxes_at_specified_offsets() {
    let boxes = vec![
        BoxSpec {
            lines: vec![BoxContent {
                text: "Alice".into(),
                color: Color::Green,
                modifier: Modifier::empty(),
            }],
            border: BorderStyle::Single,
            align: ContentAlign::Center,
            border_color: Color::Gray,
            min_width: None,
        },
        BoxSpec {
            lines: vec![BoxContent {
                text: "Bob".into(),
                color: Color::Blue,
                modifier: Modifier::empty(),
            }],
            border: BorderStyle::Single,
            align: ContentAlign::Center,
            border_color: Color::Gray,
            min_width: None,
        },
    ];
    let offsets = vec![5, 30];
    let (rendered, positions) = render_row_at(&boxes, &offsets, 15, 60, &default_glyphs());
    let texts: Vec<String> = rendered
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect();
    let all_text = texts.join("\n");
    assert!(all_text.contains("Alice"), "should contain Alice");
    assert!(all_text.contains("Bob"), "should contain Bob");
    assert_eq!(positions.len(), 2);
    assert_eq!(positions[0].col, 5);
    assert_eq!(positions[0].center, 12); // 5 + 15/2
    assert_eq!(positions[1].col, 30);
    assert_eq!(positions[1].center, 37); // 30 + 15/2
}
