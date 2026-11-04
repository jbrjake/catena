//! The adapter against `CellGrid`, catena's own readable `Surface`, as its oracle: the same
//! writes leave the same cells, inside an area offset in a larger buffer, and nothing outside
//! it (plan §7.1, §16.6's style translation).

use catena::raster::{Attrs, CellGrid, CellStyle, PaletteColor, Surface};
use proptest::prelude::*;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier};

use super::*;

#[test]
fn the_sixteen_ansi_colors_map_to_ratatuis_names_by_their_low_four_bits() {
    let names = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];
    for (n, name) in (0u8..).zip(names) {
        assert_eq!(color(PaletteColor::Ansi(n)), name, "ansi {n}");
        assert_eq!(
            color(PaletteColor::Ansi(n + 16)),
            name,
            "ansi {} reads as {n}",
            n + 16
        );
    }
    assert_eq!(color(PaletteColor::Rgb(1, 2, 3)), Color::Rgb(1, 2, 3));
    assert_eq!(color(PaletteColor::Indexed(201)), Color::Indexed(201));
}

#[test]
fn attributes_keep_their_bits() {
    // Decision M1: `Attrs` uses ratatui's `Modifier` bit values.
    for (attrs, wanted) in [
        (Attrs::NONE, Modifier::empty()),
        (Attrs::BOLD, Modifier::BOLD),
        (Attrs::DIM, Modifier::DIM),
        (Attrs::ITALIC, Modifier::ITALIC),
        (Attrs::REVERSED, Modifier::REVERSED),
        (
            Attrs::BOLD.union(Attrs::REVERSED),
            Modifier::BOLD | Modifier::REVERSED,
        ),
    ] {
        assert_eq!(modifier(attrs), wanted, "{attrs:?}");
    }
}

/// One cell as the terminal will show it: its symbol (a continuation's is a space, which the
/// terminal never draws), colors and modifier.
type Shown = (String, Color, Color, Modifier);

fn from_grid(grid: &CellGrid, x: u16, y: u16) -> Shown {
    let cell = grid.cell(x, y).expect("on the grid");
    let symbol = if cell.is_continuation() {
        " ".to_string()
    } else {
        cell.symbol().to_string()
    };
    (
        symbol,
        cell.fg().map_or(Color::Reset, color),
        cell.bg().map_or(Color::Reset, color),
        modifier(cell.attrs()),
    )
}

fn from_buffer(buf: &Buffer, x: u16, y: u16) -> Shown {
    let cell = &buf[(x, y)];
    (cell.symbol().to_string(), cell.fg, cell.bg, cell.modifier)
}

const SYMBOLS: [&str; 9] = ["a", "─", "漢", "🙂", "e\u{301}", "", "\u{301}", "ab", "\t"];
const COLORS: [PaletteColor; 4] = [
    PaletteColor::Ansi(7),
    PaletteColor::Ansi(9),
    PaletteColor::Rgb(10, 20, 30),
    PaletteColor::Indexed(99),
];

#[derive(Debug, Clone)]
enum Op {
    Put(u16, u16, usize, usize, Option<usize>, u8),
    PatchBg(u16, u16, usize),
}

fn ops(width: u16, height: u16) -> impl Strategy<Value = Vec<Op>> {
    let put = (
        0..width + 2,
        0..height + 2,
        0..SYMBOLS.len(),
        0..COLORS.len(),
        prop::option::of(0..COLORS.len()),
        any::<u8>(),
    )
        .prop_map(|(x, y, s, fg, bg, attrs)| Op::Put(x, y, s, fg, bg, attrs));
    let patch =
        (0..width + 2, 0..height + 2, 0..COLORS.len()).prop_map(|(x, y, bg)| Op::PatchBg(x, y, bg));
    prop::collection::vec(prop_oneof![3 => put, 1 => patch], 0..40)
}

