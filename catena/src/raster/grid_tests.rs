use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use super::*;
use crate::raster::text::display_width;

const RED: PaletteColor = PaletteColor::Ansi(1);
const BLUE: PaletteColor = PaletteColor::Ansi(4);
const GLOW: PaletteColor = PaletteColor::Rgb(40, 30, 60);

fn style(fg: PaletteColor) -> CellStyle {
    CellStyle::new(fg)
}

fn at(grid: &CellGrid, x: u16, y: u16) -> &Cell {
    grid.cell(x, y).expect("in range")
}

fn symbols(grid: &CellGrid, y: u16) -> Vec<&str> {
    (0..grid.size().0)
        .map(|x| at(grid, x, y).symbol())
        .collect()
}

fn backgrounds(grid: &CellGrid, y: u16) -> Vec<Option<PaletteColor>> {
    (0..grid.size().0).map(|x| at(grid, x, y).bg()).collect()
}

#[test]
fn a_new_grid_is_blank() {
    let grid = CellGrid::new(3, 2);
    assert_eq!(grid.size(), (3, 2));
    for y in 0..2 {
        for x in 0..3 {
            let cell = at(&grid, x, y);
            assert_eq!(cell.symbol(), " ");
            assert_eq!(
                (cell.fg(), cell.bg(), cell.attrs(), cell.width()),
                (None, None, Attrs::NONE, 1)
            );
        }
    }
    assert_eq!(grid.cell(3, 0), None);
    assert_eq!(grid.cell(0, 2), None);
    assert_eq!(grid.to_string(), "\n", "blank rows trim to nothing");
}

#[test]
fn put_writes_a_symbol_and_its_style() {
    let mut grid = CellGrid::new(3, 1);
    let bold_red = CellStyle {
        fg: RED,
        bg: Some(BLUE),
        attrs: Attrs::BOLD,
    };
    grid.put(1, 0, "x", bold_red);
    let cell = at(&grid, 1, 0);
    assert_eq!(
        (cell.symbol(), cell.fg(), cell.bg(), cell.attrs()),
        ("x", Some(RED), Some(BLUE), Attrs::BOLD)
    );
    assert_eq!(grid.to_string(), " x");
}

#[test]
fn writes_outside_the_grid_are_ignored() {
    let mut grid = CellGrid::new(3, 2);
    grid.put(3, 0, "x", style(RED));
    grid.put(0, 2, "x", style(RED));
    grid.put(u16::MAX, u16::MAX, "x", style(RED));
    grid.put_char(0, 9, 'y', style(RED));
    grid.patch_bg(3, 1, GLOW);
    assert_eq!(grid, CellGrid::new(3, 2));
}

#[test]
fn only_the_first_text_cell_of_a_symbol_is_written() {
    let mut grid = CellGrid::new(4, 1);
    grid.put(0, 0, "ab", style(RED));
    grid.put(1, 0, "e\u{301}z", style(RED));
    grid.put(2, 0, "\u{301}q", style(RED));
    assert_eq!(symbols(&grid, 0), ["a", "e\u{301}", "q", " "]);
}

#[test]
fn a_symbol_without_a_text_cell_writes_nothing() {
    let mut grid = CellGrid::new(2, 1);
    for symbol in ["", "\u{301}", "\u{200d}", "\x1b", "\n", "\t"] {
        grid.put(0, 0, symbol, style(RED));
    }
    assert_eq!(grid, CellGrid::new(2, 1));
}

#[test]
fn a_symbol_keeps_zero_width_followers_up_to_its_byte_ceiling() {
    let mut grid = CellGrid::new(1, 1);
    let piled: String = std::iter::once('e')
        .chain(std::iter::repeat_n('\u{301}', 40))
        .collect();
    grid.put(0, 0, &piled, style(RED));
    let kept = at(&grid, 0, 0).symbol();
    assert_eq!(
        kept.chars().count(),
        8,
        "e and seven two-byte marks fill fifteen bytes"
    );
    assert!(piled.starts_with(kept));
    assert_eq!(display_width(kept), 1);
}

