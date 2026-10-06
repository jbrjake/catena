//! Text measured in terminal display columns (plan §5, §7.1).
//!
//! Width is `unicode-width`'s, char by char: a character of width 1 or 2 takes that many
//! columns, and a width-0 character (a combining mark, a variation selector, a zero-width
//! joiner) takes none and rides with the character before it. Byte length and char count are
//! never a width. [`text_cells`] splits text into terminal cells by the same measure, so what is
//! measured is what is drawn.

use unicode_width::UnicodeWidthChar;

use super::grid::SYMBOL_BYTES;

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

/// Cuts `text` to its longest prefix of at most `max_cols` display columns and at most
/// `max_cols` × [`SYMBOL_BYTES`] bytes, the most a drawn cell holds, and frees what it cut.
/// The byte bound is what keeps a run of width-0 characters, which take no column, from making
/// a label unbounded (plan §4.1).
pub(crate) fn truncate_to_columns(text: &mut String, max_cols: usize) {
    let max_bytes = max_cols.saturating_mul(SYMBOL_BYTES);
    let mut cols = 0;
    for (at, c) in text.char_indices() {
        cols += char_width(c);
        if cols > max_cols || at + c.len_utf8() > max_bytes {
            text.truncate(at);
            text.shrink_to_fit();
            return;
        }
    }
}

/// One terminal cell of text: a character of display width 1 or 2 and the zero-width characters
/// after it, as [`text_cells`] splits text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCell<'a> {
    /// The cell's content, a slice of the split text.
    pub symbol: &'a str,
    /// Its display width, 1 or 2.
    pub width: u16,
}

/// Splits `text` into terminal cells by the text-cell rule of plan §7.1, with `unicode-width`
/// alone: each character of width 1 or 2 starts a cell, each width-0 character (a combining
/// mark, a variation selector, a zero-width joiner) joins the cell before it, and a leading one
/// is dropped. A control character is dropped too, and ends the cell before it, so a width-0
/// character right after one is dropped as leading.
///
/// The widths sum to the text's display width, `unicode-width`'s char by char, so a label's
/// measured width and its rendered width agree (invariant N).
#[must_use]
pub fn text_cells(text: &str) -> TextCells<'_> {
    TextCells { rest: text }
}

/// The iterator [`text_cells`] returns.
#[derive(Debug, Clone)]
pub struct TextCells<'a> {
    rest: &'a str,
}

impl<'a> Iterator for TextCells<'a> {
    type Item = TextCell<'a>;

    fn next(&mut self) -> Option<TextCell<'a>> {
        loop {
            let mut chars = self.rest.char_indices();
            let (_, first) = chars.next()?;
            let width = match first.width() {
                Some(1) => 1,
                Some(0) | None => {
                    self.rest = &self.rest[first.len_utf8()..];
                    continue;
                }
                Some(_) => 2,
            };
            let mut end = first.len_utf8();
            for (at, c) in chars {
                if c.width() != Some(0) {
                    break;
                }
                end = at + c.len_utf8();
            }
            let (symbol, rest) = self.rest.split_at(end);
            self.rest = rest;
            return Some(TextCell { symbol, width });
        }
    }
}

/// Word-wrap `text` into lines of at most `width` display columns.
///
/// - Each input line (as `str::lines` splits them, so `\r\n` is one break and a trailing break
///   adds nothing) starts a new output line; an empty or all-blank input line yields one empty
///   output line.
/// - Words break on whitespace. A word wider than `width` is split at the width, by columns.
/// - A word of no width (only width-0 characters) draws nothing and is dropped, so it never
///   takes a line of its own.
/// - A character wider than `width` cannot be placed and is dropped, with the width-0
///   characters riding on it; that happens only at width 1, for a double-width character.
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

        if word_width == 0 {
            continue;
        }
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

/// Splits a word wider than `width` into chunks of at most `width` columns. A character too
/// wide for any chunk is dropped with the width-0 characters after it.
fn split_long_word(word: &str, width: usize, lines: &mut Vec<String>) {
    let mut chunk = String::new();
    let mut chunk_width = 0;
    let mut dropping = false;
    for c in word.chars() {
        let w = char_width(c);
        if w > 0 {
            dropping = w > width;
        }
        if dropping {
            continue;
        }
        if chunk_width + w > width {
            lines.push(std::mem::take(&mut chunk));
            chunk_width = 0;
        }
        chunk.push(c);
        chunk_width += w;
    }
    // Only the first chunk can be all width-0, and only when every wider character was dropped.
    if chunk_width > 0 {
        lines.push(chunk);
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
