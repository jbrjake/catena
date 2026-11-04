//! Frame diffs, the A→B→A flicker detector and the byte capture (plan §16.5, §16.8).

use std::io::Write;

use catena::raster::{CellGrid, CellStyle, PaletteColor, Surface};
use catena_testkit::oscillation::{CaptureBuf, diff_count, diff_details, flickers};

fn grid(text: &str) -> CellGrid {
    let mut grid = CellGrid::new(4, 2);
    for (i, c) in text.chars().enumerate() {
        let (x, y) = (
            u16::try_from(i % 4).expect("small"),
            u16::try_from(i / 4).expect("small"),
        );
        grid.put_char(x, y, c, CellStyle::new(PaletteColor::Ansi(7)));
    }
    grid
}

#[test]
fn identical_frames_differ_nowhere() {
    assert_eq!(diff_count(&grid("abcdefgh"), &grid("abcdefgh")), 0);
    assert_eq!(
        diff_details(&grid("abcdefgh"), &grid("abcdefgh")),
        Vec::<String>::new()
    );
}

#[test]
fn a_diff_counts_each_changed_cell_and_describes_it() {
    let (a, b) = (grid("abcdefgh"), grid("abXdefYh"));
    assert_eq!(diff_count(&a, &b), 2);
    let details = diff_details(&a, &b);
    assert_eq!(details.len(), 2);
    assert!(
        details[0].starts_with("(2,0): \"c\"→\"X\""),
        "{}",
        details[0]
    );
    assert!(
        details[1].starts_with("(2,1): \"g\"→\"Y\""),
        "{}",
        details[1]
    );
}

#[test]
fn a_style_change_is_a_diff() {
    let a = grid("abcdefgh");
    let mut b = a.clone();
    b.put_char(0, 0, 'a', CellStyle::new(PaletteColor::Ansi(1)));
    assert_eq!(diff_count(&a, &b), 1);
    assert!(
        diff_details(&a, &b)[0].contains("fg:"),
        "{:?}",
        diff_details(&a, &b)
    );
}

#[test]
fn frames_of_different_sizes_differ_in_every_cell_of_the_larger() {
    let small = CellGrid::new(2, 2);
    let large = CellGrid::new(4, 3);
    assert_eq!(diff_count(&small, &large), 12);
    assert_eq!(diff_count(&large, &small), 12);
}

#[test]
fn diff_details_list_at_most_twenty_cells() {
    let a = CellGrid::new(10, 10);
    let mut b = a.clone();
    for y in 0..10 {
        for x in 0..10 {
            b.put_char(x, y, '#', CellStyle::new(PaletteColor::Ansi(7)));
        }
    }
    assert_eq!(diff_count(&a, &b), 100);
    assert_eq!(diff_details(&a, &b).len(), 20);
}

#[test]
fn the_flicker_detector_finds_a_frame_that_returns_two_frames_later() {
    let (a, b, c) = (grid("aaaaaaaa"), grid("bbbbbbbb"), grid("cccccccc"));
    // A → B → A: frame 2 equals frame 0 while differing from frame 1.
    assert_eq!(flickers(&[a.clone(), b.clone(), a.clone()]), [2]);
    // A → B → C, A → A → B and a steady run do not flicker.
    assert_eq!(
        flickers(&[a.clone(), b.clone(), c.clone()]),
        Vec::<usize>::new()
    );
    assert_eq!(
        flickers(&[a.clone(), a.clone(), b.clone()]),
        Vec::<usize>::new()
    );
    assert_eq!(
        flickers(&[a.clone(), a.clone(), a.clone()]),
        Vec::<usize>::new()
    );
    // Each return counts: A B A B A flickers at 2, 3 and 4.
    assert_eq!(
        flickers(&[a.clone(), b.clone(), a.clone(), b, a]),
        [2, 3, 4]
    );
    assert_eq!(flickers(&[]), Vec::<usize>::new());
}

#[test]
fn a_capture_buffer_keeps_every_byte_until_drained() {
    let capture = CaptureBuf::new();
    let mut writer = capture.clone();
    writer.write_all(b"\x1b[?1049h").expect("in memory");
    writer.write_all(b"frame").expect("in memory");
    writer.flush().expect("in memory");
    assert_eq!(capture.len(), 13);
    assert_eq!(capture.drain(), b"\x1b[?1049hframe");
    assert_eq!(capture.len(), 0);
    assert!(capture.is_empty());
}
