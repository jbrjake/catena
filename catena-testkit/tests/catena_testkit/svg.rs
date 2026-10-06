//! The T3 tier's renderer, hash and snapshot files (plan §16.4).

use std::fs;
use std::path::PathBuf;

use catena::raster::{Attrs, CellGrid, CellStyle, PaletteColor, Surface};
use catena_testkit::svg::{
    DEFAULT_BG, DEFAULT_FG, Mismatch, Mode, VisualSnapshots, assert_visual_snapshot, color_to_css,
    grid_to_hash, grid_to_svg, hash_text, sha256_hex,
};

const RED: PaletteColor = PaletteColor::Ansi(1);
const GLOW: PaletteColor = PaletteColor::Rgb(40, 30, 60);

fn style(fg: PaletteColor) -> CellStyle {
    CellStyle::new(fg)
}

/// Every color kind as foreground and background, every attribute, and a reversed cell.
fn swatch() -> CellGrid {
    let mut grid = CellGrid::new(16, 4);
    for n in 0..16u8 {
        grid.put_char(u16::from(n), 0, '█', style(PaletteColor::Ansi(n)));
        let on_n = CellStyle {
            bg: Some(PaletteColor::Ansi(n)),
            ..style(PaletteColor::Ansi(15 - n))
        };
        grid.put_char(u16::from(n), 1, char::from(b'a' + n), on_n);
    }
    for (x, n) in (0..8u16).zip([16u8, 21, 46, 196, 231, 232, 244, 255]) {
        grid.put_char(x, 2, '▀', style(PaletteColor::Indexed(n)));
    }
    for (x, (r, g, b)) in (8..16u16).zip([
        (255, 0, 0),
        (0, 255, 0),
        (0, 0, 255),
        (12, 10, 18),
        (220, 215, 235),
        (255, 128, 0),
        (1, 2, 3),
        (250, 250, 250),
    ]) {
        grid.put_char(x, 2, '▄', style(PaletteColor::Rgb(r, g, b)));
    }
    let attrs = [
        Attrs::NONE,
        Attrs::BOLD,
        Attrs::DIM,
        Attrs::ITALIC,
        Attrs::REVERSED,
        Attrs::BOLD | Attrs::REVERSED,
    ];
    for (x, attrs) in (0..6u16).zip(attrs) {
        let styled = CellStyle {
            fg: RED,
            bg: Some(PaletteColor::Indexed(17)),
            attrs,
        };
        grid.put_char(x, 3, 'A', styled);
    }
    grid.patch_bg(8, 3, GLOW);
    grid.put_char(8, 3, '·', style(PaletteColor::Ansi(14)));
    grid
}

/// Labels with wide, combining and emoji characters, one on a glow halo.
fn text_cells() -> CellGrid {
    let mut grid = CellGrid::new(12, 3);
    let mut write = |y: u16, text: &str, style: CellStyle| {
        let mut x = 0;
        for cell in catena::raster::text_cells(text) {
            grid.put(x, y, cell.symbol, style);
            x += cell.width;
        }
    };
    write(0, "日本語 ok", style(PaletteColor::Ansi(2)));
    write(1, "e\u{301}te\u{301} <&>", style(PaletteColor::Ansi(3)));
    write(2, "😀 bridge", style(PaletteColor::Rgb(200, 120, 255)));
    grid.patch_bg(0, 2, GLOW);
    grid
}

/// A directory of its own under cargo's per-target scratch space.
fn scratch(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("t3")
        .join(test);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

#[test]
fn sha256_hex_matches_the_fips_180_test_vector() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn the_hash_is_sha256_of_one_canonical_line_per_cell() {
    let mut grid = CellGrid::new(3, 1);
    let reversed_bold = CellStyle {
        fg: RED,
        bg: Some(PaletteColor::Indexed(21)),
        attrs: Attrs::BOLD | Attrs::REVERSED,
    };
    grid.put(0, 0, "x", reversed_bold);
    grid.put(1, 0, "日", style(PaletteColor::Rgb(1, 2, 255)));
    let text = hash_text(&grid);
    assert_eq!(text.lines().count(), 3);
    assert_eq!(
        text, "x|#0000ff|#e06c75|41\n日|#0102ff|#0c0a12|0\n |#0102ff|#0c0a12|0\n",
        "{{glyph}}|{{fg}}|{{bg}}|{{attrs:x}}: reversed colors swapped, a continuation as a space"
    );
    assert_eq!(grid_to_hash(&grid), sha256_hex(text.as_bytes()));
}

#[test]
fn colors_map_to_css() {
    assert_eq!(color_to_css(None, true), DEFAULT_FG);
    assert_eq!(color_to_css(None, false), DEFAULT_BG);
    let cases = [
        (PaletteColor::Ansi(1), "#e06c75"),
        (PaletteColor::Indexed(1), "#e06c75"),
        (PaletteColor::Ansi(17), "#e06c75"),
        (PaletteColor::Ansi(15), "#ffffff"),
        (PaletteColor::Indexed(16), "#000000"),
        (PaletteColor::Indexed(21), "#0000ff"),
        (PaletteColor::Indexed(67), "#5f87af"),
        (PaletteColor::Indexed(231), "#ffffff"),
        (PaletteColor::Indexed(232), "#080808"),
        (PaletteColor::Indexed(255), "#eeeeee"),
        (PaletteColor::Rgb(1, 2, 255), "#0102ff"),
    ];
    for (color, css) in cases {
        assert_eq!(color_to_css(Some(color), true), css, "{color:?}");
        assert_eq!(color_to_css(Some(color), false), css, "{color:?}");
    }
}

#[test]
fn the_svg_draws_backgrounds_then_styled_text_runs() {
    let mut grid = CellGrid::new(4, 2);
    let bold_on_glow = CellStyle {
        fg: RED,
        bg: Some(GLOW),
        attrs: Attrs::BOLD,
    };
    grid.put(0, 0, "<", bold_on_glow);
    grid.put(1, 0, "&", bold_on_glow);
    grid.put(2, 1, "z", style(PaletteColor::Ansi(2)));
    let svg = grid_to_svg(&grid, "a \"title\"");
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.ends_with("</svg>\n"));
    assert!(svg.contains("aria-label=\"a &quot;title&quot;\""));
    assert!(svg.contains("a &quot;title&quot; | 4×2"));
    assert_eq!(
        svg.matches("fill=\"#281e3c\"").count(),
        1,
        "one background run for the two glow cells"
    );
    assert!(
        svg.contains(
            "<rect x=\"4.0\" y=\"26.0\" width=\"15.6\" height=\"16.0\" fill=\"#281e3c\"/>"
        )
    );
    assert!(
        svg.contains("<tspan x=\"4.0\" fill=\"#e06c75\" font-weight=\"bold\">&lt;&amp;</tspan>")
    );
    assert!(svg.contains("<tspan x=\"19.6\" fill=\"#98c379\">z</tspan>"));
}

