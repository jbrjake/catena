//! The masker blanks prose and keeps code; `test_lines` finds test-only items.

use crate::lint::mask::{mask, test_lines};

fn masked_lines(src: &str) -> Vec<String> {
    let out = mask(src);
    assert_eq!(
        out.split('\n').count(),
        src.split('\n').count(),
        "line count changed"
    );
    out.split('\n').map(str::to_string).collect()
}

#[test]
fn comments_are_blanked_and_code_kept() {
    let lines = masked_lines("let a = 1; // Instant\n/* x /* nested */ still */ let b;\n/// doc\n");
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].trim_end(), "let a = 1;");
    assert_eq!(lines[1].trim(), "let b;");
    assert_eq!(lines[2].trim(), "");
}

#[test]
fn string_contents_are_blanked_across_lines_and_escapes() {
    let lines = masked_lines("f(\"a \\\" Instant\", x);\nlet s = \"line one\nline two\"; g();\n");
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], format!("f(\"{}\", x);", " ".repeat(12)));
    assert_eq!(lines[1], format!("let s = \"{}", " ".repeat(8)));
    assert_eq!(lines[2], format!("{}\"; g();", " ".repeat(8)));
}

#[test]
fn raw_and_byte_strings_are_blanked() {
    let lines = masked_lines("let r = r#\"say \"Instant\"\"#; let b = br\"x\"; let c = b\"y\";\n");
    assert_eq!(lines.len(), 2);
    assert!(!lines[0].contains("Instant"), "{:?}", lines[0]);
    assert!(lines[0].contains("let b = br\" \";"), "{:?}", lines[0]);
    assert!(lines[0].contains("let c = b\" \";"), "{:?}", lines[0]);
    assert!(lines[0].starts_with("let r = r#\""), "{:?}", lines[0]);
    assert!(lines[0].contains("\"#; let b"), "{:?}", lines[0]);
}

#[test]
fn raw_identifiers_are_code() {
    let lines = masked_lines("let r#type = 1; for_r(\"x\");\n");
    assert_eq!(lines[0], "let r#type = 1; for_r(\" \");");
}

#[test]
fn char_literals_are_blanked_and_lifetimes_kept() {
    let lines =
        masked_lines("fn f<'a>(x: &'a str) { let c = '{'; let q = '\\''; let u = '\\u{7b}'; }\n");
    assert_eq!(
        lines[0],
        "fn f<'a>(x: &'a str) { let c = ' '; let q = '  '; let u = '      '; }"
    );
}

#[test]
fn test_lines_cover_cfg_test_items_and_test_fns() {
    let src = mask(
        "fn a() {}\n#[cfg(test)]\nmod t {\n  fn b() { '}'; }\n}\nfn c() {}\n#[test]\nfn d() {}\n\
         #[cfg(test)]\nuse x::y;\nfn e() {}\n",
    );
    let flags = test_lines(&src);
    assert_eq!(flags.len(), 12);
    let expected = [
        false, true, true, true, true, false, true, true, true, true, false, false,
    ];
    assert_eq!(flags, expected);
}

#[test]
fn inner_cfg_test_marks_the_whole_file() {
    let flags = test_lines(&mask("#![cfg(test)]\nfn a() {}\n"));
    assert_eq!(flags, [true, true, true]);
}
