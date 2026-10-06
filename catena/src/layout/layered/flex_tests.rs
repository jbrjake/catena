use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use super::*;

fn item(min: u16, preferred: u16, max: u16) -> FlexItem {
    FlexItem {
        min,
        preferred,
        max,
    }
}

fn sizes(results: &[FlexResult]) -> Vec<u16> {
    results.iter().map(|r| r.size).collect()
}

#[test]
fn distribute_basic_three_items() {
    // 3 items each min=5/pref=10/max=15 in 40 cols.
    // Each gets at least preferred. Total == 40.
    let items = vec![item(5, 10, 15); 3];
    let results = flex_distribute(40, &items);
    assert_eq!(results.len(), 3);
    for r in &results {
        assert!(
            r.size >= 10,
            "each item should get at least preferred (10), got {}",
            r.size
        );
    }
    let total: u16 = results.iter().map(|r| r.size).sum();
    assert_eq!(total, 40);
}

#[test]
fn distribute_insufficient_space() {
    // available (12) < sum mins (15). First two get min, last gets remainder.
    let items = vec![item(5, 10, 15); 3];
    let results = flex_distribute(12, &items);
    assert_eq!(results.len(), 3);
    assert_eq!(sizes(&results), [5, 5, 2]);
}

#[test]
fn overflow_goes_to_the_first_item_that_does_not_fit_and_later_items_get_nothing() {
    // The seed's doc comment said overflow "truncates the last item". The code has always
    // given the remainder to the first item whose min no longer fits and zero to every item
    // after it, which keeps the leading items whole; this pins that behavior.
    let items = vec![item(5, 7, 9); 3];
    let results = flex_distribute(7, &items);
    assert_eq!(results.len(), 3);
    assert_eq!(sizes(&results), [5, 2, 0]);
    assert_eq!(sizes(&flex_distribute(4, &items)), [4, 0, 0]);
    assert_eq!(sizes(&flex_distribute(0, &items)), [0, 0, 0]);
}

#[test]
fn distribute_exact_min() {
    // available == sum mins. Each gets exactly min.
    let items = vec![item(5, 10, 15); 3];
    let results = flex_distribute(15, &items);
    assert_eq!(results.len(), 3);
    assert_eq!(sizes(&results), [5, 5, 5]);
}

#[test]
fn distribute_single_item() {
    // 1 item min=5/pref=20/max=30 in 25 cols. Gets size=25.
    let results = flex_distribute(25, &[item(5, 20, 30)]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].offset, 0);
    assert_eq!(results[0].size, 25);
}

#[test]
fn distribute_empty() {
    let results = flex_distribute(100, &[]);
    assert_eq!(results, [] as [FlexResult; 0]);
}

#[test]
fn distribute_offsets_are_contiguous() {
    // For any result, item[i].offset == item[i-1].offset + item[i-1].size.
    let items = vec![
        item(3, 8, 20),
        item(5, 12, 25),
        item(4, 10, 18),
        item(2, 6, 10),
    ];
    let results = flex_distribute(50, &items);
    assert_eq!(results.len(), 4);
    assert_eq!(results[0].offset, 0);
    for i in 1..results.len() {
        assert_eq!(
            results[i].offset,
            results[i - 1].offset + results[i - 1].size,
            "offset[{i}] should be contiguous with previous item",
        );
    }
}

#[test]
fn distribute_respects_max() {
    // Even with 100 cols, items with max=12 don't exceed 12.
    let items = vec![item(5, 10, 12); 3];
    let results = flex_distribute(100, &items);
    assert_eq!(results.len(), 3);
    for r in &results {
        assert!(r.size <= 12, "item size {} exceeds max 12", r.size);
    }
}

// --- flex_center_with_gaps tests ---

#[test]
fn center_gaps_three_items() {
    // 3 items of size 10 in 70 cols. Total=30, remaining=40, 4 gaps of 10.
    // Positions: 10, 30, 50.
    let offsets = flex_center_with_gaps(70, &[10, 10, 10]);
    assert_eq!(offsets, vec![10, 30, 50]);
}

#[test]
fn center_gaps_single_item() {
    // 1 item of size 20 in 60 cols. remaining=40, 2 gaps of 20.
    // Position: 20.
    let offsets = flex_center_with_gaps(60, &[20]);
    assert_eq!(offsets, vec![20]);
}

#[test]
fn center_gaps_items_fill_space() {
    // Items exactly fill available (30 cols, 3x10). Gaps=0.
    // Positions: 0, 10, 20.
    let offsets = flex_center_with_gaps(30, &[10, 10, 10]);
    assert_eq!(offsets, vec![0, 10, 20]);
}

#[test]
fn center_gaps_items_exceed_space() {
    // Items exceed available (20 cols, 3x10). Flush left.
    // Positions: 0, 10, 20.
    let offsets = flex_center_with_gaps(20, &[10, 10, 10]);
    assert_eq!(offsets, vec![0, 10, 20]);
}

#[test]
fn center_gaps_empty() {
    // Empty items returns empty vec.
    let offsets = flex_center_with_gaps(50, &[]);
    assert_eq!(offsets, [] as [u16; 0]);
}

