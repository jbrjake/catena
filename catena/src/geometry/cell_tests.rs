use super::*;

#[test]
fn a_sub_cell_point_lies_in_the_cell_it_rounds_to() {
    assert_eq!(SubPt::new(2.49, 3.5).cell(), CellPt::new(2, 4));
    assert_eq!(SubPt::new(-0.4, -0.6).cell(), CellPt::new(0, -1));
    assert_eq!(
        SubPt::new(1e300, f64::NAN).cell(),
        CellPt::new(i32::MAX, 0),
        "saturating, NaN to 0"
    );
    assert_eq!(SubPt::from(CellPt::new(-3, 7)), SubPt::new(-3.0, 7.0));
}

#[test]
fn around_is_the_smallest_box_holding_every_cell() {
    assert_eq!(CellBox::around([]), None);
    let cells = [CellPt::new(3, -1), CellPt::new(-2, 4), CellPt::new(0, 0)];
    let around = CellBox::around(cells).expect("three cells");
    assert_eq!(around, CellBox::new(-2, -1, 6, 6));
    for cell in cells {
        assert!(around.contains(cell));
    }
    assert!(
        !around.contains(CellPt::new(4, 0)),
        "one past the right column"
    );
    assert!(
        !around.contains(CellPt::new(0, 5)),
        "one past the bottom row"
    );
}

#[test]
fn boxes_spanning_the_whole_i32_range_stay_exact() {
    let wide =
        CellBox::around([CellPt::new(i32::MIN, 0), CellPt::new(i32::MAX, 0)]).expect("two cells");
    assert_eq!(wide.width, u32::MAX, "2^32 columns saturate to u32::MAX");
    assert_eq!(wide.right(), i64::from(i32::MIN) + i64::from(u32::MAX));
    assert!(wide.contains(CellPt::new(0, 0)));
    assert_eq!(CellBox::new(0, 0, 3, 0).area(), 0);
    assert!(CellBox::new(0, 0, 3, 0).is_empty());
    assert!(!CellBox::new(0, 0, 3, 0).contains(CellPt::new(0, 0)));
}