#[test]
fn a_wide_symbol_owns_a_continuation_cell() {
    let mut grid = CellGrid::new(4, 1);
    let reversed = CellStyle {
        fg: RED,
        bg: Some(BLUE),
        attrs: Attrs::REVERSED,
    };
    grid.put(1, 0, "日", reversed);
    let lead = at(&grid, 1, 0);
    assert_eq!((lead.symbol(), lead.width()), ("日", 2));
    assert!(!lead.is_continuation());
    let continuation = at(&grid, 2, 0);
    assert!(continuation.is_continuation());
    assert_eq!((continuation.symbol(), continuation.width()), ("", 0));
    assert_eq!(
        (continuation.fg(), continuation.bg(), continuation.attrs()),
        (Some(RED), Some(BLUE), Attrs::REVERSED),
        "the continuation shares the glyph's style"
    );
    assert_eq!(
        grid.to_string(),
        " 日",
        "the glyph prints once and covers both columns"
    );
}

#[test]
fn a_wide_symbol_that_would_overhang_the_right_edge_is_not_written() {
    let mut grid = CellGrid::new(3, 1);
    grid.put(1, 0, "x", style(RED));
    grid.put(2, 0, "日", style(BLUE));
    assert_eq!(symbols(&grid, 0), [" ", "x", " "]);
}

#[test]
fn overwriting_the_continuation_blanks_the_glyph() {
    let mut grid = CellGrid::new(3, 1);
    let on_blue = CellStyle {
        bg: Some(BLUE),
        ..style(RED)
    };
    grid.put(0, 0, "日", on_blue);
    grid.put(1, 0, "x", style(RED));
    let blanked = at(&grid, 0, 0);
    assert_eq!(
        (blanked.symbol(), blanked.width(), blanked.bg()),
        (" ", 1, Some(BLUE)),
        "the blanked half keeps its background"
    );
    assert_eq!(grid.to_string(), " x");
}

#[test]
fn overwriting_the_glyph_blanks_its_continuation() {
    let mut grid = CellGrid::new(3, 1);
    grid.put(0, 0, "日", style(RED));
    grid.put(0, 0, "x", style(RED));
    let freed = at(&grid, 1, 0);
    assert_eq!((freed.symbol(), freed.width()), (" ", 1));
    assert_eq!(grid.to_string(), "x");
}

#[test]
fn a_wide_symbol_over_two_wide_symbols_blanks_both_outer_halves() {
    let mut grid = CellGrid::new(6, 1);
    grid.put(0, 0, "日", style(RED));
    grid.put(2, 0, "本", style(RED));
    grid.put(1, 0, "語", style(BLUE));
    assert_eq!(symbols(&grid, 0), [" ", "語", "", " ", " ", " "]);
    assert_eq!(grid.to_string(), " 語");
}

#[test]
fn a_style_without_a_background_keeps_the_cells() {
    let mut grid = CellGrid::new(2, 1);
    grid.patch_bg(0, 0, GLOW);
    grid.put(0, 0, "x", style(RED));
    let on_blue = CellStyle {
        bg: Some(BLUE),
        ..style(RED)
    };
    grid.put(1, 0, "y", on_blue);
    grid.put(1, 0, "z", style(RED));
    assert_eq!(backgrounds(&grid, 0), [Some(GLOW), Some(BLUE)]);
}

#[test]
fn patch_bg_keeps_glyph_foreground_and_attributes() {
    let mut grid = CellGrid::new(1, 1);
    let styled = CellStyle {
        attrs: Attrs::BOLD | Attrs::ITALIC,
        ..style(RED)
    };
    grid.put(0, 0, "x", styled);
    grid.patch_bg(0, 0, GLOW);
    let cell = at(&grid, 0, 0);
    assert_eq!(
        (cell.symbol(), cell.fg(), cell.bg(), cell.attrs()),
        ("x", Some(RED), Some(GLOW), Attrs::BOLD | Attrs::ITALIC)
    );
}

#[test]
fn both_halves_of_a_wide_symbol_share_one_background() {
    let mut grid = CellGrid::new(4, 1);
    grid.put(0, 0, "日", style(RED));
    grid.put(2, 0, "本", style(RED));
    grid.patch_bg(0, 0, GLOW);
    grid.patch_bg(3, 0, BLUE);
    assert_eq!(
        backgrounds(&grid, 0),
        [Some(GLOW), Some(GLOW), Some(BLUE), Some(BLUE)]
    );
}

