use super::*;

fn drawn(grid: &Arms, glyphs: &BoxGlyphs) -> String {
    let mut rows = Vec::new();
    for y in 0..grid.height {
        let row: String = (0..grid.width)
            .map(|x| grid.arms(x, y).map_or(' ', |arms| glyphs.glyph(arms)))
            .collect();
        rows.push(row.trim_end().to_string());
    }
    rows.join("\n")
}

fn route(cells: &[(i32, i32)]) -> Vec<CellPt> {
    cells.iter().map(|&(x, y)| CellPt::new(x, y)).collect()
}

#[test]
fn each_arm_set_has_its_glyph() {
    let cases = [
        (N, '│'),
        (S, '│'),
        (N | S, '│'),
        (E, '─'),
        (W | E, '─'),
        (S | E, '┌'),
        (S | W, '┐'),
        (N | E, '└'),
        (N | W, '┘'),
        (N | S | E, '├'),
        (N | S | W, '┤'),
        (E | W | S, '┬'),
        (E | W | N, '┴'),
        (N | E | S | W, '┼'),
    ];
    for (arms, glyph) in cases {
        assert_eq!(BoxGlyphs::BOX.glyph(arms), glyph, "{arms:#06b}");
    }
    assert_eq!(BoxGlyphs::ASCII.glyph(N | S), '|');
    assert_eq!(BoxGlyphs::ASCII.glyph(E | W), '-');
    assert_eq!(BoxGlyphs::ASCII.glyph(S | E), '+');
    assert_eq!(BoxGlyphs::ASCII.glyph(N | E | S | W), '+');
}

#[test]
fn a_route_draws_runs_with_corners_at_its_bends() {
    let mut grid = Arms::default();
    grid.reset(8, 5);
    grid.add_route(&route(&[(0, 0), (4, 0), (4, 3), (7, 3)]), StyleId(1));
    assert_eq!(
        drawn(&grid, &BoxGlyphs::BOX),
        "────┐\n    │\n    │\n    └───\n"
    );
}

#[test]
fn routes_meeting_in_a_cell_merge_into_a_junction() {
    let mut grid = Arms::default();
    grid.reset(7, 5);
    grid.add_route(&route(&[(0, 2), (6, 2)]), StyleId(1));
    grid.add_route(&route(&[(3, 0), (3, 4)]), StyleId(2));
    grid.add_route(&route(&[(0, 4), (3, 4)]), StyleId(3));
    assert_eq!(
        drawn(&grid, &BoxGlyphs::BOX),
        "   │\n   │\n───┼───\n   │\n───┘"
    );
    assert_eq!(
        grid.style(3, 2),
        Some(StyleId(2)),
        "the last route through a cell styles it"
    );
    assert_eq!(grid.style(0, 2), Some(StyleId(1)));
    assert_eq!(grid.style(0, 0), None);
}

#[test]
fn a_diagonal_step_bends_horizontal_first_and_off_grid_cells_clip() {
    let mut grid = Arms::default();
    grid.reset(5, 4);
    grid.add_route(&route(&[(-3, 1), (2, 3)]), StyleId(0));
    assert_eq!(
        drawn(&grid, &BoxGlyphs::BOX),
        "\n──┐\n  │\n  │",
        "an end cell draws its one arm as a full line"
    );
    let mut far = Arms::default();
    far.reset(3, 3);
    far.add_route(&route(&[(i32::MIN, 1), (i32::MAX, 1)]), StyleId(0));
    assert_eq!(drawn(&far, &BoxGlyphs::BOX), "\n───\n");
}

#[test]
fn a_one_cell_route_draws_nothing() {
    let mut grid = Arms::default();
    grid.reset(3, 3);
    grid.add_route(&route(&[(1, 1)]), StyleId(0));
    grid.add_route(&route(&[]), StyleId(0));
    assert_eq!(drawn(&grid, &BoxGlyphs::BOX), "\n\n");
}
