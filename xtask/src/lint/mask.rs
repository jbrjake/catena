//! Source preprocessing for the line rules: blank out prose and find test-only code.

use std::sync::LazyLock;

use regex::Regex;

/// Returns `src` with every comment, and the contents of every string, byte-string, raw-string
/// and char literal, replaced by spaces. Newlines survive, so line `n` of the result is line `n`
/// of the input, and so do the delimiters of literals.
#[must_use]
pub fn mask(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            i = block_comment(&chars, i, &mut out);
        } else if let Some((hashes, quote)) = raw_string_start(&chars, i) {
            out.extend(&chars[i..=quote]);
            i = raw_string_body(&chars, quote + 1, hashes, &mut out);
        } else if c == '"' {
            out.push('"');
            i = string_body(&chars, i + 1, &mut out);
        } else if c == '\'' {
            i = char_or_lifetime(&chars, i, &mut out);
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn blank(c: char) -> char {
    if c == '\n' { '\n' } else { ' ' }
}

/// Masks a (nestable) block comment starting at `i`; returns the index after it.
fn block_comment(chars: &[char], mut i: usize, out: &mut String) -> usize {
    let mut depth = 0usize;
    while i < chars.len() {
        let pair = (chars[i], chars.get(i + 1).copied());
        if pair == ('/', Some('*')) {
            depth += 1;
            out.push_str("  ");
            i += 2;
        } else if pair == ('*', Some('/')) {
            depth -= 1;
            out.push_str("  ");
            i += 2;
            if depth == 0 {
                break;
            }
        } else {
            out.push(blank(chars[i]));
            i += 1;
        }
    }
    i
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// If a raw string literal (`r"…"`, `r#"…"#`, `br"…"`, `cr"…"`) starts at `i`, returns its hash
/// count and the index of its opening quote.
fn raw_string_start(chars: &[char], i: usize) -> Option<(usize, usize)> {
    if chars[i] != 'r' {
        return None;
    }
    let token_start = match i.checked_sub(1).map(|p| chars[p]) {
        None => true,
        Some('b' | 'c') => i < 2 || !is_ident(chars[i - 2]),
        Some(prev) => !is_ident(prev),
    };
    if !token_start {
        return None;
    }
    let mut j = i + 1;
    while chars.get(j) == Some(&'#') {
        j += 1;
    }
    (chars.get(j) == Some(&'"')).then_some((j - i - 1, j))
}

/// Masks a raw string's contents from `i`; returns the index after its closing delimiter.
fn raw_string_body(chars: &[char], mut i: usize, hashes: usize, out: &mut String) -> usize {
    while i < chars.len() {
        if chars[i] == '"' && (1..=hashes).all(|h| chars.get(i + h) == Some(&'#')) {
            out.extend(&chars[i..=i + hashes]);
            return i + hashes + 1;
        }
        out.push(blank(chars[i]));
        i += 1;
    }
    i
}

/// Masks a string's contents from `i`, honouring escapes; returns the index after its closing
/// quote.
fn string_body(chars: &[char], mut i: usize, out: &mut String) -> usize {
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                out.push(' ');
                if let Some(&escaped) = chars.get(i + 1) {
                    out.push(blank(escaped));
                }
                i += 2;
            }
            '"' => {
                out.push('"');
                return i + 1;
            }
            c => {
                out.push(blank(c));
                i += 1;
            }
        }
    }
    i
}

/// At a `'`: masks a char literal, or copies a lifetime or label as code. Returns the index of
/// the next unprocessed char.
fn char_or_lifetime(chars: &[char], i: usize, out: &mut String) -> usize {
    let close = if chars.get(i + 1) == Some(&'\\') {
        // An escape: the escaped char sits at i + 2, so the closing quote is at i + 3 or later.
        (i + 3..chars.len()).find(|&j| chars[j] == '\'')
    } else if chars.get(i + 2) == Some(&'\'') {
        Some(i + 2)
    } else {
        None
    };
    out.push('\'');
    let Some(end) = close else {
        return i + 1;
    };
    for &c in &chars[i + 1..end] {
        out.push(blank(c));
    }
    out.push('\'');
    end + 1
}

static TEST_ATTR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#\s*\[\s*(?:cfg\s*\(\s*test\s*\)|test)\s*\]").expect("valid regex")
});
static INNER_CFG_TEST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#!\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]").expect("valid regex"));

/// For each line of `masked` (the output of [`mask`]), whether it belongs to test-only code: an
/// item under `#[cfg(test)]` or `#[test]`, or the whole file under `#![cfg(test)]`.
#[must_use]
pub fn test_lines(masked: &str) -> Vec<bool> {
    let bytes = masked.as_bytes();
    let line_count = masked.split('\n').count();
    if INNER_CFG_TEST.is_match(masked) {
        return vec![true; line_count];
    }
    let newlines: Vec<usize> = (0..bytes.len()).filter(|&i| bytes[i] == b'\n').collect();
    let line_of = |offset: usize| newlines.partition_point(|&nl| nl < offset);
    let mut flags = vec![false; line_count];
    for attr in TEST_ATTR.find_iter(masked) {
        let end = item_end(bytes, attr.end()).unwrap_or(bytes.len().saturating_sub(1));
        for flag in &mut flags[line_of(attr.start())..=line_of(end)] {
            *flag = true;
        }
    }
    flags
}

/// The offset of the `;` or closing `}` that ends the item whose attributes end at `from`.
/// Brackets and parentheses are skipped, so `[u8; 3]` in a signature does not end the item.
fn item_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut nesting = 0i32;
    for (i, &b) in bytes.iter().enumerate().skip(from) {
        match b {
            b'(' | b'[' => nesting += 1,
            b')' | b']' => nesting -= 1,
            b';' if nesting == 0 => return Some(i),
            b'{' if nesting == 0 => return matching_brace(bytes, i),
            _ => {}
        }
    }
    None
}

fn matching_brace(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}