#[test]
fn a_wide_symbol_takes_the_background_under_its_first_cell() {
    let mut grid = CellGrid::new(2, 1);
    grid.patch_bg(0, 0, GLOW);
    grid.put(0, 0, "日", style(RED));
    assert_eq!(backgrounds(&grid, 0), [Some(GLOW), Some(GLOW)]);
}

#[test]
fn put_char_is_put_of_the_encoded_char() {
    let mut by_char = CellGrid::new(4, 1);
    let mut by_str = CellGrid::new(4, 1);
    for (x, c) in [(0, 'a'), (1, '日'), (3, '\u{301}')] {
        by_char.put_char(x, 0, c, style(RED));
        by_str.put(x, 0, c.encode_utf8(&mut [0; 4]), style(RED));
    }
    assert_eq!(by_char, by_str);
    assert_eq!(by_char.to_string(), "a日");
}

#[test]
fn to_string_trims_trailing_blanks_but_keeps_inner_ones() {
    let mut grid = CellGrid::new(5, 3);
    grid.put(0, 0, "a", style(RED));
    grid.put(2, 0, "b", style(RED));
    let on_blue = CellStyle {
        bg: Some(BLUE),
        ..style(RED)
    };
    grid.put(3, 1, " ", on_blue);
    grid.put(4, 2, "c", style(RED));
    assert_eq!(grid.to_string(), "a b\n\n    c");
}

#[test]
fn to_ansi_string_emits_sgr_only_where_the_style_changes() {
    let mut grid = CellGrid::new(4, 2);
    let bold_red = CellStyle {
        attrs: Attrs::BOLD,
        ..style(PaletteColor::Ansi(1))
    };
    grid.put(0, 0, "a", bold_red);
    grid.put(1, 0, "b", bold_red);
    let rgb_on_indexed = CellStyle {
        bg: Some(PaletteColor::Indexed(200)),
        ..style(PaletteColor::Rgb(1, 2, 3))
    };
    grid.put(2, 0, "c", rgb_on_indexed);
    grid.put(1, 1, "d", style(PaletteColor::Ansi(9)));
    assert_eq!(
        grid.to_ansi_string(),
        "\x1b[0;1;31mab\x1b[0;38;2;1;2;3;48;5;200mc\x1b[0m\n \x1b[0;91md\x1b[0m"
    );
}

#[test]
fn ansi_codes_cover_every_color_kind_and_attribute() {
    use PaletteColor::{Ansi, Indexed, Rgb};
    let cases = [
        (Ansi(7), Ansi(0), Attrs::NONE, "\x1b[0;37;40m"),
        (Ansi(15), Ansi(8), Attrs::NONE, "\x1b[0;97;100m"),
        (Ansi(17), Ansi(0x1c), Attrs::NONE, "\x1b[0;31;104m"),
        (
            Indexed(16),
            Rgb(255, 0, 10),
            Attrs::DIM | Attrs::ITALIC | Attrs::REVERSED,
            "\x1b[0;2;3;7;38;5;16;48;2;255;0;10m",
        ),
    ];
    for (fg, bg, attrs, sgr) in cases {
        let mut grid = CellGrid::new(1, 1);
        let styled = CellStyle {
            fg,
            bg: Some(bg),
            attrs,
        };
        grid.put(0, 0, "x", styled);
        assert_eq!(grid.to_ansi_string(), format!("{sgr}x\x1b[0m"));
    }
}

#[test]
fn a_blank_with_a_background_is_not_trimmed_from_the_ansi_string() {
    let mut grid = CellGrid::new(3, 1);
    grid.patch_bg(1, 0, PaletteColor::Ansi(4));
    assert_eq!(grid.to_ansi_string(), " \x1b[0;44m \x1b[0m");
    assert_eq!(grid.to_string(), "");
}