fn apply(surface: &mut impl Surface, ops: &[Op]) {
    for op in ops {
        match *op {
            Op::Put(x, y, s, fg, bg, attrs) => {
                let style = CellStyle {
                    fg: COLORS[fg],
                    bg: bg.map(|i| COLORS[i]),
                    attrs: [Attrs::BOLD, Attrs::DIM, Attrs::ITALIC, Attrs::REVERSED]
                        .into_iter()
                        .enumerate()
                        .filter(|(bit, _)| attrs & (1 << bit) != 0)
                        .fold(Attrs::NONE, |all, (_, a)| all.union(a)),
                };
                surface.put(x, y, SYMBOLS[s], style);
            }
            Op::PatchBg(x, y, bg) => surface.patch_bg(x, y, COLORS[bg]),
        }
    }
}

proptest! {
    #[test]
    fn the_adapter_leaves_every_cell_as_cell_grid_does_and_none_outside_its_area(
        (width, height, ops) in (1u16..12, 1u16..6)
            .prop_flat_map(|(w, h)| (Just(w), Just(h), ops(w, h))),
        (ox, oy) in (0u16..4, 0u16..3),
    ) {
        let mut grid = CellGrid::new(width, height);
        apply(&mut grid, &ops);
        let whole = Rect::new(0, 0, ox + width + 3, oy + height + 2);
        let mut buf = Buffer::empty(whole);
        for y in 0..whole.height {
            for x in 0..whole.width {
                buf[(x, y)].set_symbol("#").set_fg(Color::Red);
            }
        }
        let area = Rect::new(ox, oy, width, height);
        for y in 0..height {
            for x in 0..width {
                buf[(ox + x, oy + y)].reset();
            }
        }
        let mut surface = BufferSurface::new(&mut buf, area);
        prop_assert_eq!(surface.size(), (width, height));
        prop_assert_eq!(surface.origin(), (ox, oy));
        apply(&mut surface, &ops);
        let mut inside = 0;
        for y in 0..whole.height {
            for x in 0..whole.width {
                let shown = from_buffer(&buf, x, y);
                if area.contains((x, y).into()) {
                    inside += 1;
                    prop_assert_eq!(shown, from_grid(&grid, x - ox, y - oy), "({}, {})", x, y);
                } else {
                    let sentinel = ("#".to_string(), Color::Red, Color::Reset, Modifier::empty());
                    prop_assert_eq!(shown, sentinel, "({}, {}) is outside", x, y);
                }
            }
        }
        prop_assert_eq!(inside, usize::from(width) * usize::from(height));
    }
}

#[test]
fn the_graph_widget_draws_what_the_core_draws_wherever_its_area_is() {
    // The crate holds no logic (plan §3): a frame through the widget is the core's frame,
    // cell for cell and style for style, moved to the area, with nothing drawn outside it.
    let graph = catena_testkit::fixtures::community();
    let mut core = graph
        .view(catena::GraphView::builder())
        .expect("fixtures are consistent");
    let mut widget_state = core.clone();
    let mut grid = CellGrid::new(60, 20);
    core.render(&mut grid);
    for (ox, oy) in [(0, 0), (7, 3)] {
        let whole = Rect::new(0, 0, 80, 26);
        let mut buf = Buffer::empty(whole);
        let area = Rect::new(ox, oy, 60, 20);
        Graph::new().render(area, &mut buf, &mut widget_state);
        let mut drawn = 0;
        for y in 0..whole.height {
            for x in 0..whole.width {
                let shown = from_buffer(&buf, x, y);
                if area.contains((x, y).into()) {
                    let want = from_grid(&grid, x - ox, y - oy);
                    drawn += usize::from(want.0 != " ");
                    assert_eq!(shown, want, "({x}, {y}) in the area at ({ox}, {oy})");
                } else {
                    assert_eq!(shown, from_buffer(&Buffer::empty(whole), x, y));
                }
            }
        }
        assert!(drawn > 100, "it draws: {drawn} cells");
    }
}

#[test]
fn an_area_past_the_buffer_is_cut_to_it() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 4));
    let surface = BufferSurface::new(&mut buf, Rect::new(6, 2, 10, 10));
    assert_eq!((surface.size(), surface.origin()), ((4, 2), (6, 2)));
}
