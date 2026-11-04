#![allow(
    clippy::float_cmp,
    reason = "steps of a quarter and centers of whole cells are exact"
)]

use super::*;

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

#[test]
fn a_frame_occluded_everywhere_falls_back_to_its_middle() {
    let all = CellBox::new(0, 0, 80, 24);
    assert_eq!(visible_center((80, 24), &[all]), SubPt::new(39.5, 11.5));
}