#[test]
fn a_wide_glyph_prints_once_in_the_ansi_string() {
    let mut grid = CellGrid::new(3, 1);
    grid.put(0, 0, "日", style(PaletteColor::Ansi(2)));
    grid.put(2, 0, "x", style(PaletteColor::Ansi(2)));
    assert_eq!(grid.to_ansi_string(), "\x1b[0;32m日x\x1b[0m");
}

#[test]
fn a_grid_request_past_the_cell_ceiling_is_bounded() {
    let mut grid = CellGrid::new(u16::MAX, u16::MAX);
    let (width, height) = grid.size();
    assert_eq!((width, height), (u16::MAX, 64));
    assert!(usize::from(width) * usize::from(height) <= MAX_CELLS);
    grid.put(0, height, "x", style(RED));
    assert_eq!(grid.cell(0, height), None);
    grid.resize(u16::MAX, u16::MAX);
    assert_eq!(grid.size(), (u16::MAX, 64));
}

#[test]
fn a_zero_sized_grid_takes_no_writes() {
    for (width, height) in [(0, 0), (0, 3), (3, 0)] {
        let mut grid = CellGrid::new(width, height);
        grid.put(0, 0, "x", style(RED));
        grid.patch_bg(0, 0, GLOW);
        assert_eq!(grid, CellGrid::new(width, height));
        assert_eq!(grid.cell(0, 0), None);
    }
    assert_eq!(CellGrid::new(0, 3).to_string(), "\n\n");
    assert_eq!(CellGrid::new(3, 0).to_string(), "");
    assert_eq!(CellGrid::new(0, 3).to_ansi_string(), "\n\n");
}

#[test]
fn clear_and_resize_leave_a_blank_grid() {
    let mut grid = CellGrid::new(2, 2);
    grid.put(0, 0, "日", style(RED));
    grid.patch_bg(1, 1, GLOW);
    grid.clear();
    assert_eq!(grid, CellGrid::new(2, 2));
    grid.put(0, 0, "x", style(RED));
    grid.resize(3, 1);
    assert_eq!(grid, CellGrid::new(3, 1));
}

// ── Properties ─────────────────────────────────────────────────────────────

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x6772_6964),
        failure_persistence: None,
        ..Config::default()
    }
}

#[derive(Debug, Clone)]
enum Op {
    Put(u16, u16, &'static str, u8),
    PatchBg(u16, u16, u8),
}

/// Writes at and just past a 6×2 grid's edges, with narrow, wide and degenerate symbols.
fn op() -> impl Strategy<Value = Op> {
    let symbol = prop::sample::select(vec![
        "a", "日", "😀", "e\u{301}", " ", "\u{301}", "ab", "", "\x1b",
    ]);
    prop_oneof![
        (0u16..7, 0u16..3, symbol, any::<u8>()).prop_map(|(x, y, s, c)| Op::Put(x, y, s, c)),
        (0u16..7, 0u16..3, any::<u8>()).prop_map(|(x, y, c)| Op::PatchBg(x, y, c)),
    ]
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn wide_glyphs_and_continuations_always_pair_up(ops in prop::collection::vec(op(), 0..40)) {
        let mut grid = CellGrid::new(6, 2);
        for op in ops {
            match op {
                Op::Put(x, y, s, c) => grid.put(x, y, s, style(PaletteColor::Indexed(c))),
                Op::PatchBg(x, y, c) => grid.patch_bg(x, y, PaletteColor::Indexed(c)),
            }
        }
        for y in 0..2 {
            let mut columns = 0;
            for x in 0..6 {
                let cell = at(&grid, x, y);
                match cell.width() {
                    0 => {
                        prop_assert!(x > 0, "a continuation in column 0");
                        let lead = at(&grid, x - 1, y);
                        prop_assert_eq!(lead.width(), 2);
                        prop_assert_eq!(lead.bg(), cell.bg());
                    }
                    2 => prop_assert_eq!(grid.cell(x + 1, y).map(Cell::width), Some(0)),
                    w => prop_assert_eq!(w, 1),
                }
                prop_assert_eq!(display_width(cell.symbol()), usize::from(cell.width()));
                columns += usize::from(cell.width());
            }
            prop_assert_eq!(columns, 6, "a row's glyphs cover exactly its columns");
        }
    }
}