#[test]
fn center_gaps_unequal_sizes() {
    // 3 items: 8, 12, 6 in 50 cols. Total=26, remaining=24, 4 gaps of 6.
    // Positions: 6, 20, 38.
    let offsets = flex_center_with_gaps(50, &[8, 12, 6]);
    assert_eq!(offsets, vec![6, 20, 38]);
}

// --- Invariant L (plan §16.2) ---

/// proptest's own RNG at a fixed seed: the gate stays deterministic (plan §17), and the 256
/// cases are the same on every run and machine.
fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x666c_6578),
        failure_persistence: None,
        ..Config::default()
    }
}

/// A valid item: three draws, sorted into min ≤ preferred ≤ max.
fn valid_item(bound: u16) -> impl Strategy<Value = FlexItem> {
    (0..=bound, 0..=bound, 0..=bound).prop_map(|(a, b, c)| {
        let mut v = [a, b, c];
        v.sort_unstable();
        item(v[0], v[1], v[2])
    })
}

/// Small items, where the interesting boundaries are dense, and full-range ones, where
/// overflow would hide.
fn case() -> impl Strategy<Value = (u16, Vec<FlexItem>)> {
    prop_oneof![
        (0u16..=400, prop::collection::vec(valid_item(60), 0..12)),
        (
            any::<u16>(),
            prop::collection::vec(valid_item(u16::MAX), 0..6)
        ),
    ]
}

/// Which bound of an item a distribution phase starts from or fills toward.
type Bound = fn(&FlexItem) -> u16;

fn sum(values: impl Iterator<Item = u16>) -> u32 {
    values.map(u32::from).sum()
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn invariant_l_holds((available, items) in case()) {
        let out = flex_distribute(available, &items);
        prop_assert_eq!(out.len(), items.len());

        // Offsets are contiguous from zero.
        let mut expected_offset = 0u32;
        for r in &out {
            prop_assert_eq!(u32::from(r.offset), expected_offset);
            expected_offset += u32::from(r.size);
        }

        // The sizes sum exactly to what the items can take.
        let total = sum(out.iter().map(|r| r.size));
        let sum_min = sum(items.iter().map(|i| i.min));
        let sum_pref = sum(items.iter().map(|i| i.preferred));
        let sum_max = sum(items.iter().map(|i| i.max));
        prop_assert_eq!(total, u32::from(available).min(sum_max));

        let avail = u32::from(available);
        if avail >= sum_min {
            for (r, it) in out.iter().zip(&items) {
                prop_assert!(it.min <= r.size && r.size <= it.max, "{:?} got {}", it, r.size);
                if avail <= sum_pref {
                    prop_assert!(r.size <= it.preferred, "preferred is filled before max");
                }
                if avail >= sum_pref {
                    prop_assert!(r.size >= it.preferred, "everyone reaches preferred first");
                }
            }
            // Within a phase the extra space is shared in proportion to each item's want, up to
            // the rounding remainder (fewer columns than there are items).
            let (floor, ceiling, extra): (Bound, Bound, u32) =
                if avail <= sum_pref {
                    (|i| i.min, |i| i.preferred, avail - sum_min)
                } else {
                    (|i| i.preferred, |i| i.max, avail.min(sum_max) - sum_pref)
                };
            let wanted = sum(items.iter().map(|i| ceiling(i) - floor(i)));
            let n = u64::try_from(items.len()).expect("few items");
            if wanted > 0 {
                for (r, it) in out.iter().zip(&items) {
                    let got = u64::from(r.size - floor(it)) * u64::from(wanted);
                    let fair = u64::from(ceiling(it) - floor(it)) * u64::from(extra);
                    prop_assert!(
                        got.abs_diff(fair) < n * u64::from(wanted),
                        "{:?} got {} of {} spare columns", it, r.size - floor(it), extra
                    );
                }
            }
        } else {
            // Overflow: whole minimums for a prefix, the remainder to the next item, then zeros.
            let mut left = avail;
            for (r, it) in out.iter().zip(&items) {
                let expected = u32::from(it.min).min(left);
                prop_assert_eq!(u32::from(r.size), expected);
                left -= expected;
            }
        }
    }

    #[test]
    fn centered_items_never_overlap_and_gaps_are_equal(
        available in any::<u16>(),
        item_sizes in prop::collection::vec(0u16..=2000, 0..10),
    ) {
        let offsets = flex_center_with_gaps(available, &item_sizes);
        prop_assert_eq!(offsets.len(), item_sizes.len());
        let total = sum(item_sizes.iter().copied());
        if total >= u32::from(available) {
            let mut x = 0u16;
            for (&offset, &size) in offsets.iter().zip(&item_sizes) {
                prop_assert_eq!(offset, x);
                x = x.saturating_add(size);
            }
        } else if let Some(&first) = offsets.first() {
            let gap = u32::from(first);
            let mut x = gap;
            for (&offset, &size) in offsets.iter().zip(&item_sizes) {
                prop_assert_eq!(u32::from(offset), x);
                x += u32::from(size) + gap;
            }
            // x is now the right edge plus one gap: everything fits, and the gap is the
            // largest equal one (a bigger gap would not fit n + 1 times).
            prop_assert!(x <= u32::from(available));
            let gaps = u32::try_from(item_sizes.len()).expect("small") + 1;
            prop_assert!(x + gaps > u32::from(available));
        }
    }
}
