#![allow(
    clippy::float_cmp,
    reason = "steps of a quarter and centers of whole cells are exact"
)]

use super::*;
use crate::interact::Dir;

#[test]
fn zoom_steps_accumulate_on_the_target_and_stop_at_the_clamp() {
    assert_eq!(step_zoom(1.0, KEY_ZOOM_STEP), 1.25);
    assert_eq!(step_zoom(step_zoom(1.0, KEY_ZOOM_STEP), KEY_ZOOM_STEP), 1.5);
    assert_eq!(step_zoom(3.9, KEY_ZOOM_STEP), MAX_ZOOM);
    assert_eq!(step_zoom(0.2, -KEY_ZOOM_STEP), MIN_ZOOM);
    assert!((step_zoom(1.0, -SCROLL_ZOOM_STEP) - 0.92).abs() < 1e-12);
}

#[test]
fn a_node_three_cells_or_more_off_center_is_panned_to_and_a_nearer_one_is_not() {
    let center = SubPt::new(40.0, 12.0);
    assert_eq!(pan_to(CellPt::new(10, 12), center), Some((30.0, 0.0)));
    assert_eq!(pan_to(CellPt::new(40, 9), center), Some((0.0, 3.0)));
    assert_eq!(pan_to(CellPt::new(42, 10), center), None);
    assert_eq!(pan_to(CellPt::new(40, 12), center), None);
}

#[test]
fn the_visible_center_of_an_unoccluded_frame_is_its_middle_cell() {
    assert_eq!(visible_center((81, 25), &[]), SubPt::new(40.0, 12.0));
    assert_eq!(visible_center((80, 24), &[]), SubPt::new(39.5, 11.5));
    assert_eq!(visible_center((0, 0), &[]), SubPt::new(0.0, 0.0));
}

#[test]
fn a_window_over_the_left_half_moves_the_center_into_the_right_half() {
    // Columns 0..40 of 80 occluded: the free columns are 40..80, whose middle is 59.5.
    let left = CellBox::new(0, 0, 40, 24);
    assert_eq!(visible_center((80, 24), &[left]), SubPt::new(59.5, 11.5));
}

#[test]
fn overlapping_windows_count_each_occluded_cell_once() {
    // The seed subtracted the overlap twice: one window over another changed its center.
    let left = CellBox::new(0, 0, 40, 24);
    let same = visible_center((80, 24), &[left]);
    assert_eq!(visible_center((80, 24), &[left, left]), same);
    let inner = CellBox::new(10, 5, 10, 10);
    assert_eq!(visible_center((80, 24), &[left, inner]), same);
    // And windows partly off the frame occlude only their part on it.
    let past = CellBox::new(-40, -5, 80, 40);
    assert_eq!(visible_center((80, 24), &[past]), same);
}

fn candidates(points: &[(i32, i32)]) -> Vec<CellPt> {
    points.iter().map(|&(x, y)| CellPt::new(x, y)).collect()
}

fn nav(from: (i32, i32), dir: Dir, points: &[(i32, i32)]) -> Option<usize> {
    nearest_toward(CellPt::new(from.0, from.1), dir, &candidates(points), 0.5)
}

#[test]
fn spatial_navigation_takes_the_best_scored_node_in_the_cone() {
    // From (20, 10): straight up 2 rows (4 corrected cells), up-and-right at 45° farther, and
    // one beside it. Rows count double, as a cell is twice as tall as wide.
    let points = [(20, 8), (26, 7), (25, 10), (20, 14)];
    assert_eq!(nav((20, 10), Dir::Up, &points), Some(0));
    assert_eq!(nav((20, 10), Dir::Right, &points), Some(2));
    assert_eq!(nav((20, 10), Dir::Down, &points), Some(3));
}

#[test]
fn the_angle_costs_more_than_the_distance_saves() {
    // Up from (20, 10). (20, 7) is 3 rows up, 6 corrected cells dead ahead: it scores 6.
    // (23, 8) is 3 across and 2 rows (4) up, 5 away at cos θ = 0.8: 5 × (1 + 2 × 0.2) = 7.
    // The farther, straighter node wins.
    assert_eq!(nav((20, 10), Dir::Up, &[(20, 7), (23, 8)]), Some(0));
    // (30, 7) is 10 across and 6 up, about 11.7 away at cos θ ≈ 0.51, still in the cone, but
    // it scores about 23 against (23, 8)'s 7.
    assert_eq!(nav((20, 10), Dir::Up, &[(30, 7), (23, 8)]), Some(1));
}

#[test]
fn outside_the_sixty_degree_cone_counts_only_when_the_cone_is_empty() {
    // Right from (10, 10): (12, 13) is 2 across, 3 rows (6) down, about 72° off: out of the
    // cone. With nothing in it, the half-plane's nearest is taken; anything in the cone wins.
    assert_eq!(nav((10, 10), Dir::Right, &[(12, 13)]), Some(0));
    assert_eq!(nav((10, 10), Dir::Right, &[(12, 13), (40, 10)]), Some(1));
}

#[test]
fn nothing_ahead_is_the_boundary() {
    assert_eq!(nav((10, 10), Dir::Left, &[(12, 13), (40, 10)]), None);
    assert_eq!(nav((10, 10), Dir::Up, &[(10, 10)]), None, "the node itself");
    assert_eq!(nav((10, 10), Dir::Up, &[]), None);
}

#[test]
fn a_tie_goes_to_the_earlier_node() {
    assert_eq!(nav((10, 10), Dir::Up, &[(12, 9), (8, 9)]), Some(0));
    assert_eq!(nav((10, 10), Dir::Up, &[(8, 9), (12, 9)]), Some(0));
}

#[test]
fn a_frame_occluded_everywhere_falls_back_to_its_middle() {
    let all = CellBox::new(0, 0, 80, 24);
    assert_eq!(visible_center((80, 24), &[all]), SubPt::new(39.5, 11.5));
}