#[test]
fn a_gap_or_a_wide_glyph_ends_the_text_run() {
    let mut grid = CellGrid::new(6, 1);
    for (x, symbol) in [(0, "a"), (2, "b"), (3, "日"), (5, "c")] {
        grid.put(x, 0, symbol, style(RED));
    }
    let svg = grid_to_svg(&grid, "runs");
    let runs: Vec<&str> = svg.matches("<tspan").collect();
    assert_eq!(runs.len(), 3, "{svg}");
    for run in [
        "<tspan x=\"4.0\" fill=\"#e06c75\">a</tspan>",
        "<tspan x=\"19.6\" fill=\"#e06c75\">b日</tspan>",
        "<tspan x=\"43.0\" fill=\"#e06c75\">c</tspan>",
    ] {
        assert!(svg.contains(run), "{run} missing from {svg}");
    }
}

#[test]
fn a_reversed_cell_draws_its_foreground_as_background() {
    let mut grid = CellGrid::new(1, 1);
    let reversed = CellStyle {
        attrs: Attrs::REVERSED,
        ..style(RED)
    };
    grid.put(0, 0, "r", reversed);
    let svg = grid_to_svg(&grid, "reversed");
    assert!(
        svg.contains("fill=\"#e06c75\"/>"),
        "the red background rect"
    );
    assert!(svg.contains(&format!("<tspan x=\"4.0\" fill=\"{DEFAULT_BG}\">r</tspan>")));
}

#[test]
fn a_new_snapshot_fails_until_it_is_accepted() {
    let snapshots = VisualSnapshots::new(scratch("new"));
    let grid = swatch();
    let missing = snapshots.verify(&grid, "swatch", Mode::Compare);
    assert!(
        matches!(missing, Err(Mismatch::Missing { .. })),
        "{missing:?}"
    );
    assert!(snapshots.dir().join("swatch.fail.svg").is_file());
    assert!(!snapshots.dir().join("swatch.hash").exists());

    assert_eq!(snapshots.verify(&grid, "swatch", Mode::Update), Ok(()));
    let stored = fs::read_to_string(snapshots.dir().join("swatch.hash")).expect("hash written");
    assert_eq!(stored.trim(), grid_to_hash(&grid));
    assert!(snapshots.dir().join("swatch.svg").is_file());
    assert_eq!(snapshots.verify(&grid, "swatch", Mode::Compare), Ok(()));
    assert!(
        !snapshots.dir().join("swatch.fail.svg").exists(),
        "a pass removes the stale failure"
    );
}

#[test]
fn a_changed_frame_fails_and_leaves_its_svg() {
    let snapshots = VisualSnapshots::new(scratch("changed"));
    let grid = swatch();
    assert_eq!(snapshots.verify(&grid, "swatch", Mode::Update), Ok(()));
    let mut changed = grid.clone();
    changed.put_char(0, 0, '█', style(PaletteColor::Ansi(9)));
    match snapshots.verify(&changed, "swatch", Mode::Compare) {
        Err(Mismatch::Differs {
            stored, current, ..
        }) => {
            assert_eq!(stored, grid_to_hash(&grid));
            assert_eq!(current, grid_to_hash(&changed));
        }
        other => panic!("expected a hash mismatch, got {other:?}"),
    }
    let fail_svg = fs::read_to_string(snapshots.dir().join("swatch.fail.svg")).expect("fail.svg");
    assert!(fail_svg.contains("swatch [FAIL]"));
}

#[test]
#[should_panic(expected = "snapshot names are")]
fn a_snapshot_name_cannot_leave_its_directory() {
    let snapshots = VisualSnapshots::new(scratch("name"));
    let _ = snapshots.verify(&swatch(), "../escape", Mode::Update);
}

// ── The committed T3 set (tests/visual/) ───────────────────────────────────

#[test]
fn t3_swatch() {
    assert_visual_snapshot(&swatch(), "swatch");
}

#[test]
fn t3_text_cells() {
    assert_visual_snapshot(&text_cells(), "text_cells");
}

#[test]
fn perturbing_one_color_fails_the_committed_snapshot() {
    let snapshots = VisualSnapshots::for_package();
    assert_eq!(snapshots.check(&swatch(), "swatch"), Ok(()));
    let mut perturbed = swatch();
    perturbed.put_char(3, 2, '▀', style(PaletteColor::Indexed(197)));
    let result = snapshots.check(&perturbed, "swatch");
    assert!(
        matches!(result, Err(Mismatch::Differs { .. })),
        "one cell's foreground moved from palette 196 to 197, so T3 must fail: {result:?}"
    );
}
