//! Text measured in terminal display columns (plan §5, §7.1).
//!
//! Width is `unicode-width`'s, char by char: a character of width 1 or 2 takes that many
//! columns, and a width-0 character (a combining mark, a variation selector, a zero-width
//! joiner) takes none and rides with the character before it. Byte length and char count are
//! never a width.

use unicode_width::UnicodeWidthChar;

/// Columns `c` occupies. Control characters, which have no width of their own, take none.
pub(crate) fn char_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

/// Columns `text` occupies: the sum of its characters' widths. Sequences that terminals draw
/// narrower (emoji ZWJ families, flags) measure as the sum of their parts, so a box reserved
/// for them can show a gap but never an overrun (plan §7.1).
pub(crate) fn display_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

/// Word-wrap `text` into lines of at most `width` display columns.
///
/// - Each input line (as `str::lines` splits them, so `\r\n` is one break and a trailing break
///   adds nothing) starts a new output line; an empty or all-blank input line yields one empty
///   output line.
/// - Words break on whitespace. A word wider than `width` is split at the width, by columns.
/// - A character wider than `width` cannot be placed and is dropped; that happens only at
///   width 1, for a double-width character.
/// - Empty input, or a width of zero, yields an empty `Vec`.
///
/// Every output line measures at most `width` columns.
pub(crate) fn word_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let before = lines.len();
        wrap_paragraph(paragraph, width, &mut lines);
        if lines.len() == before {
            lines.push(String::new());
        }
    }
    lines
}

fn wrap_paragraph(paragraph: &str, width: usize, lines: &mut Vec<String>) {
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in paragraph.split_whitespace() {
        let word_width = display_width(word);

        if word_width > width {
            // Flush current line if non-empty.
            if !current_line.is_empty() {
                lines.push(std::mem::take(&mut current_line));
                current_width = 0;
            }
            split_long_word(word, width, lines);
            continue;
        }

        let needed = if current_line.is_empty() {
            word_width
        } else {
            current_width + 1 + word_width
        };

        if needed > width {
            // Current word doesn't fit — start a new line.
            if !current_line.is_empty() {
                lines.push(std::mem::take(&mut current_line));
            }
            current_line.push_str(word);
            current_width = word_width;
        } else {
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
            current_width = needed;
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
}

/// Splits a word wider than `width` into chunks of at most `width` columns.
fn split_long_word(word: &str, width: usize, lines: &mut Vec<String>) {
    let mut chunk = String::new();
    let mut chunk_width = 0;
    for c in word.chars() {
        let w = char_width(c);
        if w > width {
            continue;
        }
        if chunk_width + w > width {
            lines.push(std::mem::take(&mut chunk));
            chunk_width = 0;
        }
        chunk.push(c);
        chunk_width += w;
    }
    if !chunk.is_empty() {
        lines.push(chunk);
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
