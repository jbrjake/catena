use ratatui_core::style::Color;

use super::*;

fn row(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

#[test]
fn each_row_is_a_swatch_in_its_color_and_its_label() {
    let rows = [
        ("person", PaletteColor::Ansi(9)),
        ("organization", PaletteColor::Rgb(0, 128, 255)),
        ("place", PaletteColor::Indexed(70)),
    ];
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    Legend::new(&rows).render(Rect::new(2, 1, 16, 2), &mut buf);
    assert_eq!(row(&buf, 0).trim_end(), "");
    assert_eq!(row(&buf, 1), "  ■ person          ");
    assert_eq!(row(&buf, 2), "  ■ organization    ");
    assert_eq!(row(&buf, 3).trim_end(), "", "a third row has no room");
    assert_eq!(buf[(2, 1)].fg, Color::LightRed);
    assert_eq!(buf[(2, 2)].fg, Color::Rgb(0, 128, 255));
    assert_eq!(buf[(4, 1)].fg, Color::Gray);
}

#[test]
fn a_label_is_cut_at_the_edge_and_never_through_a_wide_glyph() {
    let rows = [
        ("graph", PaletteColor::Ansi(2)),
        ("漢字漢字", PaletteColor::Ansi(3)),
    ];
    let mut buf = Buffer::empty(Rect::new(0, 0, 7, 2));
    Legend::new(&rows)
        .swatch('#')
        .render(Rect::new(0, 0, 7, 2), &mut buf);
    assert_eq!(row(&buf, 0), "# graph");
    // Two columns for the swatch and its gap leave five: two wide glyphs and a blank.
    let second: Vec<String> = (0..7).map(|x| buf[(x, 1)].symbol().to_string()).collect();
    assert_eq!(second, ["#", " ", "漢", " ", "字", " ", " "]);
}

#[test]
fn the_legend_clears_its_area_so_it_can_float_over_the_graph() {
    let rows = [("graph", PaletteColor::Ansi(2))];
    let area = Rect::new(0, 0, 10, 2);
    let mut buf = Buffer::empty(area);
    for y in 0..2 {
        for x in 0..10 {
            buf[(x, y)].set_symbol("@");
        }
    }
    Legend::new(&rows).render(area, &mut buf);
    assert_eq!(row(&buf, 0), "■ graph   ");
    assert_eq!(row(&buf, 1), "          ");
}

#[test]
fn an_empty_area_draws_nothing() {
    let rows = [("graph", PaletteColor::Ansi(2))];
    let mut buf = Buffer::empty(Rect::new(0, 0, 4, 4));
    Legend::new(&rows).render(Rect::new(1, 1, 0, 3), &mut buf);
    Legend::new(&rows).render(Rect::new(9, 9, 3, 3), &mut buf);
    assert_eq!(buf, Buffer::empty(Rect::new(0, 0, 4, 4)));
}
